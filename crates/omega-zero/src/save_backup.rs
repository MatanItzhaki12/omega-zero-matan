//! Save Backup: every save on the card copied to a folder on a NAS, as it is saved.
//!
//! What is backed up is the card's Saves folder as it stands, whatever wrote it: omega-zero's
//! own `.sav` files, and whatever a core keeps there (a PlayStation's memory cards, a disc
//! game's disc), its folders and all. A system or emulator added later is backed up with no
//! change here, since nothing here knows one from another.
//!
//! A list on the card, `Config/save_backup.list`, holds what the NAS has: each file's path in
//! Saves, its size and its CRC-32, as last copied. A pass copies every file whose size or CRC
//! differs, the newest of it, over the one on the NAS; so a save made twice while the NAS was
//! away is copied once, as it stands. The first pass, or one after the NAS or the folder
//! changes, finds the list empty and copies everything.
//!
//! Passes run on a thread of their own: when a game is saved, when Wi-Fi comes up, every few
//! minutes while anything waits, and now and then besides for what a core wrote by itself. A
//! pass that cannot reach the NAS leaves the list as it was, and the next one tries again.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::nas::{Login, Nas, NasFail};

/// The list of what the NAS has, under the card's root.
const LIST: &str = "Config/save_backup.list";

/// How long after a pass that left saves waiting the next tries: 5 seconds, then twice as long
/// after each that fails, up to a minute, as the NAS may only be waking or the Wi-Fi coming
/// up. And how long between passes otherwise, for a save a core wrote with no word to
/// omega-zero.
const RETRY_FIRST: Duration = Duration::from_secs(5);
const RETRY_MOST: Duration = Duration::from_secs(60);
const LOOK_AGAIN: Duration = Duration::from_secs(300);

/// How long a pass asked for waits before it looks: a game's save is written on a thread of its
/// own as the pass is asked for, and a pass that looked at once would find the save before it,
/// call everything copied, and not look again for minutes.
pub const SETTLE: Duration = Duration::from_secs(3);

/// How often a pass looks again while the handheld is on no network: a look costs nothing, and
/// saves go as soon as one is back.
const OFFLINE_LOOK: Duration = Duration::from_secs(5);

/// How long a connection that failed is given before it is tried once more: a radio that dozed
/// may drop the first.
const AGAIN: Duration = Duration::from_secs(2);

/// Whether the handheld is on a network: it has an address on one. A pass with none does not
/// try the NAS.
pub fn on_network() -> bool {
    crate::nas::local_address().is_some()
}

/// How long to wait after the `failed`th pass in a row that left saves waiting.
pub fn retry_after(failed: u32) -> Duration {
    RETRY_FIRST
        .saturating_mul(1 << failed.saturating_sub(1).min(8))
        .min(RETRY_MOST)
}

/// Where saves are copied to: the NAS, the share on it, and the folder in the share, its parts
/// apart by `/`, none for the share's own top.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Target {
    pub login: Login,
    pub share: String,
    pub dir: String,
}

impl Target {
    /// The target as the list names it, so a list kept for another is known not to be its.
    fn key(&self) -> String {
        format!(
            "{}|{}|{}",
            self.login.host.trim().to_lowercase(),
            self.share.to_lowercase(),
            self.dir.trim_matches('/').to_lowercase()
        )
    }

    /// Where the file at `rel` in Saves goes in the share.
    pub fn remote(&self, rel: &str) -> String {
        match (self.dir.trim_matches('/'), rel) {
            ("", rel) => rel.to_string(),
            (dir, "") => dir.to_string(),
            (dir, rel) => format!("{dir}/{rel}"),
        }
    }
}

/// A file as the list keeps it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    pub size: u64,
    pub crc: u32,
}

/// What the NAS has, by each file's path in Saves, and the target it is had for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct List {
    pub target: String,
    pub files: HashMap<String, Kept>,
}

