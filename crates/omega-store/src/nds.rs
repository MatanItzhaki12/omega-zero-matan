//! DS games: what a game's cartridge header and banner say it is.
//!
//! A DS ROM begins with a 512-byte header: its title in twelve ASCII bytes at 0x00 and its game
//! code, four letters such as `CPUE`, at 0x0C, whose last letter is its region. The header's
//! word at 0x68 is where the banner is, whose English title (UTF-16, 0x100 bytes at 0x340 in
//! it) is the game's name in lines, the publisher's last: `Pokémon Platinum\nVersion\nNintendo`.
//!
//! Only the header and the banner's title are read, never the whole ROM, which runs to 512 MB.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// What a DS game says it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Info {
    /// The header's game code: `CPUE` is Pokémon Platinum in the USA.
    pub code: String,
    /// The banner's English title without its publisher, its lines joined by spaces; the
    /// header's twelve letters where the banner has none.
    pub title: String,
}

/// Where the English title is in the banner, and how long it is.
const BANNER_ENGLISH: u64 = 0x340;
const BANNER_TITLE_LEN: usize = 0x100;

/// What the DS game at `path` says it is, where its header can be read.
pub fn info(path: &Path) -> Option<Info> {
    let mut file = File::open(path).ok()?;
    let mut header = [0u8; 0x6C];
    file.read_exact(&mut header).ok()?;
    let code = ascii(&header[0x0C..0x10]);
    let banner = u32::from_le_bytes(header[0x68..0x6C].try_into().ok()?) as u64;
    let title = (banner != 0)
        .then(|| banner_title(&mut file, banner))
        .flatten()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| ascii(&header[0x00..0x0C]));
    Some(Info { code, title })
}

/// The English title in the banner at `banner`, without the publisher's line.
fn banner_title(file: &mut File, banner: u64) -> Option<String> {
    file.seek(SeekFrom::Start(banner + BANNER_ENGLISH)).ok()?;
    let mut raw = [0u8; BANNER_TITLE_LEN];
    file.read_exact(&mut raw).ok()?;
    Some(title_lines(&utf16(&raw)))
}

/// A banner title's lines without the last, the publisher's, joined by spaces; one line alone
/// is kept.
pub fn title_lines(text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let kept = match lines.len() {
        0 | 1 => &lines[..],
        n => &lines[..n - 1],
    };
    kept.join(" ")
}

/// The region a DS game's code says, by its last letter: `E` the USA, `P` Europe, `J` Japan.
pub fn code_region(code: &str) -> Option<crate::Region> {
    match code.chars().nth(3)?.to_ascii_uppercase() {
        'E' | 'O' => Some(crate::Region::Usa),
        'P' | 'D' | 'F' | 'I' | 'S' | 'H' | 'U' | 'X' | 'Y' | 'Z' => Some(crate::Region::Europe),
        'J' => Some(crate::Region::Japan),
        'K' | 'C' | 'T' | 'W' => Some(crate::Region::Other),
        _ => None,
    }
}

fn ascii(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take_while(|&&b| b != 0)
        .filter(|b| b.is_ascii_graphic() || **b == b' ')
        .map(|&b| b as char)
        .collect::<String>()
        .trim()
        .to_string()
}

fn utf16(raw: &[u8]) -> String {
    let units: Vec<u16> = raw
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ROM's first bytes: the header, and a banner at 0x200 with `english` as its title.
    fn rom(title: &[u8], code: &[u8], english: &str) -> Vec<u8> {
        let mut rom = vec![0u8; 0x200 + 0x340 + 0x100];
        rom[..title.len()].copy_from_slice(title);
        rom[0x0C..0x10].copy_from_slice(code);
        rom[0x68..0x6C].copy_from_slice(&0x200u32.to_le_bytes());
        for (i, u) in english.encode_utf16().enumerate() {
            rom[0x200 + 0x340 + i * 2..][..2].copy_from_slice(&u.to_le_bytes());
        }
        rom
    }

    fn read(bytes: &[u8]) -> Option<Info> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("game.nds");
        std::fs::write(&path, bytes).unwrap();
        info(&path)
    }

    #[test]
    fn a_game_is_named_by_its_banner_without_the_publisher() {
        let got = read(&rom(
            b"POKEMON PL",
            b"CPUE",
            "Pokémon Platinum\nVersion\nNintendo",
        ))
        .unwrap();
        assert_eq!(got.code, "CPUE");
        assert_eq!(got.title, "Pokémon Platinum Version");
    }

    #[test]
    fn a_game_with_no_banner_goes_by_its_header() {
        let mut bytes = rom(b"HOMEBREW", b"####", "");
        bytes[0x68..0x6C].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(read(&bytes).unwrap().title, "HOMEBREW");
        assert_eq!(read(&bytes[..0x20]), None, "too short for a header");
    }

    #[test]
    fn the_code_says_the_region() {
        assert_eq!(code_region("CPUE"), Some(crate::Region::Usa));
        assert_eq!(code_region("IRDO"), Some(crate::Region::Usa));
        assert_eq!(code_region("CPUP"), Some(crate::Region::Europe));
        assert_eq!(code_region("IPKJ"), Some(crate::Region::Japan));
        assert_eq!(code_region("CP"), None);
        assert_eq!(title_lines("One line"), "One line");
    }
}
