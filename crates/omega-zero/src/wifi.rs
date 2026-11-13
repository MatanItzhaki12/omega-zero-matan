//! Home Wi-Fi: what the radio reports, and the jobs that drive `System/wifi.sh`.
//!
//! The script does the talking to wpa_supplicant. Its output is read here, off the frame
//! loop: every job runs on one worker thread, and what it learns comes back as news the app
//! takes each tick.

use std::fmt::Write as _;
#[cfg(feature = "device")]
use std::path::{Path, PathBuf};
#[cfg(feature = "device")]
use std::process::Stdio;
#[cfg(feature = "device")]
use std::sync::mpsc::{channel, Receiver, Sender};

use omega_input::Millis;

/// How long a join may take before it is called failed. wpa_supplicant gives up on a wrong
/// passphrase well within this, and marks the network so it can be told apart.
pub const JOIN_TIMEOUT_MS: Millis = 30_000;
/// While the Wi-Fi screen or the menu that names the network is open.
pub const REFRESH_MS: Millis = 2_000;
/// While only the top bar shows whether a network is joined, how often the link is glanced at.
/// A glance reads one file, so the bar follows a network joined or lost within a second.
pub const GLANCE_MS: Millis = 1_000;

/// What `wifi.sh status` says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// Home Wi-Fi is wanted, whether or not the radio has come up yet.
    pub enabled: bool,
    /// wpa_supplicant's own state, such as `COMPLETED` or `SCANNING`; empty when it is not
    /// running.
    pub state: String,
    pub ssid: Option<Vec<u8>>,
    pub ip: Option<String>,
    pub rssi: Option<i32>,
    /// The radio's own hardware address, as wpa_supplicant gives it while it runs.
    pub mac: Option<String>,
    pub hostname: String,
    pub ntp: bool,
    pub ntp_synced: bool,
}

impl Status {
    pub fn joined(&self) -> bool {
        self.state == "COMPLETED"
    }
}

/// A network the radio can hear.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seen {
    pub ssid: Vec<u8>,
    pub rssi: i32,
    pub secured: bool,
}