impl List {
    pub fn read(root: &Path) -> List {
        let Ok(text) = std::fs::read_to_string(root.join(LIST)) else {
            return List::default();
        };
        let mut lines = text.lines();
        let target = lines
            .next()
            .and_then(|l| l.strip_prefix("target\t"))
            .unwrap_or_default()
            .to_string();
        let files = lines
            .filter_map(|line| {
                let mut f = line.split('\t');
                let (path, size, crc) = (f.next()?, f.next()?, f.next()?);
                Some((
                    path.to_string(),
                    Kept {
                        size: size.parse().ok()?,
                        crc: u32::from_str_radix(crc, 16).ok()?,
                    },
                ))
            })
            .collect();
        List { target, files }
    }

    pub fn write(&self, root: &Path) -> std::io::Result<()> {
        let mut paths: Vec<&String> = self.files.keys().collect();
        paths.sort();
        let mut text = format!("target\t{}\n", self.target);
        for path in paths {
            let k = self.files[path];
            let _ = writeln!(text, "{path}\t{}\t{:08x}", k.size, k.crc);
        }
        let at = root.join(LIST);
        if let Some(dir) = at.parent() {
            std::fs::create_dir_all(dir)?;
        }
        omega_store::atomic_write(&at, text.as_bytes())
    }
}

/// Every file in `saves`, as its path from there with `/` between its parts: hidden files, as a
/// write half done is, left out.
pub fn saved_files(saves: &Path) -> Vec<String> {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = match rel {
                "" => name.clone(),
                rel => format!("{rel}/{name}"),
            };
            match entry.file_type() {
                Ok(t) if t.is_dir() => walk(&entry.path(), &path, out),
                Ok(t) if t.is_file() => out.push(path),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(saves, "", &mut out);
    out.sort();
    out
}

/// The files in `saves` the NAS does not have as they stand, by `list`, with what they hold
/// now.
pub fn changed(saves: &Path, list: &List) -> Vec<(String, Vec<u8>)> {
    saved_files(saves)
        .into_iter()
        .filter_map(|rel| {
            let bytes = std::fs::read(saves.join(&rel)).ok()?;
            let now = Kept {
                size: bytes.len() as u64,
                crc: crate::label_sync::crc32(&bytes),
            };
            (list.files.get(&rel) != Some(&now)).then_some((rel, bytes))
        })
        .collect()
}

/// Where Save Backup stands, as Settings shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// A pass is copying now.
    pub busy: bool,
    /// How many saves the NAS does not have yet, as last counted.
    pub waiting: usize,
    /// When the last pass that left nothing waiting ended, in seconds since 1970.
    pub done_at: Option<u64>,
    /// Why the last pass stopped short, while saves still wait.
    pub fail: Option<NasFail>,
    /// How many passes have copied saves, and how many have failed, since the thread started:
    /// each a toast's worth.
    pub copied: u64,
    pub failed: u64,
    /// Saves wait for a network: the handheld had none when the last pass looked, so it did not
    /// try the NAS. No failure, and no toast.
    pub offline: bool,
}

enum Ask {
    /// Look for saves to copy now.
    Pass,
    /// Copy to `target` from now on, or to nothing.
    Target(Option<Target>),
}

/// Save Backup's thread, which copies saves while it has a target.
pub struct SaveBackup {
    tx: Sender<Ask>,
    status: Arc<Mutex<Status>>,
}

impl SaveBackup {
    /// The thread for the card at `root`, copying to `target` while it is one.
    pub fn start(root: PathBuf, target: Option<Target>) -> SaveBackup {
        let (tx, rx) = channel();
        let status = Arc::new(Mutex::new(Status::default()));
        let shared = status.clone();
        let _ = std::thread::Builder::new()
            .name("omega-backup".into())
            .spawn(move || {
                crate::spawn::stand_back();
                run(&root, target, &rx, &shared);
            });
        SaveBackup { tx, status }
    }

