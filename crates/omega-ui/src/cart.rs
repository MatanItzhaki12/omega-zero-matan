use std::path::PathBuf;

use omega_store::gb::Class;
use omega_store::{Cart, Outline, Platform};

use crate::label_cut::label_art;
use crate::models::{disc, snes, snes::SnesShell, umd};
use crate::shell::{shell_for, Finish, Shell};
use crate::silhouette::{
    cart_depth, cart_mask, detail_mask, gb_cart_depth, gb_cart_mask, gb_detail_mask, Detail,
    GbShell,
};
use crate::spin::CartModel;
use crate::text;

pub const CART_W: u32 = 360;
pub const CART_H: u32 = (CART_W * SEATED_H + SEATED_W / 2) / SEATED_W;

pub const GB_CART_W: u32 = 312;

pub const GB_CART_H: u32 = (GB_CART_W * GB_SEATED_H + SEATED_W / 2) / SEATED_W;

pub const fn label_panel(w: u32, h: u32) -> (u32, u32, u32, u32) {
    (
        (w * 135 + 500) / 1000,
        (h * 231 + 500) / 1000,
        (w * 875 + 500) / 1000,
        (h * 882 + 500) / 1000,
    )
}

pub const LABEL_X: u32 = label_panel(CART_W, CART_H).0;
pub const LABEL_Y: u32 = label_panel(CART_W, CART_H).1;
pub const LABEL_W: u32 = label_panel(CART_W, CART_H).2 - LABEL_X;
pub const LABEL_H: u32 = label_panel(CART_W, CART_H).3 - LABEL_Y;

pub const fn gb_label_panel(w: u32, h: u32) -> (u32, u32, u32, u32) {
    (
        (w * 147 + 500) / 1000,
        (h * 291 + 500) / 1000,
        (w * 854 + 500) / 1000,
        (h * 868 + 500) / 1000,
    )
}

pub const GB_LABEL_X: u32 = gb_label_panel(GB_CART_W, GB_CART_H).0;
pub const GB_LABEL_Y: u32 = gb_label_panel(GB_CART_W, GB_CART_H).1;
pub const GB_LABEL_W: u32 = gb_label_panel(GB_CART_W, GB_CART_H).2 - GB_LABEL_X;
pub const GB_LABEL_H: u32 = gb_label_panel(GB_CART_W, GB_CART_H).3 - GB_LABEL_Y;

pub(crate) const PAD: u32 = 10;
const MAX_LINES: usize = 3;
/// A generated label's title is set in Pixel Code at one art pixel (two of the face's pixels) to
/// one of the face's, the size every word in the UI is set at, on up to three lines.
const MAX_PX: f32 = 18.0;
const MIN_PX: f32 = 18.0;

const RIM: u32 = (4 * CART_W + SEATED_W / 2) / SEATED_W;
const GB_RIM: u32 = (7 * GB_CART_W + SEATED_W / 2) / SEATED_W;

