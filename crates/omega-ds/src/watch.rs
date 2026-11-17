//! Which of a game's functions ran lately, for the layouts that tell its screens apart by them:
//! after Pokémon Fulldex's FuncWatch (frontend/funcwatch.h). The core marks each entry into a
//! watched function in a bitmap (cores/melonds/one-screen.patch); once a frame the watch reads
//! which were entered and keeps the frame each last ran.
//!
//! Many overlays are loaded at the same addresses, so a function in one counts as entered only
//! where the first bytes in memory are its own, as the table has them, or as the player's ROM has
//! them where the table leaves them to it (`?`).

use std::path::Path;

use crate::rom::Overlays;

/// Main RAM, and the ITCM below it where some of the game's code runs: the addresses watched.
pub const BASE: u32 = 0x01FF_8000;
pub const END: u32 = 0x0240_0000;
/// Where main RAM starts, and how big it is.
const MAIN_RAM: u32 = 0x0200_0000;
const MAIN_RAM_LEN: usize = 0x40_0000;

/// One watched function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Func {
    pub addr: u32,
    pub name: String,
    /// Its first four bytes, little-endian, where they must be checked.
    pub first: Option<u32>,
}

/// A table's line: `addr kind module first name`, its first bytes `-`, `?` or eight hex digits.
#[derive(Debug, PartialEq, Eq)]
struct Line<'a> {
    addr: u32,
    module: &'a str,
    first: &'a str,
    name: &'a str,
}

fn parse(line: &str) -> Option<Line<'_>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let mut w = line.split_whitespace();
    let addr = u32::from_str_radix(w.next()?, 16).ok()?;
    let _kind = w.next()?;
    let module = w.next()?;
    let first = w.next()?;
    let name = w.next()?;
    let hex = first.len() == 8 && first.bytes().all(|b| b.is_ascii_hexdigit());
    (first == "-" || first == "?" || hex).then_some(Line {
        addr,
        module,
        first,
        name,
    })
}

/// The function table `text`, its `?` first bytes read from the ROM at `rom` (a function whose
/// overlay the ROM does not reach is not checked).
pub fn load(text: &str, rom: &Path) -> Vec<Func> {
    let mut overlays = None;
    let mut out = Vec::new();
    for line in text.lines().filter_map(parse) {
        if !(BASE..END).contains(&line.addr) {
            continue;
        }
        let first = match line.first {
            "-" => None,
            "?" => {
                let ovl = overlays.get_or_insert_with(|| Overlays::open(rom));
                first_bytes(ovl.as_mut(), line.module, line.addr)
            }
            hex => u32::from_str_radix(hex, 16).ok().map(u32::swap_bytes),
        };
        out.push(Func {
            addr: line.addr,
            name: line.name.to_string(),
            first,
        });
    }
    out
}

/// The first four bytes at `addr` of overlay `module` (`ov16`), as the ROM loads it.
fn first_bytes(rom: Option<&mut Overlays>, module: &str, addr: u32) -> Option<u32> {
    let id: u32 = module.strip_prefix("ov")?.parse().ok()?;
    let ov = rom?.get(id)?;
    let at = addr.checked_sub(ov.ram)? as usize;
    let b = ov.data.get(at..at + 4)?;
    Some(u32::from_le_bytes(b.try_into().ok()?))
}

/// The functions watched and when each last ran.
pub struct Watch {
    funcs: Vec<Func>,
    /// One bit per halfword from [`BASE`] to [`END`]: the functions' first addresses, and the
    /// ones the core entered since the last frame was read.
    bitmap: Vec<u8>,
    hits: Vec<u8>,
    last: Vec<u32>,
    frame: u32,
}

impl Watch {
    pub fn new(funcs: Vec<Func>) -> Watch {
        let bytes = ((END - BASE) / 2 / 8) as usize + 1;
        let mut bitmap = vec![0u8; bytes];
        for f in &funcs {
            let bit = ((f.addr - BASE) / 2) as usize;
            bitmap[bit / 8] |= 1 << (bit % 8);
        }
        let last = vec![0; funcs.len()];
        Watch {
            funcs,
            bitmap,
            hits: vec![0; bytes],
            last,
            frame: 1,
        }
    }

