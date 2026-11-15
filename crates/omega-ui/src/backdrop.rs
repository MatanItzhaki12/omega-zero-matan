use std::path::Path;

use omega_gfx::{out_h, out_w, Draw, TexId};
use omega_store::{custom_hue, BackdropStyle, BACKDROP_COLOURS};

use crate::kit::palette::fit;

use crate::art;
use crate::plate::UndoFace;

const SCRIM: f32 = 0.62;

pub fn wallpaper_face(path: &Path) -> Option<Vec<u8>> {
    art::cover(path, out_w(), out_h())
}

pub fn draw_backdrop(face: Option<TexId>, out: &mut Vec<Draw>) {
    let Some(tex) = face else {
        return;
    };
    out.push(Draw::Tex {
        x: 0.0,
        y: 0.0,
        w: out_w() as f32,
        h: out_h() as f32,
        tex,
        alpha: 1.0,
    });
    out.push(Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: out_w() as f32,
        h: out_h() as f32,
        colour: [0.0, 0.0, 0.0, SCRIM],
    });
}

/// What each of [`BACKDROP_COLOURS`] looks like: dark enough that a cart's label always
/// reads over it, the first the near-black omega-zero drew before it had a choice.
pub const BACKDROP_PALETTE: [[u8; 3]; BACKDROP_COLOURS.len()] = [
    [0x05, 0x05, 0x08],
    [0x22, 0x25, 0x2c],
    [0x10, 0x1b, 0x36],
    [0x0b, 0x2b, 0x2e],
    [0x10, 0x2c, 0x16],
    [0x29, 0x14, 0x31],
    [0x30, 0x0f, 0x16],
    [0x30, 0x22, 0x08],
    [0x17, 0x17, 0x19],
    [0x25, 0x17, 0x1c],
    [0x18, 0x1c, 0x28],
    [0x17, 0x23, 0x1f],
    [0x28, 0x1c, 0x1a],
    [0x1d, 0x25, 0x1b],
];

/// The red Trainer's menus take their hue from: a capture ball's top. The capture balls the
/// pattern draws are this red too, whatever the colour.
pub const TRAINER_RED: [u8; 3] = [0xd8, 0x30, 0x30];

/// The dark grey Trainer is, and that the game colours and the custom ones lean a little toward
/// their own colour from.
const TRAINER_GREY: [u8; 3] = [0x17, 0x17, 0x19];
/// How far a game or custom colour's grey leans toward its colour.
const GROUND_SHARE: f32 = 0.08;
/// How light and how strong a custom colour is, in OKLCH, as far as sRGB lets it.
const CUSTOM_LIGHTNESS: f64 = 0.62;
const CUSTOM_CHROMA: f64 = 0.2;

const fn hex(rgb: u32) -> [u8; 3] {
    [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]
}

/// The colour backdrop colour `colour`'s menus wear when it is not its own: a capture ball's
/// red for Trainer, a game's colour, or a custom colour's hue. The backdrop is then a dark grey.
pub fn backdrop_tone(colour: u8) -> Option<[u8; 3]> {
    if let Some(hue) = custom_hue(colour) {
        return Some(fit(CUSTOM_LIGHTNESS, CUSTOM_CHROMA, hue as f64));
    }
    match BACKDROP_COLOURS.get(colour as usize).copied() {
        Some("trainer") => Some(TRAINER_RED),
        Some("ruby") => Some(hex(0xc0183c)),
        Some("sapphire") => Some(hex(0x2058d0)),
        Some("emerald") => Some(hex(0x18a860)),
        Some("firered") => Some(hex(0xe85020)),
        Some("leafgreen") => Some(hex(0x68c038)),
        _ => None,
    }
}

/// The grey under a game's or a custom colour's menus: Trainer's, leant a little toward `tone`.
pub fn toned_ground(tone: [u8; 3]) -> [u8; 3] {
    let lean = |g: u8, c: u8| (g as f32 + (c as f32 - g as f32) * GROUND_SHARE).round() as u8;
    [
        lean(TRAINER_GREY[0], tone[0]),
        lean(TRAINER_GREY[1], tone[1]),
        lean(TRAINER_GREY[2], tone[2]),
    ]
}