pub struct CartFace {
    pub rgba: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

/// How a model's face is laid out: its size in pixels, where its label goes, its outline and
/// relief, and the board a clear shell shows.
pub(crate) struct Spec {
    pub(crate) w: u32,
    pub(crate) h: u32,
    pub(crate) label: (u32, u32, u32, u32),
    /// How round the label's corners are, in pixels, its well's bevel round with them: none, a
    /// square label, but on the Super Famicom cart.
    pub(crate) label_round: u32,
    pub(crate) mask: &'static [u8],
    pub(crate) depth: &'static [u8],
    pub(crate) detail: &'static Detail,
    pub(crate) max_h: f32,
    pub(crate) rim: u32,
    pub(crate) inside: Inside,
}

/// What a clear shell shows inside it.
pub(crate) enum Inside {
    /// A cart's board and the parts on it.
    Board(Board),
    /// The UMD's disc.
    Disc,
    /// Nothing: a disc's face, which has no shell.
    Nothing,
}

pub(crate) struct Board {
    pub(crate) x: (f32, f32),
    pub(crate) top: f32,
    pub(crate) pins: (f32, f32),
    pub(crate) contacts_from: f32,
    pub(crate) traces_from: f32,
}

pub fn gb_shell_of(cart: &Cart) -> Option<GbShell> {
    match cart.platform {
        Platform::Gba | Platform::Snes | Platform::Ps1 | Platform::Psp => None,
        Platform::Gb | Platform::Gbc => Some(match cart.shell.map(|c| c.outline) {
            Some(Outline::Notched) => GbShell::Notched,
            Some(Outline::Rounded) => GbShell::Rounded,
            _ => match omega_store::gb::class(&cart.rom) {
                Class::ColourOnly => GbShell::Rounded,
                Class::Original | Class::DualMode => GbShell::Notched,
            },
        }),
    }
}

/// The model `cart` stands on the shelf as.
pub fn model_of(cart: &Cart) -> CartModel {
    match cart.platform {
        Platform::Gba => CartModel::Gba,
        Platform::Gb | Platform::Gbc => {
            CartModel::Gb(gb_shell_of(cart).unwrap_or(GbShell::Notched))
        }
        Platform::Snes => CartModel::Snes(snes::shell_of(cart)),
        Platform::Ps1 => CartModel::Disc,
        Platform::Psp => CartModel::Umd,
    }
}

/// The size of `model`'s face in pixels, the size it is drawn at in the handheld.
pub fn model_box(model: CartModel) -> (u32, u32) {
    let s = spec(model);
    (s.w, s.h)
}

/// `model`'s outline in black, to back a cart that is not drawn as a model.
pub fn model_shadow(model: CartModel) -> CartFace {
    let s = spec(model);
    shadow(s.w, s.h, s.mask)
}

pub(crate) fn spec(model: CartModel) -> Spec {
    match model {
        CartModel::Snes(shell) => snes::spec(shell),
        CartModel::Disc => disc::spec(),
        CartModel::Umd => umd::spec(),
        CartModel::Gba => Spec {
            w: CART_W,
            h: CART_H,
            label: (LABEL_X, LABEL_Y, LABEL_W, LABEL_H),
            label_round: 0,
            mask: cart_mask(),
            depth: cart_depth(),
            detail: detail_mask(),
            max_h: (LABEL_H - 2 * PAD) as f32,
            rim: RIM,
            inside: Inside::Board(Board {
                // wider than the label, so the board's edge stands clear of the label's well
                x: (0.07, 0.93),
                // the board's top edge shows through over the label, as on a clear pak
                top: 0.12,
                pins: (0.17, 0.84),
                contacts_from: 0.85,
                traces_from: 0.76,
            }),
        },
        CartModel::Gb(shell) => Spec {
            w: GB_CART_W,
            h: GB_CART_H,
            label: (GB_LABEL_X, GB_LABEL_Y, GB_LABEL_W, GB_LABEL_H),
            label_round: 0,
            mask: gb_cart_mask(shell),
            depth: gb_cart_depth(shell),
            detail: gb_detail_mask(shell),
            max_h: (GB_LABEL_H - 2 * PAD) as f32,
            rim: GB_RIM,
            inside: Inside::Board(Board {
                x: (0.075, 0.925),
                top: 0.07,
                pins: (0.1, 0.9),
                contacts_from: 0.905,
                traces_from: 0.84,
            }),
        },
    }
}

pub const SEATED_W: u32 = 240;
const SEATED_H: u32 = 141;
const GB_SEATED_H: u32 = 259;

pub fn seated_box(platform: Platform) -> (u32, u32) {
    match platform {
        Platform::Gba => (SEATED_W, SEATED_H),
        Platform::Gb | Platform::Gbc => (SEATED_W, GB_SEATED_H),
        Platform::Snes => {
            let (_, (w, h)) = snes::outline(SnesShell::Famicom);
            (w.round() as u32, h.round() as u32)
        }
        Platform::Ps1 | Platform::Psp => {
            let model = match platform {
                Platform::Ps1 => CartModel::Disc,
                _ => CartModel::Umd,
            };
            let (_, (w, h)) = crate::spin::outline(model);
            (w.round() as u32, h.round() as u32)
        }
    }
}

/// The size of a face for `platform` in pixels: its first model's, where it has several.
pub fn cart_box(platform: Platform) -> (u32, u32) {
    model_box(match platform {
        Platform::Gba => CartModel::Gba,
        Platform::Gb | Platform::Gbc => CartModel::Gb(GbShell::Notched),
        Platform::Snes => CartModel::Snes(SnesShell::Famicom),
        Platform::Ps1 => CartModel::Disc,
        Platform::Psp => CartModel::Umd,
    })
}

pub fn cart_shadow() -> CartFace {
    shadow(CART_W, CART_H, cart_mask())
}

pub fn gb_cart_shadow(shell: GbShell) -> CartFace {
    shadow(GB_CART_W, GB_CART_H, gb_cart_mask(shell))
}

fn shadow(w: u32, h: u32, mask: &[u8]) -> CartFace {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for cover in mask {
        rgba.extend_from_slice(&[0, 0, 0, *cover]);
    }
    CartFace { rgba, w, h }
}

/// The label scans `cart`'s face is drawn with, each with the size it is drawn at there.
pub(crate) fn label_scans(cart: &Cart) -> Vec<(PathBuf, u32, u32)> {
    let model = model_of(cart);
    if model == CartModel::Disc {
        return disc::label_scans(cart);
    }
    let (_, _, w, h) = spec(model).label;
    cart.label.iter().map(|p| (p.clone(), w, h)).collect()
}

/// The sizes `platform`'s label scans are drawn at, one for each of its models: a Game Boy cart
/// can wear either outline and a SNES cart either shell.
pub fn face_labels(platform: Platform) -> Vec<(u32, u32)> {
    CartModel::ALL
        .into_iter()
        .filter(|model| match model {
            CartModel::Gba => platform == Platform::Gba,
            CartModel::Gb(_) => matches!(platform, Platform::Gb | Platform::Gbc),
            CartModel::Snes(_) => platform == Platform::Snes,
            CartModel::Disc => platform == Platform::Ps1,
            CartModel::Umd => platform == Platform::Psp,
        })
        .map(|model| match model {
            CartModel::Disc => (disc::DISC_PX, disc::DISC_PX),
            _ => {
                let (_, _, w, h) = spec(model).label;
                (w, h)
            }
        })
        .collect()
}

/// `cart`'s face, its label drawn from the cut kept for its scan, or from the scan itself when
/// there is no cut of it yet.
pub fn cart_face(cart: &Cart) -> CartFace {
    face_of(cart, true)
}

/// `cart`'s face in a generated label whatever scan it has: what a boot draws until the scan
/// is cut.
pub fn generated_face(cart: &Cart) -> CartFace {
    face_of(cart, false)
}

fn face_of(cart: &Cart, scans: bool) -> CartFace {
    let model = model_of(cart);
    if model == CartModel::Disc {
        if let Some(face) = disc::face(cart, scans) {
            return face;
        }
    }
    let s = spec(model);
    let shell = shell_for(cart);
    let mut face = shell_face(&s, &shell);
    let (_, _, lw, lh) = s.label;
    let scan = cart.label.as_deref().filter(|_| scans);
    let label = match scan.and_then(|p| label_art(p, lw, lh)) {
        Some(rgba) if model == CartModel::Umd => umd::with_hub(rgba, lw, lh),
        Some(rgba) => rgba,
        None if model == CartModel::Umd => umd::generated_label(&s, &label_text(cart)),
        None => generated_label(&s, &label_text(cart)),
    };
    mould_detail(&s, &mut face);
    recess_label(&s, &mut face, &shell);
    paste_label(&s, &mut face, &label);
    clip_to_silhouette(&s, &mut face);
    face
}

/// A UMD's disc on its own, its print round and its hub in the middle, the size its window
/// shows it: what spins in the window while the game loads. None for a cart that is no UMD.
pub fn umd_disc(cart: &Cart) -> Option<CartFace> {
    if model_of(cart) != CartModel::Umd {
        return None;
    }
    let s = spec(CartModel::Umd);
    let (_, _, lw, lh) = s.label;
    let mut rgba = match cart.label.as_deref().and_then(|p| label_art(p, lw, lh)) {
        Some(scan) => umd::with_hub(scan, lw, lh),
        None => umd::generated_label(&s, &label_text(cart)),
    };
    // round, as the disc is: clear outside its edge
    let r = lw.min(lh) as f32 / 2.0;
    for y in 0..lh {
        for x in 0..lw {
            let d = (x as f32 + 0.5 - lw as f32 / 2.0).hypot(y as f32 + 0.5 - lh as f32 / 2.0);
            let cover = (r - d).clamp(0.0, 1.0);
            let i = ((y * lw + x) * 4 + 3) as usize;
            rgba[i] = (rgba[i] as f32 * cover) as u8;
        }
    }
    Some(CartFace { rgba, w: lw, h: lh })
}

/// Where a UMD's disc shows in its face, as shares of the face: x, y, width and height.
pub fn umd_window() -> (f32, f32, f32, f32) {
    let s = spec(CartModel::Umd);
    let (lx, ly, lw, lh) = s.label;
    let (w, h) = (s.w as f32, s.h as f32);
    (lx as f32 / w, ly as f32 / h, lw as f32 / w, lh as f32 / h)
}

fn clip_to_silhouette(s: &Spec, face: &mut CartFace) {
    for (px, cover) in face.rgba.chunks_exact_mut(4).zip(s.mask) {
        px[3] = ((px[3] as u32 * *cover as u32 + 127) / 255) as u8;
    }
}

pub fn label_colour(title: &str) -> [u8; 3] {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in title.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hsv_to_rgb((h % 360) as f32, 0.52, 0.74)
}

pub fn label_text(cart: &Cart) -> String {
    clean_label(&cart.stem)
}

pub fn clean_label(stem: &str) -> String {
    let stem = without_set_number(stem);
    let mut bare = String::with_capacity(stem.len());
    let mut depth = 0u32;
    for ch in stem.chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => bare.push(ch),
            _ => {}
        }
    }

    let mut out = String::with_capacity(bare.len());
    for word in bare.split_whitespace().filter(|w| *w != "-") {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.is_empty() {
        stem.to_string()
    } else {
        out
    }
}

