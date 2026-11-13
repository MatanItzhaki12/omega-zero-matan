//! The Network screen, opened from Settings: switching Wi-Fi, network time and SSH, the way to
//! each one's panel, the networks in reach, and typing a passphrase for a new one.

use omega_input::{Action, Btn, Millis};
use omega_ui::network::{network_window, networks_shown};
use omega_ui::Keyboard;

use crate::wifi::{
    bars, passphrase_ok, shown, AskFail, Saved, Seen, Status, WifiJob, WifiJobs, WifiNews,
    GLANCE_MS, JOIN_TIMEOUT_MS, REFRESH_MS,
};

/// How closely the radio is being watched, which sets how often it is asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Watch {
    Idle,
    /// The top bar shows whether a network is joined: the link is glanced at every second, and
    /// the status is asked again only when a glance finds the link otherwise than it says.
    Glance,
    /// Something on screen names the network: its status is kept fresh.
    Status,
    /// The Network screen is open, or a panel opened from it: its status is kept fresh,
    /// and what a scan finds is kept until it closes. Nothing scans by itself: X on the screen
    /// asks for each scan.
    Networks,
}

/// Why the last attempt went wrong, said under the list until the next one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Note {
    NoRadio,
    WrongPassphrase(Vec<u8>),
    NoJoin(Vec<u8>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryState {
    Joined,
    Joining,
    Saved,
    New,
}

/// A network the list offers: one in reach, one saved, or both.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub ssid: Vec<u8>,
    pub rssi: Option<i32>,
    pub secured: bool,
    pub saved: Option<u32>,
    pub state: EntryState,
}

/// What Settings says beside Network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Summary {
    Off,
    On,
    Joined(String),
}

/// The radio as the app knows it, and the jobs that change it.
pub struct Wifi {
    jobs: Box<dyn WifiJobs>,
    enabled: bool,
    status: Status,
    /// What the last scan heard, while the Network screen stays open: None until X has asked
    /// for one, and again once the screen closes or Wi-Fi goes off.
    seen: Option<Vec<Seen>>,
    saved: Vec<Saved>,
    joining: Option<(Vec<u8>, Millis)>,
    note: Option<Note>,
    refreshing: bool,
    /// A scan is on the worker. It finishes there whatever the screen does meanwhile.
    scanning: bool,
    glancing: bool,
    /// Whether the last glance found a network joined, until a status newer than it comes.
    glanced: Option<bool>,
    refresh_at: Millis,
    glance_at: Millis,
}

impl Wifi {
    pub fn new(jobs: Box<dyn WifiJobs>) -> Self {
        Wifi {
            jobs,
            enabled: false,
            status: Status::default(),
            seen: None,
            saved: Vec::new(),
            joining: None,
            note: None,
            refreshing: false,
            scanning: false,
            glancing: false,
            glanced: None,
            refresh_at: 0,
            glance_at: 0,
        }
    }

    pub fn present(&self) -> bool {
        self.jobs.present()
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn note(&self) -> Option<&Note> {
        self.note.as_ref()
    }

    /// At boot: brings Wi-Fi up if it was left on.
    pub fn start(&mut self, enabled: bool) {
        self.enabled = enabled && self.present();
        if self.enabled {
            self.jobs.ask(WifiJob::Up);
        }
    }

    /// Asks the radio to join a saved network again, as it does when switched on, and for the
    /// status after: Save Backup's saves wait for one. Nothing while Wi-Fi is off: switched off,
    /// it stays off.
    pub fn wake(&mut self, now: Millis) {
        if !self.enabled {
            return;
        }
        self.jobs.ask(WifiJob::Up);
        self.refresh_at = now;
    }

    pub fn set_enabled(&mut self, on: bool, now: Millis) {
        if on == self.enabled || !self.present() {
            return;
        }
        self.enabled = on;
        self.note = None;
        self.joining = None;
        if on {
            self.jobs.ask(WifiJob::Up);
            self.refresh_at = now;
        } else {
            self.jobs.ask(WifiJob::Down);
            self.seen = None;
            self.status = Status {
                hostname: std::mem::take(&mut self.status.hostname),
                ntp: self.status.ntp,
                ..Status::default()
            };
        }
    }

    pub fn ntp(&self) -> bool {
        self.status.ntp
    }

    pub fn set_ntp(&mut self, on: bool) {
        self.status.ntp = on;
        self.jobs.ask(WifiJob::Clock(on));
    }

    pub fn join(&mut self, ssid: Vec<u8>, psk: String, replace: Option<u32>, now: Millis) {
        self.note = None;
        self.joining = Some((ssid.clone(), now));
        self.jobs.ask(WifiJob::Join { ssid, psk, replace });
    }

    pub fn select(&mut self, id: u32, ssid: Vec<u8>, now: Millis) {
        self.note = None;
        self.joining = Some((ssid, now));
        self.jobs.ask(WifiJob::Select(id));
    }

    /// X on the Network screen: one look for the networks in reach, whose answer the list takes
    /// in. Nothing while Wi-Fi is off or a scan is already under way.
    pub fn scan(&mut self) {
        if !self.enabled || self.scanning {
            return;
        }
        self.scanning = true;
        self.jobs.ask(WifiJob::Scan);
    }

    /// A scan asked for by X has not answered yet.
    pub fn scanning(&self) -> bool {
        self.scanning
    }

    pub fn forget(&mut self, id: u32) {
        let gone = self
            .saved
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.ssid.clone());
        if gone.is_some() && self.joining.as_ref().map(|(s, _)| s) == gone.as_ref() {
            self.joining = None;
        }
        self.saved.retain(|s| s.id != id);
        self.jobs.ask(WifiJob::Forget(id));
    }

