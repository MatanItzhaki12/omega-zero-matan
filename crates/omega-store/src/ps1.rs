//! PlayStation content: the games in Games/PS1, the discs each is made of, the playlists that
//! tie a game's discs together, and the serial a disc carries.
//!
//! What the shelf shows of Games/PS1, and of each folder directly inside it (one level, for the
//! common `Games/PS1/<Game>/<Game>.cue` layout; folders deeper than that are not looked in):
//!
//! - an `.m3u` playlist that lists at least one disc there is, as one game named after the
//!   playlist; the discs it lists do not stand on their own as well, wherever they are;
//! - a `.cue`, `.chd` or `.pbp` that no playlist lists, as a game of one disc;
//! - discs in one folder whose names differ only by a `(Disc N)` tag, as one game named without
//!   the tag. A game of several such discs is given a playlist of its own, with whole paths, at
//!   `Config/playlists/PS1/<name>.m3u`, rewritten whenever its discs change; one of a single
//!   such disc is that disc.
//!
//! A `.bin` is never a game: it is the data a `.cue` points at.

use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use crate::platform::Platform;
use crate::Region;

/// Where the playlists made for games on several discs with none of their own are kept, under
/// the content root: outside Games/, so the shelf never shows them twice.
pub const PLAYLISTS_DIR: &str = "Config/playlists/PS1";

/// One game found in Games/PS1: the name it stands on the shelf under, which its saves, states
/// and label are kept by, and the file a core is given.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Game {
    pub stem: String,
    pub rom: PathBuf,
}

/// The disc images the game at `path` is made of, in order: one for a lone disc, every entry of
/// an `.m3u` playlist, each taken from the playlist's own folder unless it is a whole path. A
/// playlist that lists nothing stands for no disc.
pub fn discs(path: &Path) -> Vec<PathBuf> {
    if !has_ext(path, "m3u") {
        return vec![path.to_path_buf()];
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let dir = path.parent().unwrap_or_else(|| Path::new(""));
    text.lines()
        .map(|line| line.trim().trim_start_matches('\u{feff}'))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| dir.join(line))
        .collect()
}

/// Where the label of one disc is kept: `Labels/PS1/<disc's own name>.png`, the way a lone
/// disc's label is kept under its own name.
pub fn disc_label(root: &Path, disc: &Path) -> PathBuf {
    let stem = disc
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    root.join("Labels").join("PS1").join(format!("{stem}.png"))
}

/// The games in `dir`, which is `root`'s Games/PS1, by the rules at the top of this file, in no
/// particular order. Writes the playlists games on several discs need, and clears away those no
/// game needs any more. Fails only when `dir` itself cannot be read.
pub fn games(root: &Path, dir: &Path) -> std::io::Result<Vec<Game>> {
    let (mut files, folders) = list(dir)?;
    for folder in folders {
        match list(&folder) {
            Ok((inner, _)) => files.extend(inner),
            Err(e) => eprintln!("omega-zero: scan: {}: {e}", folder.display()),
        }
    }
    files.retain(|f| Platform::Ps1.accepts(f));
    files.sort();

    let (playlists, discs_found): (Vec<PathBuf>, Vec<PathBuf>) =
        files.into_iter().partition(|f| has_ext(f, "m3u"));
    let mut listed = HashSet::new();
    let mut out = Vec::new();
    for m3u in playlists {
        let entries = discs(&m3u);
        listed.extend(entries.iter().map(|d| same_file_key(d)));
        if entries.iter().any(|d| d.is_file()) {
            if let Some(stem) = stem_of(&m3u) {
                out.push(Game { stem, rom: m3u });
            }
        }
    }

    let mut sets: BTreeMap<SetKey, DiscSet> = BTreeMap::new();
    for disc in discs_found {
        if listed.contains(&same_file_key(&disc)) {
            continue;
        }
        let Some(stem) = stem_of(&disc) else {
            continue;
        };
        match disc_tag(&stem) {
            None => out.push(Game { stem, rom: disc }),
            Some((base, n)) => {
                let folder = disc.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
                let kind = ext_of(&disc);
                sets.entry((folder, base.to_lowercase(), kind))
                    .or_insert_with(|| (base, Vec::new()))
                    .1
                    .push((n, disc));
            }
        }
    }

    let playlist_dir = root.join(PLAYLISTS_DIR);
    let mut written = HashSet::new();
    for (_, (base, mut set)) in sets {
        set.sort();
        let rom = if set.len() == 1 {
            set.remove(0).1
        } else {
            let disc_paths: Vec<PathBuf> = set.into_iter().map(|(_, d)| d).collect();
            let playlist = playlist_dir.join(format!("{base}.m3u"));
            match write_playlist(&playlist, &disc_paths) {
                Ok(()) => {
                    written.insert(playlist.clone());
                    playlist
                }
                Err(e) => {
                    eprintln!("omega-zero: scan: {}: {e}", playlist.display());
                    disc_paths[0].clone()
                }
            }
        };
        out.push(Game { stem: base, rom });
    }
    clear_old_playlists(&playlist_dir, &written);
    Ok(out)
}

