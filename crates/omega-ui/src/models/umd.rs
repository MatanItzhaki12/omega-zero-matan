//! The PSP's UMD, as it is made: a shell whose top is a half circle about the disc inside and
//! whose foot is square, its corners rounded; a round clear window across almost all of its
//! front, the disc's printed side showing through it, a white hub in its middle; and on the
//! foot either side of the window, the PSP logo on the left and a pad on the right, moulded in.
//!
//! The UMD is drawn 4 units to the millimetre, 65 by 66 mm as straight-on photos of real ones
//! measure, and its face about 4.25 pixels to the millimetre. The label is the disc's: round,
//! filling the window, as a scan of a UMD's disc is.

use std::f32::consts::PI;
use std::sync::OnceLock;

use crate::cart::{ink, label_colour, Inside, Spec, PAD};
use crate::silhouette::{depth_map, rasterise_detail, rasterise_svg, Detail};
use crate::text;

/// How thick the UMD is, as a share of its width: 4.2 mm on 65.
pub const THICK: f32 = 0.065;

/// How big the UMD stands on the shelf, as a share of its own size.
pub const SHELF_SCALE: f32 = 1.0;

/// The drawing's size in units.
const UNITS: (f32, f32) = (260.0, 264.0);

/// The top's half circle, in units: as wide as the UMD, its middle the disc's.
const TOP_R: f32 = 130.0;
/// How round the foot's corners are, in units.
const FOOT_R: f32 = 20.0;

/// The cuts the top's half circle is made of, and each of the foot's corners.
const TOP_CUTS: usize = 16;
const FOOT_CUTS: usize = 2;

/// The face in pixels: each an even number, so its halves stand on whole art pixels.
pub const UMD_W: u32 = 276;
pub const UMD_H: u32 = 280;

/// The window's middle on the face, in pixels: the top's half circle's.
const MIDDLE: (f32, f32) = (UMD_W as f32 / 2.0, UMD_W as f32 / 2.0);

/// The window, in pixels out from its middle: the disc's printed side fills it but for a thin
/// ring at its edge, where the disc's own edge shows.
const WINDOW: f32 = 130.0;
const PRINT: f32 = 126.0;

/// The label: the disc's printed side, round, in the face's pixels.
const LABEL: (u32, u32, u32, u32) = (12, 12, 252, 252);
const LABEL_ROUND: u32 = 126;

/// The hub in the middle of a generated label, as a share of the print's radius.
const HUB: f32 = 0.45;

/// How far in from the outline the clear shell lightens at its rim, in pixels.
const RIM: u32 = 6;

/// The disc's edge, showing through the window round its print.
const DISC_EDGE: [u8; 3] = [0x6c, 0x70, 0x7a];

/// How far the middle of the art pixel holding `x, y` is from the window's middle, in pixels.
fn from_middle(x: u32, y: u32) -> f32 {
    let (cx, cy) = ((x & !1) as f32 + 1.0, (y & !1) as f32 + 1.0);
    (cx - MIDDLE.0).hypot(cy - MIDDLE.1)
}

/// What stands behind `x, y` in the shell: the disc's edge round its print, or nothing.
pub(crate) fn behind(x: u32, y: u32) -> Option<[u8; 3]> {
    (PRINT..WINDOW)
        .contains(&from_middle(x, y))
        .then_some(DISC_EDGE)
}

/// The print of a UMD with no scan, without its hub: what spins under the hub.
pub(crate) fn generated_print(s: &Spec, title: &str) -> Vec<u8> {
    let (_, _, lw, lh) = s.label;
    let mut rgba = generated_label(s, title);
    // the middle back to plain white, the hub taken off
    let r = lw as f32 / 2.0;
    for y in 0..lh {
        for x in 0..lw {
            let d = (x as f32 + 0.5 - lw as f32 / 2.0).hypot(y as f32 + 0.5 - lh as f32 / 2.0) / r;
            if d < HUB_SHARE + 0.01 {
                let i = ((y * lw + x) * 4) as usize;
                rgba[i..i + 3].copy_from_slice(&HUB_WHITE);
            }
        }
    }
    rgba
}

