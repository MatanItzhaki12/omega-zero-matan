//! What the kit draws with, as assets/ui/foundation/pixel.js: sprites written as rows of colour
//! keys, one character to an art pixel, the mirrors that make a whole sprite from a quarter, the
//! 9-slices that stretch one to any size, and Pixel Code's letters as sprite rows.
//!
//! The kit's faces are baked at art resolution, one texel to an art pixel. They are uploaded with
//! nearest filtering (`Compositor::create_texture_nearest`) and drawn at exactly `SPRITE_PX` (2x)
//! on even screen offsets, as [`quad`] does, so every edge lands on a whole art pixel.

use omega_gfx::{Draw, TexId};

use super::palette::Palette;
use crate::plate::UndoFace;
use crate::sprite::SPRITE_PX;
use crate::text;

/// Pixel Code is drawn on a grid of nine pixels to the em: two rows for accents over the
/// capitals, seven of capitals, two for descenders.
pub const ACCENT: usize = 2;
pub const CAP: usize = 7;
pub const DESCENT: usize = 2;

/// Pixel Code at one face pixel to an art pixel.
pub const LETTER_PX: f32 = 9.0;

/// How far one of Pixel Code's letters is from the next at `LETTER_PX`: five pixels of letter
/// and one of air. Every glyph of the face is this wide.
pub const ADVANCE: usize = 6;

/// How tall a line of text is in the kit, so a 9 px face's capitals sit on rows 2 to 8 of it.
pub const LINE: u32 = 10;

/// `rows` painted in `pal`, a texel to a character; '.' (and any key `pal` has no colour for)
/// is left clear.
pub fn keyed(rows: &[impl AsRef<str>], pal: &Palette) -> UndoFace {
    let w = rows
        .iter()
        .map(|r| r.as_ref().chars().count())
        .max()
        .unwrap_or(0)
        .max(1) as u32;
    let mut face = blank(w, rows.len().max(1) as u32);
    paint(&mut face, rows, 0, 0, pal);
    face
}

/// A sprite symmetric both ways, from its top-left quarter. The quarter's last row and column
/// are the sprite's middle ones, which both halves share (and a 9-slice stretches).
pub fn mirror(quarter: &[impl AsRef<str>]) -> Vec<String> {
    let rows: Vec<String> = quarter
        .iter()
        .map(|row| {
            let row: Vec<char> = row.as_ref().chars().collect();
            let back = row[..row.len().saturating_sub(1)].iter().rev();
            row.iter().chain(back).collect()
        })
        .collect();
    let back = rows[..rows.len().saturating_sub(1)].iter().rev().cloned();
    rows.iter().cloned().chain(back).collect()
}

/// Each row turned around, for the other side's sprite.
pub fn flip(rows: &[impl AsRef<str>]) -> Vec<String> {
    rows.iter()
        .map(|row| row.as_ref().chars().rev().collect())
        .collect()
}

/// `text` in Pixel Code at one face pixel to a sprite pixel, as rows of '#' (ink) and '.', each
/// letter `advance` pixels after the one before (`ADVANCE` is the face's own). The capitals are
/// rows `ACCENT` to `ACCENT + CAP - 1`, the baseline under row `ACCENT + CAP - 1`, as pixel.js
/// draws them with a canvas: each letter set on its own, a pixel inked where its coverage is
/// at least half. A character the face has no glyph for is spelled as `text::printed` does.
pub fn letters(text: &str, advance: usize) -> Vec<String> {
    letters_over(text, advance, 0)
}

/// `letters` with `over` more rows on top, for what rises above the line: a capital's accent
/// (the Á of OTÁVIO) stands a row higher than the `ACCENT` rows, where the browser draws it over
/// the line's top and `letters` leaves it out. Ink the rows `over` higher than `letters`' to set
/// the line in the same place.
pub fn letters_over(text: &str, advance: usize, over: usize) -> Vec<String> {
    let rows = over + ACCENT + CAP + DESCENT;
    let Some(font) = text::label_font() else {
        return vec![".".repeat((text.chars().count() * advance).max(1)); rows];
    };
    let chars: Vec<char> = text::printed(font, text).chars().collect();
    let w = (chars.len() * advance).max(1);
    let mut grid = vec![vec![b'.'; w]; rows];
    let baseline = (over + ACCENT + CAP) as i32;
    for (i, &ch) in chars.iter().enumerate() {
        let (m, cov) = font.rasterize(ch, LETTER_PX);
        let left = (i * advance) as i32 + m.xmin;
        let top = baseline - (m.height as i32 + m.ymin) - raised_colon(&chars, i) as i32;
        for gy in 0..m.height {
            for gx in 0..m.width {
                let (x, y) = (left + gx as i32, top + gy as i32);
                let inside = (0..w as i32).contains(&x) && (0..rows as i32).contains(&y);
                if inside && cov[gy * m.width + gx] >= 128 {
                    grid[y as usize][x as usize] = b'#';
                }
            }
        }
    }
    grid.into_iter()
        .map(|row| String::from_utf8(row).expect("ascii"))
        .collect()
}

