//! The star a favorite wears on the shelf, at its top right corner, as pixel art: a gold star
//! with a dark outline and a lighter point, so it reads over any label or backdrop.

use crate::UndoFace;

const STAR: [&str; 11] = [
    ".....#.....",
    "....#h#....",
    "....#h#....",
    "#####hy####",
    "#hhhyyyyyy#",
    ".#yyyyyyy#.",
    "..#yyyyy#..",
    "..#yy#yy#..",
    ".#yy#.#yy#.",
    ".#y#...#y#.",
    ".##.....##.",
];

const OUTLINE: [u8; 4] = [0x2a, 0x1e, 0x10, 0xff];
const GOLD: [u8; 4] = [0xf4, 0xc2, 0x30, 0xff];
const LIGHT: [u8; 4] = [0xff, 0xec, 0x9a, 0xff];

/// The star's art pixels, a texel each.
pub fn star_face() -> UndoFace {
    let (w, h) = (STAR[0].len() as u32, STAR.len() as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for (y, row) in STAR.iter().enumerate() {
        for (x, c) in row.bytes().enumerate() {
            let px = match c {
                b'#' => OUTLINE,
                b'y' => GOLD,
                b'h' => LIGHT,
                _ => continue,
            };
            let at = (y * w as usize + x) * 4;
            rgba[at..at + 4].copy_from_slice(&px);
        }
    }
    UndoFace { rgba, w, h }
}

/// The Trainer style's mark in place of the star: a capture ball of its own drawing, its top red
/// and lit, its band and button dark, its foot cream, in the star's dark outline.
const BALL: [&str; 11] = [
    "...#####...",
    ".##rrrrr##.",
    ".#rhhrrrr#.",
    "#rhrrrrrrr#",
    "#rrrr###rr#",
    "#####w#####",
    "#cccc###cc#",
    "#ccccccccc#",
    ".#ccccccc#.",
    ".##ccccc##.",
    "...#####...",
];

const RED: [u8; 4] = [0xd2, 0x3c, 0x3c, 0xff];
const RED_LIT: [u8; 4] = [0xf4, 0x8a, 0x80, 0xff];
const CREAM: [u8; 4] = [0xee, 0xe8, 0xd8, 0xff];

/// The capture ball's art pixels, a texel each, the star's size: the favorite's mark while the
/// shelf is drawn over one of the Trainer backgrounds.
pub fn ball_face() -> UndoFace {
    let (w, h) = (BALL[0].len() as u32, BALL.len() as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for (y, row) in BALL.iter().enumerate() {
        for (x, c) in row.bytes().enumerate() {
            let px = match c {
                b'#' => OUTLINE,
                b'r' => RED,
                b'h' => RED_LIT,
                b'c' | b'w' => CREAM,
                _ => continue,
            };
            let at = (y * w as usize + x) * 4;
            rgba[at..at + 4].copy_from_slice(&px);
        }
    }
    UndoFace { rgba, w, h }
}

/// How big the star is drawn beside a cart at full size, in canvas pixels.
pub const STAR_PX: f32 = 22.0;
