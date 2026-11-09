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

/// What the running build is: the version it was released as.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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
            Update::Idle => format!("V{VERSION}"),
            Update::Checking => "CHECKING…".to_string(),
            Update::Latest => "UP TO DATE".to_string(),
            Update::Available { release, .. } => format!("GET V{}", release.version),
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
                "A downloads {} and installs v{}",
                size_text(*bytes),
                release.version
            ),
            Update::Available { release, .. } => {
                format!("A downloads and installs v{}", release.version)
            }
            Update::Downloading(version) => format!("Downloading v{version}"),
            Update::Installed(version) => format!("A restarts onto v{version}"),
        }
    }

    pub fn busy(&self) -> bool {
        matches!(self, Update::Checking | Update::Downloading(_))
    }
}

/// The update under way, shared with the thread doing it.
#[derive(Clone)]
pub struct Updater {
    state: Arc<Mutex<Update>>,
}

impl Default for Updater {
    fn default() -> Self {
        Updater {
            state: Arc::new(Mutex::new(Update::Idle)),
        }
    }
}

impl Updater {
    pub fn state(&self) -> Update {
        self.state.lock().map(|s| s.clone()).unwrap_or(Update::Idle)
    }

    fn set(&self, to: Update) {
        if let Ok(mut s) = self.state.lock() {
            *s = to;
        }
    }

    /// Settings closing: what the row said, a check's answer or what went wrong, goes, and the
    /// version running shows again. A check or a download under way goes on, and an update put
    /// in place still asks for its restart.
    pub fn settle(&self) {
        if let Ok(mut s) = self.state.lock() {
            if matches!(
                *s,
                Update::Latest | Update::Available { .. } | Update::Failed(_)
            ) {
                *s = Update::Idle;
            }
        }
    }

    /// Asks GitHub for the latest release, on a thread of its own, and, for one with a
    /// manifest, what of it the card rooted at `root` lacks.
    pub fn check(&self, root: Option<PathBuf>) {
        if self.state().busy() {
            return;
        }
        self.set(Update::Checking);
        let me = self.clone();
        let spawned = std::thread::Builder::new()
            .name("omega-update".into())
            .spawn(move || {
                crate::spawn::stand_back();
                let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
                let found = match fetch(&url) {
                    Fetched::Body(body) => match latest(&String::from_utf8_lossy(&body)) {
                        Some(release) if newer(&release.version, VERSION) => {
                            let system = root.and_then(|root| lacking(&root, &release));
                            Update::Available { release, system }
                        }
                        // this release, where the card lacks some of it: an update from before
                        // 2.7.0 brought omega-zero alone
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
        let me = self.clone();
        let spawned = std::thread::Builder::new()
            .name("omega-update".into())
            .spawn(move || {
                crate::spawn::stand_back();
                let done = match system {
                    Some((manifest, _)) => install_system(&root, &release, &manifest),
                    None => install(&program(&root), &release.program, &release.sum),
                };
                let done = match done {
                    Ok(()) => Update::Installed(version),
                    Err(why) => {
                        eprintln!("omega-zero: update: {why}");
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
    let release = Json::parse(body)?;
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

/// Whether version `a` is newer than `b`, each as dotted numbers: `1.10.0` is newer than
/// `1.9.2`. A part that is not a number counts as none.
pub fn newer(a: &str, b: &str) -> bool {
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

fn install(program: &Path, url: &str, sum_url: &str) -> Result<(), &'static str> {
    if !program.is_file() {
        return Err("NOT ON CARD");
    }
    let sum = fetch(sum_url).body()?;
    let want = sum_of(&String::from_utf8_lossy(&sum)).ok_or("BAD RELEASE")?;
    let bytes = fetch(url).body()?;
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
fn fetch_range(url: &str, start: u64, len: u64) -> Result<Vec<u8>, &'static str> {
    if len == 0 {
        return Ok(Vec::new());
    }
    let range = format!("{start}-{}", start + len - 1);
    let body = fetch_with(url, &["-r", &range]).body()?;
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
fn install_system(root: &Path, release: &Release, manifest: &Manifest) -> Result<(), &'static str> {
    let [_, files_url, patches_url] = release.system.as_ref().ok_or("BAD RELEASE")?;
    if !program(root).is_file() {
        return Err("NOT ON CARD");
    }
    let had = read_installed(root);
    let steps = omega_pack::plan(manifest, |rel| sum_on_card(root, rel, &had));
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut make = || -> Result<(), &'static str> {
        for step in &steps {
            let file = step.file();
            let to = root.join(&file.path);
            let (piece, old) = match step {
                Step::Whole(f) => (fetch_range(files_url, f.offset, f.len)?, None),
                Step::Patch(_, p) => (
                    fetch_range(patches_url, p.offset, p.len)?,
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
    if let Err(why) = make() {
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
            record += &format!("{} {size} {mtime} {}\n", f.sha, f.path);
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
    fetch_with(url, &[])
}

/// What `url` answers, following redirects, curl given `extra` as well.
fn fetch_with(url: &str, extra: &[&str]) -> Fetched {
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
    let mut out = Vec::new();
    let read = child
        .stdout
        .take()
        .is_some_and(|mut s| s.read_to_end(&mut out).is_ok());
    let _ = child.wait();
    match read {
        true => answered(&out),
        false => Fetched::Unreachable,
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
        for (said, kept) in [
            (Update::Failed("CARD FULL"), Update::Idle),
            (Update::Latest, Update::Idle),
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
    fn a_sum_file_gives_its_hex() {
        let hex = "ab".repeat(32);
        assert_eq!(sum_of(&format!("{hex}  omega-zero-aarch64\n")), Some(hex));
        assert_eq!(sum_of("Not Found"), None);
    }
}
