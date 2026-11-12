use std::path::{Path, PathBuf};

use omega_store::{atomic_write, read_app_state, write_app_state, Core, Platform, StateRing};

pub trait Snapshot {
    fn state(&self) -> Option<Vec<u8>>;
    fn save_ram(&self) -> Option<Vec<u8>>;
    fn thumb(&self) -> Option<Vec<u8>>;
    fn load(&self, state: Vec<u8>);

    /// Starts the game over from power on, as the console's reset button does.
    fn reset(&self) {}

    fn resume_trusted(&self) -> bool {
        true
    }

    fn save_ram_trusted(&self) -> bool {
        true
    }

    /// The snapshot as one another thread can take the state through, so the screen does not
    /// wait while the core gives it up: none for one that is only taken on the screen's
    /// thread, as a test's is.
    fn detached(&self) -> Option<Box<dyn Snapshot + Send>> {
        None
    }
}

pub fn flush(
    root: &Path,
    platform: Platform,
    core: Core,
    stem: &str,
    state: Option<&[u8]>,
    sav: Option<&[u8]>,
) -> std::io::Result<()> {
    if let Some(state) = state {
        StateRing::new(root, platform, core, stem).write_resume(state)?;
    }
    if let Some(sav) = sav {
        write_sav(root, platform, stem, sav)?;
    }
    Ok(())
}

/// As [`flush`], for the autosave while the game plays on: the state is written paced, so the
/// game's own reads of the card go between its slices.
pub fn autosave_flush(
    root: &Path,
    platform: Platform,
    core: Core,
    stem: &str,
    state: Option<&[u8]>,
    sav: Option<&[u8]>,
) -> std::io::Result<()> {
    if let Some(state) = state {
        StateRing::new(root, platform, core, stem).write_resume_paced(state)?;
    }
    if let Some(sav) = sav {
        write_sav(root, platform, stem, sav)?;
    }
    Ok(())
}

/// Where the game whose core is opening is named, as `<system>/<stem>`, until it has run a
/// moment. A boot that finds it names a game whose start took omega-zero down, and leaves that
/// game on the shelf: a core that aborts as it opens a game would otherwise have every boot open
/// it again, and the console would never get past its boot screen.
const STARTING: &str = "Config/.starting";

fn starting_key(platform: Platform, stem: &str) -> String {
    format!("{}/{stem}", platform.dir_name())
}

/// Notes that `stem`'s core is opening, on the card before it opens.
pub fn mark_starting(root: &Path, platform: Platform, stem: &str) {
    let key = starting_key(platform, stem);
    // through the card's writer, after a note of the game before being taken off, and on the
    // card before the core opens
    if let Err(e) = crate::card_writer::write(&root.join(STARTING), key.clone().into_bytes()) {
        crate::log!("could not note {key} starting: {e}");
    }
    crate::card_writer::settle();
}

pub fn clear_starting(root: &Path) {
    crate::card_writer::remove(&root.join(STARTING));
}

/// The game noted as starting when omega-zero last stopped, the note taken off the card.
pub fn take_starting(root: &Path) -> Option<String> {
    let key = std::fs::read_to_string(root.join(STARTING)).ok()?;
    clear_starting(root);
    Some(key)
}

/// The system and stem a note of a game starting names, as `mark_starting` wrote it.
pub fn starting_game(key: &str) -> Option<(Platform, String)> {
    let (dir, stem) = key.trim_end_matches(['\r', '\n']).split_once('/')?;
    let platform = Platform::ALL.into_iter().find(|p| p.dir_name() == dir)?;
    (!stem.is_empty()).then(|| (platform, stem.to_string()))
}

pub fn eject(
    root: &Path,
    platform: Platform,
    core: Core,
    stem: &str,
    state: Option<&[u8]>,
    sav: Option<&[u8]>,
) -> std::io::Result<()> {
    flush(root, platform, core, stem, state, sav)?;
    let mut state = read_app_state(root);
    state.cart = None;
    state.cart_platform = None;
    write_app_state(root, &state)
}

pub fn write_sav(root: &Path, platform: Platform, stem: &str, sav: &[u8]) -> std::io::Result<bool> {
    let path = sav_path(root, platform, stem);
    if let Some(old) = read_sav(root, platform, stem) {
        if old == sav {
            return Ok(false);
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    atomic_write(&path, sav)?;
    Ok(true)
}

pub fn read_sav(root: &Path, platform: Platform, stem: &str) -> Option<Vec<u8>> {
    std::fs::read(sav_path(root, platform, stem))
        .or_else(|_| {
            std::fs::read(
                crate::root::saves_dir(root)
                    .join(platform.dir_name())
                    .join(format!("{stem}.srm")),
            )
        })
        .ok()
}

pub fn read_resume(root: &Path, platform: Platform, core: Core, stem: &str) -> Option<Vec<u8>> {
    StateRing::new(root, platform, core, stem)
        .read_resume()
        .ok()
        .flatten()
}

fn sav_path(root: &Path, platform: Platform, stem: &str) -> PathBuf {
    crate::root::saves_dir(root)
        .join(platform.dir_name())
        .join(format!("{stem}.sav"))
}

/// Where a game on several discs keeps the disc it was last swapped to, beside its save: the
/// disc's number counting from 0 on one line and the file the core reads it from on the next.
fn disc_path(root: &Path, platform: Platform, stem: &str) -> PathBuf {
    crate::root::saves_dir(root)
        .join(platform.dir_name())
        .join(format!("{stem}.disc"))
}

/// Keeps disc `index`, read from `path`, as the one the game is on.
pub fn write_disc(
    root: &Path,
    platform: Platform,
    stem: &str,
    index: usize,
    path: &str,
) -> std::io::Result<()> {
    let at = disc_path(root, platform, stem);
    if let Some(dir) = at.parent() {
        std::fs::create_dir_all(dir)?;
    }
    atomic_write(&at, format!("{index}\n{path}\n").as_bytes())
}

/// The disc a game was last swapped to and the file it is read from, if it ever was.
pub fn read_disc(root: &Path, platform: Platform, stem: &str) -> Option<(usize, String)> {
    let text = std::fs::read_to_string(disc_path(root, platform, stem)).ok()?;
    let mut lines = text.lines();
    let index = lines.next()?.trim().parse().ok()?;
    let path = lines.next()?.to_string();
    Some((index, path))
}
