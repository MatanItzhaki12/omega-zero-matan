//! Update, the row above About in Settings: omega-zero updating itself from its GitHub
//! releases. A asks GitHub for the latest release; when it is newer than this build, A again
//! brings the card's `System/` folder to it and A once more restarts the handheld onto it.
//!
//! A release publishes its `System/` as a manifest of every file and its SHA-256, beside a data
//! file of every file and one of patches from the releases before (see omega-pack). The check
//! sums the files the card has and fetches only the ones that differ, by byte range: a patch
//! where one runs from the file the card has, else the whole file. Each comes out checked
//! against its sum, all are written beside the files they replace, and only then put in
//! place, omega-zero itself kept as `omega-zero.previous`. What the card was given is kept in
//! `System/.installed`, with each file's size and time, so the next check sums only files that
//! changed since, and a file a release drops is taken away.
//!
//! A release with no manifest, from before, is installed as it was: its `omega-zero-aarch64`
//! alone, checked against the SHA-256 published beside it and that it is an aarch64 program.
//!
//! The work runs on a thread of its own, through `curl`, as the label downloads do; the row
//! reads where it stands each frame.

use std::fmt::Write as _;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use omega_pack::{Manifest, Step};

use crate::json::Json;

/// The repository whose releases omega-zero updates from.
pub const REPO: &str = "MatanItzhaki12/omega-zero-matan";

/// The release's omega-zero for the handheld, and its SHA-256 as `sha256sum` writes it.
pub const ASSET: &str = "omega-zero-aarch64";
pub const ASSET_SUM: &str = "omega-zero-aarch64.sha256";

/// What the running build is: the version it was released as. A test build is named by the
/// release workflow, as `2.7.3-test.12`, the twelfth run that built `test`.
pub const VERSION: &str = match option_env!("OMEGA_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

/// Where updates come from: the releases, or the test builds of the `test` branch, each a
/// prerelease of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Test,
}

impl Channel {
    /// The channel Settings has picked: test builds, or the releases.
    pub fn of(test: bool) -> Channel {
        match test {
            true => Channel::Test,
            false => Channel::Stable,
        }
    }

    /// The channel a build of `version` came from.
    pub fn of_version(version: &str) -> Channel {
        Channel::of(version.contains(TEST_TAG))
    }

    /// Where GitHub lists what the channel has: the latest release, or the newest releases,
    /// the test builds among them.
    fn url(self) -> String {
        match self {
            Channel::Stable => format!("https://api.github.com/repos/{REPO}/releases/latest"),
            Channel::Test => format!("https://api.github.com/repos/{REPO}/releases?per_page=30"),
        }
    }

    /// The channel's newest, from what GitHub answered for `url`.
    fn newest(self, body: &str) -> Option<Release> {
        match self {
            Channel::Stable => latest(body),
            Channel::Test => newest_test(body),
        }
    }
}

/// What names a test build's version: `2.7.3-test.12`.
pub const TEST_TAG: &str = "-test.";

/// Where omega-zero is on the card, from its root: what an update replaces. Not the running
/// program's own path, which is the loader `launch_frontend.sh` starts it through,
/// `/lib/ld-linux-aarch64.so.1`.
pub const PROGRAM: &str = "System/omega-zero";

/// omega-zero on the card rooted at `root`.
pub fn program(root: &Path) -> PathBuf {
    root.join(PROGRAM)
}

/// Where the update stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Update {
    /// Nothing asked yet.
    Idle,
    Checking,
    /// This build is the latest release, or newer.
    Latest,
    /// A newer release, and, for one with a manifest, it and how much of its data the card
    /// lacks.
    Available {
        release: Release,
        system: Option<(Arc<Manifest>, u64)>,
    },
    Downloading(String),
    /// The newer release is in place, by its version: a restart runs it.
    Installed(String),
    /// What went wrong, in the row's few capitals.
    Failed(&'static str),
}

impl Update {
    /// What the Update row says on its right.
    pub fn text(&self) -> String {
        match self {
            Update::Idle => format!("V{}", shown(VERSION)),
            Update::Checking => "CHECKING…".to_string(),
            Update::Latest => "UP TO DATE".to_string(),
            Update::Available { release, .. } => format!("INSTALL V{}", shown(&release.version)),
            Update::Downloading(_) => "DOWNLOADING…".to_string(),
            Update::Installed(_) => "RESTART".to_string(),
            Update::Failed(why) => why.to_string(),
        }
    }

    /// The row's note: what A does now.
    pub fn note(&self) -> String {
        match self {
            Update::Idle | Update::Latest | Update::Failed(_) => {
                format!("A checks for a newer omega-zero-matan than v{VERSION}")
            }
            Update::Checking => "Asking GitHub for the latest release".to_string(),
            Update::Available {
                release,
                system: Some((_, bytes)),
            } => format!(
                "A installs v{} ({}), X puts it off",
                release.version,
                size_text(*bytes)
            ),
            Update::Available { release, .. } => {
                format!("A installs v{}, X puts it off", release.version)
            }
            Update::Downloading(version) => format!("Downloading v{version}: X stops it"),
            Update::Installed(version) => format!("A restarts onto v{version}"),
        }
    }