/// `stem` without the number a numbered set puts before each game, `0017 - Need for Speed`'s
/// `0017 - `: four digits and a spaced dash, with a name after them.
fn without_set_number(stem: &str) -> &str {
    match stem.split_once(" - ") {
        Some((n, rest))
            if n.len() == 4 && n.bytes().all(|b| b.is_ascii_digit()) && !rest.is_empty() =>
        {
            rest
        }
        _ => stem,
    }
}

pub fn label_tags(stem: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0u32;
    let mut cur = String::new();
    for ch in stem.chars() {
        match ch {
            '(' | '[' => {
                depth += 1;
                if depth == 1 {
                    cur.clear();
                    continue;
                }
            }
            ')' | ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let t = cur.trim();
                    if !t.is_empty() {
                        out.push(t.to_string());
                    }
                    continue;
                }
            }
            _ => {}
        }
        if depth >= 1 {
            cur.push(ch);
        }
    }
    out
}

fn shell_face(s: &Spec, shell: &Shell) -> CartFace {
    let mut rgba = Vec::with_capacity((s.w * s.h * 4) as usize);
    let edge = rim_colour(through(shell.colour, shell.colour));
    for (i, depth) in s.depth.iter().enumerate() {
        let (x, y) = (i as u32 % s.w, i as u32 / s.w);
        let rim = (*depth as u32).min(s.rim);
        let (c, a) = match shell.finish {
            Finish::Solid => (shell.colour, 255),
            Finish::Translucent | Finish::Glitter => {
                let behind = inside(s, x, y);
                let body = match behind {
                    // the bare board takes the plastic's colour, so a clear shell stays as rich
                    // as a solid one
                    Some(BOARD | BOARD_EDGE | TRACE) | None => {
                        through(shell.colour, behind.unwrap_or(HOLLOW))
                    }
                    // a part on it keeps some of its own colour through the plastic: the gold
                    // of the contacts and the black of the chips read as such, not as more green
                    Some(part) => lerp(through(shell.colour, part), part, PART_SHOWS, 255),
                };
                let c = lerp(edge, body, rim, s.rim);
                // where no board stands behind it, the light comes through the plastic: the
                // shelf shows through it, less so towards the rim, where it is thickest
                let a = match behind {
                    Some(_) => 255,
                    None => 255 - (255 - CLEAR_ALPHA) * rim / s.rim,
                };
                if shell.finish == Finish::Glitter && fleck(x, y) {
                    (lerp(c, [0xff; 3], 100, 255), 255)
                } else {
                    (c, a)
                }
            }
        };
        rgba.extend_from_slice(&[c[0], c[1], c[2], a as u8]);
    }
    CartFace {
        rgba,
        w: s.w,
        h: s.h,
    }
}