/// Whether the `i`th letter is a colon Pixel Code draws a pixel higher: its `calt` swaps in
/// `colon.calt`, 112 of its 1008 units up (one pixel at 9 px), for a colon with a figure or a
/// capital on either side, a space apart at most, as in 16:00. The browser shapes the text and
/// so draws it there; fontdue doesn't shape, so the colon is lifted here.
fn raised_colon(chars: &[char], i: usize) -> bool {
    let tall = |c: Option<&char>| {
        c.is_some_and(|&c| {
            c.is_ascii_digit() || c == 'ß' || (c.is_uppercase() && (c as u32) < 0x100)
        })
    };
    let side =
        |near: Option<&char>, far: Option<&char>| tall(near) || (near == Some(&' ') && tall(far));
    chars[i] == ':'
        && side(
            i.checked_sub(1).and_then(|j| chars.get(j)),
            i.checked_sub(2).and_then(|j| chars.get(j)),
        )
        && side(chars.get(i + 1), chars.get(i + 2))
}

/// How an action's big word is drawn, as Settings > Theme > Font picks: bold, each letter drawn
/// again a pixel to its right, or thin, in Pixel Code's own one-pixel strokes.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Font {
    #[default]
    Bold,
    Thin,
}

/// An action's big word as sprite rows, as action-left.js's word(): the capitals of Pixel Code
/// drawn one face pixel to a pixel; in `Font::Bold` each letter drawn again a pixel to its right
/// (Pixel Code's own bold is fattened by part of a pixel, off the grid), so 6 wide and a pixel
/// clear of the next, and in `Font::Thin` as they are, 5 wide and a pixel clear; the rows of the
/// letters' bars, the top, the middle and the bottom (`BAR_ROWS`), doubled, so the capitals stand
/// 10 tall; and a shadow one pixel down and right of all of it. The ink is key 'L', the shadow
/// 'w'. 11 rows tall: the capitals and the shadow's row under them.
pub fn big_word(text: &str, font: Font) -> Vec<String> {
    let bold = font == Font::Bold;
    let ink = letters(text, if bold { ADVANCE + 1 } else { ADVANCE });
    let rows: Vec<usize> = (0..CAP)
        .flat_map(|i| std::iter::repeat_n(ACCENT + i, if BAR_ROWS.contains(&i) { 2 } else { 1 }))
        .collect();
    let inked = |x: i32, y: i32| {
        y >= 0
            && x >= 0
            && rows
                .get(y as usize)
                .is_some_and(|&r| ink[r].as_bytes().get(x as usize) == Some(&b'#'))
    };
    let at = |x: i32, y: i32| inked(x, y) || (bold && inked(x - 1, y));
    let w = ink[0].len() as i32;
    (0..=(rows.len() as i32))
        .map(|y| {
            (0..w)
                .map(|x| {
                    if at(x, y) {
                        'L'
                    } else if at(x - 1, y - 1) {
                        'w'
                    } else {
                        '.'
                    }
                })
                .collect()
        })
        .collect()
}

/// The rows of the capitals `big_word` doubles: where Pixel Code's letters have their bars.
pub const BAR_ROWS: [usize; 3] = [0, 3, 6];

/// How wide `text` is set in Pixel Code at `LETTER_PX`, in art pixels: what a line of it takes
/// across, as the browser lays it out.
pub fn text_width(text: &str) -> u32 {
    let Some(font) = text::label_font() else {
        return (text.chars().count() * ADVANCE) as u32;
    };
    text::printed(font, text)
        .chars()
        .map(|c| font.metrics(c, LETTER_PX).advance_width)
        .sum::<f32>()
        .round() as u32
}

/// A face `w` by `h`, clear.
pub fn blank(w: u32, h: u32) -> UndoFace {
    UndoFace {
        rgba: vec![0; (w * h * 4) as usize],
        w,
        h,
    }
}

