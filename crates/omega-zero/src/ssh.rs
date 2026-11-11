//! SSH: the dropbear server BaseOS's dev images start at boot, which Network switches off and on
//! again, and root's password, which omega-zero makes up on a card whose baseos.conf sets none.
//!
//! Whatever touches the system goes through [`SshServer`]: on the handheld it is dropbear and
//! root's shadow line, on the desktop and in the tests a stand-in, so neither ever starts or
//! stops a real server or changes a real password.

use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[cfg(feature = "device")]
use std::process::{Command, Stdio};
#[cfg(feature = "device")]
use std::sync::mpsc::{channel, Sender};

use omega_input::Millis;
use omega_store::atomic_write;

/// The file on the card BaseOS reads its settings from, `ssh_password` among them.
pub const BASEOS_CONF: &str = "baseos.conf";

/// root's password when baseos.conf sets none: the image's own.
pub const DEFAULT_PASSWORD: &str = "root";

pub const SSH_PORT: u16 = 22;

/// How long after omega-zero starts a server BaseOS starts late is stopped again while SSH is
/// off: rcS starts it in the background, after making its host keys on a first boot.
pub const SETTLE_MS: Millis = 30_000;

/// How often it is looked for in that time.
pub const SETTLE_EVERY_MS: Millis = 2_000;

/// The comment BaseOS puts over the setting, for a baseos.conf that has lost it.
const HEADING: &str = "# SSH password (plain text).";

/// The SSH server and root's password on the running system.
pub trait SshServer: Send {
    /// The system has a server to start: BaseOS's dev images ship dropbear, a release image
    /// may not.
    fn present(&self) -> bool;

    /// Starts the server, unless it is running already.
    fn start(&mut self);

    /// Stops the server listening for new logins. Logins already made carry on.
    fn stop(&mut self);

    /// The baseos.conf on the card at `root`, when it is the one BaseOS gives root its password
    /// from at boot.
    fn baseos_conf(&self, root: &Path) -> Option<PathBuf>;

    /// Gives root `password` on the running system, as BaseOS does at boot.
    fn set_root_password(&mut self, password: &str) -> io::Result<()>;
}

/// The password BaseOS gives root from the text of its baseos.conf, read as baseos-config
/// reads it: the value of the last `ssh_password=` line not commented out, with the spaces round
/// it trimmed and everything else kept, quotes and `#` too. None when there is no such line or
/// its value is empty: root then keeps the image's own.
pub fn conf_password(conf: &str) -> Option<String> {
    let mut password = None;
    for line in conf.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "ssh_password" {
            password = Some(value.trim().to_string());
        }
    }
    password.filter(|p| !p.is_empty())
}

/// root's password, from the text of baseos.conf: its own, or the image's when it sets none.
pub fn ssh_password(conf: &str) -> String {
    conf_password(conf).unwrap_or_else(|| DEFAULT_PASSWORD.to_string())
}

/// root's password on the card at `root`.
pub fn read_ssh_password(root: Option<&Path>) -> String {
    root.and_then(|root| std::fs::read(root.join(BASEOS_CONF)).ok())
        .map_or_else(
            || DEFAULT_PASSWORD.to_string(),
            |conf| ssh_password(&String::from_utf8_lossy(&conf)),
        )
}

/// A new password for root: six digits, the first not a 0, drawn from `random` without
/// favouring any.
pub fn new_password(random: &mut impl Read) -> io::Result<String> {
    const SPAN: u32 = 900_000;
    // A draw at or above the last whole multiple of SPAN would favour the low numbers, so it is
    // drawn again. Fewer than one in 20 000 are, so a source that keeps landing there is broken.
    let fair = u32::MAX - u32::MAX % SPAN;
    for _ in 0..16 {
        let mut bytes = [0; 4];
        random.read_exact(&mut bytes)?;
        let n = u32::from_le_bytes(bytes);
        if n < fair {
            return Ok((100_000 + n % SPAN).to_string());
        }
    }
    Err(io::Error::other(
        "the random source gives only draws too high to use",
    ))
}