/// How much of a clear shell's colour is its own surface, of 255; the rest is what shows through
/// it, tinted: low, so the board and the hollow inside read through the plastic.
const SURFACE: u32 = 40;

/// How much of a part's own colour shows through a clear shell, of 255.
const PART_SHOWS: u32 = 80;
/// How opaque a clear shell is, of 255, where no board stands behind it, inside its rim: the
/// shelf shows through the rest.
const CLEAR_ALPHA: u32 = 175;

/// What a clear shell shows where no board stands behind it: the cart's hollow, a pale grey the
/// plastic tints, so it reads as the plastic itself, lit, and its colour stays as rich as a
/// solid shell's.
const HOLLOW: [u8; 3] = [0xd2, 0xd4, 0xd8];

const BOARD: [u8; 3] = [0x62, 0xb6, 0x78];
const BOARD_EDGE: [u8; 3] = [0x3c, 0x88, 0x52];
const TRACE: [u8; 3] = [0x8c, 0xd2, 0x9c];
const GOLD: [u8; 3] = [0xe6, 0xb4, 0x46];
const CHIP: [u8; 3] = [0x40, 0x42, 0x48];
const CHIP_MARK: [u8; 3] = [0x5c, 0x5e, 0x64];
const PIN: [u8; 3] = [0xc8, 0xca, 0xcc];
const SILK: [u8; 3] = [0xe4, 0xe6, 0xdc];
const CELL_FACE: [u8; 3] = [0xc4, 0xc8, 0xcc];
const CELL_RIM: [u8; 3] = [0x3c, 0x6c, 0xc4];
const CELL_TAB: [u8; 3] = [0x9c, 0xa0, 0xa6];
const PART: [u8; 3] = [0x9a, 0x76, 0x4e];

