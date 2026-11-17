//! A DS ROM's ARM9 overlays, read from the file as the game loads them: for the first bytes of
//! the watched functions that live in overlays, which tell one overlay loaded at an address from
//! another. After Pokémon Fulldex's frontend/ndsoverlays.h. Only the overlay table and the
//! overlays asked for are read, never the whole ROM.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// One overlay: where the game loads it, and its bytes as loaded (decompressed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overlay {
    pub ram: u32,
    pub data: Vec<u8>,
}

/// An open ROM's overlays.
pub struct Overlays {
    file: File,
    fat: u32,
    table: Vec<u8>,
    cache: HashMap<u32, Option<Overlay>>,
}

impl Overlays {
    /// The overlays of the ROM at `path`, where its header can be read.
    pub fn open(path: &Path) -> Option<Overlays> {
        let mut file = File::open(path).ok()?;
        let mut header = [0u8; 0x58];
        file.read_exact(&mut header).ok()?;
        let word = |at: usize| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
        let (fat, table_at, table_len) = (word(0x48), word(0x50), word(0x54));
        if table_len > 0x10000 {
            return None;
        }
        let mut table = vec![0u8; table_len as usize];
        file.seek(SeekFrom::Start(table_at as u64)).ok()?;
        file.read_exact(&mut table).ok()?;
        Some(Overlays {
            file,
            fat,
            table,
            cache: HashMap::new(),
        })
    }

    /// Overlay `id`, by its number in the overlay table, or none where the ROM has none.
    pub fn get(&mut self, id: u32) -> Option<&Overlay> {
        if !self.cache.contains_key(&id) {
            let found = self.read(id);
            self.cache.insert(id, found);
        }
        self.cache.get(&id).and_then(Option::as_ref)
    }

    fn read(&mut self, id: u32) -> Option<Overlay> {
        let entry = self.table.chunks_exact(32).find(|e| word(e, 0) == id)?;
        let (ram, file_id, flags) = (word(entry, 4), word(entry, 24), word(entry, 28));
        let mut range = [0u8; 8];
        self.file
            .seek(SeekFrom::Start(self.fat as u64 + file_id as u64 * 8))
            .ok()?;
        self.file.read_exact(&mut range).ok()?;
        let (start, end) = (word(&range, 0), word(&range, 4));
        if end < start || end - start > 0x40_0000 {
            return None;
        }
        let mut raw = vec![0u8; (end - start) as usize];
        self.file.seek(SeekFrom::Start(start as u64)).ok()?;
        self.file.read_exact(&mut raw).ok()?;
        let data = match flags & (1 << 24) {
            0 => raw,
            _ => unblz(&raw)?,
        };
        (!data.is_empty()).then_some(Overlay { ram, data })
    }
}

fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// Nintendo's backward LZ, the compression of the ARM9 binary and its overlays: read from the
/// end of `pak` towards its start, written the same way, its last eight bytes saying how.
pub fn unblz(pak: &[u8]) -> Option<Vec<u8>> {
    let len = pak.len();
    if len < 8 {
        return Some(pak.to_vec());
    }
    let grows = word(pak, len - 4) as usize;
    if grows == 0 {
        return Some(pak.to_vec());
    }
    let header = pak[len - 5] as usize;
    let packed = (word(pak, len - 8) & 0xFF_FFFF) as usize;
    if packed > len || header > packed {
        return None;
    }
    let plain = len - packed;
    let want = packed + grows;
    // the packed part reversed, read from its start, and the output reversed at the end
    let src: Vec<u8> = pak[plain..plain + packed - header]
        .iter()
        .rev()
        .copied()
        .collect();
    let mut out = Vec::with_capacity(want);
    let (mut i, mut mask, mut flags) = (0usize, 0u32, 0u32);
    while out.len() < want {
        mask >>= 1;
        if mask == 0 {
            flags = *src.get(i)? as u32;
            i += 1;
            mask = 0x80;
        }
        if flags & mask == 0 {
            out.push(*src.get(i)?);
            i += 1;
        } else {
            let pos = ((*src.get(i)? as usize) << 8) | *src.get(i + 1)? as usize;
            i += 2;
            let n = ((pos >> 12) + 3).min(want - out.len());
            let back = (pos & 0xFFF) + 3;
            if back > out.len() {
                return None;
            }
            for _ in 0..n {
                out.push(out[out.len() - back]);
            }
        }
    }
    out.reverse();
    let mut result = pak[..plain].to_vec();
    result.extend_from_slice(&out);
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Twenty sevens, packed: three literals, then one copy of 17 bytes from three back, the
    /// stream reversed, an eight-byte trailer after it, and a plain byte before it that is kept.
    #[test]
    fn a_packed_run_comes_back_whole_after_the_plain_part() {
        // flags 0x10: three literals, then a copy; 0xE000: 14 + 3 bytes from 0 + 3 back
        let stream = [0x10u8, 7, 7, 7, 0xE0, 0x00];
        let mut pak = vec![0x55];
        pak.extend(stream.iter().rev());
        let packed = stream.len() + 8;
        pak.extend_from_slice(&(packed as u32 | 8 << 24).to_le_bytes());
        pak.extend_from_slice(&((20 - packed) as u32).to_le_bytes());
        let mut want = vec![0x55];
        want.extend_from_slice(&[7; 20]);
        assert_eq!(unblz(&pak).unwrap(), want);
    }

    #[test]
    fn a_file_that_grows_nothing_is_taken_as_it_is() {
        let mut raw = vec![1, 2, 3, 4, 5, 6, 7, 8];
        raw.extend_from_slice(&[0, 0, 0, 0]);
        assert_eq!(unblz(&raw).unwrap(), raw);
    }

    #[test]
    fn a_trailer_that_points_outside_the_file_is_refused() {
        let mut raw = vec![0u8; 8];
        raw.extend_from_slice(&0x00FF_FFFFu32.to_le_bytes());
        raw.extend_from_slice(&16u32.to_le_bytes());
        assert_eq!(unblz(&raw), None);
    }
}