    /// Takes what the radio has said since the last tick, settles a join that has finished,
    /// and asks again whatever is being watched. It never asks for a scan.
    pub fn tick(&mut self, now: Millis, watch: Watch) {
        for news in self.jobs.news() {
            self.take(news, now, watch);
        }
        // Closed, the Network screen forgets what its scans found: opened again, it starts from
        // the network joined and the saved ones.
        if watch != Watch::Networks {
            self.seen = None;
        }
        if let Some((ssid, since)) = &self.joining {
            let refused = self.saved.iter().any(|s| s.ssid == *ssid && s.refused);
            if self.status.joined() && self.status.ssid.as_ref() == Some(ssid) {
                self.joining = None;
            } else if refused {
                self.note = Some(Note::WrongPassphrase(ssid.clone()));
                self.joining = None;
            } else if now.saturating_sub(*since) > JOIN_TIMEOUT_MS {
                self.note = Some(Note::NoJoin(ssid.clone()));
                self.joining = None;
            }
        }
        if !self.enabled {
            return;
        }
        let stale = self.glanced.is_some_and(|up| up != self.status.joined());
        // How long after a status is due for the screen that names the network the next is
        // asked for: none at all while nothing on screen shows the radio, and for the top bar
        // only once a glance has found the link otherwise than the last status said.
        let later = match watch {
            _ if self.joining.is_some() => Some(0),
            Watch::Idle => None,
            Watch::Glance => stale.then_some(0),
            Watch::Status | Watch::Networks => Some(0),
        };
        if later.is_some_and(|later| now >= self.refresh_at + later) && !self.refreshing {
            self.refreshing = true;
            self.jobs.ask(WifiJob::Refresh);
        }
        // The top bar needs only whether a network is joined, which a glance tells for the
        // price of one file read, so it can be asked every second.
        if watch == Watch::Glance && !self.glancing && !self.refreshing && now >= self.glance_at {
            self.glancing = true;
            self.jobs.ask(WifiJob::Glance);
        }
    }

    fn take(&mut self, news: WifiNews, now: Millis, watch: Watch) {
        match news {
            WifiNews::Started(true) => {
                if self.note == Some(Note::NoRadio) {
                    self.note = None;
                }
            }
            WifiNews::Started(false) => {
                if self.enabled {
                    self.note = Some(Note::NoRadio);
                }
            }
            WifiNews::Status(status) => {
                // A status is as new as any glance taken before it.
                self.glanced = None;
                if self.enabled {
                    self.status = status;
                } else {
                    self.status.hostname = status.hostname;
                    self.status.ntp = status.ntp;
                }
            }
            WifiNews::Glanced(up) => {
                self.glancing = false;
                self.glance_at = now + GLANCE_MS;
                if self.enabled {
                    self.glanced = Some(up);
                }
            }
            WifiNews::Saved(saved) => {
                self.saved = saved;
                self.refreshing = false;
                self.refresh_at = now + REFRESH_MS;
            }
            WifiNews::Seen(seen) => {
                self.scanning = false;
                // A scan that finishes once the screen has closed, or Wi-Fi has gone off, is
                // for nobody.
                if self.enabled && watch == Watch::Networks {
                    self.seen = Some(seen);
                }
            }
            WifiNews::Asked(Ok(())) => {}
            WifiNews::Asked(Err(fail)) => {
                if let Some((ssid, _)) = self.joining.take() {
                    self.note = Some(match fail {
                        AskFail::BadPassphrase => Note::WrongPassphrase(ssid),
                        AskFail::Radio => Note::NoJoin(ssid),
                    });
                }
            }
        }
    }