/// A chip on the board: its corners as fractions of the board across and down from its top
/// left, and whether its legs stand down its left and right sides, as a TSOP's do, or along
/// its top and bottom.
struct Chip {
    at: (f32, f32, f32, f32),
    legs_on_sides: bool,
}

/// The parts of a clear cart's board, after the AGB-E05 board Pokémon Emerald is built on: the
/// flash on the left, the clock's chip and its crystal along the top, the ROM on the right under
/// the coin cell, the maker's lettering above the contacts. The label stands over most of it;
/// what shows past its edges is what a clear cart shows.
const CHIPS: [Chip; 3] = [
    // the flash save
    Chip {
        at: (0.12, 0.40, 0.30, 0.62),
        legs_on_sides: true,
    },
    // the real-time clock
    Chip {
        at: (0.46, 0.56, 0.075, 0.19),
        legs_on_sides: false,
    },
    // the ROM, mostly under the cell
    Chip {
        at: (0.60, 0.88, 0.08, 0.52),
        legs_on_sides: true,
    },
];

/// The clock's crystal, a silver can: its corners as fractions of the board.
const CRYSTAL: (f32, f32, f32, f32) = (0.16, 0.28, 0.08, 0.15);

/// The coin cell: its centre as fractions of the board, and its radius as a fraction of its
/// width.
const CELL: (f32, f32, f32) = (0.74, 0.33, 0.15);

/// The small parts, resistors and capacitors: each one's top left as fractions of the board.
const PARTS: [(f32, f32); 7] = [
    (0.32, 0.18),
    (0.48, 0.24),
    (0.50, 0.58),
    (0.44, 0.66),
    (0.62, 0.62),
    (0.80, 0.58),
    (0.22, 0.66),
];

/// Where the maker's lettering runs, as fractions of the board: across, and down.
const LETTERING: (f32, f32, f32, f32) = (0.14, 0.72, 0.74, 0.78);

/// How far in from each side of the board, as a fraction of its width, nothing stands on it.
const MARGIN: f32 = 0.1;

fn through(plastic: [u8; 3], behind: [u8; 3]) -> [u8; 3] {
    let filter =
        |a: [u8; 3], b: [u8; 3]| [0, 1, 2].map(|c| (a[c] as u32 * b[c] as u32 / 255) as u8);
    lerp(filter(plastic, behind), plastic, SURFACE, 255)
}

