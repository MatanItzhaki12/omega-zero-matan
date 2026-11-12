//! A game's saves on the card, as Settings › Saves manages them: its save, the backups of it
//! kept on the handheld, and its save states, the resume state written as it stops among them.
//!
//! A game's save is what its core keeps in `Saves`: `Saves/<system>/<stem>.<ext>` (the battery
//! save, a PlayStation's memory card), or for a PSP game every `Saves/PSP/SAVEDATA` folder its
//! ID starts. A game's place on a disc (`.disc`) is not its save.
//!
//! A backup is a copy of the save as it was, kept under `Backups/<system>/<stem>/<stamp>/`, its
//! files at their paths in `Saves`. Nothing here loses a save: putting a backup back, or deleting
//! the save, first keeps the save as it is as a backup of its own. Either sets the game's resume
//! state aside, as a state holds the save the game had and would bring it back.
//!
//! These read and write the card at once; Settings calls them on a press, the game they are
//! about out of the slot.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use omega_store::{Core, Platform, StateRing};

/// Where backups are kept, under the card's root.
pub const BACKUPS: &str = "Backups";

/// Where PPSSPP keeps a PSP game's saves in Saves, a folder each.
const PSP_SAVES: &str = "PSP/SAVEDATA";

/// The game a save is of, as the shelf has it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub platform: Platform,
    pub stem: String,
    /// The game's ID, as a PSP game's saves are named by: empty for the rest.
    pub code: String,
}

impl Game {
    pub fn of(cart: &omega_store::Cart) -> Game {
        Game {
            platform: cart.platform,
            stem: cart.stem.clone(),
            code: cart.code.clone(),
        }
    }
}

/// Which of a game's states one is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateKind {
    /// Written as the game last stopped, and picked up from as it goes in again.
    Resume,
    /// A resume state set aside, by its name: one a core refused, or one a save was put back
    /// under.
    SetAside(String),
    /// One saved from the switcher or the game's menu, by its stamp.
    Saved(String),
}

/// One of a game's states, on the core it is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub core: Core,
    pub kind: StateKind,
    pub bytes: u64,
}

impl State {
    /// Its name in the ring: a stamp, `resume`, or the name it was set aside as.
    fn name(&self) -> &str {
        match &self.kind {
            StateKind::Resume => "resume",
            StateKind::SetAside(name) | StateKind::Saved(name) => name,
        }
    }
}

/// What a game has on the card.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameSaves {
    /// Its save's files, by their paths in Saves.
    pub save: Vec<String>,
    /// How big the save is, and when it was last written.
    pub save_bytes: u64,
    pub save_written: Option<SystemTime>,
    /// Its backups, by stamp, the newest first.
    pub backups: Vec<String>,
    /// Its states, the resume state first, then those set aside, then the rest newest first.
    pub states: Vec<State>,
}

impl GameSaves {
    pub fn is_empty(&self) -> bool {
        self.save.is_empty() && self.backups.is_empty() && self.states.is_empty()
    }
}

/// What `game` has on the card at `root`.
pub fn look(root: &Path, game: &Game) -> GameSaves {
    let saves = crate::root::saves_dir(root);
    let save = save_files(root, game);
    let (mut bytes, mut written) = (0, None);
    for rel in &save {
        if let Ok(m) = std::fs::metadata(saves.join(rel)) {
            bytes += m.len();
            written = written.max(m.modified().ok());
        }
    }
    GameSaves {
        save,
        save_bytes: bytes,
        save_written: written,
        backups: backups(root, game),
        states: states(root, game),
    }
}

/// Whether `game` has anything on the card at `root`: a save, a backup or a state.
pub fn has_any(root: &Path, game: &Game) -> bool {
    !look(root, game).is_empty()
}