/// What sorts discs that differ only by their `(Disc N)` tag into one game: their folder, their
/// name without the tag with its case folded, and their kind of file.
type SetKey = (PathBuf, String, String);

/// The discs of one game sorted by [`SetKey`]: its name, and each disc with its number.
type DiscSet = (String, Vec<(u32, PathBuf)>);

/// The files and the folders in `dir`, leaving out hidden ones such as macOS's `._` sidecars.
fn list(dir: &Path) -> std::io::Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    let mut files = Vec::new();
    let mut folders = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if crate::is_hidden(&path) {
            continue;
        }
        if path.is_dir() {
            folders.push(path);
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok((files, folders))
}

/// Writes the playlist of a game's discs, unless it already says just that, so the card is not
/// written to on every scan.
fn write_playlist(playlist: &Path, discs: &[PathBuf]) -> std::io::Result<()> {
    let mut text = String::new();
    for disc in discs {
        let whole = std::path::absolute(disc)?;
        text.push_str(&whole.to_string_lossy());
        text.push('\n');
    }
    if std::fs::read_to_string(playlist).is_ok_and(|old| old == text) {
        return Ok(());
    }
    if let Some(dir) = playlist.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::atomic_write(playlist, text.as_bytes())
}

/// Removes the playlists in `dir` that no game was given this scan.
fn clear_old_playlists(dir: &Path, keep: &HashSet<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if has_ext(&path, "m3u") && path.is_file() && !keep.contains(&path) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// A disc name's `(Disc N)` or `(Disc N of M)` tag: the name without it, and N.
pub fn disc_tag(stem: &str) -> Option<(String, u32)> {
    let mut from = 0;
    while let Some(open) = stem[from..].find('(').map(|i| from + i) {
        let close = open + stem[open..].find(')')?;
        let inner = stem[open + 1..close].trim().to_ascii_lowercase();
        if let Some(rest) = inner.strip_prefix("disc") {
            let words: Vec<&str> = rest.split_whitespace().collect();
            let n = match words.as_slice() {
                [n] | [n, "of", _] => n.parse::<u32>().ok(),
                _ => None,
            };
            if let Some(n) = n {
                let left = stem[..open].trim_end();
                let right = stem[close + 1..].trim_start();
                let base = match (left.is_empty(), right.is_empty()) {
                    (_, true) => left.to_string(),
                    (true, false) => right.to_string(),
                    (false, false) => format!("{left} {right}"),
                };
                return (!base.is_empty()).then_some((base, n));
            }
        }
        from = close + 1;
    }
    None
}

/// What a playlist entry and a file found in a folder are compared by: the path with `.` and
/// `..` worked out and its case folded, as the memory card's file system does not care.
fn same_file_key(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            other => parts.push(other.as_os_str().to_string_lossy().to_lowercase()),
        }
    }
    parts.join("/")
}