/// What stands behind the face's pixel `x, y` inside a clear shell: the board and its parts, or
/// nothing, the shell's hollow.
fn inside(s: &Spec, x: u32, y: u32) -> Option<[u8; 3]> {
    let b = match &s.inside {
        Inside::Board(b) => b,
        Inside::Disc => return umd::behind(x, y),
        Inside::Nothing => return None,
    };
    let (fx, fy) = (x as f32 / s.w as f32, y as f32 / s.h as f32);
    if !(b.x.0..b.x.1).contains(&fx) || fy < b.top {
        return None;
    }
    let (c0, c1) = b.pins;
    let pitch = (c1 - c0) / 32.0;
    let on_contact = (c0..c1).contains(&fx) && ((fx - c0) % pitch) < pitch * 0.6;
    if on_contact && fy >= b.contacts_from {
        return Some(GOLD);
    }
    if on_contact && ((fx - c0) % pitch) < pitch * 0.2 && fy >= b.traces_from {
        return Some(TRACE);
    }

    // the board in pixels, from its top left corner, on the art's two-pixel grid
    let (bx0, by0) = (b.x.0 * s.w as f32, b.top * s.h as f32);
    let (bw, bh) = ((b.x.1 - b.x.0) * s.w as f32, s.h as f32 - by0);
    let (px, py) = ((x & !1) as f32 - bx0, (y & !1) as f32 - by0);
    if px < 2.0 || bw - px < 3.0 || py < 2.0 {
        return Some(BOARD_EDGE);
    }
    // the board's sides stand plain under the shell's shoulders, whose ribs read alone
    if px < MARGIN * bw || bw - px < MARGIN * bw {
        return Some(BOARD);
    }
    let (u, v) = (px / bw, py / bh);
    let within =
        |(u0, u1, v0, v1): (f32, f32, f32, f32)| (u0..u1).contains(&u) && (v0..v1).contains(&v);

    let (cx, cy, r) = (CELL.0 * bw, CELL.1 * bh, CELL.2 * bw);
    let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
    if d <= r {
        // the cell's tab, soldered across it to the right
        let tab = (py - cy).abs() < r * 0.18 && px > cx - r * 0.2;
        return Some(match (d > r - 4.0, tab) {
            (true, _) => CELL_RIM,
            (false, true) => CELL_TAB,
            (false, false) => CELL_FACE,
        });
    }

    for chip in &CHIPS {
        let (u0, u1, v0, v1) = chip.at;
        let (x0, x1, y0, y1) = (u0 * bw, u1 * bw, v0 * bh, v1 * bh);
        if (x0..x1).contains(&px) && (y0..y1).contains(&py) {
            // the dimple by pin one
            let (dx, dy) = (px - x0 - 6.0, py - y0 - 6.0);
            return Some(if dx * dx + dy * dy <= 6.0 {
                CHIP_MARK
            } else {
                CHIP
            });
        }
        let reach = 4.0;
        let legs = match chip.legs_on_sides {
            true => {
                let beside = (x0 - reach..x0).contains(&px) || (x1..x1 + reach).contains(&px);
                beside
                    && (y0 + 2.0..y1 - 2.0).contains(&py)
                    && ((py - y0) as u32 / 2).is_multiple_of(2)
            }
            false => {
                let over = (y0 - reach..y0).contains(&py) || (y1..y1 + reach).contains(&py);
                over && (x0 + 2.0..x1 - 2.0).contains(&px)
                    && ((px - x0) as u32 / 2).is_multiple_of(2)
            }
        };
        if legs {
            return Some(PIN);
        }
    }

    if within(CRYSTAL) {
        let (u0, u1, v0, v1) = CRYSTAL;
        let edge = (u - u0) * bw < 2.0
            || (u1 - u) * bw < 2.0
            || (v - v0) * bh < 2.0
            || (v1 - v) * bh < 2.0;
        return Some(if edge { CELL_TAB } else { PIN });
    }

    for &(pu, pv) in &PARTS {
        let (x0, y0) = (pu * bw, pv * bh);
        if (x0..x0 + 8.0).contains(&px) && (y0..y0 + 4.0).contains(&py) {
            let end = px < x0 + 2.0 || px >= x0 + 6.0;
            return Some(if end { PIN } else { PART });
        }
    }

    // the maker's lettering: short strokes in rows, as small print reads from afar
    if within(LETTERING) {
        let (col, row) = ((px as u32 / 2) % 4, (py as u32 / 2) % 3);
        let word = (px as u32 / 14) % 5 != 4;
        if word && col != 3 && row != 2 {
            return Some(SILK);
        }
    }

    // traces: a bus down from the chips to the contacts, and a run across under the cell
    let bus = (px as u32 / 2).is_multiple_of(6) && v > 0.64 && v < LETTERING.2 - 0.02;
    let run = (py as u32 / 2).is_multiple_of(5) && v > 0.54 && v < 0.62 && u > 0.44;
    if bus || run {
        Some(TRACE)
    } else {
        Some(BOARD)
    }
}

fn fleck(x: u32, y: u32) -> bool {
    let mut h = x.wrapping_mul(0x9e37_79b9) ^ y.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h.is_multiple_of(100)
}

fn rim_colour(base: [u8; 3]) -> [u8; 3] {
    let mean = (base[0] as u16 + base[1] as u16 + base[2] as u16) / 3;
    base.map(|c| {
        let grey = (3 * c as u16 + mean) / 4;
        (grey + (255 - grey) * 2 / 5) as u8
    })
}

fn lerp(a: [u8; 3], b: [u8; 3], num: u32, den: u32) -> [u8; 3] {
    let mut out = [0u8; 3];
    for c in 0..3 {
        out[c] = ((a[c] as u32 * (den - num) + b[c] as u32 * num) / den) as u8;
    }
    out
}