    /// Looks for saves to copy now: after a save, or once the network is back.
    pub fn pass(&self) {
        let _ = self.tx.send(Ask::Pass);
    }

    /// What asks for a pass from another thread: one writing a save, once it is written.
    pub fn passer(&self) -> impl Fn() + Send + 'static {
        let tx = self.tx.clone();
        move || {
            let _ = tx.send(Ask::Pass);
        }
    }

    /// Copies to `target` from now on; none stops.
    pub fn set_target(&self, target: Option<Target>) {
        let _ = self.tx.send(Ask::Target(target));
    }

    pub fn status(&self) -> Status {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn run(root: &Path, mut target: Option<Target>, rx: &Receiver<Ask>, status: &Mutex<Status>) {
    let mut wait = Duration::ZERO;
    let mut failed = 0;
    loop {
        let mut asked = match rx.recv_timeout(wait) {
            Ok(ask) => vec![ask],
            Err(RecvTimeoutError::Timeout) => Vec::new(),
            Err(RecvTimeoutError::Disconnected) => return,
        };
        // asks that came together make one pass; one for a pass waits for the save being
        // written as it was asked, and takes what came in the while
        asked.extend(rx.try_iter());
        if asked.iter().any(|a| matches!(a, Ask::Pass)) {
            std::thread::sleep(SETTLE);
            asked.extend(rx.try_iter());
        }
        for ask in asked {
            if let Ask::Target(t) = ask {
                target = t;
                if let Ok(mut s) = status.lock() {
                    *s = Status {
                        copied: s.copied,
                        failed: s.failed,
                        ..Status::default()
                    };
                }
            }
        }
        let Some(to) = &target else {
            wait = Duration::from_secs(3600);
            continue;
        };
        let left = pass_on(root, to, status, on_network);
        let offline = status.lock().is_ok_and(|s| s.offline);
        failed = match (left, offline) {
            (0, _) => 0,
            (_, true) => failed,
            _ => failed + 1,
        };
        wait = match (failed, offline && left > 0) {
            (_, true) => OFFLINE_LOOK,
            (0, false) => LOOK_AGAIN,
            (n, false) => retry_after(n),
        };
    }
}

/// One pass: copies what the NAS does not have to `to`, keeping the list as each is copied.
/// Returns how many are left waiting.
pub fn pass(root: &Path, to: &Target, status: &Mutex<Status>) -> usize {
    pass_on(root, to, status, || true)
}

/// As [`pass`], trying the NAS only while `online` says the handheld is on a network: with
/// none, the saves wait, and nothing fails.
pub fn pass_on(
    root: &Path,
    to: &Target,
    status: &Mutex<Status>,
    online: impl Fn() -> bool,
) -> usize {
    let set = |f: &dyn Fn(&mut Status)| {
        if let Ok(mut s) = status.lock() {
            f(&mut s);
        }
    };
    let mut list = List::read(root);
    if list.target != to.key() {
        // another NAS or folder: it has nothing yet, and gets everything
        list = List {
            target: to.key(),
            files: HashMap::new(),
        };
    }
    let saves = crate::root::saves_dir(root);
    let todo = changed(&saves, &list);
    let total = todo.len();
    set(&|s| s.waiting = total);
    if todo.is_empty() {
        set(&|s| {
            s.fail = None;
            s.offline = false;
            s.done_at.get_or_insert_with(now_secs);
        });
        return 0;
    }
    if !online() {
        set(&|s| {
            s.offline = true;
            s.fail = None;
        });
        return total;
    }
    set(&|s| {
        s.busy = true;
        s.offline = false;
    });
    let outcome = copy(root, to, &mut list, todo, status);
    let left = match &outcome {
        Ok(()) => 0,
        Err((left, _)) => *left,
    };
    set(&|s| {
        s.busy = false;
        s.waiting = left;
        match &outcome {
            Ok(()) => {
                s.fail = None;
                s.done_at = Some(now_secs());
                s.copied += 1;
            }
            Err((_, e)) => {
                s.fail = Some(e.clone());
                s.failed += 1;
            }
        }
    });
    left
}

fn copy(
    root: &Path,
    to: &Target,
    list: &mut List,
    todo: Vec<(String, Vec<u8>)>,
    status: &Mutex<Status>,
) -> Result<(), (usize, NasFail)> {
    let total = todo.len();
    // a radio that dozed may drop the first connection: it is tried once more
    let mut nas = Nas::open(&to.login)
        .or_else(|e| match e {
            NasFail::Unreachable => {
                std::thread::sleep(AGAIN);
                Nas::open(&to.login)
            }
            e => Err(e),
        })
        .map_err(|e| {
            crate::log!("save backup: {}: {e}", to.login.host);
            (total, e)
        })?;
    for (done, (rel, bytes)) in todo.into_iter().enumerate() {
        if let Err(e) = nas.put(&to.share, &to.remote(&rel), &bytes) {
            crate::log!("save backup: {rel}: {e}");
            let _ = list.write(root);
            return Err((total - done, e));
        }
        list.files.insert(
            rel,
            Kept {
                size: bytes.len() as u64,
                crc: crate::label_sync::crc32(&bytes),
            },
        );
        if let Ok(mut s) = status.lock() {
            s.waiting = total - done - 1;
        }
    }
    if let Err(e) = list.write(root) {
        crate::log!("save backup: {LIST}: {e}");
    }
    crate::log!("save backup: {total} copied to {}", to.login.host);
    Ok(())
}

/// The largest file Sync From Backup reads back to compare: a save is far smaller.
const LARGEST: u64 = 8 << 20;

/// Where PPSSPP keeps a PSP game's saves in Saves: a folder for each, its files together.
const PSP_SAVES: &str = "PSP/SAVEDATA/";

/// The save the file at `rel` in Saves is part of: a PSP save's folder, whose files go together,
/// or the file itself. `None` for what a core keeps in Saves that is no game's save, the rest of
/// PPSSPP's memory stick: backed up, but not offered back one file at a time.
pub fn save_of(rel: &str) -> Option<String> {
    match rel.strip_prefix(PSP_SAVES) {
        Some(rest) => rest
            .split_once('/')
            .map(|(folder, _)| format!("{PSP_SAVES}{folder}")),
        None if rel.starts_with("PSP/") => None,
        None => Some(rel.to_string()),
    }
}

/// A save on the NAS that the card does not have as it is there: its path in Saves (a file, or
/// a PSP save's folder), each of its files as the NAS holds it, by its path in Saves, and
/// whether the card has it at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Differs {
    pub rel: String,
    pub files: Vec<(String, Vec<u8>)>,
    pub on_card: bool,
}