    pub fn busy(&self) -> bool {
        matches!(self, Update::Checking | Update::Downloading(_))
    }
}

/// How long UP TO DATE, or what went wrong, stays on the row before the version shows again.
pub const SAID_FOR: std::time::Duration = std::time::Duration::from_secs(4);

/// Where the update stands, and since when.
struct Held {
    update: Update,
    at: std::time::Instant,
}

/// The update under way, shared with the thread doing it.
#[derive(Clone)]
pub struct Updater {
    state: Arc<Mutex<Held>>,
    /// Whether the install under way is to stop, or is past where it can.
    gate: Arc<Gate>,
}

impl Default for Updater {
    fn default() -> Self {
        Updater {
            state: Arc::new(Mutex::new(Held {
                update: Update::Idle,
                at: std::time::Instant::now(),
            })),
            gate: Arc::default(),
        }
    }
}

impl Updater {
    /// Where the update stands: UP TO DATE and what went wrong only for `SAID_FOR`, the
    /// version after.
    pub fn state(&self) -> Update {
        let Ok(mut held) = self.state.lock() else {
            return Update::Idle;
        };
        if matches!(held.update, Update::Latest | Update::Failed(_))
            && held.at.elapsed() >= SAID_FOR
        {
            held.update = Update::Idle;
        }
        held.update.clone()
    }

    fn set(&self, to: Update) {
        if let Ok(mut held) = self.state.lock() {
            held.update = to;
            held.at = std::time::Instant::now();
        }
    }

    /// Stops the download under way before its next file: nothing it fetched is put in place,
    /// and the card stays as it was. False where none is under way to stop.
    pub fn stop(&self) -> bool {
        matches!(self.state(), Update::Downloading(_)) && self.gate.ask_stop()
    }

    /// Forgets an update found, as Settings switches channel and the other's is looked for.
    /// A check or a download under way, or an update put in place, stays.
    pub fn forget(&self) {
        if matches!(
            self.state(),
            Update::Available { .. } | Update::Latest | Update::Failed(_)
        ) {
            self.set(Update::Idle);
        }
    }

    /// Settings closing: UP TO DATE or what went wrong goes, and the version running shows
    /// again. An update found stays, for INSTALL; a check or a download under way goes on, and
    /// an update put in place still asks for its restart.
    pub fn settle(&self) {
        if let Ok(mut held) = self.state.lock() {
            if matches!(held.update, Update::Latest | Update::Failed(_)) {
                held.update = Update::Idle;
            }
        }
    }

    /// Checks `channel` without saying so: an update found shows as INSTALL, and nothing else
    /// shows, neither UP TO DATE nor what went wrong. For the check as Wi-Fi first joins a
    /// network, and as Settings switches channel.
    pub fn check_quietly(&self, root: Option<PathBuf>, channel: Channel) {
        if !matches!(self.state(), Update::Idle) {
            return;
        }
        let me = self.clone();
        let _ = std::thread::Builder::new()
            .name("omega-update".into())
            .spawn(move || {
                crate::spawn::stand_back();
                let Fetched::Body(body) = fetch(&channel.url()) else {
                    return;
                };
                let Some(release) = channel.newest(&String::from_utf8_lossy(&body)) else {
                    return;
                };
                crate::log!("update: {channel:?} has {}", release.version);
                if !offered(channel, &release.version, VERSION) {
                    return;
                }
                let system = root.and_then(|root| lacking(&root, &release));
                // a check asked for meanwhile has the say
                if matches!(me.state(), Update::Idle) {
                    me.set(Update::Available { release, system });
                }
            });
    }

    /// Asks GitHub for `channel`'s newest, on a thread of its own, and, for one with a
    /// manifest, what of it the card rooted at `root` lacks.
    pub fn check(&self, root: Option<PathBuf>, channel: Channel) {
        if self.state().busy() {
            return;
        }
        self.set(Update::Checking);
        let me = self.clone();
        let spawned = std::thread::Builder::new()
            .name("omega-update".into())
            .spawn(move || {
                crate::spawn::stand_back();
                let found = match fetch(&channel.url()) {
                    Fetched::Body(body) => match channel.newest(&String::from_utf8_lossy(&body)) {
                        Some(release) if offered(channel, &release.version, VERSION) => {
                            let system = root.and_then(|root| lacking(&root, &release));
                            Update::Available { release, system }
                        }
                        // this release, where the card lacks some of it: an update from before
                        // 2.6.1 brought omega-zero alone
                        Some(release) if release.version == VERSION => {
                            match root.and_then(|root| lacking(&root, &release)) {
                                Some((manifest, bytes)) if bytes > 0 => Update::Available {
                                    release,
                                    system: Some((manifest, bytes)),
                                },
                                _ => Update::Latest,
                            }
                        }
                        Some(_) => Update::Latest,
                        None => Update::Failed("NO RELEASE YET"),
                    },
                    fetched => Update::Failed(fetched.why()),
                };
                me.set(found);
            });
        if spawned.is_err() {
            self.set(Update::Failed("FAILED"));
        }
    }

