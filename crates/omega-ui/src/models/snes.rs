//! The SNES cart, in its two shells: the American one, and the Super Famicom's, which Japan and
//! Europe shared.
//!
//! Both are drawn 4 units to the millimetre. The American cart is 136 by 88 mm, the Super
//! Famicom's 130 by 86, both 20 mm thick (Wikipedia's Game Pak infobox); the rest is measured off
//! straight-on photos of real carts. The American cart is boxy: a raised column down its middle,
//! its top standing a little proud of the ribbed panels either side, the label well open at the
//! top of the column and a deep well across its bottom. The Super Famicom's is a rounded oblong,
//! its sides rolled back into the cart, five grip slots high on each, the label wide across its
//! top with its corners round, and a ribbed slot across the cover under it. The ribs and slots
//! are relief in the face, not geometry: the sides take their colour from the face just inside
//! the outline, so a rib that reaches that far runs on round the side, as it does on the real
//! carts.

use std::sync::OnceLock;

use omega_store::{Cart, Outline, Region};

use crate::cart::{Board, Inside, Spec, PAD};
use crate::silhouette::{depth_map, rasterise_detail, rasterise_svg, Detail};

const NTSC_SVG: &str = include_str!("../../assets/snes_cart.svg");
const NTSC_DETAIL_SVG: &str = include_str!("../../assets/snes_cart_detail.svg");
const SFC_SVG: &str = include_str!("../../assets/sfc_cart.svg");
const SFC_DETAIL_SVG: &str = include_str!("../../assets/sfc_cart_detail.svg");

/// The SNES cart shells a model is built for.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum SnesShell {
    /// The American cart.
    Ntsc,
    /// The Super Famicom's, also Europe's.
    Famicom,
}

/// How thick the cart is, as a share of its width: 20 mm on the American cart's 136 and the
/// Super Famicom's 130.
pub const THICK: f32 = 0.15;

/// How big the cart stands on the shelf, as a share of its own size: its face is made at the
/// size it stands at, so its label lands a texel to a pixel there.
pub const SHELF_SCALE: f32 = 1.0;

/// How far down the American cart's shoulders start, in drawing units: its column stands this
/// much proud of them, 17% of the width in from each side. Those few rows come in further than
/// `OUTLINE_INSET`, so a face spun flat, with no model, leaves the step's side undrawn; the model
/// draws it as a wall like any other.
pub(crate) const NTSC_SHOULDER: f32 = 11.0;

/// The American cart's outline, right half, in drawing units (544 by 352), clockwise from the
/// top: the column's top corner cut in two, the step down to the shoulder, the shoulder's
/// corner and the foot's in two. The left half is its mirror.
const NTSC_HALF: &[(f32, f32)] = &[
    (448.0, 0.0),
    (450.83, 1.17),
    (452.0, 4.0),
    (452.0, NTSC_SHOULDER),
    (538.0, NTSC_SHOULDER),
    (542.24, 12.76),
    (544.0, 17.0),
    (544.0, 346.0),
    (542.24, 350.24),
    (538.0, 352.0),
];
const NTSC_UNITS: (f32, f32) = (544.0, 352.0);

/// The Super Famicom cart's, right half, in drawing units (520 by 344): its round top corner in
/// three cuts and its foot in two.
const SFC_HALF: &[(f32, f32)] = &[
    (506.0, 0.0),
    (513.0, 1.88),
    (518.12, 7.0),
    (520.0, 14.0),
    (520.0, 340.0),
    (518.83, 342.83),
    (516.0, 344.0),
];
const SFC_UNITS: (f32, f32) = (520.0, 344.0);

/// The size of each face in pixels: about 2.75 pixels to the millimetre, so the American cart is
/// a little the bigger of the two, as it is in the hand. Each is an even number of pixels across
/// its half, so it stands on whole art pixels in the middle of the row, and its height leaves it
/// on them between the bars.
pub const NTSC_W: u32 = 376;
pub const NTSC_H: u32 = 242;
pub const SFC_W: u32 = 360;
pub const SFC_H: u32 = 238;

/// The label on each face, in its pixels: the American cart's well runs from just under the top
/// of the column to two fifths of the way down, 83 mm by 36; the Super Famicom's is wider and
/// a little shorter, 101 mm by 33, across the top of the cart, 5 mm under its top edge as on a
/// scanned real cart.
const NTSC_LABEL: (u32, u32, u32, u32) = (74, 6, 228, 96);
const SFC_LABEL: (u32, u32, u32, u32) = (40, 14, 280, 92);