fn stem_of(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_string)
}

fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn has_ext(path: &Path, ext: &str) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// The serial printed on the disc at `path`, such as `SLUS-00594`, if it can be read: from the
/// `BOOT` line of SYSTEM.CNF on a `.cue`'s first data track, from PARAM.SFO's `DISC_ID` in a
/// `.pbp`, or from a playlist's first disc. A `.chd` is compressed, and its serial is not read.
/// Only the few sectors that lead to SYSTEM.CNF are read, never the whole image.
pub fn serial(path: &Path) -> Option<String> {
    match ext_of(path).as_str() {
        "m3u" => discs(path)
            .into_iter()
            .next()
            .filter(|d| !has_ext(d, "m3u"))
            .and_then(|d| serial(&d)),
        "cue" => cue_serial(path),
        "pbp" => pbp_serial(path),
        _ => None,
    }
}

/// Where the game at `path`, named `stem`, comes from: a region in its file name's tags wins
/// over its serial.
pub fn region(path: &Path, stem: &str) -> Option<Region> {
    Region::from_tags(stem).or_else(|| serial(path).as_deref().and_then(serial_region))
}

/// The region a serial's prefix says: SLUS and SCUS are the USA's; SLES, SCES and their demo
/// discs Europe's; SLPS, SCPS, SLPM, SIPS, SCPM and the PAPX and PCPX demos Japan's; Korea's
/// and Asia's are another region.
pub fn serial_region(serial: &str) -> Option<Region> {
    let prefix = serial.get(..4)?.to_ascii_uppercase();
    match prefix.as_str() {
        "SLUS" | "SCUS" => Some(Region::Usa),
        "SLES" | "SCES" | "SLED" | "SCED" => Some(Region::Europe),
        "SLPS" | "SCPS" | "SLPM" | "SIPS" | "SCPM" | "PAPX" | "PCPX" => Some(Region::Japan),
        "SCKA" | "SLKA" | "SCAJ" | "SLAJ" => Some(Region::Other),
        _ => None,
    }
}

/// A serial written the way it is printed, `SLUS-00594`, from the way a disc's files name it,
/// such as `SLUS_005.94` or `SLUS00594`: four letters and five digits, or nothing.
pub fn tidy_serial(raw: &str) -> Option<String> {
    let kept: String = raw
        .chars()
        .filter(|c| !matches!(c, '_' | '.' | '-' | ' '))
        .collect::<String>()
        .to_ascii_uppercase();
    let (letters, digits) = (kept.get(..4)?, kept.get(4..)?);
    (letters.chars().all(|c| c.is_ascii_uppercase())
        && digits.len() == 5
        && digits.chars().all(|c| c.is_ascii_digit()))
    .then(|| format!("{letters}-{digits}"))
}

/// The serial the `BOOT` line of a SYSTEM.CNF names: `BOOT = cdrom:\SLUS_005.94;1` gives
/// `SLUS-00594`. A disc that boots `PSX.EXE` has no serial to give.
pub fn boot_serial(system_cnf: &str) -> Option<String> {
    system_cnf.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        if !key.trim().eq_ignore_ascii_case("BOOT") {
            return None;
        }
        let file = value.trim().rsplit(['\\', '/', ':']).next()?;
        tidy_serial(file.split(';').next()?)
    })
}

/// How the sectors of a disc image's data track are laid out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Track {
    /// Bytes from the start of the image file to the track's first sector.
    pub start: u64,
    /// Bytes per sector: 2352 raw, 2336 without the sync and header, 2048 cooked.
    pub sector: u64,
}

/// The largest cue sheet worth reading: real ones are a few hundred bytes.
const CUE_MAX: u64 = 64 * 1024;