/// A network in the card's `Config/wifi.conf`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub id: u32,
    pub ssid: Vec<u8>,
    pub current: bool,
    /// wpa_supplicant stopped trying it for a while, which after a join means it was turned
    /// away: almost always the wrong passphrase.
    pub refused: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WifiJob {
    Up,
    Down,
    Refresh,
    /// Whether a network is joined, as the kernel says of the link: a read of one file, where
    /// `Refresh` runs the script twice.
    Glance,
    /// One look for the networks in reach: `wifi.sh scan`, which asks wpa_supplicant to scan and
    /// then reads what it heard. Only X on the Network screen asks for one, never a timer: while
    /// the radio scans its driver will not calibrate, and on the RG35XX SP a reboot after a run of
    /// them has sometimes hung in the kernel.
    Scan,
    Join {
        ssid: Vec<u8>,
        psk: String,
        replace: Option<u32>,
    },
    Select(u32),
    Forget(u32),
    Clock(bool),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WifiNews {
    /// `Up` finished: false when the radio never appeared.
    Started(bool),
    Status(Status),
    /// What a `Glance` saw: whether a network is joined.
    Glanced(bool),
    Saved(Vec<Saved>),
    Seen(Vec<Seen>),
    /// A join or select was taken up, or turned down before reaching the air.
    Asked(Result<(), AskFail>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskFail {
    /// wpa_supplicant would not take the passphrase: shorter than 8 or longer than 63.
    BadPassphrase,
    Radio,
}

pub trait WifiJobs: Send {
    /// The handheld has a radio for home Wi-Fi.
    fn present(&self) -> bool;

    fn ask(&mut self, job: WifiJob);

    fn news(&mut self) -> Vec<WifiNews>;
}

/// A passphrase wpa_supplicant takes: 8 to 63 printable ASCII characters.
pub fn passphrase_ok(psk: &str) -> bool {
    (8..=63).contains(&psk.len()) && psk.bytes().all(|b| (0x20..0x7f).contains(&b))
}

/// Undoes wpa_supplicant's `printf_encode`, which writes an SSID's bytes with `\xNN`, `\\`,
/// `\"`, `\e`, `\n`, `\r` and `\t` escapes.
pub fn decode(text: &str) -> Vec<u8> {
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'\\' || i + 1 == b.len() {
            out.push(b[i]);
            i += 1;
            continue;
        }
        let (byte, used) = match b[i + 1] {
            b'\\' => (b'\\', 2),
            b'"' => (b'"', 2),
            b'e' => (0x1b, 2),
            b'n' => (b'\n', 2),
            b'r' => (b'\r', 2),
            b't' => (b'\t', 2),
            b'x' => match text
                .get(i + 2..i + 4)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                Some(v) => (v, 4),
                None => (b'\\', 1),
            },
            _ => (b'\\', 1),
        };
        out.push(byte);
        i += used;
    }
    out
}

pub fn hex(ssid: &[u8]) -> String {
    ssid.iter()
        .fold(String::with_capacity(ssid.len() * 2), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

/// An SSID as it is shown: its bytes as UTF-8, with anything that is not printed as a
/// stand-in.
pub fn shown(ssid: &[u8]) -> String {
    String::from_utf8_lossy(ssid)
        .chars()
        .map(|c| if c.is_control() { '\u{fffd}' } else { c })
        .collect()
}

/// Signal strength as bars out of four.
pub fn bars(rssi: i32) -> u8 {
    match rssi {
        r if r >= -55 => 4,
        r if r >= -66 => 3,
        r if r >= -77 => 2,
        _ => 1,
    }
}

pub fn parse_status(text: &str) -> Status {
    let mut s = Status::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let some = || (!value.is_empty()).then(|| value.to_string());
        match key {
            "enabled" => s.enabled = value == "yes",
            "wpa_state" => s.state = value.to_string(),
            "ssid" => s.ssid = Some(decode(value)),
            "ip_address" => s.ip = some(),
            "RSSI" => s.rssi = value.parse().ok(),
            "address" => s.mac = some(),
            "hostname" => s.hostname = value.to_string(),
            "ntp" => s.ntp = value == "yes",
            "ntp_synced" => s.ntp_synced = value == "yes",
            _ => {}
        }
    }
    if !s.joined() {
        s.ip = None;
    }
    s
}

/// Whether the kernel's operstate for the radio says a network is joined. wpa_supplicant holds
/// the link dormant until it has joined one and agreed its keys, then marks it up.
pub fn link_up(operstate: &str) -> bool {
    operstate.trim() == "up"
}

/// `wpa_cli scan_results`: one network per name, at its strongest, without the hidden ones
/// and the enterprise ones that need more than a passphrase.
pub fn parse_scan(text: &str) -> Vec<Seen> {
    let mut seen: Vec<Seen> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.splitn(5, '\t').collect();
        let [_, _, rssi, flags, ssid] = f[..] else {
            continue;
        };
        let Ok(rssi) = rssi.parse::<i32>() else {
            continue;
        };
        let ssid = decode(ssid);
        if ssid.iter().all(|b| *b == 0) {
            continue;
        }
        if flags.contains("EAP") && !flags.contains("PSK") {
            continue;
        }
        let secured = ["WPA", "RSN", "WEP", "SAE"]
            .iter()
            .any(|k| flags.contains(k));
        match seen.iter_mut().find(|s| s.ssid == ssid) {
            Some(s) if s.rssi >= rssi => {}
            Some(s) => {
                s.rssi = rssi;
                s.secured = secured;
            }
            None => seen.push(Seen {
                ssid,
                rssi,
                secured,
            }),
        }
    }
    seen.sort_by_key(|s| std::cmp::Reverse(s.rssi));
    seen
}

/// `wpa_cli list_networks`.
pub fn parse_saved(text: &str) -> Vec<Saved> {
    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            let [id, ssid, _, flags] = f[..] else {
                return None;
            };
            Some(Saved {
                id: id.parse().ok()?,
                ssid: decode(ssid),
                current: flags.contains("[CURRENT]"),
                refused: flags.contains("TEMP-DISABLED"),
            })
        })
        .collect()
}

pub fn wifi_jobs() -> Box<dyn WifiJobs> {
    #[cfg(feature = "device")]
    {
        Box::new(ScriptWifi::spawn(card_root()))
    }
    #[cfg(not(feature = "device"))]
    {
        Box::new(SimWifi::default())
    }
}

/// The radio's link as the kernel keeps it, for a glance.
#[cfg(feature = "device")]
const OPERSTATE: &str = "/sys/class/net/wlan0/operstate";

#[cfg(feature = "device")]
fn card_root() -> PathBuf {
    std::env::var_os("OMEGA_ROOT")
        .unwrap_or_else(|| "/mnt/sdcard".into())
        .into()
}

/// The handheld's radio, through the card's script.
#[cfg(feature = "device")]
pub struct ScriptWifi {
    present: bool,
    jobs: Sender<WifiJob>,
    news: Receiver<WifiNews>,
}