/// A colour's name to show, from the key omega-zero.state keeps it by.
pub fn backdrop_colour_name(colour: u8) -> &'static str {
    match BACKDROP_COLOURS.get(colour as usize).copied() {
        Some("slate") => "Slate",
        Some("navy") => "Navy",
        Some("teal") => "Teal",
        Some("green") => "Green",
        Some("plum") => "Plum",
        Some("crimson") => "Crimson",
        Some("amber") => "Amber",
        Some("trainer") => "Trainer",
        Some("ruby") => "Ruby",
        Some("sapphire") => "Sapphire",
        Some("emerald") => "Emerald",
        Some("firered") => "Fire Red",
        Some("leafgreen") => "Leaf Green",
        _ if custom_hue(colour).is_some() => "Custom",
        _ => "Night",
    }
}

/// The backdrop colour `colour` itself: a named one's, or a custom one's grey. Out of range is
/// the last named.
pub fn backdrop_rgb(colour: u8) -> [u8; 3] {
    if custom_hue(colour).is_some() {
        if let Some(tone) = backdrop_tone(colour) {
            return toned_ground(tone);
        }
    }
    BACKDROP_PALETTE[(colour as usize).min(BACKDROP_PALETTE.len() - 1)]
}

/// How far a pattern goes before it looks the same again: a tiled pattern's square, the bits'
/// scatter.
/// Both divide 640, 720 and 480, so a still pattern sits square on every canvas.
pub fn backdrop_period(style: BackdropStyle) -> u32 {
    match style {
        BackdropStyle::Bits | BackdropStyle::Capture | BackdropStyle::Grass => 80,
        _ => 40,
    }
}

/// How fast a drifting pattern moves, in canvas pixels a second: slow enough to sit behind the
/// shelf rather than pull the eye from it, and a whole number of milliseconds to a period.
pub fn backdrop_speed(style: BackdropStyle) -> f64 {
    match style {
        BackdropStyle::Bits | BackdropStyle::Capture | BackdropStyle::Grass => 10.0,
        _ => 16.0,
    }
}

/// How far left a pattern has drifted `ms` milliseconds in, in whole art pixels (`ART` screen
/// pixels at a time) so its pixels stay on the grid the rest of the UI is drawn on, starting
/// over each period.
pub fn backdrop_shift(style: BackdropStyle, moving: bool, ms: u64) -> u32 {
    if !moving || !style.moves() {
        return 0;
    }
    let moved = ms as f64 / 1000.0 * backdrop_speed(style);
    let art = (moved / ART as f64).floor() as u64 * ART as u64;
    (art % backdrop_period(style) as u64) as u32
}

/// `base` lifted toward white by `lift`, or sunk toward black by as much when it is below 0.
fn tint(base: [u8; 3], lift: f32) -> [u8; 4] {
    let c = |v: u8| {
        let v = v as f32;
        let to = if lift < 0.0 {
            v * (1.0 + lift)
        } else {
            v + (255.0 - v) * lift
        };
        to.round() as u8
    };
    [c(base[0]), c(base[1]), c(base[2]), 255]
}

/// One of a tiled pattern's pixels is 2x2 of the canvas's, as the words' and the sprites' are.
const ART: u32 = 2;
/// A tiled pattern's square, in its own pixels: one period.
const TILE: u32 = 20;
const BIT: u32 = 4;

/// Ordered dither thresholds, out of 16.
const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// A plate lit from the top left: a light edge above and to the left, a dark one below and to
/// the right, and a face a shade above the colour so the dark edge shows even on Night.
fn plate(tx: u32, ty: u32) -> f32 {
    let (top, left) = (ty == 0, tx == 0);
    let (bottom, right) = (ty == TILE - 1, tx == TILE - 1);
    if (top && right) || (bottom && left) {
        0.045
    } else if top || left {
        0.13
    } else if bottom || right {
        -0.6
    } else {
        0.045
    }
}