impl Differs {
    /// A save that is one file.
    pub fn file(rel: &str, bytes: Vec<u8>, on_card: bool) -> Differs {
        Differs {
            rel: rel.to_string(),
            files: vec![(rel.to_string(), bytes)],
            on_card,
        }
    }

    /// The game's name, as the list shows it: a PSP save's title, as its PARAM.SFO gives it,
    /// else the file's name without its extension or region.
    pub fn name(&self) -> String {
        let sfo = self
            .files
            .iter()
            .find(|(rel, _)| rel.ends_with("/PARAM.SFO"))
            .and_then(|(_, bytes)| omega_store::psp::save_title(bytes));
        if let Some(title) = sfo {
            return title;
        }
        let file = self.rel.rsplit('/').next().unwrap_or(&self.rel);
        let stem = match self.rel.starts_with(PSP_SAVES) {
            true => file,
            false => file.rsplit_once('.').map_or(file, |(stem, _)| stem),
        };
        stem.split(" (").next().unwrap_or(stem).trim().to_string()
    }

    /// The system's folder it is in: GBA, PS1 and the rest.
    pub fn system(&self) -> &str {
        self.rel
            .split('/')
            .next()
            .filter(|_| self.rel.contains('/'))
            .unwrap_or("")
    }
}

/// Every file under `to`'s folder on the NAS, as its path from there.
fn remote_files(nas: &mut Nas, to: &Target) -> Result<Vec<(String, u64)>, NasFail> {
    let mut out = Vec::new();
    let mut dirs = vec![String::new()];
    while let Some(rel) = dirs.pop() {
        for (name, is_dir, size) in nas.entries(&to.share, &to.remote(&rel))? {
            let path = match rel.as_str() {
                "" => name.clone(),
                rel => format!("{rel}/{name}"),
            };
            match is_dir {
                true => dirs.push(path),
                false if !name.ends_with(".part") && size <= LARGEST => out.push((path, size)),
                false => {}
            }
        }
    }
    out.sort();
    Ok(out)
}