    /// The bitmap and hits the core is handed, and the range they cover.
    pub fn for_core(&mut self) -> (&[u8], &mut [u8], u32, u32) {
        (&self.bitmap, &mut self.hits, BASE, END)
    }

    /// The function named `name`, by its place in the table.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.funcs.iter().position(|f| f.name == name)
    }

    /// Counts the frame the core just ran: each function whose bit it set ran in it, where the
    /// code in `main_ram` (the DS's 4 MB from 0x02000000) is that function's.
    pub fn frame_done(&mut self, main_ram: &[u8]) {
        self.frame += 1;
        for (i, f) in self.funcs.iter().enumerate() {
            let bit = ((f.addr - BASE) / 2) as usize;
            let (byte, mask) = (bit / 8, 1u8 << (bit % 8));
            if self.hits[byte] & mask == 0 {
                continue;
            }
            let own = match f.first {
                None => true,
                Some(first) => {
                    f.addr >= MAIN_RAM && {
                        let at = (f.addr - MAIN_RAM) as usize % MAIN_RAM_LEN;
                        main_ram
                            .get(at..at + 4)
                            .is_some_and(|b| u32::from_le_bytes(b.try_into().unwrap()) == first)
                    }
                }
            };
            if own {
                self.last[i] = self.frame;
            }
        }
        for f in &self.funcs {
            let bit = ((f.addr - BASE) / 2) as usize;
            self.hits[bit / 8] = 0;
        }
    }

    /// Whether function `f` ran in the last `frames` frames (0: the frame just run).
    pub fn ran(&self, f: Option<usize>, frames: u32) -> bool {
        f.and_then(|f| self.last.get(f))
            .is_some_and(|&last| last != 0 && last + frames >= self.frame)
    }

    /// The frame function `f` last ran in, 0 for never.
    pub fn last_frame(&self, f: Option<usize>) -> u32 {
        f.and_then(|f| self.last.get(f)).copied().unwrap_or(0)
    }

    /// Forgets when every function ran: after a state is loaded.
    pub fn forget(&mut self) {
        self.last.fill(0);
        self.hits.fill(0);
    }

    /// Marks function `f` as run in the frame just counted, as the core would have: for tests.
    #[cfg(test)]
    pub(crate) fn hit(&mut self, f: usize) {
        let bit = ((self.funcs[f].addr - BASE) / 2) as usize;
        self.hits[bit / 8] |= 1 << (bit % 8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "\
# a comment
0203ac44 t main - StartMenu_Main
021d0f68 t ov5 12345678 FieldMap_Main
01000000 a itcm - OutOfRange
nonsense line
";

    fn watch() -> Watch {
        Watch::new(load(TABLE, Path::new("/nowhere.nds")))
    }

    #[test]
    fn a_table_keeps_the_functions_in_the_watched_range() {
        let w = watch();
        assert_eq!(w.funcs.len(), 2);
        assert_eq!(
            w.funcs[1].first,
            Some(0x7856_3412),
            "the bytes as they lie in memory"
        );
        assert_eq!(w.find("FieldMap_Main"), Some(1));
        assert_eq!(w.find("OutOfRange"), None);
    }

    #[test]
    fn a_function_counts_as_run_only_while_its_own_code_is_there() {
        let mut w = watch();
        let mut ram = vec![0u8; MAIN_RAM_LEN];
        w.hit(0);
        w.hit(1);
        w.frame_done(&ram);
        assert!(w.ran(Some(0), 0), "main code needs no check");
        assert!(
            !w.ran(Some(1), 0),
            "another overlay's code is at its address"
        );
        let at = (0x021d_0f68 - MAIN_RAM) as usize;
        ram[at..at + 4].copy_from_slice(&[0x12, 0x34, 0x56, 0x78]);
        w.hit(1);
        w.frame_done(&ram);
        assert!(w.ran(Some(1), 0));
        assert!(w.ran(Some(0), 1) && !w.ran(Some(0), 0), "one frame ago");
        assert_eq!(w.last_frame(Some(1)), 3);
        w.forget();
        assert!(!w.ran(Some(1), 10));
    }
}