/// The files of `game`'s save, by their paths in Saves, sorted.
pub fn save_files(root: &Path, game: &Game) -> Vec<String> {
    let saves = crate::root::saves_dir(root);
    let mut out = Vec::new();
    if game.platform == Platform::Psp {
        let Ok(dir) = std::fs::read_dir(saves.join(PSP_SAVES)) else {
            return out;
        };
        for entry in dir.filter_map(|e| e.ok()) {
            let folder = entry.file_name().to_string_lossy().into_owned();
            let ours = !game.code.is_empty()
                && omega_store::psp::save_id(&folder).as_deref() == Some(game.code.as_str());
            if ours && entry.path().is_dir() {
                files_under(&saves, &format!("{PSP_SAVES}/{folder}"), &mut out);
            }
        }
    } else {
        let system = game.platform.dir_name();
        let Ok(dir) = std::fs::read_dir(saves.join(system)) else {
            return out;
        };
        for entry in dir.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some((stem, ext)) = name.rsplit_once('.') else {
                continue;
            };
            if stem == game.stem && ext != "disc" && entry.path().is_file() {
                out.push(format!("{system}/{name}"));
            }
        }
    }
    out.sort();
    out
}

/// Every file under `rel` in `base` (all of it for an empty `rel`), by its path from `base`.
fn files_under(base: &Path, rel: &str, out: &mut Vec<String>) {
    let Ok(dir) = std::fs::read_dir(base.join(rel)) else {
        return;
    };
    for entry in dir.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let child = match rel {
            "" => name,
            rel => format!("{rel}/{name}"),
        };
        match entry.path().is_dir() {
            true => files_under(base, &child, out),
            false => out.push(child),
        }
    }
}

/// Where `game`'s backups are kept.
fn backups_dir(root: &Path, game: &Game) -> PathBuf {
    root.join(BACKUPS)
        .join(game.platform.dir_name())
        .join(&game.stem)
}

/// `game`'s backups, by stamp, the newest first.
pub fn backups(root: &Path, game: &Game) -> Vec<String> {
    let Ok(dir) = std::fs::read_dir(backups_dir(root, game)) else {
        return Vec::new();
    };
    let mut stamps: Vec<String> = dir
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|s| omega_store::parse_stamp(s).is_some())
        .collect();
    stamps.sort_by(|a, b| b.cmp(a));
    stamps
}

/// `game`'s states on every core it runs on.
fn states(root: &Path, game: &Game) -> Vec<State> {
    let mut out = Vec::new();
    for core in Core::for_platform(game.platform) {
        let ring = StateRing::new(root, game.platform, *core, &game.stem);
        let state = |kind: StateKind| {
            let mut s = State {
                core: *core,
                kind,
                bytes: 0,
            };
            s.bytes = ring.size_of(s.name());
            s
        };
        if ring.has_resume() {
            out.push(state(StateKind::Resume));
        }
        out.extend(
            ring.set_aside()
                .into_iter()
                .map(|n| state(StateKind::SetAside(n))),
        );
        out.extend(
            ring.list()
                .unwrap_or_default()
                .into_iter()
                .map(|e| state(StateKind::Saved(e.stamp))),
        );
    }
    out
}

/// Keeps `game`'s save as it is as a backup stamped `stamp`. False, and nothing kept, for a game
/// with no save.
pub fn back_up(root: &Path, game: &Game, stamp: &str) -> std::io::Result<bool> {
    let files = save_files(root, game);
    if files.is_empty() {
        return Ok(false);
    }
    let saves = crate::root::saves_dir(root);
    let to = backups_dir(root, game).join(free_stamp(root, game, stamp));
    for rel in &files {
        let bytes = std::fs::read(saves.join(rel))?;
        let at = to.join(rel);
        if let Some(dir) = at.parent() {
            std::fs::create_dir_all(dir)?;
        }
        omega_store::atomic_write(&at, &bytes)?;
    }
    crate::log!(
        "saves: backed up {} {}",
        game.platform.dir_name(),
        game.stem
    );
    Ok(true)
}

/// `stamp`, or the next second free of a backup already kept, so two in a second keep both.
fn free_stamp(root: &Path, game: &Game, stamp: &str) -> String {
    let dir = backups_dir(root, game);
    let Some(secs) = omega_store::parse_stamp(stamp) else {
        return stamp.to_string();
    };
    (0..)
        .map(|n| omega_store::format_stamp(secs + n))
        .find(|s| !dir.join(s).exists())
        .unwrap_or_else(|| stamp.to_string())
}

