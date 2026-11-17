//! The DS Game Card, as it is made: a small card of dark grey plastic, 33 mm wide and 35 tall,
//! its corners a little round; a paper label filling almost all of its front, set in a shallow
//! well whose lower left corner is cut off at 45 degrees; under the label a band of plastic
//! with an arrow moulded in its middle, pointing the way the card goes in; and the card's sides
//! stepped in a little along that band, where the slot's rails take it.
//!
//! The card is drawn 10 units to the millimetre, as a straight-on photo of a real one measures,
//! and its face about 7.15 pixels to the millimetre.

use std::sync::OnceLock;

use crate::cart::{label_colour, Board, Inside, Spec, PAD};
use crate::silhouette::{depth_map, rasterise_detail, rasterise_svg, Detail};
use crate::text;

/// How thick the card is, as a share of its width: 3.8 mm on 33.
pub const THICK: f32 = 0.115;

/// How big the card stands on the shelf, as a share of its own size.
pub const SHELF_SCALE: f32 = 1.0;

/// The drawing's size in units.
const UNITS: (f32, f32) = (330.0, 350.0);

/// How round the card's corners are, in units.
const CORNER_R: f32 = 10.0;
/// Where the band under the label starts, in units down, and how far the sides step in along
/// it.
const BAND_Y: f32 = 326.0;
const STEP: f32 = 4.0;

/// The cuts each of the card's corners is made of.
const CORNER_CUTS: usize = 3;

/// The face in pixels: each an even number, so its halves stand on whole art pixels.
pub const DS_W: u32 = 236;
pub const DS_H: u32 = 250;

/// Units to the face's pixels.
const PX: f32 = DS_W as f32 / UNITS.0;

/// The label, in the face's pixels: the paper in its well, a little more plastic round it on
/// the right and under it than on the left and over it, as on the card.
const LABEL: (u32, u32, u32, u32) = (18, 14, 198, 212);

/// How far the label's lower left corner is cut back, in the face's pixels, the well's with it.
pub const LABEL_CUT: u32 = 16;

/// How far in from the outline a clear shell lightens at its rim, in pixels.
const RIM: u32 = 4;

/// The band of white across the top of a generated label, as a card's label has above its art,
/// as a share of the label's height.
const LABEL_BAND: f32 = 0.13;
const LABEL_WHITE: [u8; 3] = [0xf4, 0xf3, 0xef];