const BEVEL: u32 = 3;

fn mould_detail(s: &Spec, face: &mut CartFace) {
    let mix = |px: &mut [u8], to: [u8; 3], a: u32| {
        for c in 0..3 {
            px[c] = ((to[c] as u32 * a + px[c] as u32 * (255 - a) + 127) / 255) as u8;
        }
    };
    let sides = s.detail.shadow.iter().zip(&s.detail.highlight);
    for (px, (shade, light)) in face.rgba.chunks_exact_mut(4).zip(sides) {
        let under = [px[0], px[1], px[2]];
        if *shade > 0 {
            mix(px, under.map(|c| (c as f32 * 0.62) as u8), *shade as u32);
        }
        if *light > 0 {
            mix(
                px,
                under.map(|c| c + ((255 - c) as f32 * 0.24) as u8),
                *light as u32,
            );
        }
    }
}

fn recess_label(s: &Spec, face: &mut CartFace, shell: &Shell) {
    let shade = |c: [u8; 3], f: f32| -> [u8; 3] {
        [
            (c[0] as f32 * f).clamp(0.0, 255.0) as u8,
            (c[1] as f32 * f).clamp(0.0, 255.0) as u8,
            (c[2] as f32 * f).clamp(0.0, 255.0) as u8,
        ]
    };
    let dark = shade(shell.colour, 0.55);
    let lit = shade(shell.colour, 1.45);

    let (lx, ly, lw, lh) = s.label;
    let (w, h) = (s.w, s.h);
    let bevel = (BEVEL * w + SEATED_W / 2) / SEATED_W;
    let (x0, y0) = (lx - bevel, ly - bevel);
    let (x1, y1) = (lx + lw + bevel, ly + lh + bevel);
    let clear = shell.finish != Finish::Solid;
    let mut put = |x: u32, y: u32, upper: bool| {
        if x >= w || y >= h {
            return;
        }
        let d = ((y * w + x) * 4) as usize;
        let c = if clear {
            let under = [face.rgba[d], face.rgba[d + 1], face.rgba[d + 2]];
            shade(under, if upper { 0.7 } else { 1.18 })
        } else if upper {
            dark
        } else {
            lit
        };
        face.rgba[d] = c[0];
        face.rgba[d + 1] = c[1];
        face.rgba[d + 2] = c[2];
    };
    let round = s.label_round;
    for y in y0..y1 {
        for x in x0..x1 {
            let inside = match round {
                0 => (lx..lx + lw).contains(&x) && (ly..ly + lh).contains(&y),
                _ => in_round_rect(x, y, s.label, round),
            };
            if inside {
                continue;
            }
            // a round label's well is round with it, its corners as far out as its sides
            if round > 0 && !in_round_rect(x, y, (x0, y0, x1 - x0, y1 - y0), round + bevel) {
                continue;
            }
            let from_top = y.saturating_sub(y0);
            let from_left = x.saturating_sub(x0);
            let from_bottom = y1.saturating_sub(y + 1);
            let from_right = x1.saturating_sub(x + 1);
            let upper = from_top.min(from_left);
            let lower = from_bottom.min(from_right);
            put(x, y, upper <= lower);
        }
    }
}

/// Whether the face's pixel `x, y` is inside the rect `rx, ry, rw, rh` with its corners
/// rounded `r` pixels: decided for each two-pixel cell of the face as a whole, so a round
/// corner steps on the art grid as everything else does.
fn in_round_rect(x: u32, y: u32, (rx, ry, rw, rh): (u32, u32, u32, u32), r: u32) -> bool {
    if !(rx..rx + rw).contains(&x) || !(ry..ry + rh).contains(&y) {
        return false;
    }
    let (cx, cy) = ((x & !1) as f32 + 1.0, (y & !1) as f32 + 1.0);
    let r = r as f32;
    let dx = (rx as f32 + r - cx).max(cx - (rx + rw) as f32 + r).max(0.0);
    let dy = (ry as f32 + r - cy).max(cy - (ry + rh) as f32 + r).max(0.0);
    dx * dx + dy * dy <= r * r
}