fn cue_serial(cue: &Path) -> Option<String> {
    let mut text = String::new();
    File::open(cue)
        .ok()?
        .take(CUE_MAX)
        .read_to_string(&mut text)
        .ok()?;
    let (file, track) = first_data_track(&text)?;
    let image = cue.parent().unwrap_or_else(|| Path::new("")).join(file);
    image_serial(&image, track)
}

/// The image file and the layout of a cue sheet's first data track.
pub fn first_data_track(cue: &str) -> Option<(String, Track)> {
    let mut file: Option<String> = None;
    let mut sector: Option<u64> = None;
    for line in cue.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        match word.to_ascii_uppercase().as_str() {
            "FILE" => {
                file = Some(cue_file_name(rest.trim()));
                sector = None;
            }
            "TRACK" => {
                let mode = rest.split_whitespace().nth(1).unwrap_or_default();
                sector = match mode.to_ascii_uppercase().as_str() {
                    "MODE1/2352" | "MODE2/2352" => Some(2352),
                    "MODE2/2336" => Some(2336),
                    "MODE1/2048" | "MODE2/2048" => Some(2048),
                    _ => None,
                };
            }
            "INDEX" => {
                let mut words = rest.split_whitespace();
                if words.next().and_then(|n| n.parse::<u32>().ok()) != Some(1) {
                    continue;
                }
                if let (Some(f), Some(size)) = (&file, sector) {
                    let frames = msf_frames(words.next().unwrap_or("00:00:00"))?;
                    return Some((
                        f.clone(),
                        Track {
                            start: frames * size,
                            sector: size,
                        },
                    ));
                }
            }
            _ => {}
        }
    }
    None
}

/// A cue sheet's `FILE` name, quoted or not, without the file type after it.
fn cue_file_name(rest: &str) -> String {
    if let Some(quoted) = rest.strip_prefix('"') {
        return quoted.split('"').next().unwrap_or_default().to_string();
    }
    match rest.rsplit_once(char::is_whitespace) {
        Some((name, _kind)) => name.trim().to_string(),
        None => rest.to_string(),
    }
}

/// `mm:ss:ff` as a count of frames, 75 to the second.
fn msf_frames(msf: &str) -> Option<u64> {
    let parts: Vec<u64> = msf
        .split(':')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    match parts.as_slice() {
        [m, s, f] => Some((m * 60 + s) * 75 + f),
        _ => None,
    }
}

/// The ISO 9660 sector size, which is what each sector of a data track holds for a program.
pub(crate) const USER: usize = 2048;

/// The serial on the data track laid out as `track` in the image file at `image`: the primary
/// volume descriptor at sector 16 leads to the root folder, the root folder to SYSTEM.CNF, and
/// its `BOOT` line to the serial. Four or five sectors are read.
pub fn image_serial(image: &Path, track: Track) -> Option<String> {
    let mut file = File::open(image).ok()?;
    let mut sector = |lba: u32| read_sector(&mut file, track, lba);
    let pvd = sector(16)?;
    if pvd[0] != 1 || &pvd[1..6] != b"CD001" {
        return None;
    }
    let root = &pvd[156..156 + 34];
    let (root_lba, root_len) = (le32(&root[2..]), le32(&root[10..]));
    // the root of a PlayStation disc is a sector or two; more than 16 is not a real one
    let root_sectors = root_len.div_ceil(USER as u32).min(16);
    let mut found = None;
    for i in 0..root_sectors {
        let dir = sector(root_lba.checked_add(i)?)?;
        found = find_in_folder(&dir, "SYSTEM.CNF");
        if found.is_some() {
            break;
        }
    }
    let (lba, len) = found?;
    let cnf = sector(lba)?;
    let text = String::from_utf8_lossy(&cnf[..(len as usize).min(USER)]);
    boot_serial(&text)
}

/// The sector and length of the file named `name` in one sector of an ISO 9660 folder.
fn find_in_folder(dir: &[u8; USER], name: &str) -> Option<(u32, u32)> {
    find_entry(dir, name, false)
}