/// The label a card with no scan wears, `s.label`'s size: the game's colour under a white band,
/// as a card's label has its art under the band the system's name is printed on, and the title
/// in the middle of the colour.
pub(crate) fn generated_label(s: &Spec, title: &str) -> Vec<u8> {
    let (_, _, lw, lh) = s.label;
    let bg = label_colour(title);
    let band = (lh as f32 * LABEL_BAND).round() as u32;
    let mut rgba = Vec::with_capacity((lw * lh * 4) as usize);
    for y in 0..lh {
        let c = if y < band { LABEL_WHITE } else { bg };
        for _ in 0..lw {
            rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    if let Some(font) = text::label_font() {
        let (bw, bh) = (lw - 2 * PAD, lh - band);
        let mut part = vec![0u8; (bw * bh * 4) as usize];
        let layout = text::fit_box(font, title, bw as f32, s.max_h, 3, 18.0, 18.0);
        text::draw_centred(&mut part, bw, bh, &layout, crate::cart::ink(bg));
        for (i, px) in part.chunks_exact(4).enumerate() {
            let a = px[3] as u32;
            let (x, y) = (PAD + i as u32 % bw, band + i as u32 / bw);
            let d = ((y * lw + x) * 4) as usize;
            for c in 0..3 {
                rgba[d + c] = ((px[c] as u32 * a + rgba[d + c] as u32 * (255 - a)) / 255) as u8;
            }
        }
    }
    rgba
}

/// One rounded corner of the outline, `CORNER_CUTS` cuts from `from` to `to` radians about
/// `centre`: the corners between its ends stand a little outside the arc and the cuts between
/// them a little inside, so the walls follow the edge the face's alpha cuts.
fn corner(out: &mut Vec<(f32, f32)>, centre: (f32, f32), (from, to): (f32, f32)) {
    let cuts = CORNER_CUTS;
    let out_by = f32::midpoint(1.0, 1.0 / ((to - from) / cuts as f32 / 2.0).cos());
    for i in 0..=cuts {
        let a = from + (to - from) * i as f32 / cuts as f32;
        let r = if i == 0 || i == cuts {
            CORNER_R
        } else {
            CORNER_R * out_by
        };
        out.push((centre.0 + r * a.cos(), centre.1 + r * a.sin()));
    }
}

/// The card's outline in drawing units, all the way round clockwise from its top left corner,
/// and the drawing's size.
pub(crate) fn outline() -> (Vec<(f32, f32)>, (f32, f32)) {
    use std::f32::consts::PI;
    let (w, h) = UNITS;
    let r = CORNER_R;
    let mut all = Vec::new();
    corner(&mut all, (r, r), (PI, 1.5 * PI));
    corner(&mut all, (w - r, r), (1.5 * PI, 2.0 * PI));
    // down the right side to the band, in by the step, and down along it
    all.push((w, BAND_Y - STEP));
    all.push((w - STEP, BAND_Y));
    corner(&mut all, (w - STEP - r, h - r), (0.0, 0.5 * PI));
    corner(&mut all, (STEP + r, h - r), (0.5 * PI, PI));
    all.push((STEP, BAND_Y));
    all.push((0.0, BAND_Y - STEP));
    (all, UNITS)
}

/// The outline as an SVG path, its corners true arcs.
fn outline_path() -> String {
    let (w, h) = UNITS;
    format!(
        "M 0 {r} A {r} {r} 0 0 1 {r} 0 L {tr} 0 A {r} {r} 0 0 1 {w} {r} \
         L {w} {sy} L {ws} {by} L {ws} {hb} A {r} {r} 0 0 1 {br} {h} \
         L {bl} {h} A {r} {r} 0 0 1 {s} {hb} L {s} {by} L 0 {sy} Z",
        r = CORNER_R,
        tr = w - CORNER_R,
        sy = BAND_Y - STEP,
        by = BAND_Y,
        ws = w - STEP,
        s = STEP,
        hb = h - CORNER_R,
        br = w - STEP - CORNER_R,
        bl = STEP + CORNER_R,
    )
}

fn svg(body: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\">{body}</svg>",
        UNITS.0, UNITS.1
    )
}

/// The relief moulded in the card: its edges, the top and left catching the light and the
/// right and foot turned from it; the line where the band under the label starts; and the
/// arrow in the band, pressed in, its lower edges lit.
fn detail_svg() -> String {
    let (w, h) = UNITS;
    let path = outline_path();
    let (cx, ay) = (w / 2.0, BAND_Y + 6.0);
    let arrow = format!("M {} {ay} H {} L {cx} {} Z", cx - 13.0, cx + 13.0, ay + 9.0);
    let band = (LABEL.1 + LABEL.3) as f32 / PX + 4.0;
    svg(&format!(
        "<clipPath id=\"c\"><path d=\"{path}\"/></clipPath><g clip-path=\"url(#c)\">\
         <path d=\"{path}\" fill=\"none\" stroke=\"#fff\" stroke-opacity=\"0.35\" stroke-width=\"7\"/>\
         <rect x=\"{right}\" y=\"0\" width=\"4\" height=\"{h}\" fill=\"#000\" fill-opacity=\"0.4\"/>\
         <rect x=\"0\" y=\"{foot}\" width=\"{w}\" height=\"4\" fill=\"#000\" fill-opacity=\"0.35\"/>\
         <rect x=\"0\" y=\"{band}\" width=\"{w}\" height=\"1.6\" fill=\"#000\" fill-opacity=\"0.3\"/>\
         </g>\
         <path d=\"{arrow}\" transform=\"translate(0 1.4)\" fill=\"#fff\" fill-opacity=\"0.35\"/>\
         <path d=\"{arrow}\" fill=\"#000\" fill-opacity=\"0.5\"/>",
        right = w - 4.0,
        foot = h - 4.0,
    ))
}

fn mask() -> &'static [u8] {
    static MASK: OnceLock<Vec<u8>> = OnceLock::new();
    MASK.get_or_init(|| {
        let shape = svg(&format!("<path d=\"{}\" fill=\"#fff\"/>", outline_path()));
        rasterise_svg(&shape, DS_W, DS_H).unwrap_or_else(|| vec![255; (DS_W * DS_H) as usize])
    })
}

fn depth() -> &'static [u8] {
    static DEPTH: OnceLock<Vec<u8>> = OnceLock::new();
    DEPTH.get_or_init(|| depth_map(mask(), DS_W as usize, DS_H as usize))
}

fn detail() -> &'static Detail {
    static DETAIL: OnceLock<Detail> = OnceLock::new();
    DETAIL.get_or_init(|| {
        rasterise_detail(&detail_svg(), DS_W, DS_H).unwrap_or_else(|| Detail::blank(DS_W, DS_H))
    })
}

/// How the card's face is laid out: a clear one shows its board, its contacts along the foot.
pub(crate) fn spec() -> Spec {
    Spec {
        w: DS_W,
        h: DS_H,
        label: LABEL,
        label_round: 0,
        label_cut: LABEL_CUT,
        mask: mask(),
        depth: depth(),
        detail: detail(),
        max_h: 96.0,
        rim: RIM,
        inside: Inside::Board(Board {
            x: (0.06, 0.94),
            top: 0.05,
            pins: (0.1, 0.9),
            contacts_from: 0.92,
            traces_from: 0.84,
        }),
    }
}