    fn joined_ssid(&self) -> Option<&Vec<u8>> {
        self.status
            .ssid
            .as_ref()
            .filter(|_| self.enabled && self.status.joined())
    }

    fn joining_ssid(&self) -> Option<&Vec<u8>> {
        let handshaking = matches!(
            self.status.state.as_str(),
            "AUTHENTICATING" | "ASSOCIATING" | "ASSOCIATED" | "4WAY_HANDSHAKE" | "GROUP_HANDSHAKE"
        );
        self.joining
            .as_ref()
            .map(|(s, _)| s)
            .or_else(|| self.status.ssid.as_ref().filter(|_| handshaking))
    }

    /// The networks to offer: the one joined or being joined first, then those the last scan
    /// heard, strongest first, then the saved ones it did not. Before a scan that is the one
    /// joined and the saved ones, with no signal.
    pub fn entries(&self) -> Vec<Entry> {
        if !self.enabled {
            return Vec::new();
        }
        let saved_id = |ssid: &[u8]| self.saved.iter().find(|s| s.ssid == ssid).map(|s| s.id);
        let state = |ssid: &[u8], saved: Option<u32>| {
            if self.joining_ssid().is_some_and(|s| s == ssid) {
                EntryState::Joining
            } else if self.joined_ssid().is_some_and(|s| s == ssid) {
                EntryState::Joined
            } else if saved.is_some() {
                EntryState::Saved
            } else {
                EntryState::New
            }
        };
        let mut out: Vec<Entry> = self
            .seen
            .iter()
            .flatten()
            .map(|s| {
                let saved = saved_id(&s.ssid);
                Entry {
                    ssid: s.ssid.clone(),
                    rssi: Some(s.rssi),
                    secured: s.secured,
                    saved,
                    state: state(&s.ssid, saved),
                }
            })
            .collect();
        for s in &self.saved {
            if !out.iter().any(|e| e.ssid == s.ssid) {
                out.push(Entry {
                    ssid: s.ssid.clone(),
                    rssi: None,
                    secured: true,
                    saved: Some(s.id),
                    state: state(&s.ssid, Some(s.id)),
                });
            }
        }
        // the network the status says is joined or being joined, should no list name it yet
        for ssid in [self.joining_ssid(), self.joined_ssid()]
            .into_iter()
            .flatten()
        {
            if !out.iter().any(|e| e.ssid == *ssid) {
                out.push(Entry {
                    ssid: ssid.clone(),
                    rssi: None,
                    secured: true,
                    saved: None,
                    state: state(ssid, None),
                });
            }
        }
        out.sort_by_key(|e| match e.state {
            EntryState::Joined | EntryState::Joining => 0,
            _ if e.rssi.is_some() => 1,
            _ => 2,
        });
        out
    }

    pub fn summary(&self) -> Summary {
        match (self.enabled, self.joined_ssid()) {
            (false, _) => Summary::Off,
            (true, Some(ssid)) => Summary::Joined(shown(ssid)),
            (true, None) => Summary::On,
        }
    }