    /// Brings the card rooted at `root` to the release found, on a thread of its own.
    pub fn install(&self, root: PathBuf) {
        let Update::Available { release, system } = self.state() else {
            return;
        };
        let version = release.version.clone();
        self.set(Update::Downloading(version.clone()));
        self.gate.reset();
        let me = self.clone();
        let spawned = std::thread::Builder::new()
            .name("omega-update".into())
            .spawn(move || {
                crate::spawn::stand_back();
                let gate = &*me.gate;
                let done = match system {
                    Some((manifest, _)) => install_system(&root, &release, &manifest, gate),
                    None => install(&program(&root), &release.program, &release.sum, gate),
                };
                let done = match done {
                    Ok(()) => Update::Installed(version),
                    // stopped: the card as it was, and the update offered again
                    Err(STOPPED) => {
                        crate::log!("update: stopped before v{version} was put in place");
                        Update::Idle
                    }
                    Err(why) => {
                        crate::log!("update: {why}");
                        Update::Failed(why)
                    }
                };
                me.set(done);
            });
        if spawned.is_err() {
            self.set(Update::Failed("FAILED"));
        }
    }
}

/// Where an install stands for X on Update: under way, asked to stop, or putting its files in
/// place, past where it can stop. Settings asks and the install's thread commits, each in one
/// step, so a stop is either taken before anything is in place or refused.
#[derive(Default)]
pub struct Gate(std::sync::atomic::AtomicU8);

const UNDER_WAY: u8 = 0;
const STOP_ASKED: u8 = 1;
const COMMITTED: u8 = 2;

impl Gate {
    fn reset(&self) {
        self.0
            .store(UNDER_WAY, std::sync::atomic::Ordering::Release);
    }

    /// Asks the install to stop; false where it is already putting its files in place.
    pub fn ask_stop(&self) -> bool {
        use std::sync::atomic::Ordering;
        match self
            .0
            .compare_exchange(UNDER_WAY, STOP_ASKED, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => true,
            Err(now) => now == STOP_ASKED,
        }
    }

    /// Whether a stop was asked for.
    pub fn stopped(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Acquire) == STOP_ASKED
    }

    /// The install about to put its files in place: true where no stop came first, and none
    /// can now.
    pub fn commit(&self) -> bool {
        use std::sync::atomic::Ordering;
        self.0
            .compare_exchange(UNDER_WAY, COMMITTED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}

/// A release, as GitHub lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    /// Its omega-zero-aarch64, and that one's sum.
    pub program: String,
    pub sum: String,
    /// Its System manifest, files data and patches data, for a release that has them.
    pub system: Option<[String; 3]>,
}

/// `bytes` as the note says it: in KB under a megabyte, else in MB to a tenth.
pub fn size_text(bytes: u64) -> String {
    if bytes < 1_000_000 {
        format!("{} KB", bytes.div_ceil(1000).max(1))
    } else {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    }
}

/// The latest release, from GitHub's answer for it: none where it has no omega-zero for the
/// handheld.
pub fn latest(body: &str) -> Option<Release> {
    release_of(&Json::parse(body)?)
}

/// A release, from GitHub's object for it: none where it has no omega-zero for the handheld.
fn release_of(release: &Json) -> Option<Release> {
    let tag = release.get("tag_name")?.text()?;
    let version = tag.trim_start_matches('v').to_string();
    let asset = |name: &str| {
        release
            .get("assets")?
            .list()
            .iter()
            .find(|a| a.get("name").and_then(Json::text) == Some(name))?
            .get("browser_download_url")?
            .text()
            .map(str::to_string)
    };
    let system = (|| {
        Some([
            asset(omega_pack::MANIFEST_ASSET)?,
            asset(omega_pack::FILES_ASSET)?,
            asset(omega_pack::PATCHES_ASSET)?,
        ])
    })();
    Some(Release {
        version,
        program: asset(ASSET)?,
        sum: asset(ASSET_SUM)?,
        system,
    })
}

/// Whether `found`, the newest on `channel`, is offered over the running build: a newer one on
/// the build's own channel, and on the other, its newest whatever it is, as switching channel
/// is asking to be on it.
pub fn offered(channel: Channel, found: &str, running: &str) -> bool {
    match Channel::of_version(running) == channel {
        true => newer(found, running),
        false => found != running,
    }
}

/// The newest test build in GitHub's list of releases.
pub fn newest_test(body: &str) -> Option<Release> {
    let list = Json::parse(body)?;
    list.list()
        .iter()
        .filter(|r| {
            r.get("tag_name")
                .and_then(Json::text)
                .is_some_and(|t| t.contains(TEST_TAG))
        })
        .filter_map(release_of)
        .max_by(
            |a, b| match (newer(&a.version, &b.version), a.version == b.version) {
                (true, _) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Equal,
                (false, false) => std::cmp::Ordering::Less,
            },
        )
}