/// The label a UMD with no scan wears, `s.label`'s size: its disc printed in the game's colour,
/// a pale hub in its middle with the UMD's name on it, and the title under the hub.
pub(crate) fn generated_label(s: &Spec, title: &str) -> Vec<u8> {
    let (lx, ly, lw, lh) = s.label;
    let bg = label_colour(title);
    let r = lw as f32 / 2.0;
    let mut rgba = Vec::with_capacity((lw * lh * 4) as usize);
    for y in 0..lh {
        for x in 0..lw {
            let d = from_middle(x + lx, y + ly) / r;
            let c = match d {
                d if d < HUB => HUB_WHITE,
                _ => bg,
            };
            rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    lay_hub(&mut rgba, lw, lh);
    let Some(font) = text::label_font() else {
        return rgba;
    };
    let mut put = |words: &str, (bx, by, bw, bh): (u32, u32, u32, u32), lines, colour| {
        let mut part = vec![0u8; (bw * bh * 4) as usize];
        let layout = text::fit_box(font, words, bw as f32, bh as f32, lines, 18.0, 18.0);
        text::draw_centred(&mut part, bw, bh, &layout, colour);
        for (i, px) in part.chunks_exact(4).enumerate() {
            let a = px[3] as u32;
            let (x, y) = (bx + i as u32 % bw, by + i as u32 / bw);
            let d = ((y * lw + x) * 4) as usize;
            for c in 0..3 {
                rgba[d + c] = ((px[c] as u32 * a + rgba[d + c] as u32 * (255 - a)) / 255) as u8;
            }
        }
    };
    // the title on the print under the hub, clear of its round edge
    let title_y = (lh as f32 * (0.5 + HUB / 2.0)) as u32 + PAD;
    let title_w = lw * 3 / 5;
    put(
        title,
        ((lw - title_w) / 2, title_y, title_w, s.max_h as u32),
        2,
        ink(bg),
    );
    rgba
}

/// The disc's middle, under its hub: white, as a real UMD's is.
const HUB_WHITE: [u8; 3] = [0xf6, 0xf6, 0xf4];

/// The hub, an element of its own laid over the disc's middle: white, "UMD" across it in blue
/// and "UNIVERSAL MEDIA DISC" under that in small grey letters, drawn twice the size it shows.
const HUB_PNG: &[u8] = include_bytes!("../../assets/umd_hub.png");

/// How wide the hub is, as a share of the disc's print: over the hole a print leaves, and a
/// little of the print round it.
pub const HUB_SHARE: f32 = 0.47;

/// The hub as RGBA, `size` pixels across, each pixel the average of the drawing's under it.
pub fn hub_face(size: u32) -> Vec<u8> {
    static DRAWN: OnceLock<Option<(Vec<u8>, u32)>> = OnceLock::new();
    let drawn = DRAWN.get_or_init(|| {
        let mut dec = png::Decoder::new(std::io::Cursor::new(HUB_PNG));
        dec.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = dec.read_info().ok()?;
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).ok()?;
        (info.color_type == png::ColorType::Rgba)
            .then(|| (buf[..info.buffer_size()].to_vec(), info.width))
    });
    let mut out = vec![0u8; (size * size * 4) as usize];
    let Some((src, n)) = drawn else {
        return out;
    };
    let k = *n as f32 / size as f32;
    for y in 0..size {
        for x in 0..size {
            let (x0, y0) = ((x as f32 * k) as u32, (y as f32 * k) as u32);
            let (x1, y1) = (
                (((x + 1) as f32 * k) as u32).max(x0 + 1),
                (((y + 1) as f32 * k) as u32).max(y0 + 1),
            );
            let mut acc = [0u32; 4];
            let mut count = 0;
            for sy in y0..y1.min(*n) {
                for sx in x0..x1.min(*n) {
                    let i = ((sy * n + sx) * 4) as usize;
                    // premultiplied, so the clear round the hub bleeds no colour into its edge
                    let a = src[i + 3] as u32;
                    for c in 0..3 {
                        acc[c] += src[i + c] as u32 * a;
                    }
                    acc[3] += a;
                    count += 1;
                }
            }
            let o = ((y * size + x) * 4) as usize;
            for c in 0..3 {
                out[o + c] = acc[c].checked_div(acc[3]).unwrap_or(0) as u8;
            }
            out[o + 3] = (acc[3] / count.max(1)) as u8;
        }
    }
    out
}

/// The hub laid over the middle of `rgba`, a disc's print `w` by `h`.
pub(crate) fn lay_hub(rgba: &mut [u8], w: u32, h: u32) {
    let size = (w.min(h) as f32 * HUB_SHARE).round() as u32;
    let hub = hub_face(size);
    let (ox, oy) = ((w - size) / 2, (h - size) / 2);
    for (j, px) in hub.chunks_exact(4).enumerate() {
        let a = px[3] as u32;
        if a == 0 {
            continue;
        }
        let i = (((oy + j as u32 / size) * w + ox + j as u32 % size) * 4) as usize;
        for c in 0..3 {
            rgba[i + c] = ((px[c] as u32 * a + rgba[i + c] as u32 * (255 - a)) / 255) as u8;
        }
    }
}

/// The scan `rgba`, `w` by `h`, white where its print leaves the disc's middle clear, as a real
/// UMD's is: the print alone, for the hub to be laid over. A white ring a render of a UMD leaves
/// round its hole is taken for the middle's white.
pub(crate) fn with_middle(mut rgba: Vec<u8>, w: u32, h: u32) -> Vec<u8> {
    let r = w.min(h) as f32 / 2.0;
    for y in 0..h {
        for x in 0..w {
            let d = (x as f32 + 0.5 - w as f32 / 2.0).hypot(y as f32 + 0.5 - h as f32 / 2.0) / r;
            let i = ((y * w + x) * 4) as usize;
            if d > 1.0 {
                continue;
            }
            let whiteish = rgba[i..i + 3].iter().all(|&c| c > 214);
            let a = if d < 0.58 && whiteish {
                0
            } else {
                rgba[i + 3] as u32
            };
            if a == 255 {
                continue;
            }
            for c in 0..3 {
                rgba[i + c] =
                    ((rgba[i + c] as u32 * a + HUB_WHITE[c] as u32 * (255 - a)) / 255) as u8;
            }
            rgba[i + 3] = 255;
        }
    }
    rgba
}

/// The scan with the hub over its middle, as the shelf shows a UMD.
pub(crate) fn with_hub(rgba: Vec<u8>, w: u32, h: u32) -> Vec<u8> {
    let mut rgba = with_middle(rgba, w, h);
    lay_hub(&mut rgba, w, h);
    rgba
}

/// One rounded corner of the outline, `cuts` cuts from `from` to `to` radians about `centre`,
/// `r` units out: the corners between its ends stand a little outside the arc and the cuts
/// between them a little inside, so the walls follow the edge the face's alpha cuts.
fn corner(
    out: &mut Vec<(f32, f32)>,
    centre: (f32, f32),
    r: f32,
    (from, to): (f32, f32),
    cuts: usize,
) {
    let out_by = (1.0 + 1.0 / ((to - from) / cuts as f32 / 2.0).cos()) / 2.0;
    for i in 0..=cuts {
        let a = from + (to - from) * i as f32 / cuts as f32;
        let r = if i == 0 || i == cuts { r } else { r * out_by };
        out.push((centre.0 + r * a.cos(), centre.1 + r * a.sin()));
    }
}

/// The UMD's outline in drawing units, all the way round clockwise from the left of the top's
/// half circle, and the drawing's size.
pub(crate) fn outline() -> (Vec<(f32, f32)>, (f32, f32)) {
    let (w, h) = UNITS;
    let mut all = Vec::new();
    corner(&mut all, (TOP_R, TOP_R), TOP_R, (PI, 2.0 * PI), TOP_CUTS);
    let foot = h - FOOT_R;
    corner(
        &mut all,
        (w - FOOT_R, foot),
        FOOT_R,
        (0.0, PI / 2.0),
        FOOT_CUTS,
    );
    corner(&mut all, (FOOT_R, foot), FOOT_R, (PI / 2.0, PI), FOOT_CUTS);
    (all, UNITS)
}

/// The outline as an SVG path, its curves true arcs.
fn outline_path() -> String {
    let (w, h) = UNITS;
    format!(
        "M 0 {t} A {t} {t} 0 0 1 {w} {t} L {w} {b} A {f} {f} 0 0 1 {c} {h} \
         L {f} {h} A {f} {f} 0 0 1 0 {b} Z",
        t = TOP_R,
        f = FOOT_R,
        b = h - FOOT_R,
        c = w - FOOT_R,
    )
}

fn svg(body: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\">{body}</svg>",
        UNITS.0, UNITS.1
    )
}