/// How far the pixel (`tx`, `ty`) of a tiled pattern's square lifts the colour (see [`tint`]).
fn tile_lift(style: BackdropStyle, tx: u32, ty: u32) -> f32 {
    let last = TILE - 1;
    match style {
        BackdropStyle::Plates => plate(tx, ty),
        BackdropStyle::Rivets => {
            let at = |v: u32| v == 2 || v == last - 2;
            if at(tx) && at(ty) {
                0.2
            } else if tx > 0 && ty > 0 && at(tx - 1) && at(ty - 1) {
                -0.6
            } else {
                plate(tx, ty)
            }
        }
        BackdropStyle::Dotted => {
            let cross =
                (tx == 0 && [0, 1, last].contains(&ty)) || (ty == 0 && [1, last].contains(&tx));
            if cross {
                0.17
            } else if (tx == 0 && ty % 2 == 1) || (ty == 0 && tx % 2 == 1) {
                0.09
            } else {
                0.0
            }
        }
        BackdropStyle::Dither => {
            if tx == 0 || ty == 0 {
                return 0.1;
            }
            let light = 1.0 - (tx + ty - 2) as f32 / (2 * (TILE - 2)) as f32;
            if (BAYER[(ty % 4) as usize][(tx % 4) as usize] as f32) < light * light * 11.0 {
                0.045
            } else {
                0.0
            }
        }
        BackdropStyle::Grid => {
            if tx == 0 || ty == 0 {
                0.09
            } else {
                0.0
            }
        }
        BackdropStyle::Bits
        | BackdropStyle::Grass
        | BackdropStyle::Capture
        | BackdropStyle::Solid
        | BackdropStyle::Wallpaper => 0.0,
    }
}

/// A tuft of tall grass: blades leaning every way out of a dark root, as a route's grass is drawn.
const TUFT: [&str; 9] = [
    ".....h......h.....",
    "....hh.....hh..h..",
    "....h.h...h.h.hh..",
    "..h.h.h...h..hh.h.",
    "..hh...h.h...h..h.",
    ".h.h...hh...h...h.",
    ".h..h..h.h..h....h",
    "h...h.h...h.h....h",
    "dddddddddddddddddd",
];

/// Where the tufts stand in the grass's square: two, staggered.
const TUFTS: [(u32, u32); 2] = [(3, 6), (23, 26)];

/// The grass's and the capture balls' square, in art pixels.
const WIDE: u32 = 40;

fn grass(x: u32, y: u32) -> f32 {
    let (x, y) = (x % WIDE, y % WIDE);
    for (tx, ty) in TUFTS {
        let (Some(dx), Some(dy)) = (x.checked_sub(tx), y.checked_sub(ty)) else {
            continue;
        };
        let Some(row) = TUFT.get(dy as usize) else {
            continue;
        };
        match row.as_bytes().get(dx as usize) {
            Some(b'h') => return 0.11,
            Some(b'd') => return -0.45,
            _ => {}
        }
    }
    0.0
}

/// A capture ball, `BALL` art pixels across.
const BALL: f32 = 12.0;

/// The parts of a capture ball, as [`capture`] finds them.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Ball {
    /// Not the ball: the backdrop, or the shadow under it.
    Ground(f32),
    /// The dark outline, the band across its middle and the ring round its button.
    Line,
    /// Its red top.
    Top,
    /// The glint on its top.
    Glint,
    /// Its white foot.
    Foot,
    /// The button in the middle of the band.
    Button,
}

/// How much of a capture ball's own colours shows over the backdrop: enough to read as red and
/// white, little enough that a cart's label always stands out from it.
const BALL_SHOW: f32 = 0.42;
const BALL_WHITE: [u8; 3] = [0xec, 0xe8, 0xe0];
const BALL_LINE: [u8; 3] = [0x0a, 0x0a, 0x0c];

/// The backdrop's pixel at (`x`, `y`) of the Capture pattern, in art pixels, over `base`.
fn capture_px(base: [u8; 3], x: u32, y: u32) -> [u8; 4] {
    let over = |c: [u8; 3], k: f32| {
        let m = |b: u8, c: u8| (b as f32 + (c as f32 - b as f32) * k).round() as u8;
        [m(base[0], c[0]), m(base[1], c[1]), m(base[2], c[2]), 255]
    };
    let lit = |c: [u8; 3]| c.map(|v| v + ((255 - v) as f32 * 0.35).round() as u8);
    match capture(x, y) {
        Ball::Ground(lift) => tint(base, lift),
        Ball::Line => over(BALL_LINE, BALL_SHOW),
        Ball::Top => over(TRAINER_RED, BALL_SHOW),
        Ball::Glint => over(lit(TRAINER_RED), BALL_SHOW),
        Ball::Foot => over(BALL_WHITE, BALL_SHOW * 0.85),
        Ball::Button => over(BALL_WHITE, BALL_SHOW),
    }
}