    /// The notes under the networks, a line each, short enough for the list's panel at 640x480:
    /// a scan under way, why a join went wrong, what is under way, or what to do next. Joined and
    /// addressed, the handheld's address on the network, IP 192.168.1.20, and under it, until
    /// a scan, what to press for one.
    pub fn notes(&self) -> Vec<String> {
        if !self.present() {
            return vec!["This handheld has no Wi-Fi.".into()];
        }
        if !self.enabled {
            return vec!["Wi-Fi is off.".into()];
        }
        // X was the last thing asked: its scan says it is under way over anything else
        if self.scanning {
            return vec!["Looking for networks…".into()];
        }
        match &self.note {
            Some(Note::NoRadio) => return vec!["The Wi-Fi radio did not start.".into()],
            Some(Note::WrongPassphrase(s)) => {
                return vec![
                    format!("Could not join {}.", noted(s)),
                    "Wrong passphrase?".into(),
                ]
            }
            Some(Note::NoJoin(s)) => return vec![format!("Could not join {}.", noted(s))],
            None => {}
        }
        if let Some(ssid) = self.joining_ssid() {
            return vec![format!("Joining {}…", noted(ssid))];
        }
        let joined = self.joined_ssid().is_some();
        if joined && self.status.ip.is_none() {
            return vec!["Waiting for an address…".into()];
        }
        // joined and addressed: the address, as WI-FI's panel and SSH say it
        let address = self
            .status
            .ip
            .as_ref()
            .filter(|_| joined)
            .map(|ip| format!("IP {ip}"));
        if self.seen.is_none() {
            return address
                .into_iter()
                .chain(["Press X to look for networks.".to_string()])
                .collect();
        }
        if joined {
            return address.into_iter().collect();
        }
        vec![if self.entries().is_empty() {
            "No networks found. Press X to look again.".into()
        } else {
            "Not connected. Pick a network to join.".into()
        }]
    }
}

/// The most of a network's name a note says: with the words around it, a note then fits the
/// list's panel at 640x480 (43 letters).
const NOTED: usize = 24;

/// A network's name as a note says it, cut short with an ellipsis when it is long.
fn noted(ssid: &[u8]) -> String {
    let name = shown(ssid);
    if name.chars().count() <= NOTED {
        return name;
    }
    let cut: String = name.chars().take(NOTED - 1).collect();
    format!("{}…", cut.trim_end())
}

/// A row of the Network screen, held by what it is rather than where it is, so a scan that
/// reorders the list leaves the cursor on the same network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sel {
    Wifi,
    Clock,
    Ssh,
    Network(Vec<u8>),
}

/// What a panel opened from one of NETWORK's rows by Y tells about: Wi-Fi, network time or SSH.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Topic {
    Wifi,
    Time,
    Ssh,
}

impl Topic {
    /// The topic of NETWORK's row `sel`, or None for a network.
    pub fn of(sel: &Sel) -> Option<Topic> {
        match sel {
            Sel::Wifi => Some(Topic::Wifi),
            Sel::Clock => Some(Topic::Time),
            Sel::Ssh => Some(Topic::Ssh),
            Sel::Network(_) => None,
        }
    }