#[cfg(feature = "device")]
impl ScriptWifi {
    pub fn spawn(root: PathBuf) -> Self {
        let (jobs, rx) = channel::<WifiJob>();
        let (tx, news) = channel::<WifiNews>();
        std::thread::spawn(move || {
            for job in rx {
                for n in work(&root, job) {
                    if tx.send(n).is_err() {
                        return;
                    }
                }
            }
        });
        // BaseOS says which images have a radio. The RG35XX 2024 shares the Plus's image and
        // has none, which shows when the radio never appears.
        let present = omega_power::baseos_release()
            .and_then(|r| r.wifi)
            .unwrap_or(true);
        ScriptWifi {
            present,
            jobs,
            news,
        }
    }
}

#[cfg(feature = "device")]
impl WifiJobs for ScriptWifi {
    fn present(&self) -> bool {
        self.present
    }

    fn ask(&mut self, job: WifiJob) {
        let _ = self.jobs.send(job);
    }

    fn news(&mut self) -> Vec<WifiNews> {
        self.news.try_iter().collect()
    }
}

#[cfg(any(feature = "device", test))]
fn script(root: &std::path::Path, args: &[&str]) -> std::process::Command {
    let mut cmd = std::process::Command::new("/bin/sh");
    cmd.arg(root.join("System/wifi.sh")).args(args);
    // what it leaves running, wpa_supplicant and udhcpc, starts as BaseOS's own daemons do
    crate::spawn::as_a_daemon(&mut cmd);
    cmd
}