/// Which part of a capture ball the pixel (`x`, `y`) of the pattern is: its outline, its red top
/// with a glint, the band across its middle and the ring round its button, the button, its white
/// foot, or the backdrop with a thin shadow under the ball. Every other row of balls stands half
/// a square over, so they do not line up into a grid.
fn capture(x: u32, y: u32) -> Ball {
    let row = y / WIDE;
    let x = (x + (row % 2) * WIDE / 2) % WIDE;
    let y = y % WIDE;
    let c = WIDE as f32 / 2.0 - 0.5;
    let (dx, dy) = (x as f32 - c, y as f32 - c);
    let d = dx.hypot(dy);
    let r = BALL / 2.0;
    if d > r + 0.5 {
        let (sx, sy) = (dx - 1.0, dy - 1.0);
        return Ball::Ground(if sx.hypot(sy) <= r + 0.5 && dy > 2.0 {
            -0.3
        } else {
            0.0
        });
    }
    if d > r - 0.6 {
        return Ball::Line;
    }
    if d <= 1.6 {
        return Ball::Button;
    }
    if d <= 2.7 || dy.abs() <= 0.6 {
        return Ball::Line;
    }
    if dy < 0.0 && (dx + 2.5).hypot(dy + 3.0) <= 0.8 {
        return Ball::Glint;
    }
    if dy < 0.0 {
        Ball::Top
    } else {
        Ball::Foot
    }
}

/// Whether the bit at (`cx`, `cy`) of a tile is lit, and how brightly: a scatter that looks
/// random but is the same every time.
fn bit(cx: u32, cy: u32) -> Option<f32> {
    let mut h = cx.wrapping_mul(0x9e37_79b1) ^ cy.wrapping_mul(0x85eb_ca77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    match h % 100 {
        0..=5 => Some(0.07),
        6..=8 => Some(0.14),
        _ => None,
    }
}

/// The picture a drifting pattern is drawn from: the canvas and one period more, so sliding it
/// left by up to a period always covers the canvas. Nothing for a style with no pattern.
pub fn backdrop_pattern_face(style: BackdropStyle, colour: u8) -> Option<UndoFace> {
    let period = backdrop_period(style);
    let (w, h) = (out_w() + period, out_h());
    let base = backdrop_rgb(colour);
    let ground = tint(base, 0.0);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let px = match style {
                BackdropStyle::Bits => {
                    let cells = period / BIT;
                    match bit((x / BIT) % cells, (y / BIT) % cells) {
                        Some(lift) => tint(base, lift),
                        None => ground,
                    }
                }
                BackdropStyle::Capture => capture_px(base, x / ART, y / ART),
                BackdropStyle::Grass => tint(base, grass(x / ART, y / ART)),
                BackdropStyle::Solid | BackdropStyle::Wallpaper => return None,
                tiled => tint(base, tile_lift(tiled, (x / ART) % TILE, (y / ART) % TILE)),
            };
            rgba.extend_from_slice(&px);
        }
    }
    Some(UndoFace { rgba, w, h })
}

/// The pattern `face` (made by [`backdrop_pattern_face`], `w` wide), drifted `shift` left.
pub fn draw_pattern(face: (TexId, u32), shift: u32, out: &mut Vec<Draw>) {
    out.push(Draw::Tex {
        x: -(shift as f32),
        y: 0.0,
        w: face.1 as f32,
        h: out_h() as f32,
        tex: face.0,
        alpha: 1.0,
    });
}

pub fn draw_solid(colour: u8, out: &mut Vec<Draw>) {
    let [r, g, b] = backdrop_rgb(colour);
    out.push(Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: out_w() as f32,
        h: out_h() as f32,
        colour: [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0],
    });
}