    /// The row on NETWORK it is opened from, which B goes back to.
    pub fn row(self) -> Sel {
        match self {
            Topic::Wifi => Sel::Wifi,
            Topic::Time => Sel::Clock,
            Topic::Ssh => Sel::Ssh,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Typing {
    pub ssid: Vec<u8>,
    pub replace: Option<u32>,
    pub keys: Keyboard,
    /// The last Join was refused for being too short or too long.
    pub refused: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WifiScreen {
    pub sel: Sel,
    /// Where the cursor was, for when its network drops out of the list.
    pub at: usize,
    /// The first network in view, in a list longer than its panel.
    pub first: usize,
    pub typing: Option<Typing>,
}

impl Default for WifiScreen {
    fn default() -> Self {
        WifiScreen {
            sel: Sel::Wifi,
            at: 0,
            first: 0,
            typing: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Stay,
    Close,
    /// Open the panel that tells about a row: Y on WI-FI, NETWORK TIME or SSH.
    Info(Topic),
    /// Switch SSH on or off.
    SwitchSsh(bool),
}

/// A row of the NETWORK panel, as a kit row shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingLine {
    pub label: &'static str,
    pub value: Option<&'static str>,
    /// Left and Right change its value: carets either side of it while it is lit.
    pub carets: bool,
    /// It opens a screen of its own.
    pub opens: bool,
}

/// A network in the AVAILABLE NETWORKS list, as a kit list row shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkLine {
    /// Its name, in its own case.
    pub name: String,
    /// What it is to the handheld: CONNECTED, JOINING or SAVED.
    pub value: Option<&'static str>,
    /// The bars lit, 0 to 3, while it is in reach.
    pub signal: Option<u8>,
    /// It needs a passphrase.
    pub lock: bool,
}

/// The row under the cursor: one of NETWORK's, or one of the networks in view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lit {
    Setting(usize),
    Network(usize),
}

/// What the Network modal shows, for its faces to be made from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkView {
    /// NETWORK's rows: Wi-Fi, then while it is on Network Time and SSH.
    pub settings: Vec<SettingLine>,
    /// The networks in view, top first: as many as AVAILABLE NETWORKS holds, from the first
    /// in view on.
    pub networks: Vec<NetworkLine>,
    pub lit: Lit,
    /// The notes under the networks, a line each.
    pub notes: Vec<String>,
}

impl WifiScreen {
    fn rows(&self, wifi: &Wifi) -> Vec<Sel> {
        let mut rows = vec![Sel::Wifi];
        if wifi.enabled() {
            rows.push(Sel::Clock);
            rows.push(Sel::Ssh);
            rows.extend(wifi.entries().into_iter().map(|e| Sel::Network(e.ssid)));
        }
        rows
    }

    /// The cursor's row now, settling it on a neighbour if its network has gone, and the list
    /// scrolled, a whole row at a time, so that a network under the cursor is in view.
    fn settle(&mut self, rows: &[Sel], wifi: &Wifi) -> usize {
        let at = rows
            .iter()
            .position(|r| *r == self.sel)
            .unwrap_or_else(|| self.at.min(rows.len() - 1));
        self.sel = rows[at].clone();
        self.at = at;
        let settings = settings_of(rows);
        let networks = rows.len() - settings;
        let in_view = networks_shown(settings, wifi.notes().len());
        // the cursor up in NETWORK leaves the list where it was
        let lit = at.checked_sub(settings).unwrap_or(self.first);
        self.first = network_window(lit, networks, self.first, in_view);
        at
    }

    fn entry(&self, wifi: &Wifi) -> Option<Entry> {
        let Sel::Network(ssid) = &self.sel else {
            return None;
        };
        wifi.entries().into_iter().find(|e| e.ssid == *ssid)
    }

    /// Takes a press, with SSH `ssh_on` or not.
    pub fn input(&mut self, wifi: &mut Wifi, ssh_on: bool, action: Action, now: Millis) -> Outcome {
        if self.typing.is_some() {
            self.type_input(wifi, action, now);
            return Outcome::Stay;
        }
        let rows = self.rows(wifi);
        let at = self.settle(&rows, wifi);
        let mut outcome = Outcome::Stay;
        match action {
            Action::GbaDown(Btn::B) | Action::QuickMenu => return Outcome::Close,
            Action::GbaDown(Btn::Up) => self.sel = rows[(at + rows.len() - 1) % rows.len()].clone(),
            Action::GbaDown(Btn::Down) => self.sel = rows[(at + 1) % rows.len()].clone(),
            Action::GbaDown(Btn::Left | Btn::Right | Btn::A) if self.sel == Sel::Wifi => {
                wifi.set_enabled(!wifi.enabled(), now);
            }
            Action::GbaDown(Btn::Left | Btn::Right | Btn::A) if self.sel == Sel::Clock => {
                wifi.set_ntp(!wifi.ntp());
            }
            Action::GbaDown(Btn::Left | Btn::Right | Btn::A) if self.sel == Sel::Ssh => {
                outcome = Outcome::SwitchSsh(!ssh_on);
            }
            Action::GbaDown(Btn::A) => self.pick(wifi, now),
            // on NETWORK's rows Y opens the row's panel, on a saved network it forgets it
            Action::GbaDown(Btn::Y) => match Topic::of(&self.sel) {
                Some(topic) => return Outcome::Info(topic),
                None => {
                    if let Some(id) = self.entry(wifi).and_then(|e| e.saved) {
                        wifi.forget(id);
                    }
                }
            },
            Action::GbaDown(Btn::X) => wifi.scan(),
            _ => {}
        }
        let rows = self.rows(wifi);
        self.settle(&rows, wifi);
        outcome
    }

    fn pick(&mut self, wifi: &mut Wifi, now: Millis) {
        let Some(e) = self.entry(wifi) else {
            return;
        };
        let turned_away = wifi.note() == Some(&Note::WrongPassphrase(e.ssid.clone()));
        match (e.state, e.saved) {
            (EntryState::Joined | EntryState::Joining, _) => {}
            (_, Some(id)) if !turned_away => wifi.select(id, e.ssid, now),
            _ if !e.secured => wifi.join(e.ssid, String::new(), e.saved, now),
            _ => {
                self.typing = Some(Typing {
                    ssid: e.ssid,
                    replace: e.saved,
                    keys: Keyboard::default(),
                    refused: false,
                });
            }
        }
    }

    fn type_input(&mut self, wifi: &mut Wifi, action: Action, now: Millis) {
        let Some(t) = &mut self.typing else {
            return;
        };
        let keys = &mut t.keys;
        let mut submit = false;
        match action {
            Action::GbaDown(Btn::Up) => keys.up(),
            Action::GbaDown(Btn::Down) => keys.down(),
            Action::GbaDown(Btn::Left) => keys.left(),
            Action::GbaDown(Btn::Right) => keys.right(),
            Action::GbaDown(Btn::A) => submit = keys.press(),
            Action::GbaDown(Btn::B) if keys.text.is_empty() => {
                self.typing = None;
                return;
            }
            Action::GbaDown(Btn::B) => keys.delete(),
            Action::GbaDown(Btn::X) => keys.shift(),
            Action::GbaDown(Btn::Y) => keys.type_char(' '),
            Action::GbaDown(Btn::L1 | Btn::R1) => keys.symbols(),
            Action::GbaDown(Btn::Start) => submit = true,
            Action::QuickMenu => {
                self.typing = None;
                return;
            }
            _ => return,
        }
        t.refused = false;
        if !submit {
            return;
        }
        if !passphrase_ok(&t.keys.text) {
            t.refused = true;
            return;
        }
        let Some(t) = self.typing.take() else {
            return;
        };
        self.sel = Sel::Network(t.ssid.clone());
        wifi.join(t.ssid, t.keys.text, t.replace, now);
    }

    /// Whether Y forgets the network under the cursor: it is one the handheld has saved.
    pub fn forgets(&self, wifi: &Wifi) -> bool {
        self.entry(wifi).is_some_and(|e| e.saved.is_some())
    }

    /// Whether Y opens a panel: the cursor is on one of NETWORK's rows.
    pub fn informs(&self) -> bool {
        Topic::of(&self.sel).is_some()
    }

    /// What the modal shows, with SSH `ssh_on` or not.
    pub fn view(&mut self, wifi: &Wifi, ssh_on: bool) -> NetworkView {
        let rows = self.rows(wifi);
        let at = self.settle(&rows, wifi);
        let settings = settings_of(&rows);
        let notes = wifi.notes();
        let in_view = networks_shown(settings, notes.len());
        let on_off = |on: bool| Some(if on { "ON" } else { "OFF" });
        let setting_lines = rows[..settings]
            .iter()
            .map(|r| match r {
                Sel::Clock => SettingLine {
                    label: "NETWORK TIME",
                    value: on_off(wifi.ntp()),
                    carets: true,
                    opens: false,
                },
                Sel::Ssh => SettingLine {
                    label: "SSH",
                    value: on_off(ssh_on),
                    carets: true,
                    opens: false,
                },
                _ => SettingLine {
                    label: "WI-FI",
                    value: if wifi.present() {
                        on_off(wifi.enabled())
                    } else {
                        None
                    },
                    carets: wifi.present(),
                    opens: false,
                },
            })
            .collect();
        let networks = wifi
            .entries()
            .iter()
            .skip(self.first)
            .take(in_view)
            .map(|e| NetworkLine {
                name: shown(&e.ssid),
                value: match e.state {
                    EntryState::Joined => Some("CONNECTED"),
                    EntryState::Joining => Some("JOINING"),
                    EntryState::Saved => Some("SAVED"),
                    EntryState::New => None,
                },
                // four steps of strength on the kit's dot and two arcs: the weakest lights the
                // dot, the two strongest everything, so a network in reach never looks dead
                signal: e.rssi.map(|r| bars(r).min(3)),
                lock: e.rssi.is_some() && e.secured,
            })
            .collect();
        let lit = match at.checked_sub(settings) {
            None => Lit::Setting(at),
            Some(i) => Lit::Network(i - self.first),
        };
        NetworkView {
            settings: setting_lines,
            networks,
            lit,
            notes,
        }
    }
}

/// How many of `rows` are NETWORK's: the ones before the networks.
fn settings_of(rows: &[Sel]) -> usize {
    rows.iter()
        .position(|r| matches!(r, Sel::Network(_)))
        .unwrap_or(rows.len())
}