/// Whether a line of baseos.conf sets `ssh_password`: Some(true) when it is commented out, as
/// BaseOS ships it, Some(false) when it is not.
fn sets_password(line: &[u8]) -> Option<bool> {
    let text = String::from_utf8_lossy(line);
    let text = text.trim();
    let (commented, rest) = match text.strip_prefix('#') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, text),
    };
    let (key, _) = rest.split_once('=')?;
    (key.trim() == "ssh_password").then_some(commented)
}

fn is_comment(line: &[u8]) -> bool {
    String::from_utf8_lossy(line).trim_start().starts_with('#')
}

fn is_blank(line: &[u8]) -> bool {
    line.iter().all(u8::is_ascii_whitespace)
}

/// The line ending `line` has: `\r\n`, `\n`, or none for a last line without one.
fn ending(line: &[u8]) -> &'static [u8] {
    if line.ends_with(b"\r\n") {
        b"\r\n"
    } else if line.ends_with(b"\n") {
        b"\n"
    } else {
        b""
    }
}

/// `conf`, baseos.conf as it is on the card, with root's password set to `password` the BaseOS
/// way: in place of the line BaseOS reads it from, or of the commented `# ssh_password=` line it
/// ships, or else under the comment about it, or with that comment at the end of the file.
/// Every other line is kept as it was, byte for byte, and a new line ends as the file's do.
pub fn with_password(conf: &[u8], password: &str) -> Vec<u8> {
    let set = format!("ssh_password={password}");
    let lines: Vec<&[u8]> = conf.split_inclusive(|&b| b == b'\n').collect();
    let eol: &[u8] = if conf.windows(2).any(|w| w == b"\r\n") {
        b"\r\n"
    } else {
        b"\n"
    };
    let mut out = Vec::with_capacity(conf.len() + HEADING.len() + set.len() + 4);

    let replace = (0..lines.len())
        .rev()
        .find(|&i| sets_password(lines[i]) == Some(false))
        .or_else(|| (0..lines.len()).find(|&i| sets_password(lines[i]) == Some(true)));
    if let Some(at) = replace {
        for (i, line) in lines.iter().enumerate() {
            if i == at {
                out.extend_from_slice(set.as_bytes());
                out.extend_from_slice(ending(line));
            } else {
                out.extend_from_slice(line);
            }
        }
        return out;
    }

    // under the comment about the setting, past the lines of comment that go on from it
    let heading = lines.iter().position(|line| {
        is_comment(line)
            && String::from_utf8_lossy(line)
                .to_ascii_lowercase()
                .contains("ssh password")
    });
    let at = heading.map_or(lines.len(), |h| {
        (h + 1..lines.len())
            .find(|&i| !is_comment(lines[i]))
            .unwrap_or(lines.len())
    });
    for line in &lines[..at] {
        out.extend_from_slice(line);
    }
    if !out.is_empty() && !out.ends_with(b"\n") {
        out.extend_from_slice(eol);
    }
    if heading.is_none() {
        if lines.last().is_some_and(|line| !is_blank(line)) {
            out.extend_from_slice(eol);
        }
        out.extend_from_slice(HEADING.as_bytes());
        out.extend_from_slice(eol);
    }
    out.extend_from_slice(set.as_bytes());
    out.extend_from_slice(eol);
    for line in &lines[at..] {
        out.extend_from_slice(line);
    }
    out
}

/// What went wrong giving root a password of its own, for the SSH panel to say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trouble {
    /// baseos.conf could not be written: root keeps the password it has.
    NotSaved,
    /// baseos.conf has the new password, but the running system could not be given it: root
    /// has it from the next start.
    NotApplied,
}

impl Trouble {
    /// The SSH panel's note, in place of the one about SFTP.
    pub fn note(self) -> &'static str {
        match self {
            Trouble::NotSaved => "Could not save a new password.",
            Trouble::NotApplied => "Restart to use this password.",
        }
    }
}

/// What omega-zero did about root's password on a card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Password {
    /// baseos.conf gives root one already: the owner's, or one made on an earlier run.
    Kept,
    /// A new one, saved in baseos.conf and given to root.
    Made(String),
    Failed(Trouble),
}