/// Puts the backup stamped `stamp` back as `game`'s save. The save as it is is kept first as a
/// backup stamped `now`, and the resume state set aside.
pub fn restore(root: &Path, game: &Game, stamp: &str, now: &str) -> std::io::Result<()> {
    let from = backups_dir(root, game).join(stamp);
    if omega_store::parse_stamp(stamp).is_none() || !from.is_dir() {
        return Err(missing(stamp));
    }
    let mut files = Vec::new();
    files_under(&from, "", &mut files);
    back_up(root, game, now)?;
    remove_save(root, game)?;
    let saves = crate::root::saves_dir(root);
    for rel in &files {
        let bytes = std::fs::read(from.join(rel))?;
        let at = saves.join(rel);
        if let Some(dir) = at.parent() {
            std::fs::create_dir_all(dir)?;
        }
        omega_store::atomic_write(&at, &bytes)?;
    }
    set_resume_aside(root, game, now);
    crate::log!(
        "saves: put back {} {} from {stamp}",
        game.platform.dir_name(),
        game.stem
    );
    Ok(())
}

/// Deletes the backup stamped `stamp`.
pub fn delete_backup(root: &Path, game: &Game, stamp: &str) -> std::io::Result<()> {
    if omega_store::parse_stamp(stamp).is_none() {
        return Err(missing(stamp));
    }
    std::fs::remove_dir_all(backups_dir(root, game).join(stamp))?;
    let _ = std::fs::remove_dir(backups_dir(root, game));
    crate::log!(
        "saves: deleted the {stamp} backup of {} {}",
        game.platform.dir_name(),
        game.stem
    );
    Ok(())
}

/// Deletes `game`'s save, so it starts anew. It is kept first as a backup stamped `now`, and the
/// resume state set aside.
pub fn delete_save(root: &Path, game: &Game, now: &str) -> std::io::Result<()> {
    back_up(root, game, now)?;
    remove_save(root, game)?;
    set_resume_aside(root, game, now);
    crate::log!(
        "saves: deleted the save of {} {}",
        game.platform.dir_name(),
        game.stem
    );
    Ok(())
}

/// Deletes `state`.
pub fn delete_state(root: &Path, game: &Game, state: &State) -> std::io::Result<()> {
    let ring = StateRing::new(root, game.platform, state.core, &game.stem);
    match &state.kind {
        StateKind::Resume => ring.remove_resume()?,
        StateKind::SetAside(name) => ring.remove_set_aside(name)?,
        StateKind::Saved(stamp) => ring.remove(stamp)?,
    }
    crate::log!(
        "saves: deleted the {} state of {} {}",
        state.name(),
        game.platform.dir_name(),
        game.stem
    );
    Ok(())
}

/// Takes `game`'s save's files off the card, and a PSP save's folders with them.
fn remove_save(root: &Path, game: &Game) -> std::io::Result<()> {
    let saves = crate::root::saves_dir(root);
    let files = save_files(root, game);
    for rel in &files {
        std::fs::remove_file(saves.join(rel))?;
    }
    if game.platform == Platform::Psp {
        let mut folders: Vec<&str> = files
            .iter()
            .filter_map(|f| f.strip_prefix(&format!("{PSP_SAVES}/")))
            .filter_map(|f| f.split('/').next())
            .collect();
        folders.dedup();
        for folder in folders {
            let _ = std::fs::remove_dir_all(saves.join(PSP_SAVES).join(folder));
        }
    }
    Ok(())
}

/// Sets `game`'s resume state aside on every core, as a save was put back or deleted under it.
fn set_resume_aside(root: &Path, game: &Game, stamp: &str) {
    for core in Core::for_platform(game.platform) {
        let ring = StateRing::new(root, game.platform, *core, &game.stem);
        if let Ok(Some(to)) = ring.retire_resume(stamp) {
            crate::log!("saves: set {} aside", to.display());
        }
    }
}

fn missing(stamp: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("no backup stamped {stamp}"),
    )
}