/// How round the Super Famicom label's corners are, in pixels: 2.4 mm, as a scanned real cart's
/// are. The American cart's label is square.
const SFC_LABEL_ROUND: u32 = 7;

/// How far in from the outline a clear shell's plastic lightens at its rim, in pixels.
const RIM: u32 = 6;

/// The shell `cart` comes in: the one `cart_shell.ini` names, else the American cart for a game
/// from the USA and the Super Famicom's for any other, or one from nowhere it can tell.
pub fn shell_of(cart: &Cart) -> SnesShell {
    match cart.shell.map(|c| c.outline) {
        Some(Outline::Ntsc) => SnesShell::Ntsc,
        Some(Outline::Sfc) => SnesShell::Famicom,
        _ => match omega_store::snes::region(&cart.rom, &cart.stem) {
            Some(Region::Usa) => SnesShell::Ntsc,
            _ => SnesShell::Famicom,
        },
    }
}

/// The cart's outline in drawing units, all the way round clockwise, and the drawing's size.
pub(crate) fn outline(shell: SnesShell) -> (Vec<(f32, f32)>, (f32, f32)) {
    let (half, units) = match shell {
        SnesShell::Ntsc => (NTSC_HALF, NTSC_UNITS),
        SnesShell::Famicom => (SFC_HALF, SFC_UNITS),
    };
    let mut all = half.to_vec();
    all.extend(half.iter().rev().map(|&(x, y)| (units.0 - x, y)));
    (all, units)
}

/// The cart's silhouette `w` by `h`, as coverage: opaque inside the outline.
pub fn silhouette(shell: SnesShell, w: u32, h: u32) -> Vec<u8> {
    let svg = match shell {
        SnesShell::Ntsc => NTSC_SVG,
        SnesShell::Famicom => SFC_SVG,
    };
    rasterise_svg(svg, w, h).unwrap_or_else(|| vec![255; (w * h) as usize])
}

/// The size of the cart's face in pixels.
fn face_box(shell: SnesShell) -> (u32, u32) {
    match shell {
        SnesShell::Ntsc => (NTSC_W, NTSC_H),
        SnesShell::Famicom => (SFC_W, SFC_H),
    }
}

fn mask(shell: SnesShell) -> &'static [u8] {
    static NTSC: OnceLock<Vec<u8>> = OnceLock::new();
    static SFC: OnceLock<Vec<u8>> = OnceLock::new();
    let lock = match shell {
        SnesShell::Ntsc => &NTSC,
        SnesShell::Famicom => &SFC,
    };
    lock.get_or_init(|| {
        let (w, h) = face_box(shell);
        silhouette(shell, w, h)
    })
}

fn depth(shell: SnesShell) -> &'static [u8] {
    static NTSC: OnceLock<Vec<u8>> = OnceLock::new();
    static SFC: OnceLock<Vec<u8>> = OnceLock::new();
    let lock = match shell {
        SnesShell::Ntsc => &NTSC,
        SnesShell::Famicom => &SFC,
    };
    lock.get_or_init(|| {
        let (w, h) = face_box(shell);
        depth_map(mask(shell), w as usize, h as usize)
    })
}

fn detail(shell: SnesShell) -> &'static Detail {
    static NTSC: OnceLock<Detail> = OnceLock::new();
    static SFC: OnceLock<Detail> = OnceLock::new();
    let (lock, svg) = match shell {
        SnesShell::Ntsc => (&NTSC, NTSC_DETAIL_SVG),
        SnesShell::Famicom => (&SFC, SFC_DETAIL_SVG),
    };
    lock.get_or_init(|| {
        let (w, h) = face_box(shell);
        rasterise_detail(svg, w, h).unwrap_or_else(|| Detail::blank(w, h))
    })
}

/// How the cart's face is laid out. A clear shell shows the board inside: wide and tall in the
/// shell, its contacts on a tab across the middle of its foot.
pub(crate) fn spec(shell: SnesShell) -> Spec {
    let (w, h) = face_box(shell);
    let (label, label_round) = match shell {
        SnesShell::Ntsc => (NTSC_LABEL, 0),
        SnesShell::Famicom => (SFC_LABEL, SFC_LABEL_ROUND),
    };
    Spec {
        w,
        h,
        label,
        label_round,
        label_cut: 0,
        mask: mask(shell),
        depth: depth(shell),
        detail: detail(shell),
        max_h: (label.3 - 2 * PAD) as f32,
        rim: RIM,
        inside: Inside::Board(Board {
            x: (0.1, 0.9),
            top: 0.1,
            pins: (0.27, 0.73),
            contacts_from: 0.86,
            traces_from: 0.78,
        }),
    }
}