/// A version as the Update row says it: in capitals, a test build's run after a T, as
/// `2.7.3-T12`, so INSTALL fits beside it.
pub fn shown(version: &str) -> String {
    version.replace(TEST_TAG, "-T").to_uppercase()
}

/// Whether version `a` is newer than `b`, each as dotted numbers: `1.10.0` is newer than
/// `1.9.2`. A part that is not a number counts as none. A test build comes before the release
/// of its version, `2.7.3-test.4` before `2.7.3`, and after the one before it.
pub fn newer(a: &str, b: &str) -> bool {
    let test = |v: &str| {
        v.split_once(TEST_TAG)
            .map_or(u64::MAX, |(_, n)| n.parse().unwrap_or(0))
    };
    let base = |v: &str| v.split(['-', '+']).next().unwrap_or("").to_string();
    if base(a) == base(b) {
        return test(a) > test(b);
    }
    newer_numbers(&base(a), &base(b))
}

fn newer_numbers(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.split(['.', '-', '+'])
            .take(3)
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    parts(a) > parts(b)
}

/// Whether `bytes` start as an aarch64 Linux program: a 64-bit little-endian ELF for machine
/// 183. Anything else would leave the handheld with no omega-zero to start.
pub fn is_aarch64_elf(bytes: &[u8]) -> bool {
    bytes.len() > 20
        && bytes[..4] == *b"\x7fELF"
        && bytes[4] == 2
        && bytes[5] == 1
        && u16::from_le_bytes([bytes[18], bytes[19]]) == 183
}

/// The first word of `sum`, a file `sha256sum` wrote, if it is a SHA-256 in hex.
pub fn sum_of(sum: &str) -> Option<String> {
    let hex = sum.split_whitespace().next()?.to_ascii_lowercase();
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hex)
}

fn install(program: &Path, url: &str, sum_url: &str, gate: &Gate) -> Result<(), &'static str> {
    if !program.is_file() {
        return Err("NOT ON CARD");
    }
    let sum = fetch_with(sum_url, &[], Some(gate)).body()?;
    let want = sum_of(&String::from_utf8_lossy(&sum)).ok_or("BAD RELEASE")?;
    let bytes = fetch_with(url, &[], Some(gate)).body()?;
    if sha256_hex(&bytes) != want {
        return Err("BAD DOWNLOAD");
    }
    if !is_aarch64_elf(&bytes) {
        return Err("BAD RELEASE");
    }
    let dir = program.parent().ok_or("NOT ON CARD")?;
    let name = program.file_name().ok_or("NOT ON CARD")?.to_string_lossy();
    let fresh = dir.join(format!("{name}.download"));
    let previous = dir.join(format!("{name}.previous"));
    if !gate.commit() {
        return Err(STOPPED);
    }
    std::fs::write(&fresh, &bytes).map_err(|e| unwritten(&e))?;
    set_executable(&fresh);
    // the running one is kept to go back to; the new one takes its name in one step
    std::fs::copy(program, &previous).map_err(|e| unwritten(&e))?;
    std::fs::rename(&fresh, program).map_err(|e| unwritten(&e))?;
    Ok(())
}

/// Where the card keeps what it was given: one line a file, its sum, size, time and path.
pub const INSTALLED: &str = "System/.installed";

/// A file as `System/.installed` has it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Had {
    pub sha: String,
    pub size: u64,
    pub mtime: u64,
    pub path: String,
}

pub fn read_installed(root: &Path) -> Vec<Had> {
    let text = std::fs::read_to_string(root.join(INSTALLED)).unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let mut f = line.splitn(4, ' ');
            Some(Had {
                sha: f.next()?.to_string(),
                size: f.next()?.parse().ok()?,
                mtime: f.next()?.parse().ok()?,
                path: f.next()?.to_string(),
            })
        })
        .collect()
}

/// A file's size and time, as `System/.installed` keeps them.
fn stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    meta.is_file().then_some((meta.len(), mtime))
}

/// The sum of the file at `rel` on the card rooted at `root`: the one `had` gives while its
/// size and time are as they were, else summed afresh. None for no file.
pub fn sum_on_card(root: &Path, rel: &str, had: &[Had]) -> Option<String> {
    let path = root.join(rel);
    let (size, mtime) = stamp(&path)?;
    if let Some(h) = had
        .iter()
        .find(|h| h.path == rel && h.size == size && h.mtime == mtime)
    {
        return Some(h.sha.clone());
    }
    omega_pack::sha256::file(&path).ok()
}

/// The release's manifest, and how much of its data the card rooted at `root` lacks.
fn lacking(root: &Path, release: &Release) -> Option<(Arc<Manifest>, u64)> {
    let [manifest_url, ..] = release.system.as_ref()?;
    let text = fetch(manifest_url).body().ok()?;
    let manifest = Manifest::parse(&String::from_utf8_lossy(&text))?;
    let had = read_installed(root);
    let bytes = omega_pack::plan(&manifest, |rel| sum_on_card(root, rel, &had))
        .iter()
        .map(Step::bytes)
        .sum();
    Some((Arc::new(manifest), bytes))
}