/// The PSP logo on the left of the foot, its letters' strokes, in units.
const LOGO: [&str; 3] = [
    "M 14 250 V 236 H 23 Q 27 236 27 240 Q 27 244 23 244 H 16",
    "M 42 236 H 33 Q 30 236 30 239.5 Q 30 243 33 243 H 39 Q 42 243 42 246.5 Q 42 250 39 250 H 30",
    "M 46 250 V 236 H 55 Q 59 236 59 240 Q 59 244 55 244 H 48",
];

/// The pad on the right of the foot, in units.
const PAD_ICON: &str = "M 213 240 Q 213 237 217 237 H 233 Q 237 237 237 240 L 239 248 \
                        Q 239.5 251 236.5 250 L 232 246 H 218 L 213.5 250 Q 210.5 251 211 248 Z";

/// The relief moulded in the shell: its edges, the top and left catching the light and the
/// right and foot turned from it; the window's lip; and the logo and the pad, pressed in, their
/// lower edges lit.
fn detail_svg() -> String {
    let (w, h) = UNITS;
    let path = outline_path();
    let pressed = |d: &str, fill: bool| {
        let paint = |colour: &str, opacity: f32, dy: f32| match fill {
            true => format!(
                "<path d=\"{d}\" transform=\"translate(0 {dy})\" fill=\"{colour}\" \
                 fill-opacity=\"{opacity}\"/>"
            ),
            false => format!(
                "<path d=\"{d}\" transform=\"translate(0 {dy})\" fill=\"none\" \
                 stroke=\"{colour}\" stroke-opacity=\"{opacity}\" stroke-width=\"2.6\" \
                 stroke-linejoin=\"round\" stroke-linecap=\"round\"/>"
            ),
        };
        paint("#fff", 0.45, 1.2) + &paint("#000", 0.55, 0.0)
    };
    let logo: String = LOGO.iter().map(|d| pressed(d, false)).collect();
    svg(&format!(
        "<clipPath id=\"c\"><path d=\"{path}\"/></clipPath><g clip-path=\"url(#c)\">\
         <path d=\"{path}\" fill=\"none\" stroke=\"#fff\" stroke-opacity=\"0.4\" stroke-width=\"8\"/>\
         <rect x=\"{r}\" y=\"{t}\" width=\"5\" height=\"{side}\" fill=\"#000\" fill-opacity=\"0.4\"/>\
         <rect x=\"0\" y=\"{fy}\" width=\"{w}\" height=\"5\" fill=\"#000\" fill-opacity=\"0.35\"/>\
         </g>\
         <circle cx=\"{t}\" cy=\"{t}\" r=\"{lip}\" fill=\"none\" stroke=\"#000\" \
         stroke-opacity=\"0.35\" stroke-width=\"3\"/>{logo}{pad}",
        r = w - 5.0,
        t = TOP_R,
        side = h - TOP_R,
        fy = h - 5.0,
        lip = WINDOW / (UMD_W as f32 / w) + 1.5,
        pad = pressed(PAD_ICON, true),
    ))
}

