use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// A paced write's slice, and the rest after each: the card takes a slice, and what a game
/// reads from it, as a PSP game reads its disc, goes between slices rather than waiting behind
/// the whole file.
const PACED_SLICE: usize = 512 * 1024;
const PACED_REST: Duration = Duration::from_millis(15);

pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_dated(path, bytes, None, false)
}

/// As [`atomic_write`], the file dated `modified` from the moment it is there to be read.
pub fn atomic_write_dated(path: &Path, bytes: &[u8], modified: SystemTime) -> std::io::Result<()> {
    write_dated(path, bytes, Some(modified), false)
}

/// As [`atomic_write`], a slice at a time, each on the card before the next: for a large file
/// that can wait, written while a game reads the same card. A PSP state is 40 MB, and written
/// whole it held the game's reads for seconds.
pub fn atomic_write_paced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_dated(path, bytes, None, true)
}

fn write_dated(
    path: &Path,
    bytes: &[u8],
    modified: Option<SystemTime>,
    paced: bool,
) -> std::io::Result<()> {
    let tmp = temp_path(path);
    match write_then_rename(&tmp, path, bytes, modified, paced) {
        Ok(()) => {}
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
    }
    sync_dir(path);
    Ok(())
}

pub(crate) fn sync_dir(path: &Path) {
    if let Some(dir) = path.parent() {
        if let Ok(d) = File::open(dir) {
            let _ = d.sync_all();
        }
    }
}

fn write_then_rename(
    tmp: &Path,
    path: &Path,
    bytes: &[u8],
    modified: Option<SystemTime>,
    paced: bool,
) -> std::io::Result<()> {
    let mut f = File::create(tmp)?;
    if paced {
        for slice in bytes.chunks(PACED_SLICE) {
            f.write_all(slice)?;
            f.sync_data()?;
            std::thread::sleep(PACED_REST);
        }
    } else {
        f.write_all(bytes)?;
    }
    if let Some(time) = modified {
        f.set_modified(time)?;
    }
    f.sync_all()?;
    drop(f);
    std::fs::rename(tmp, path)
}

const NAME_MAX: usize = 255;

fn temp_path(path: &Path) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let tail = format!(".{}.{seq}.tmp", std::process::id());
    let mut room = NAME_MAX.saturating_sub(tail.len() + 1).min(name.len());
    while room > 0 && !name.is_char_boundary(room) {
        room -= 1;
    }
    let tmp = format!(".{}{tail}", &name[..room]);
    match path.parent() {
        Some(dir) => dir.join(tmp),
        None => PathBuf::from(tmp),
    }
}
