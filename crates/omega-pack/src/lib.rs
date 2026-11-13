//! The card's `System/` folder as a release publishes it for Settings' Update: a manifest of
//! every file, with its SHA-256, and two data files, one holding every file and one holding
//! patches from the files of the releases before. The handheld sums what it has, then fetches
//! only what differs, by byte range: a patch where one runs from the file it has, else the
//! whole file.
//!
//! Each piece in a data file is LZ4, its size before it; a patch is a bsdiff patch, LZ4 the
//! same way.

pub mod sha256;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The manifest's first line, and its version: a handheld reads only what it knows.
pub const MAGIC: &str = "omega-system 1";

/// The release's assets, by name.
pub const MANIFEST_ASSET: &str = "system-manifest.txt";
pub const FILES_ASSET: &str = "system-files.bin";
pub const PATCHES_ASSET: &str = "system-patches.bin";

/// The folder the manifest covers, from the card's root.
pub const SYSTEM: &str = "System";

/// A file of the release: where it goes from the card's root, its sum and size, whether it
/// runs, and where its piece is in the files data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub sha: String,
    pub size: u64,
    pub exec: bool,
    pub offset: u64,
    pub len: u64,
}

/// A patch in the patches data: from the file summing `from`, at `path`, to the one summing
/// `to`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchEntry {
    pub path: String,
    pub from: String,
    pub to: String,
    pub offset: u64,
    pub len: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    pub version: String,
    pub files: Vec<FileEntry>,
    pub patches: Vec<PatchEntry>,
}

fn sha_ok(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A path the manifest may name: one under `System/`, with no way out of it.
pub fn path_ok(p: &str) -> bool {
    p.starts_with("System/")
        && !p.contains('\\')
        && !p.contains('\n')
        && p.split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

impl Manifest {
    pub fn parse(text: &str) -> Option<Manifest> {
        let mut lines = text.lines();
        if lines.next()?.trim_end() != MAGIC {
            return None;
        }
        let mut m = Manifest::default();
        for line in lines {
            let line = line.trim_end_matches('\r');
            let mut f = line.splitn(7, ' ');
            match f.next() {
                Some("version") => m.version = f.next()?.to_string(),
                Some("file") => {
                    let sha = f.next()?.to_ascii_lowercase();
                    let size = f.next()?.parse().ok()?;
                    let exec = f.next()? == "x";
                    let offset = f.next()?.parse().ok()?;
                    let len = f.next()?.parse().ok()?;
                    let path = f.next()?.to_string();
                    if !sha_ok(&sha) || !path_ok(&path) {
                        return None;
                    }
                    m.files.push(FileEntry {
                        path,
                        sha,
                        size,
                        exec,
                        offset,
                        len,
                    });
                }
                Some("patch") => {
                    let from = f.next()?.to_ascii_lowercase();
                    let to = f.next()?.to_ascii_lowercase();
                    let offset = f.next()?.parse().ok()?;
                    let len = f.next()?.parse().ok()?;
                    let path = f.next()?.to_string();
                    if !sha_ok(&from) || !sha_ok(&to) || !path_ok(&path) {
                        return None;
                    }
                    m.patches.push(PatchEntry {
                        path,
                        from,
                        to,
                        offset,
                        len,
                    });
                }
                _ => {}
            }
        }
        (!m.version.is_empty()).then_some(m)
    }

    pub fn text(&self) -> String {
        let mut out = format!("{MAGIC}\nversion {}\n", self.version);
        for f in &self.files {
            let x = if f.exec { "x" } else { "-" };
            let _ = writeln!(
                out,
                "file {} {} {x} {} {} {}",
                f.sha, f.size, f.offset, f.len, f.path
            );
        }
        for p in &self.patches {
            let _ = writeln!(
                out,
                "patch {} {} {} {} {}",
                p.from, p.to, p.offset, p.len, p.path
            );
        }
        out
    }
}

/// How the handheld comes by one file of the release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step<'a> {
    /// The whole file, from the files data.
    Whole(&'a FileEntry),
    /// A patch to the file it has.
    Patch(&'a FileEntry, &'a PatchEntry),
}

impl Step<'_> {
    pub fn file(&self) -> &FileEntry {
        match self {
            Step::Whole(f) | Step::Patch(f, _) => f,
        }
    }

    /// How much of the data it fetches.
    pub fn bytes(&self) -> u64 {
        match self {
            Step::Whole(f) => f.len,
            Step::Patch(_, p) => p.len,
        }
    }
}

/// What the handheld fetches to have release `m`, from what it has: `have` gives the sum of the
/// file it has at a path, or none. A file it has as the release has it is left be.
pub fn plan<'a>(m: &'a Manifest, have: impl Fn(&str) -> Option<String>) -> Vec<Step<'a>> {
    m.files
        .iter()
        .filter_map(|f| {
            let old = have(&f.path);
            if old.as_deref() == Some(f.sha.as_str()) {
                return None;
            }
            let patch = old.and_then(|old| {
                m.patches
                    .iter()
                    .find(|p| p.path == f.path && p.from == old && p.to == f.sha)
            });
            Some(match patch {
                Some(p) => Step::Patch(f, p),
                None => Step::Whole(f),
            })
        })
        .collect()
}