#[cfg(feature = "device")]
fn output(root: &Path, args: &[&str]) -> Option<String> {
    let out = script(root, args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(feature = "device")]
fn status_code(root: &Path, args: &[&str], stdin: Option<&str>) -> Option<i32> {
    use std::io::Write;
    let mut child = script(root, args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    if let (Some(text), Some(mut pipe)) = (stdin, child.stdin.take()) {
        let _ = writeln!(pipe, "{text}");
    }
    child.wait().ok()?.code()
}

#[cfg(feature = "device")]
fn refreshed(root: &Path) -> Vec<WifiNews> {
    let mut news = Vec::new();
    if let Some(text) = output(root, &["status"]) {
        news.push(WifiNews::Status(parse_status(&text)));
    }
    news.push(WifiNews::Saved(
        output(root, &["saved"])
            .map(|t| parse_saved(&t))
            .unwrap_or_default(),
    ));
    news
}

#[cfg(feature = "device")]
fn work(root: &Path, job: WifiJob) -> Vec<WifiNews> {
    let asked = |code: Option<i32>| {
        WifiNews::Asked(match code {
            Some(0) => Ok(()),
            Some(4) => Err(AskFail::BadPassphrase),
            _ => Err(AskFail::Radio),
        })
    };
    let mut news = match job {
        WifiJob::Up => vec![WifiNews::Started(
            status_code(root, &["up"], None) == Some(0),
        )],
        WifiJob::Down => {
            status_code(root, &["down"], None);
            Vec::new()
        }
        WifiJob::Refresh => Vec::new(),
        WifiJob::Glance => {
            let up = std::fs::read_to_string(OPERSTATE).is_ok_and(|s| link_up(&s));
            return vec![WifiNews::Glanced(up)];
        }
        WifiJob::Scan => {
            let seen = output(root, &["scan"]).map(|t| parse_scan(&t));
            return vec![WifiNews::Seen(seen.unwrap_or_default())];
        }
        WifiJob::Join { ssid, psk, replace } => {
            let (hex, replace) = (hex(&ssid), replace.map(|id| id.to_string()));
            let mut args = vec!["join", hex.as_str()];
            args.extend(replace.as_deref());
            vec![asked(status_code(root, &args, Some(&psk)))]
        }
        WifiJob::Select(id) => vec![asked(status_code(root, &["select", &id.to_string()], None))],
        WifiJob::Forget(id) => {
            status_code(root, &["forget", &id.to_string()], None);
            Vec::new()
        }
        WifiJob::Clock(on) => {
            status_code(root, &["clock", if on { "on" } else { "off" }], None);
            Vec::new()
        }
    };
    news.extend(refreshed(root));
    news
}

/// A radio for the desktop and the tests: a few networks within reach, any of which takes
/// any passphrase wpa_supplicant would.
pub struct SimWifi {
    enabled: bool,
    ntp: bool,
    saved: Vec<(u32, Vec<u8>)>,
    current: Option<u32>,
    next_id: u32,
    news: Vec<WifiNews>,
}

impl Default for SimWifi {
    fn default() -> Self {
        SimWifi {
            enabled: false,
            ntp: true,
            saved: Vec::new(),
            current: None,
            next_id: 0,
            news: Vec::new(),
        }
    }
}

impl SimWifi {
    pub const NEARBY: [(&'static str, i32, bool); 4] = [
        ("Home", -48, true),
        ("Café Wi-Fi", -63, false),
        ("Upstairs 5G", -71, true),
        ("PrinterSetup", -84, true),
    ];

    fn status(&self) -> Status {
        let joined = self
            .current
            .and_then(|id| self.saved.iter().find(|(i, _)| *i == id))
            .filter(|_| self.enabled);
        Status {
            enabled: self.enabled,
            state: match (self.enabled, joined) {
                (false, _) => String::new(),
                (true, Some(_)) => "COMPLETED".into(),
                (true, None) => "SCANNING".into(),
            },
            ssid: joined.map(|(_, s)| s.clone()),
            ip: joined.map(|_| "192.168.1.20".into()),
            rssi: joined.map(|_| -48),
            mac: self.enabled.then(|| "e4:5f:01:2a:9c:33".into()),
            hostname: "rg35xxsp".into(),
            ntp: self.ntp,
            ntp_synced: self.ntp && joined.is_some(),
        }
    }

    fn refreshed(&mut self) {
        self.news.push(WifiNews::Status(self.status()));
        let saved = self
            .saved
            .iter()
            .map(|(id, ssid)| Saved {
                id: *id,
                ssid: ssid.clone(),
                current: self.current == Some(*id),
                refused: false,
            })
            .collect();
        self.news.push(WifiNews::Saved(saved));
    }
}

impl WifiJobs for SimWifi {
    fn present(&self) -> bool {
        true
    }

    fn ask(&mut self, job: WifiJob) {
        match job {
            WifiJob::Up => {
                self.enabled = true;
                self.current = self.saved.first().map(|(id, _)| *id);
                self.news.push(WifiNews::Started(true));
            }
            WifiJob::Down => {
                self.enabled = false;
                self.current = None;
            }
            WifiJob::Refresh => {}
            WifiJob::Glance => {
                self.news.push(WifiNews::Glanced(self.status().joined()));
                return;
            }
            WifiJob::Scan => {
                let seen = SimWifi::NEARBY
                    .iter()
                    .filter(|_| self.enabled)
                    .map(|(ssid, rssi, secured)| Seen {
                        ssid: ssid.as_bytes().to_vec(),
                        rssi: *rssi,
                        secured: *secured,
                    })
                    .collect();
                self.news.push(WifiNews::Seen(seen));
                return;
            }
            WifiJob::Join { ssid, psk, replace } => {
                let secured = SimWifi::NEARBY
                    .iter()
                    .find(|(s, ..)| s.as_bytes() == ssid.as_slice())
                    .is_none_or(|(_, _, secured)| *secured);
                if secured && !passphrase_ok(&psk) {
                    self.news.push(WifiNews::Asked(Err(AskFail::BadPassphrase)));
                    return self.refreshed();
                }
                self.saved
                    .retain(|(id, s)| Some(*id) != replace && *s != ssid);
                let id = self.next_id;
                self.next_id += 1;
                self.saved.push((id, ssid));
                self.enabled = true;
                self.current = Some(id);
                self.news.push(WifiNews::Asked(Ok(())));
            }
            WifiJob::Select(id) => {
                let known = self.saved.iter().any(|(i, _)| *i == id);
                if known {
                    self.current = Some(id);
                }
                self.news.push(WifiNews::Asked(if known {
                    Ok(())
                } else {
                    Err(AskFail::Radio)
                }));
            }
            WifiJob::Forget(id) => {
                self.saved.retain(|(i, _)| *i != id);
                if self.current == Some(id) {
                    self.current = None;
                }
            }
            WifiJob::Clock(on) => self.ntp = on,
        }
        self.refreshed();
    }

    fn news(&mut self) -> Vec<WifiNews> {
        std::mem::take(&mut self.news)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_runs_the_cards_script_through_sh() {
        let d = tempfile::tempdir().unwrap();
        let cmd = script(d.path(), &["join", "486f6d65", "2"]);
        let argv: Vec<String> = std::iter::once(cmd.get_program())
            .chain(cmd.get_args())
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let at = d.path().join("System/wifi.sh");
        assert_eq!(
            argv,
            ["/bin/sh", at.to_str().unwrap(), "join", "486f6d65", "2"]
        );
    }
}