/// Gives root a password of its own on a card whose baseos.conf, at `conf`, sets none: six
/// random digits, written into baseos.conf so BaseOS gives them to root at every boot, and given
/// to root at once through `server`. A password the card sets already is kept, so this makes
/// one only the first time it runs on a card.
pub fn ensure_password(conf: &Path, server: &mut dyn SshServer) -> Password {
    let text = match std::fs::read(conf) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            crate::log!("{}: {e}", conf.display());
            return Password::Failed(Trouble::NotSaved);
        }
    };
    if conf_password(&String::from_utf8_lossy(&text)).is_some() {
        return Password::Kept;
    }
    let made = std::fs::File::open("/dev/urandom").and_then(|mut random| new_password(&mut random));
    let password = match made {
        Ok(password) => password,
        Err(e) => {
            crate::log!("ssh password: /dev/urandom: {e}");
            return Password::Failed(Trouble::NotSaved);
        }
    };
    if let Err(e) = atomic_write(conf, &with_password(&text, &password)) {
        crate::log!("{}: {e}", conf.display());
        return Password::Failed(Trouble::NotSaved);
    }
    if let Err(e) = server.set_root_password(&password) {
        crate::log!("ssh password: {e}");
        return Password::Failed(Trouble::NotApplied);
    }
    Password::Made(password)
}

/// `shadow`'s text with root's password hash replaced by `hash`, every other line and field
/// as it was, as baseos-config's awk does it. None when no line is root's.
pub fn with_root_hash(shadow: &str, hash: &str) -> Option<String> {
    let mut found = false;
    let mut out = String::with_capacity(shadow.len() + hash.len());
    for line in shadow.lines() {
        let mut fields: Vec<&str> = line.split(':').collect();
        if fields.len() > 1 && fields[0] == "root" {
            fields[1] = hash;
            found = true;
            out.push_str(&fields.join(":"));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    found.then_some(out)
}

/// A process as /proc has it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proc {
    pub pid: u32,
    /// The file name it was started by, without its folder: the first word of its cmdline.
    pub name: String,
    /// The inodes of the sockets it holds open.
    pub sockets: Vec<u64>,
}

/// The inodes of the sockets listening on `port` in `table`, the text of /proc/net/tcp or
/// /proc/net/tcp6.
pub fn listening(table: &str, port: u16) -> Vec<u64> {
    let port = format!("{port:04X}");
    table
        .lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let local = fields.get(1)?;
            let listens =
                local.rsplit(':').next() == Some(port.as_str()) && fields.get(3) == Some(&"0A");
            listens.then(|| fields.get(9)?.parse().ok()).flatten()
        })
        .collect()
}

/// The dropbear servers among `procs`: each a dropbear holding one of the `listening` sockets.
/// A login dropbear serves runs in a dropbear of its own that has let go of them, so stopping the
/// servers leaves it be, even once its server is gone.
pub fn listeners(procs: &[Proc], listening: &[u64]) -> Vec<u32> {
    procs
        .iter()
        .filter(|p| p.name == "dropbear" && p.sockets.iter().any(|s| listening.contains(s)))
        .map(|p| p.pid)
        .collect()
}

/// SSH as the app keeps it: the server, what went wrong giving root a password, and while SSH
/// is off in omega-zero's first moments, when to look again for a server BaseOS started late.
pub struct Ssh {
    server: Box<dyn SshServer>,
    trouble: Option<Trouble>,
    settle_at: Option<Millis>,
}

impl Ssh {
    pub fn new(server: Box<dyn SshServer>) -> Self {
        Ssh {
            server,
            trouble: None,
            settle_at: None,
        }
    }

    /// As omega-zero starts: stops the server if SSH was left off, and gives root a password of
    /// its own on a card at `root` that sets none.
    pub fn start(&mut self, on: bool, root: Option<&Path>) {
        if !on {
            self.server.stop();
            self.settle_at = Some(SETTLE_EVERY_MS);
        }
        let conf = root.and_then(|root| self.server.baseos_conf(root));
        self.trouble = match conf.map(|conf| ensure_password(&conf, &mut *self.server)) {
            Some(Password::Failed(trouble)) => Some(trouble),
            _ => None,
        };
    }

    pub fn present(&self) -> bool {
        self.server.present()
    }