/// The sector and length of the entry named `name` in one sector of an ISO 9660 folder: a
/// folder where `folder` says, else a file.
pub(crate) fn find_entry(dir: &[u8; USER], name: &str, folder: bool) -> Option<(u32, u32)> {
    let mut at = 0;
    while at + 33 < USER {
        let len = dir[at] as usize;
        if len < 34 || at + len > USER {
            return None;
        }
        let name_len = dir[at + 32] as usize;
        let entry = dir.get(at + 33..at + 33 + name_len)?;
        let entry = entry.split(|&b| b == b';').next().unwrap_or_default();
        if entry.eq_ignore_ascii_case(name.as_bytes()) && (dir[at + 25] & 2 != 0) == folder {
            return Some((le32(&dir[at + 2..]), le32(&dir[at + 10..])));
        }
        at += len;
    }
    None
}

/// What one sector of the data track holds for a program, wherever its layout puts it: after
/// the sync, header and (for mode 2) subheader in a raw sector, after the subheader in a 2336
/// one, all of a cooked one.
fn read_sector(file: &mut File, track: Track, lba: u32) -> Option<[u8; USER]> {
    let mut raw = vec![0u8; track.sector as usize];
    file.seek(SeekFrom::Start(track.start + lba as u64 * track.sector))
        .ok()?;
    file.read_exact(&mut raw).ok()?;
    let at = match track.sector {
        2352 => {
            const SYNC: [u8; 12] = [0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0];
            if raw[..12] == SYNC && raw[15] == 1 {
                16
            } else {
                24
            }
        }
        2336 => 8,
        _ => 0,
    };
    raw.get(at..at + USER)?.try_into().ok()
}

fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// The largest PARAM.SFO worth reading from a `.pbp`: real ones are well under a kilobyte.
const SFO_MAX: u32 = 16 * 1024;

/// The `DISC_ID` in the PARAM.SFO at the start of a PSP eboot, which is how a `.pbp` names its
/// disc: a few hundred bytes are read.
fn pbp_serial(pbp: &Path) -> Option<String> {
    let mut file = File::open(pbp).ok()?;
    let mut head = [0u8; 16];
    file.read_exact(&mut head).ok()?;
    if &head[..4] != b"\0PBP" {
        return None;
    }
    let (sfo_at, next_at) = (le32(&head[8..]), le32(&head[12..]));
    let len = next_at.checked_sub(sfo_at)?.min(SFO_MAX);
    let mut sfo = vec![0u8; len as usize];
    file.seek(SeekFrom::Start(sfo_at as u64)).ok()?;
    file.read_exact(&mut sfo).ok()?;
    sfo_text(&sfo, "DISC_ID").and_then(|id| tidy_serial(&id))
}

/// The text kept under `key` in a PARAM.SFO.
pub fn sfo_text(sfo: &[u8], key: &str) -> Option<String> {
    if sfo.get(..4)? != b"\0PSF" {
        return None;
    }
    let keys = le32(sfo.get(8..12)?) as usize;
    let data = le32(sfo.get(12..16)?) as usize;
    let count = le32(sfo.get(16..20)?) as usize;
    (0..count.min(256)).find_map(|i| {
        let entry = sfo.get(20 + i * 16..20 + i * 16 + 16)?;
        let key_at = keys + u16::from_le_bytes([entry[0], entry[1]]) as usize;
        let name = sfo.get(key_at..)?.split(|&b| b == 0).next()?;
        if name != key.as_bytes() {
            return None;
        }
        let len = le32(&entry[4..]) as usize;
        let value_at = data + le32(&entry[12..]) as usize;
        let value = sfo.get(value_at..value_at + len)?;
        let value = value.split(|&b| b == 0).next()?;
        Some(String::from_utf8_lossy(value).into_owned())
    })
}