/// The bytes `start` for `len` of the file at `url`.
fn fetch_range(url: &str, start: u64, len: u64, gate: &Gate) -> Result<Vec<u8>, &'static str> {
    if len == 0 {
        return Ok(Vec::new());
    }
    let range = format!("{start}-{}", start + len - 1);
    let body = fetch_with(url, &["-r", &range], Some(gate)).body()?;
    match body.len() as u64 {
        // the range asked for
        n if n == len => Ok(body),
        // a server that sent all of it
        n if n >= start + len => Ok(body[start as usize..(start + len) as usize].to_vec()),
        _ => Err("BAD DOWNLOAD"),
    }
}

/// Brings the card rooted at `root` to `manifest`: fetches what differs, writes each file
/// beside the one it replaces, then, all of them made, puts them in place.
/// What an install stopped from Settings ends in: the card as it was.
pub const STOPPED: &str = "STOPPED";

fn install_system(
    root: &Path,
    release: &Release,
    manifest: &Manifest,
    gate: &Gate,
) -> Result<(), &'static str> {
    let [_, files_url, patches_url] = release.system.as_ref().ok_or("BAD RELEASE")?;
    if !program(root).is_file() {
        return Err("NOT ON CARD");
    }
    let had = read_installed(root);
    let steps = omega_pack::plan(manifest, |rel| sum_on_card(root, rel, &had));
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut make = || -> Result<(), &'static str> {
        for step in &steps {
            // asked to stop: before the next file, nothing yet in place (a file downloading
            // is stopped where it is, by `fetch_with`)
            if gate.stopped() {
                return Err(STOPPED);
            }
            let file = step.file();
            let to = root.join(&file.path);
            let (piece, old) = match step {
                Step::Whole(f) => (fetch_range(files_url, f.offset, f.len, gate)?, None),
                Step::Patch(_, p) => (
                    fetch_range(patches_url, p.offset, p.len, gate)?,
                    Some(std::fs::read(&to).map_err(|e| unwritten(&e))?),
                ),
            };
            let bytes =
                omega_pack::rebuild(step, &piece, old.as_deref()).map_err(|_| "BAD DOWNLOAD")?;
            if file.path == PROGRAM && !is_aarch64_elf(&bytes) {
                return Err("BAD RELEASE");
            }
            let fresh = PathBuf::from(format!("{}.download", to.display()));
            if let Some(dir) = fresh.parent() {
                std::fs::create_dir_all(dir).map_err(|e| unwritten(&e))?;
            }
            std::fs::write(&fresh, &bytes).map_err(|e| unwritten(&e))?;
            if file.exec {
                set_executable(&fresh);
            }
            staged.push((fresh, to));
        }
        Ok(())
    };
    // every file fetched: a stop asked while the last came in is taken here, and none after
    let made = make().and_then(|()| if gate.commit() { Ok(()) } else { Err(STOPPED) });
    if let Err(why) = made {
        for (fresh, _) in &staged {
            let _ = std::fs::remove_file(fresh);
        }
        return Err(why);
    }
    // the running one is kept to go back to
    let program = program(root);
    if staged.iter().any(|(_, to)| *to == program) {
        let previous = program.with_file_name("omega-zero.previous");
        std::fs::copy(&program, previous).map_err(|e| unwritten(&e))?;
    }
    for (fresh, to) in &staged {
        std::fs::rename(fresh, to).map_err(|e| unwritten(&e))?;
    }
    let had_paths: Vec<String> = had.iter().map(|h| h.path.clone()).collect();
    for rel in omega_pack::gone(&had_paths, manifest) {
        let _ = std::fs::remove_file(root.join(rel));
    }
    write_installed(root, manifest).map_err(|e| unwritten(&e))?;
    let _ = Command::new("sync").status();
    Ok(())
}

/// Records the files of `manifest` the card rooted at `root` has, as they are now.
pub fn write_installed(root: &Path, manifest: &Manifest) -> std::io::Result<()> {
    let mut record = String::new();
    for f in &manifest.files {
        if let Some((size, mtime)) = stamp(&root.join(&f.path)) {
            let _ = writeln!(record, "{} {size} {mtime} {}", f.sha, f.path);
        }
    }
    let fresh = root.join(format!("{INSTALLED}.download"));
    std::fs::write(&fresh, record)?;
    std::fs::rename(&fresh, root.join(INSTALLED))
}

/// What the row says for a write to the card that failed with `e`: CARD FULL only when the
/// card is.
pub fn unwritten(e: &std::io::Error) -> &'static str {
    const NO_SPACE: i32 = 28;
    const READ_ONLY: i32 = 30;
    match (e.kind(), e.raw_os_error()) {
        (_, Some(NO_SPACE)) | (std::io::ErrorKind::StorageFull, _) => "CARD FULL",
        (_, Some(READ_ONLY)) | (std::io::ErrorKind::PermissionDenied, _) => "CARD READ-ONLY",
        _ => "WRITE FAILED",
    }
}

#[cfg(unix)]
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) {}