    pub fn trouble(&self) -> Option<Trouble> {
        self.trouble
    }

    /// Starts the server or stops it.
    pub fn switch(&mut self, on: bool) {
        self.settle_at = None;
        if on {
            self.server.start();
        } else {
            self.server.stop();
        }
    }

    /// While SSH is off in omega-zero's first moments, stops a server BaseOS started late.
    pub fn tick(&mut self, now: Millis, on: bool) {
        let Some(at) = self.settle_at else {
            return;
        };
        if on || now >= SETTLE_MS {
            self.settle_at = None;
        } else if now >= at {
            self.server.stop();
            self.settle_at = Some(now + SETTLE_EVERY_MS);
        }
    }
}

pub fn ssh_server() -> Box<dyn SshServer> {
    #[cfg(feature = "device")]
    {
        Box::new(Dropbear::spawn())
    }
    #[cfg(not(feature = "device"))]
    {
        Box::new(SimSsh)
    }
}

/// SSH for the desktop and the tests: a server only pretended, on a system with no BaseOS to
/// give root a password.
#[derive(Clone, Copy, Debug, Default)]
pub struct SimSsh;

impl SshServer for SimSsh {
    fn present(&self) -> bool {
        true
    }

    fn start(&mut self) {}

    fn stop(&mut self) {}

    fn baseos_conf(&self, _root: &Path) -> Option<PathBuf> {
        None
    }

    fn set_root_password(&mut self, _password: &str) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(feature = "device")]
const DROPBEAR: &str = "/usr/sbin/dropbear";
#[cfg(feature = "device")]
const DROPBEARKEY: &str = "/usr/sbin/dropbearkey";
/// The host keys, where BaseOS's /etc/init.d/dev keeps them: on /data, so they outlive an
/// update and a login is not refused as a stranger.
#[cfg(feature = "device")]
const HOST_KEYS: [(&str, &str); 2] = [
    ("rsa", "/data/dropbear/dropbear_rsa_host_key"),
    ("ed25519", "/data/dropbear/dropbear_ed25519_host_key"),
];
/// Links to the shadow file baseos-config builds in RAM at boot.
#[cfg(feature = "device")]
const SHADOW: &str = "/etc/shadow";
/// The partition BaseOS reads baseos.conf from at boot: TF1's FAT one, even when the frontend
/// runs from TF2.
#[cfg(feature = "device")]
const TF1: &str = "/dev/mmcblk0p7";
#[cfg(feature = "device")]
const SIGTERM: i32 = 15;

#[cfg(feature = "device")]
extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

/// The handheld's dropbear, started and stopped on a thread of its own, in turn: making host
/// keys on a first start takes seconds.
#[cfg(feature = "device")]
pub struct Dropbear {
    present: bool,
    jobs: Sender<bool>,
}

#[cfg(feature = "device")]
impl Dropbear {
    pub fn spawn() -> Self {
        let (jobs, rx) = channel::<bool>();
        std::thread::spawn(move || {
            for on in rx {
                if on {
                    start_dropbear();
                } else {
                    stop_dropbear();
                }
            }
        });
        Dropbear {
            present: Path::new(DROPBEAR).exists(),
            jobs,
        }
    }
}

#[cfg(feature = "device")]
impl SshServer for Dropbear {
    fn present(&self) -> bool {
        self.present
    }

    fn start(&mut self) {
        if self.present {
            let _ = self.jobs.send(true);
        }
    }

    fn stop(&mut self) {
        let _ = self.jobs.send(false);
    }

    fn baseos_conf(&self, root: &Path) -> Option<PathBuf> {
        let root = std::fs::canonicalize(root).ok()?;
        let mounts = std::fs::read_to_string("/proc/mounts").ok()?;
        let tf1 = mounts.lines().any(|line| {
            let mut fields = line.split_whitespace();
            fields.next() == Some(TF1) && fields.next().map(Path::new) == Some(root.as_path())
        });
        tf1.then(|| root.join(BASEOS_CONF))
    }