/// The saves on the NAS at `to` that differ from the card's at `root`, or that the card does not
/// have: each read back to compare, since a save is the same size whatever it holds. A PSP
/// save's folder differs when any of its files does, and is offered whole.
pub fn differing(root: &Path, to: &Target) -> Result<Vec<Differs>, NasFail> {
    let mut nas = Nas::open(&to.login)?;
    let saves = crate::root::saves_dir(root);
    let mut out: Vec<Differs> = Vec::new();
    let mut changed: Vec<bool> = Vec::new();
    for (rel, _) in remote_files(&mut nas, to)? {
        let Some(unit) = save_of(&rel) else {
            continue;
        };
        let bytes = nas.get(&to.share, &to.remote(&rel))?;
        let here = std::fs::read(saves.join(&rel)).ok();
        let differs = here.as_ref() != Some(&bytes);
        // the files come sorted, so a folder's are together
        if out.last().is_none_or(|d| d.rel != unit) {
            out.push(Differs {
                on_card: saves.join(&unit).exists(),
                rel: unit,
                files: Vec::new(),
            });
            changed.push(false);
        }
        let (last, any) = (out.len() - 1, changed.len() - 1);
        out[last].files.push((rel, bytes));
        changed[any] |= differs;
    }
    Ok(out
        .into_iter()
        .zip(changed)
        .filter_map(|(d, changed)| changed.then_some(d))
        .collect())
}

/// Where the card's save is kept before one from the NAS replaces it.
pub const UNDO: &str = "Config/save_backup_undo";

/// Puts `from` on the card in place of its save. The card's own is kept first under
/// `Config/save_backup_undo/<stamp>/`, and the game's resume state is set aside, as a state the
/// core refused is: a state holds the save the game had, and would bring it back. The list
/// takes it as the NAS has it, so it is not copied back.
pub fn restore(root: &Path, from: &Differs, stamp: &str) -> std::io::Result<()> {
    let saves = crate::root::saves_dir(root);
    for (rel, bytes) in &from.files {
        let at = saves.join(rel);
        if let Ok(old) = std::fs::read(&at) {
            let kept = root.join(UNDO).join(stamp).join(rel);
            if let Some(dir) = kept.parent() {
                std::fs::create_dir_all(dir)?;
            }
            omega_store::atomic_write(&kept, &old)?;
        }
        if let Some(dir) = at.parent() {
            std::fs::create_dir_all(dir)?;
        }
        omega_store::atomic_write(&at, bytes)?;
    }
    for (platform, stem) in games_of(root, &from.rel) {
        for core in omega_store::Core::for_platform(platform) {
            let ring = omega_store::StateRing::new(root, platform, *core, &stem);
            if let Ok(Some(to)) = ring.retire_resume(stamp) {
                crate::log!("sync from backup: set {} aside", to.display());
            }
        }
    }
    let mut list = List::read(root);
    if !list.target.is_empty() {
        for (rel, bytes) in &from.files {
            list.files.insert(
                rel.clone(),
                Kept {
                    size: bytes.len() as u64,
                    crc: crate::label_sync::crc32(bytes),
                },
            );
        }
        list.write(root)?;
    }
    Ok(())
}