fn paste_label(s: &Spec, face: &mut CartFace, label: &[u8]) {
    let (lx, ly, lw, lh) = s.label;
    for y in 0..lh {
        for x in 0..lw {
            let src = ((y * lw + x) * 4) as usize;
            let a = label[src + 3] as u32;
            if a == 0 {
                continue;
            }
            if s.label_round > 0 && !in_round_rect(x + lx, y + ly, s.label, s.label_round) {
                continue;
            }
            let d = (((y + ly) * s.w + x + lx) * 4) as usize;
            for c in 0..3 {
                face.rgba[d + c] =
                    ((label[src + c] as u32 * a + face.rgba[d + c] as u32 * (255 - a) + 127) / 255)
                        as u8;
            }
            // the label is paper: what is behind a clear shell does not show through it
            let under = face.rgba[d + 3] as u32;
            face.rgba[d + 3] = (a + under * (255 - a) / 255) as u8;
        }
    }
}

/// How many frames a model's holo sheen has: the light at as many places along the label.
pub const HOLO_FRAMES: usize = 12;

/// How a label's holo sheen is drawn, as on a holographic Pokémon card: a faint rainbow over the
/// whole label, its colours running across it on the diagonal, and a brighter band of light
/// where the light catches it. Each frame has the light a little further along; the shelf
/// draws the one its cart's sway faces, so the light slides over the label as the cart turns.
/// Alphas of 255.
const HOLO_BASE: f32 = 8.0;
const HOLO_BAND: f32 = 64.0;

/// The holo sheen over a `model` cart's label, `HOLO_FRAMES` frames of it, each the face's size
/// at half its resolution, to be drawn over the face scaled up: clear but over the label. A
/// UMD's label is its disc's, round, and its rainbow runs out from its middle in rings, the way
/// a CD throws the light. A disc has none.
pub fn holo_frames(model: CartModel) -> Vec<CartFace> {
    if model == CartModel::Disc {
        return Vec::new();
    }
    let s = spec(model);
    let (lx, ly, lw, lh) = s.label;
    let (w, h) = (s.w.div_ceil(2), s.h.div_ceil(2));
    (0..HOLO_FRAMES)
        .map(|k| {
            let t = k as f32 / (HOLO_FRAMES - 1) as f32;
            let mut rgba = vec![0u8; (w * h * 4) as usize];
            for hy in 0..h {
                for hx in 0..w {
                    let (x, y) = (hx * 2 + 1, hy * 2 + 1);
                    let inside = match s.label_round {
                        0 => (lx..lx + lw).contains(&x) && (ly..ly + lh).contains(&y),
                        r => in_round_rect(x, y, s.label, r),
                    };
                    if !inside {
                        continue;
                    }
                    let (u, v) = ((x - lx) as f32 / lw as f32, (y - ly) as f32 / lh as f32);
                    let along = u * 0.75 + v * 0.45;
                    // the light's band, from past the top left to past the bottom right
                    let light = -0.15 + 1.35 * t;
                    let band = (-((along - light) / 0.16).powi(2)).exp();
                    let runs = match model {
                        CartModel::Umd => ((u - 0.5).hypot(v - 0.5) * 2.0) * 1.6,
                        _ => along * 1.4,
                    };
                    let hue = (runs - t * 0.6).rem_euclid(1.0) * 360.0;
                    let d = ((hy * w + hx) * 4) as usize;
                    let c = hsv_to_rgb(hue, 0.6, 1.0);
                    let a = (HOLO_BASE + HOLO_BAND * band) as u8;
                    rgba[d..d + 4].copy_from_slice(&[c[0], c[1], c[2], a]);
                }
            }
            CartFace { rgba, w, h }
        })
        .collect()
}

fn generated_label(s: &Spec, title: &str) -> Vec<u8> {
    let (_, _, lw, lh) = s.label;
    let bg = label_colour(title);
    let mut rgba = Vec::with_capacity((lw * lh * 4) as usize);
    for _ in 0..lw * lh {
        rgba.extend_from_slice(&[bg[0], bg[1], bg[2], 255]);
    }

    if let Some(font) = text::label_font() {
        let layout = text::fit_box(
            font,
            title,
            (lw - 2 * PAD) as f32,
            s.max_h,
            MAX_LINES,
            MAX_PX,
            MIN_PX,
        );
        text::draw_centred(&mut rgba, lw, lh, &layout, ink(bg));
    }
    rgba
}

pub(crate) fn ink(bg: [u8; 3]) -> [u8; 3] {
    let luma = 0.2126 * bg[0] as f32 + 0.7152 * bg[1] as f32 + 0.0722 * bg[2] as f32;
    if luma > 140.0 {
        [0x1a, 0x18, 0x16]
    } else {
        [0xf4, 0xf1, 0xea]
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    ]
}