fn mask() -> &'static [u8] {
    static MASK: OnceLock<Vec<u8>> = OnceLock::new();
    MASK.get_or_init(|| {
        let shape = svg(&format!("<path d=\"{}\" fill=\"#fff\"/>", outline_path()));
        rasterise_svg(&shape, UMD_W, UMD_H).unwrap_or_else(|| vec![255; (UMD_W * UMD_H) as usize])
    })
}

fn depth() -> &'static [u8] {
    static DEPTH: OnceLock<Vec<u8>> = OnceLock::new();
    DEPTH.get_or_init(|| depth_map(mask(), UMD_W as usize, UMD_H as usize))
}

fn detail() -> &'static Detail {
    static DETAIL: OnceLock<Detail> = OnceLock::new();
    DETAIL.get_or_init(|| {
        rasterise_detail(&detail_svg(), UMD_W, UMD_H).unwrap_or_else(|| Detail::blank(UMD_W, UMD_H))
    })
}

/// How the UMD's face is laid out: what a clear shell shows inside it is the disc.
pub(crate) fn spec() -> Spec {
    Spec {
        w: UMD_W,
        h: UMD_H,
        label: LABEL,
        label_round: LABEL_ROUND,
        mask: mask(),
        depth: depth(),
        detail: detail(),
        max_h: 52.0,
        rim: RIM,
        inside: Inside::Disc,
    }
}