/// What `url` answers, following redirects, or none where it cannot be fetched.
/// What asking GitHub came to.
#[derive(Debug, PartialEq, Eq)]
pub enum Fetched {
    Body(Vec<u8>),
    /// GitHub answered, with this status rather than the thing asked for: 404 for a repository
    /// with no release yet.
    Status(u16),
    /// Nothing answered: no network, or none that reaches GitHub.
    Unreachable,
    /// X on Update stopped it as it came.
    Stopped,
}

impl Fetched {
    /// What the row says for an answer that is not the thing asked for.
    pub fn why(&self) -> &'static str {
        match self {
            Fetched::Body(_) => "FAILED",
            Fetched::Status(404) => "NO RELEASE YET",
            Fetched::Status(403 | 429) => "GITHUB BUSY",
            Fetched::Status(_) => "GITHUB ERROR",
            Fetched::Unreachable => "NO NETWORK",
            Fetched::Stopped => STOPPED,
        }
    }

    fn body(self) -> Result<Vec<u8>, &'static str> {
        match self {
            Fetched::Body(body) => Ok(body),
            other => Err(other.why()),
        }
    }
}

/// Splits what `curl -w '\n%{http_code}'` wrote into the body and the status after it.
pub fn answered(out: &[u8]) -> Fetched {
    let Some(split) = out.iter().rposition(|&b| b == b'\n') else {
        return Fetched::Unreachable;
    };
    let status: u16 = std::str::from_utf8(&out[split + 1..])
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    match status {
        0 => Fetched::Unreachable,
        200..=299 => Fetched::Body(out[..split].to_vec()),
        status => Fetched::Status(status),
    }
}

/// What `url` answers, following redirects.
fn fetch(url: &str) -> Fetched {
    fetch_with(url, &[], None)
}

/// What `url` answers, following redirects, curl given `extra` as well.
/// With a `gate`, curl is ended as soon as a stop is asked for, however much of the file has
/// come: a release's program is one file of some MB, which would otherwise run to its end.
fn fetch_with(url: &str, extra: &[&str], gate: Option<&Gate>) -> Fetched {
    let child = Command::new("curl")
        .args([
            "-sL",
            "-m",
            "300",
            "-A",
            "omega-zero-matan",
            "-w",
            "\n%{http_code}",
        ])
        .args(extra)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        return Fetched::Unreachable;
    };
    let stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        let read = stdout.is_some_and(|mut s| s.read_to_end(&mut out).is_ok());
        (read, out)
    });
    while !reader.is_finished() {
        if gate.is_some_and(Gate::stopped) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Fetched::Stopped;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let _ = child.wait();
    match reader.join() {
        Ok((true, out)) => answered(&out),
        _ => Fetched::Unreachable,
    }
}