    fn set_root_password(&mut self, password: &str) -> io::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let hash = hash_password(password)?;
        let shadow = std::fs::canonicalize(SHADOW)?;
        let text = std::fs::read_to_string(&shadow)?;
        let text = with_root_hash(&text, &hash)
            .ok_or_else(|| io::Error::other("root has no line in the shadow file"))?;
        // as baseos-config writes it: beside it, readable by root alone, then over it
        let tmp = shadow.with_extension("tmp");
        let written = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .and_then(|mut f| f.write_all(text.as_bytes()))
            .and_then(|()| std::fs::rename(&tmp, &shadow));
        if written.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        written
    }
}

/// `password` hashed as baseos-config hashes it: SHA-512 crypt by busybox's mkpasswd, the
/// password on its input so it never shows in a process list.
#[cfg(feature = "device")]
fn hash_password(password: &str) -> io::Result<String> {
    use std::io::Write;

    let mut child = Command::new("mkpasswd")
        .args(["-m", "sha512", "-P", "0"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    if let Some(mut input) = child.stdin.take() {
        writeln!(input, "{password}")?;
    }
    let out = child.wait_with_output()?;
    let hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() && hash.starts_with("$6$") {
        Ok(hash)
    } else {
        Err(io::Error::other("mkpasswd did not hash the password"))
    }
}

/// The dropbear processes /proc lists, with the sockets each holds.
#[cfg(feature = "device")]
fn dropbears() -> Vec<Proc> {
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    dir.flatten()
        .filter_map(|entry| {
            let pid: u32 = entry.file_name().to_str()?.parse().ok()?;
            let path = entry.path();
            let cmdline = std::fs::read(path.join("cmdline")).ok()?;
            let first = cmdline.split(|&b| b == 0).next()?;
            let name = String::from_utf8_lossy(first);
            let name = name.rsplit('/').next()?.to_string();
            if name != "dropbear" {
                return None;
            }
            let sockets = std::fs::read_dir(path.join("fd"))
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|fd| {
                    let link = std::fs::read_link(fd.path()).ok()?;
                    let link = link.to_str()?;
                    link.strip_prefix("socket:[")?
                        .strip_suffix(']')?
                        .parse()
                        .ok()
                })
                .collect();
            Some(Proc { pid, name, sockets })
        })
        .collect()
}

/// The dropbear servers listening for logins on the SSH port.
#[cfg(feature = "device")]
fn servers() -> Vec<u32> {
    let listening: Vec<u64> = ["/proc/net/tcp", "/proc/net/tcp6"]
        .iter()
        .filter_map(|table| std::fs::read_to_string(table).ok())
        .flat_map(|table| listening(&table, SSH_PORT))
        .collect();
    listeners(&dropbears(), &listening)
}

/// Starts dropbear as BaseOS's /etc/init.d/dev does, making the host keys first if they are
/// missing, unless one is listening already.
#[cfg(feature = "device")]
fn start_dropbear() {
    if !servers().is_empty() {
        return;
    }
    let _ = std::fs::create_dir_all("/data/dropbear");
    for (kind, key) in HOST_KEYS {
        if !Path::new(key).exists() {
            let _ = Command::new(DROPBEARKEY)
                .args(["-t", kind, "-f", key])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
    // dropbear goes into the background once it listens, so this waits only for that; it
    // starts from / so it never holds the card open, and as /etc/init.d/dev starts it, with
    // nothing of omega-zero's: not its session, its signals or its framebuffer
    let started = crate::spawn::as_a_daemon(&mut Command::new(DROPBEAR))
        .args(["-r", HOST_KEYS[0].1, "-r", HOST_KEYS[1].1, "-B"])
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match started {
        Ok(status) if status.success() => {}
        Ok(status) => crate::log!("ssh: dropbear {status}"),
        Err(e) => crate::log!("ssh: {DROPBEAR}: {e}"),
    }
}

/// Stops every dropbear listening for logins, leaving the logins it already made running.
#[cfg(feature = "device")]
fn stop_dropbear() {
    for pid in servers() {
        let Ok(pid) = i32::try_from(pid) else {
            continue;
        };
        // SAFETY: kill takes plain integers and touches no memory of ours.
        if unsafe { kill(pid, SIGTERM) } != 0 {
            crate::log!(
                "ssh: stopping dropbear {pid}: {}",
                io::Error::last_os_error()
            );
        }
    }
}