/// `face`'s pixel at (`x`, `y`) set to `colour`, opaque; nothing when it falls outside.
fn put(face: &mut UndoFace, x: i32, y: i32, colour: [u8; 3]) {
    if x < 0 || y < 0 || x >= face.w as i32 || y >= face.h as i32 {
        return;
    }
    let i = ((y as u32 * face.w + x as u32) * 4) as usize;
    face.rgba[i..i + 4].copy_from_slice(&[colour[0], colour[1], colour[2], 255]);
}

/// The rect `[x, y, w, h]` of `face` filled with `colour`, as far as it falls inside.
pub fn fill(face: &mut UndoFace, [x, y, w, h]: [i32; 4], colour: [u8; 3]) {
    for py in y..y + h {
        for px in x..x + w {
            put(face, px, py, colour);
        }
    }
}

/// `rows` painted over `face` in `pal` with their top left at (`x`, `y`); '.' leaves what is
/// under it, and what falls outside `face` is dropped.
pub fn paint(face: &mut UndoFace, rows: &[impl AsRef<str>], x: i32, y: i32, pal: &Palette) {
    for (dy, row) in rows.iter().enumerate() {
        for (dx, key) in row.as_ref().chars().enumerate() {
            if let Some(colour) = pal.key(key) {
                put(face, x + dx as i32, y + dy as i32, colour);
            }
        }
    }
}

/// `letters`' ink painted over `face` in `colour` with its top left at (`x`, `y`): text set on
/// a line whose top is `y`, its baseline `ACCENT + CAP` below.
pub fn ink(face: &mut UndoFace, rows: &[impl AsRef<str>], x: i32, y: i32, colour: [u8; 3]) {
    for (dy, row) in rows.iter().enumerate() {
        for (dx, key) in row.as_ref().bytes().enumerate() {
            if key == b'#' {
                put(face, x + dx as i32, y + dy as i32, colour);
            }
        }
    }
}

/// `rows` used as a CSS border image over the box `[x, y, w, h]` of `face`, filled and
/// stretched (`border-image-slice: <slice> fill; border-image-repeat: stretch`) and drawn
/// pixelated. `slice` and `width` go top, right, bottom, left, as in CSS: each corner is the
/// sprite's as drawn (scaled from its slice to its border widths), each edge stretches the
/// sprite's between its corners, and the middle fills the rest. '.' leaves what is under it.
pub fn nine_slice(
    face: &mut UndoFace,
    rows: &[impl AsRef<str>],
    [x, y, w, h]: [i32; 4],
    slice: [u32; 4],
    width: [u32; 4],
    pal: &Palette,
) {
    let rows: Vec<Vec<char>> = rows.iter().map(|r| r.as_ref().chars().collect()).collect();
    let (src_w, src_h) = (rows[0].len() as u32, rows.len() as u32);
    let [top, right, bottom, left] = slice;
    let [top_w, right_w, bottom_w, left_w] = width;
    for dy in 0..h.max(0) as u32 {
        let sy = along(dy, h as u32, src_h, [top, bottom], [top_w, bottom_w]);
        for dx in 0..w.max(0) as u32 {
            let sx = along(dx, w as u32, src_w, [left, right], [left_w, right_w]);
            if let Some(colour) = pal.key(rows[sy as usize][sx as usize]) {
                put(face, x + dx as i32, y + dy as i32, colour);
            }
        }
    }
}

/// Which of a border image's `src` pixels across lands on pixel `at` of a box `len` long: the
/// near slice `slices[0]` scaled to its border `widths[0]`, the far one to `widths[1]`, and
/// the middle stretched over what is left, each by its nearest pixel.
fn along(at: u32, len: u32, src: u32, slices: [u32; 2], widths: [u32; 2]) -> u32 {
    let nearest = |from: u32, span: u32, i: u32, over: u32| from + (2 * i + 1) * span / (2 * over);
    let far = len.saturating_sub(widths[1]);
    if at < widths[0] {
        nearest(0, slices[0], at, widths[0])
    } else if at >= far {
        nearest(src - slices[1], slices[1], at - far, widths[1])
    } else {
        let middle = src - slices[0] - slices[1];
        nearest(slices[0], middle, at - widths[0], far - widths[0])
    }
}

/// A face `w` by `h` art pixels drawn with its top left at (`x`, `y`) on the screen, at
/// `SPRITE_PX` screen pixels to a texel. Keep `x` and `y` even so it lands on the art grid.
pub fn quad(tex: TexId, w: u32, h: u32, x: f32, y: f32) -> Draw {
    Draw::Tex {
        x,
        y,
        w: (w * SPRITE_PX) as f32,
        h: (h * SPRITE_PX) as f32,
        tex,
        alpha: 1.0,
    }
}