/// `bytes`' SHA-256, in lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    omega_pack::sha256::hex(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_the_standard_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let long = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        assert_eq!(
            sha256_hex(long),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn a_version_is_newer_by_its_numbers_not_its_letters() {
        assert!(newer("1.3.0", "1.2.0"));
        assert!(newer("1.10.0", "1.9.2"));
        assert!(newer("2.0.0", "1.99.99"));
        assert!(!newer("1.2.0", "1.2.0"));
        assert!(!newer("1.1.9", "1.2.0"));
    }

    #[test]
    fn a_test_build_comes_between_the_release_before_and_its_own() {
        assert!(newer("2.7.3-test.4", "2.7.2"));
        assert!(newer("2.7.3", "2.7.3-test.4"));
        assert!(newer("2.7.3-test.10", "2.7.3-test.9"));
        assert!(!newer("2.7.3-test.4", "2.7.3-test.4"));
        assert!(!newer("2.7.2", "2.7.3-test.1"));
    }

    #[test]
    fn a_channel_offers_newer_and_switching_offers_its_newest_whatever_it_is() {
        // on its own channel, only what is newer
        assert!(offered(Channel::Stable, "2.7.3", "2.7.2"));
        assert!(!offered(Channel::Stable, "2.7.2", "2.7.2"));
        assert!(!offered(Channel::Stable, "2.7.1", "2.7.2"));
        assert!(offered(Channel::Test, "2.7.3-test.5", "2.7.3-test.4"));
        assert!(!offered(Channel::Test, "2.7.3-test.4", "2.7.3-test.4"));
        // switched: the other channel's newest, older or not
        assert!(offered(Channel::Test, "2.7.3-test.1", "2.7.2"));
        assert!(offered(Channel::Stable, "2.7.2", "2.7.3-test.6"));
        assert_eq!(Channel::of_version("2.7.3-test.6"), Channel::Test);
        assert_eq!(Channel::of_version("2.7.2"), Channel::Stable);
    }

    #[test]
    fn the_newest_test_build_is_found_among_the_releases() {
        let asset = |v: &str| {
            format!(
                r#"{{"tag_name":"v{v}","prerelease":true,"assets":[
                {{"name":"omega-zero-aarch64","browser_download_url":"https://x/{v}/bin"}},
                {{"name":"omega-zero-aarch64.sha256","browser_download_url":"https://x/{v}/sum"}}]}}"#
            )
        };
        let body = format!(
            "[{},{},{},{}]",
            asset("2.7.3-test.9"),
            asset("2.7.2"),
            asset("2.7.3-test.10"),
            asset("2.7.3-test.2")
        );
        let found = newest_test(&body).expect("a test build");
        assert_eq!(found.version, "2.7.3-test.10");
        assert_eq!(found.program, "https://x/2.7.3-test.10/bin");
        assert_eq!(newest_test(&format!("[{}]", asset("2.7.2"))), None);
    }

    #[test]
    fn the_row_names_the_version_it_installs() {
        let release = Release {
            version: "2.7.3-test.12".into(),
            program: String::new(),
            sum: String::new(),
            system: None,
        };
        let found = Update::Available {
            release,
            system: None,
        };
        assert_eq!(found.text(), "INSTALL V2.7.3-T12");
        assert_eq!(shown("2.7.2"), "2.7.2");
    }

    #[test]
    fn the_latest_release_names_its_omega_zero_and_sum() {
        let body = r#"{"tag_name":"v1.3.0","assets":[
            {"name":"omega-zero-v1.3.0-h700.zip","browser_download_url":"https://x/zip"},
            {"name":"omega-zero-aarch64","browser_download_url":"https://x/bin"},
            {"name":"omega-zero-aarch64.sha256","browser_download_url":"https://x/sum"}]}"#;
        assert_eq!(
            latest(body),
            Some(Release {
                version: "1.3.0".to_string(),
                program: "https://x/bin".to_string(),
                sum: "https://x/sum".to_string(),
                system: None,
            })
        );
        let with_system = r#"{"tag_name":"v2.7.0","assets":[
            {"name":"omega-zero-aarch64","browser_download_url":"https://x/bin"},
            {"name":"omega-zero-aarch64.sha256","browser_download_url":"https://x/sum"},
            {"name":"system-manifest.txt","browser_download_url":"https://x/m"},
            {"name":"system-files.bin","browser_download_url":"https://x/f"},
            {"name":"system-patches.bin","browser_download_url":"https://x/p"}]}"#;
        assert_eq!(
            latest(with_system).and_then(|r| r.system),
            Some(["https://x/m", "https://x/f", "https://x/p"].map(String::from))
        );
        let zip_only = r#"{"tag_name":"v1.3.0","assets":[
            {"name":"omega-zero-v1.3.0-h700.zip","browser_download_url":"https://x/zip"}]}"#;
        assert_eq!(
            latest(zip_only),
            None,
            "a release with no omega-zero of its own"
        );
    }

    #[test]
    fn only_an_aarch64_program_is_put_in_place() {
        let mut elf = vec![0u8; 64];
        elf[..4].copy_from_slice(b"\x7fELF");
        elf[4] = 2;
        elf[5] = 1;
        elf[18..20].copy_from_slice(&183u16.to_le_bytes());
        assert!(is_aarch64_elf(&elf));
        elf[18..20].copy_from_slice(&62u16.to_le_bytes());
        assert!(!is_aarch64_elf(&elf), "an x86-64 program");
        assert!(!is_aarch64_elf(b"<html>not found</html>"));
    }

    #[test]
    fn github_with_no_release_is_not_a_missing_network() {
        assert_eq!(answered(b"{}\n200"), Fetched::Body(b"{}".to_vec()));
        assert_eq!(
            answered(b"{\"message\":\"Not Found\"}\n404").why(),
            "NO RELEASE YET"
        );
        assert_eq!(answered(b"\n403").why(), "GITHUB BUSY");
        assert_eq!(answered(b"\n000").why(), "NO NETWORK");
        assert_eq!(answered(b"").why(), "NO NETWORK");
    }

    #[test]
    fn only_a_full_card_says_it_is_full() {
        let e = |code| std::io::Error::from_raw_os_error(code);
        assert_eq!(unwritten(&e(28)), "CARD FULL");
        assert_eq!(unwritten(&e(30)), "CARD READ-ONLY");
        assert_eq!(unwritten(&e(13)), "CARD READ-ONLY");
        assert_eq!(unwritten(&e(5)), "WRITE FAILED");
    }

    #[test]
    fn the_update_replaces_omega_zero_on_the_card_never_the_loader() {
        let root = Path::new("/mnt/sdcard");
        assert_eq!(program(root), Path::new("/mnt/sdcard/System/omega-zero"));
    }

    #[test]
    fn closing_settings_takes_a_message_away_but_not_a_restart_or_a_download() {
        let u = Updater::default();
        let found = Update::Available {
            release: Release {
                version: "2.7.0".into(),
                program: String::new(),
                sum: String::new(),
                system: None,
            },
            system: None,
        };
        for (said, kept) in [
            (Update::Failed("CARD FULL"), Update::Idle),
            (Update::Latest, Update::Idle),
            (found.clone(), found),
            (
                Update::Downloading("2.2.0".into()),
                Update::Downloading("2.2.0".into()),
            ),
            (
                Update::Installed("2.2.0".into()),
                Update::Installed("2.2.0".into()),
            ),
        ] {
            u.set(said);
            u.settle();
            assert_eq!(u.state(), kept);
        }
        assert_eq!(Update::Idle.text(), format!("V{VERSION}"));
    }

    #[test]
    fn the_note_says_how_much_an_update_downloads() {
        assert_eq!(size_text(1), "1 KB");
        assert_eq!(size_text(412_300), "413 KB");
        assert_eq!(size_text(6_595_088), "6.6 MB");
    }

    #[test]
    fn a_file_is_summed_again_only_once_it_changes() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        std::fs::create_dir_all(root.join("System")).unwrap();
        std::fs::write(root.join("System/wifi.sh"), b"echo wifi").unwrap();
        let real = omega_pack::sha256::hex(b"echo wifi");
        let m = Manifest {
            version: "1".into(),
            files: vec![omega_pack::FileEntry {
                path: "System/wifi.sh".into(),
                sha: real.clone(),
                size: 9,
                exec: true,
                offset: 0,
                len: 0,
            }],
            patches: vec![],
        };
        write_installed(root, &m).unwrap();
        let mut had = read_installed(root);
        assert_eq!(had.len(), 1);
        assert_eq!(
            sum_on_card(root, "System/wifi.sh", &had),
            Some(real.clone())
        );
        // a record that says otherwise is taken while the file's size and time stand
        had[0].sha = "f".repeat(64);
        assert_eq!(
            sum_on_card(root, "System/wifi.sh", &had),
            Some("f".repeat(64))
        );
        had[0].size += 1;
        assert_eq!(sum_on_card(root, "System/wifi.sh", &had), Some(real));
        assert_eq!(sum_on_card(root, "System/none.sh", &had), None);
    }

    #[test]
    fn up_to_date_says_so_for_a_few_seconds_then_the_version_shows() {
        let u = Updater::default();
        u.set(Update::Latest);
        assert_eq!(u.state(), Update::Latest);
        if let Ok(mut held) = u.state.lock() {
            held.at -= SAID_FOR;
        }
        assert_eq!(u.state(), Update::Idle);
    }

    #[test]
    fn a_sum_file_gives_its_hex() {
        let hex = "ab".repeat(32);
        assert_eq!(sum_of(&format!("{hex}  omega-zero-aarch64\n")), Some(hex));
        assert_eq!(sum_of("Not Found"), None);
    }

    #[test]
    fn a_stopped_install_leaves_the_card_as_it_was() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        std::fs::create_dir_all(root.join("System")).unwrap();
        std::fs::write(program(root), b"the one running").unwrap();
        let manifest = Manifest {
            version: "9.9.9".into(),
            files: vec![omega_pack::FileEntry {
                path: PROGRAM.into(),
                sha: "ab".repeat(32),
                size: 4,
                exec: true,
                offset: 0,
                len: 4,
            }],
            patches: Vec::new(),
        };
        // nothing answers here: a stop has to come before any fetch
        let release = Release {
            version: "9.9.9".into(),
            program: String::new(),
            sum: String::new(),
            system: Some([
                "http://127.0.0.1:9/m".into(),
                "http://127.0.0.1:9/f".into(),
                "http://127.0.0.1:9/p".into(),
            ]),
        };
        let gate = Gate::default();
        assert!(gate.ask_stop());
        assert_eq!(
            install_system(root, &release, &manifest, &gate),
            Err(STOPPED)
        );
        assert_eq!(std::fs::read(program(root)).unwrap(), b"the one running");
        let left: Vec<_> = std::fs::read_dir(root.join("System"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, ["omega-zero"], "something was left staged");
        // with none under way there is nothing to stop
        assert!(!Updater::default().stop());
    }

    #[test]
    fn a_stop_is_taken_before_anything_is_in_place_or_refused_after() {
        let gate = Gate::default();
        assert!(!gate.stopped());
        assert!(gate.commit(), "nothing asked: the files go in");
        assert!(!gate.ask_stop(), "past where it can stop, X is refused");
        assert!(!gate.stopped());

        let gate = Gate::default();
        assert!(gate.ask_stop());
        assert!(gate.ask_stop(), "asked twice, still stopping");
        assert!(gate.stopped());
        assert!(!gate.commit(), "a stop asked first keeps the files out");
    }

    #[cfg(unix)]
    #[test]
    fn a_download_is_ended_where_it_is_when_a_stop_is_asked() {
        // a server that answers and then sends nothing more: curl would wait on it to its end
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            if let Ok((mut conn, _)) = listener.accept() {
                use std::io::Write;
                let _ = conn
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100000000\r\n\r\nsome of it");
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
        });
        let gate = Arc::new(Gate::default());
        let asks = gate.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            asks.ask_stop();
        });
        let began = std::time::Instant::now();
        let got = fetch_with(&format!("http://127.0.0.1:{port}/big"), &[], Some(&gate));
        assert!(matches!(got, Fetched::Stopped));
        assert!(
            began.elapsed() < std::time::Duration::from_secs(2),
            "the download ran on for {:?}",
            began.elapsed()
        );
        let _ = server.join();
    }
}