/// The games the save at `rel` in Saves is: a battery save's one, or every PSP game on the card
/// whose ID its folder starts with.
fn games_of(root: &Path, rel: &str) -> Vec<(omega_store::Platform, String)> {
    if let Some(folder) = rel.strip_prefix(PSP_SAVES) {
        let Some(id) = omega_store::psp::save_id(folder) else {
            return Vec::new();
        };
        return omega_store::scan(root)
            .unwrap_or_default()
            .into_iter()
            .filter(|c| c.platform == omega_store::Platform::Psp && c.code == id)
            .map(|c| (c.platform, c.stem))
            .collect();
    }
    game_of(rel).into_iter().collect()
}

/// The system and game a battery save in Saves is: `GBA/<stem>.sav` or `.srm`.
fn game_of(rel: &str) -> Option<(omega_store::Platform, String)> {
    let (dir, file) = rel.split_once('/')?;
    let stem = file
        .strip_suffix(".sav")
        .or_else(|| file.strip_suffix(".srm"))?;
    let platform = omega_store::Platform::ALL
        .into_iter()
        .find(|p| p.dir_name() == dir)?;
    Some((platform, stem.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> Target {
        Target {
            login: Login {
                host: "192.168.1.5".into(),
                user: "ash".into(),
                password: "pikachu".into(),
            },
            share: "Backups".into(),
            dir: "RG SP/Saves".into(),
        }
    }

    #[test]
    fn a_save_from_the_nas_replaces_the_cards_which_is_kept() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        let saves = crate::root::saves_dir(root);
        std::fs::create_dir_all(saves.join("GBA")).unwrap();
        std::fs::write(saves.join("GBA/Emerald.sav"), b"handheld").unwrap();
        let resume = root.join("States/GBA/gpsp/Emerald");
        std::fs::create_dir_all(&resume).unwrap();
        std::fs::write(resume.join("resume.state"), b"old state").unwrap();
        let from = Differs::file("GBA/Emerald.sav", b"from the nas".to_vec(), true);
        restore(root, &from, "2026-11-03_12-00-00").unwrap();
        assert_eq!(
            std::fs::read(saves.join("GBA/Emerald.sav")).unwrap(),
            b"from the nas"
        );
        assert_eq!(
            std::fs::read(root.join(UNDO).join("2026-11-03_12-00-00/GBA/Emerald.sav")).unwrap(),
            b"handheld"
        );
        assert!(
            !resume.join("resume.state").exists(),
            "the resume state, holding the old save, would bring it back"
        );
    }

    #[test]
    fn a_save_on_the_nas_is_named_as_its_game() {
        let d = Differs::file(
            "GBA/Pokemon - Emerald Version (USA, Europe).sav",
            Vec::new(),
            false,
        );
        assert_eq!(d.name(), "Pokemon - Emerald Version");
        assert_eq!(d.system(), "GBA");
        assert_eq!(
            game_of("GBA/Emerald.sav").map(|(p, s)| (p.dir_name(), s)),
            Some(("GBA", "Emerald".to_string()))
        );
        assert_eq!(game_of("PS1/cards/Crash.mcd"), None);
    }

    #[test]
    fn a_psp_save_is_its_folder_and_ppsspp_s_own_files_are_no_save() {
        assert_eq!(
            save_of("GBA/Emerald.sav").as_deref(),
            Some("GBA/Emerald.sav")
        );
        assert_eq!(
            save_of("PSP/SAVEDATA/ULUS10391DATA00/DATA.BIN").as_deref(),
            Some("PSP/SAVEDATA/ULUS10391DATA00")
        );
        assert_eq!(save_of("PSP/SAVEDATA/stray.bin"), None);
        assert_eq!(save_of("PSP/SYSTEM/CACHE/x.bin"), None);
    }

    /// A PSP save's folder from the NAS: its files together.
    fn psp_save(title: &str, data: &[u8]) -> Differs {
        let dir = "PSP/SAVEDATA/ULUS10391DATA00";
        Differs {
            rel: dir.into(),
            files: vec![
                (format!("{dir}/DATA.BIN"), data.to_vec()),
                (
                    format!("{dir}/PARAM.SFO"),
                    omega_store::psp::param_sfo(&[("TITLE", title)]),
                ),
            ],
            on_card: true,
        }
    }

    #[test]
    fn a_psp_save_on_the_nas_is_named_as_its_game() {
        let d = psp_save("Monster Hunter Freedom Unite", b"");
        assert_eq!(d.name(), "Monster Hunter Freedom Unite");
        assert_eq!(d.system(), "PSP");
        let mut bare = d;
        bare.files.retain(|(rel, _)| rel.ends_with("DATA.BIN"));
        assert_eq!(bare.name(), "ULUS10391DATA00");
    }

    #[test]
    fn a_psp_save_from_the_nas_replaces_the_cards_whole_and_sets_its_game_s_state_aside() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        let saves = crate::root::saves_dir(root);
        let dir = saves.join("PSP/SAVEDATA/ULUS10391DATA00");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("DATA.BIN"), b"handheld").unwrap();
        // the game on the card, an eboot whose PARAM.SFO gives its ID
        let games = root.join("Games/PSP");
        std::fs::create_dir_all(&games).unwrap();
        let sfo = omega_store::psp::param_sfo(&[("DISC_ID", "ULUS10391"), ("TITLE", "MHFU")]);
        let mut pbp = b"\0PBP".to_vec();
        pbp.extend_from_slice(&0x0001_0000u32.to_le_bytes());
        pbp.extend_from_slice(&40u32.to_le_bytes());
        pbp.extend_from_slice(&(40 + sfo.len() as u32).to_le_bytes());
        pbp.resize(40, 0);
        pbp.extend(sfo);
        std::fs::write(games.join("Monster Hunter.pbp"), pbp).unwrap();
        let resume = root.join("States/PSP/ppsspp/Monster Hunter");
        std::fs::create_dir_all(&resume).unwrap();
        std::fs::write(resume.join("resume.state"), b"old state").unwrap();

        restore(
            root,
            &psp_save("MHFU", b"from the nas"),
            "2026-11-03_12-00-00",
        )
        .unwrap();
        assert_eq!(
            std::fs::read(dir.join("DATA.BIN")).unwrap(),
            b"from the nas"
        );
        assert!(
            dir.join("PARAM.SFO").is_file(),
            "the folder came over in part"
        );
        assert_eq!(
            std::fs::read(
                root.join(UNDO)
                    .join("2026-11-03_12-00-00/PSP/SAVEDATA/ULUS10391DATA00/DATA.BIN")
            )
            .unwrap(),
            b"handheld"
        );
        assert!(
            !resume.join("resume.state").exists(),
            "the game's resume state, holding the old save, would bring it back"
        );
    }

    #[test]
    fn with_no_network_saves_wait_and_nothing_fails() {
        let d = tempfile::tempdir().unwrap();
        let saves = crate::root::saves_dir(d.path());
        std::fs::create_dir_all(saves.join("GBA")).unwrap();
        std::fs::write(saves.join("GBA/Emerald.sav"), b"e").unwrap();
        let status = Mutex::new(Status::default());
        assert_eq!(pass_on(d.path(), &target(), &status, || false), 1);
        let s = status.lock().unwrap().clone();
        assert!(s.offline, "the pass did not say it waits for a network");
        assert_eq!((s.waiting, s.failed), (1, 0), "{s:?}");
        assert!(s.fail.is_none());
    }

    #[test]
    fn a_failed_pass_is_tried_again_soon_then_less_often() {
        let waits: Vec<u64> = (1..=6).map(|n| retry_after(n).as_secs()).collect();
        assert_eq!(waits, [5, 10, 20, 40, 60, 60]);
        assert_eq!(retry_after(100).as_secs(), 60);
    }

    #[test]
    fn a_save_goes_under_the_folder_picked() {
        assert_eq!(
            target().remote("GBA/Emerald.sav"),
            "RG SP/Saves/GBA/Emerald.sav"
        );
        let top = Target {
            dir: String::new(),
            ..target()
        };
        assert_eq!(top.remote("GBA/Emerald.sav"), "GBA/Emerald.sav");
    }

    #[test]
    fn every_file_in_saves_is_found_but_the_hidden_ones() {
        let d = tempfile::tempdir().unwrap();
        let saves = d.path();
        std::fs::create_dir_all(saves.join("GBA")).unwrap();
        std::fs::create_dir_all(saves.join("PS1/cards")).unwrap();
        std::fs::write(saves.join("GBA/Emerald.sav"), b"e").unwrap();
        std::fs::write(saves.join("GBA/.Emerald.sav.12.0.tmp"), b"half").unwrap();
        std::fs::write(saves.join("PS1/cards/Crash.mcd"), b"c").unwrap();
        assert_eq!(
            saved_files(saves),
            ["GBA/Emerald.sav", "PS1/cards/Crash.mcd"]
        );
    }

    #[test]
    fn only_what_changed_since_the_list_is_copied() {
        let d = tempfile::tempdir().unwrap();
        let saves = d.path();
        std::fs::create_dir_all(saves.join("GBA")).unwrap();
        std::fs::write(saves.join("GBA/Emerald.sav"), b"one").unwrap();
        std::fs::write(saves.join("GBA/Ruby.sav"), b"two").unwrap();
        let mut list = List::default();
        assert_eq!(
            changed(saves, &list).len(),
            2,
            "a first pass copies everything"
        );
        list.files.insert(
            "GBA/Emerald.sav".into(),
            Kept {
                size: 3,
                crc: crate::label_sync::crc32(b"one"),
            },
        );
        let todo: Vec<String> = changed(saves, &list).into_iter().map(|(p, _)| p).collect();
        assert_eq!(todo, ["GBA/Ruby.sav"]);
        // saved again while the NAS was away: the newest is what goes
        std::fs::write(saves.join("GBA/Emerald.sav"), b"three").unwrap();
        let todo = changed(saves, &list);
        assert!(todo
            .iter()
            .any(|(p, b)| p == "GBA/Emerald.sav" && b == b"three"));
    }

    #[test]
    fn the_list_reads_back_as_written() {
        let d = tempfile::tempdir().unwrap();
        let mut list = List {
            target: target().key(),
            files: HashMap::new(),
        };
        list.files.insert(
            "GBA/Emerald.sav".into(),
            Kept {
                size: 131072,
                crc: 0xdead_beef,
            },
        );
        list.write(d.path()).unwrap();
        assert_eq!(List::read(d.path()), list);
        assert_eq!(List::read(&d.path().join("none")), List::default());
    }

    #[test]
    fn a_nas_out_of_reach_leaves_every_save_waiting() {
        let d = tempfile::tempdir().unwrap();
        let saves = crate::root::saves_dir(d.path());
        std::fs::create_dir_all(saves.join("GBA")).unwrap();
        std::fs::write(saves.join("GBA/Emerald.sav"), b"e").unwrap();
        // nothing listens on port 9 here: refused at once
        let to = Target {
            login: Login {
                host: "127.0.0.1:9".into(),
                ..Login::default()
            },
            ..target()
        };
        let status = Mutex::new(Status::default());
        assert_eq!(pass(d.path(), &to, &status), 1);
        let s = status.lock().unwrap().clone();
        assert_eq!(s.waiting, 1);
        assert!(s.fail.is_some());
        assert!(!s.busy);
        assert!(
            List::read(d.path()).files.is_empty(),
            "the list took a save never copied"
        );
    }
}