/// The files a release before put on the card, by `installed`, that release `m` has no more.
pub fn gone<'a>(installed: &'a [String], m: &Manifest) -> Vec<&'a str> {
    installed
        .iter()
        .filter(|p| path_ok(p) && !m.files.iter().any(|f| &f.path == *p))
        .map(String::as_str)
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub enum Bad {
    /// A piece that does not decompress, or a patch that does not apply.
    Corrupt,
    /// What came out is not the file the manifest names.
    Sum,
}

/// A piece of a data file as it is stored.
pub fn pack(bytes: &[u8]) -> Vec<u8> {
    lz4_flex::compress_prepend_size(bytes)
}

pub fn unpack(piece: &[u8]) -> Result<Vec<u8>, Bad> {
    lz4_flex::decompress_size_prepended(piece).map_err(|_| Bad::Corrupt)
}

/// The patch from `old` to `new`, as the patches data stores it.
pub fn make_patch(old: &[u8], new: &[u8]) -> Vec<u8> {
    let mut patch = Vec::new();
    bsdiff::diff(old, new, &mut patch).expect("a patch written to memory");
    pack(&patch)
}

/// The file a stored patch makes of `old`.
pub fn apply_patch(old: &[u8], piece: &[u8]) -> Result<Vec<u8>, Bad> {
    let patch = unpack(piece)?;
    let mut new = Vec::new();
    bsdiff::patch(old, &mut patch.as_slice(), &mut new).map_err(|_| Bad::Corrupt)?;
    Ok(new)
}

/// The file a step's piece makes: the whole file, or the patch applied to `old`, checked
/// against the sum the manifest gives.
pub fn rebuild(step: &Step, piece: &[u8], old: Option<&[u8]>) -> Result<Vec<u8>, Bad> {
    let bytes = match step {
        Step::Whole(_) => unpack(piece)?,
        Step::Patch(..) => apply_patch(old.ok_or(Bad::Corrupt)?, piece)?,
    };
    match sha256::hex(&bytes) == step.file().sha {
        true => Ok(bytes),
        false => Err(Bad::Sum),
    }
}

/// Every file under `root/System`, by its path from `root`, in order.
pub fn tree(root: &Path) -> std::io::Result<Vec<(String, PathBuf)>> {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<(String, PathBuf)>) -> std::io::Result<()> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            let rel = format!("{rel}/{name}");
            let kind = e.file_type()?;
            if kind.is_dir() {
                walk(&path, &rel, out)?;
            } else if kind.is_file() {
                out.push((rel, path));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(&root.join(SYSTEM), SYSTEM, &mut out)?;
    Ok(out)
}

#[cfg(unix)]
fn runs(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn runs(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "sh") || path.extension().is_none()
}

/// A core's source, which the release's zip ships beside it for its licence: a handheld has no
/// use for it, and PPSSPP's alone is most of what a release weighs, so updates leave it be.
pub fn source_archive(rel: &str) -> bool {
    rel.starts_with("System/licenses/") && [".tar.gz", ".tar.xz"].iter().any(|e| rel.ends_with(e))
}

/// The release of the card tree at `new`, version `version`: its manifest, its files data and
/// its patches data, with a patch for each file that differs from the same file in one of the
/// trees `before`, where the patch is smaller than the file.
pub fn build(
    new: &Path,
    version: &str,
    before: &[PathBuf],
) -> std::io::Result<(Manifest, Vec<u8>, Vec<u8>)> {
    let mut m = Manifest {
        version: version.to_string(),
        ..Manifest::default()
    };
    let mut files = Vec::new();
    let mut patches = Vec::new();
    for (rel, path) in tree(new)? {
        if !path_ok(&rel) || source_archive(&rel) {
            continue;
        }
        let bytes = std::fs::read(&path)?;
        let sha = sha256::hex(&bytes);
        let piece = pack(&bytes);
        m.files.push(FileEntry {
            path: rel.clone(),
            sha: sha.clone(),
            size: bytes.len() as u64,
            exec: runs(&path),
            offset: files.len() as u64,
            len: piece.len() as u64,
        });
        files.extend_from_slice(&piece);
        // a compressed archive changes all through for any change inside it
        let squeezed = [".gz", ".xz", ".zip", ".bz2"]
            .iter()
            .any(|e| rel.ends_with(e));
        for old_root in before.iter().filter(|_| !squeezed) {
            let Ok(old) = std::fs::read(old_root.join(&rel)) else {
                continue;
            };
            let from = sha256::hex(&old);
            if from == sha || m.patches.iter().any(|p| p.path == rel && p.from == from) {
                continue;
            }
            let patch = make_patch(&old, &bytes);
            if patch.len() >= piece.len() {
                continue;
            }
            m.patches.push(PatchEntry {
                path: rel.clone(),
                from,
                to: sha.clone(),
                offset: patches.len() as u64,
                len: patch.len() as u64,
            });
            patches.extend_from_slice(&patch);
        }
    }
    Ok((m, files, patches))
}
