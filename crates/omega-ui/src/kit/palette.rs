//! The colours the kit draws in, made from the backdrop colour the player picks in Settings >
//! Background > Colour, so the menus always sit in the same colour as the shelf behind them.
//! Night, the default, gives plain greys.
//!
//! The ramp is Mega Man Zero's own menu colours, which are what Amber gives. Any other backdrop
//! colour keeps how light each step is (in OKLCH) and turns the ramp to its own hue, taking as
//! much of the ramp's colour as it has itself: none for Night, a touch for Slate, all of it for
//! the rest.
//!
//! A port of assets/ui/foundation/palette.js, step for step, so the menus come out in the very
//! colours the storybook shows.

use std::f64::consts::PI;
use std::sync::OnceLock;

use omega_store::{colour_index, BACKDROP_COLOURS};

use crate::backdrop::BACKDROP_PALETTE;

/// Every colour the kit draws in for one backdrop colour: the backdrop's own, then the ramp
/// turned to its hue, darkest first. The comments name each one's --oz-* property in the kit.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    /// --oz-bg: the backdrop colour itself.
    pub bg: [u8; 3],
    /// --oz-dark: the bar under the actions, outlines.
    pub dark: [u8; 3],
    /// --oz-edge: the shadow line inside a well.
    pub edge: [u8; 3],
    /// --oz-well: a panel's body, a label's shadow.
    pub well: [u8; 3],
    /// --oz-plate: what is off on a row, the bars a signal does not reach, and in the top bar.
    pub plate: [u8; 3],
    /// --oz-tab: a panel's title tab.
    pub tab: [u8; 3],
    /// --oz-band: the top of an action's button.
    pub band: [u8; 3],
    /// --oz-stripe: the dark line between two frame lines.
    pub stripe: [u8; 3],
    /// --oz-dim: the title of a panel that is not active.
    pub dim: [u8; 3],
    /// --oz-frame: frames.
    pub frame: [u8; 3],
    /// --oz-frame-hi: the strip beside a title tab, the arrows.
    pub frame_hi: [u8; 3],
    /// --oz-label: an action's big label.
    pub label: [u8; 3],
    /// --oz-cream: the active title, an action's hint.
    pub cream: [u8; 3],
}

impl Palette {
    /// The colour a sprite key stands for, as pixel.js KEYS: k dark, e edge, w well, p plate,
    /// t tab, b band, s stripe, d dim, A frame, H frame_hi, L label, c cream; anything else
    /// (and '.') is None.
    pub fn key(&self, k: char) -> Option<[u8; 3]> {
        Some(match k {
            'k' => self.dark,
            'e' => self.edge,
            'w' => self.well,
            'p' => self.plate,
            't' => self.tab,
            'b' => self.band,
            's' => self.stripe,
            'd' => self.dim,
            'A' => self.frame,
            'H' => self.frame_hi,
            'L' => self.label,
            'c' => self.cream,
            _ => return None,
        })
    }
}

/// The palette for backdrop colour `colour` (an index in the [`BACKDROP_COLOURS`] order:
/// night, slate, navy, teal, green, plum, crimson, amber); out of range gives Night.
pub fn palette(colour: u8) -> &'static Palette {
    static PALETTES: OnceLock<[Palette; BACKDROP_COLOURS.len()]> = OnceLock::new();
    let all = PALETTES.get_or_init(|| BACKDROP_PALETTE.map(turned));
    all.get(colour as usize).unwrap_or(&all[0])
}

/// A colour as the [f32; 4] Draw takes, with alpha `a`.
pub fn rgba(c: [u8; 3], a: f32) -> [f32; 4] {
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        a,
    ]
}

/// Mega Man Zero's menu colours, darkest first: the ramp every backdrop colour turns.
const RAMP: [[u8; 3]; 12] = [
    hex(0x2a0f04), // --oz-dark
    hex(0x6a3d01), // --oz-edge
    hex(0x4f300d), // --oz-well
    hex(0x765020), // --oz-plate
    hex(0x845a1a), // --oz-tab
    hex(0xa57221), // --oz-band
    hex(0xb0731b), // --oz-stripe
    hex(0xbe9732), // --oz-dim
    hex(0xe6ab47), // --oz-frame
    hex(0xf8b83f), // --oz-frame-hi
    hex(0xf2c87d), // --oz-label
    hex(0xf5e5bb), // --oz-cream
];

/// A backdrop colour this close to grey (in OKLCH chroma) takes none of the ramp's colour.
const GREY: f64 = 0.01;
/// How far a step of the ramp may sit from the ramp's own hue, in degrees: the game's darkest
/// browns lean further toward red than this, which reads oddly once turned to another hue.
const LEAN: f64 = 15.0;

const fn hex(rgb: u32) -> [u8; 3] {
    [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]
}

/// The ramp turned to the hue of backdrop colour `base`, with as much of the ramp's colour as
/// `base` has next to Amber, whose ramp it is.
fn turned(base: [u8; 3]) -> Palette {
    let amber = colour_index("amber").expect("amber is a backdrop colour") as usize;
    let [_, chroma, hue] = oklch(base);
    let [_, ramp_chroma, ramp_hue] = oklch(BACKDROP_PALETTE[amber]);
    let take = ((chroma - GREY) / (ramp_chroma - GREY)).clamp(0.0, 1.0);
    let steps = RAMP.map(|colour| {
        let [l, c, h] = oklch(colour);
        let lean = ((h - ramp_hue + 540.0) % 360.0 - 180.0).clamp(-LEAN, LEAN);
        fit(l, c * take, if take != 0.0 { hue + lean } else { 0.0 })
    });
    let [dark, edge, well, plate, tab, band, stripe, dim, frame, frame_hi, label, cream] = steps;
    Palette {
        bg: base,
        dark,
        edge,
        well,
        plate,
        tab,
        band,
        stripe,
        dim,
        frame,
        frame_hi,
        label,
        cream,
    }
}

fn to_linear(c: u8) -> f64 {
    let c = c as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(c: f64) -> f64 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// A channel from 0 to 255 as a byte. JS's Math.round takes a half up and f64::round takes it
/// away from zero, which is the same thing here: a channel is never below 0.
fn to_byte(c: f64) -> u8 {
    c.round() as u8
}

/// OKLCH of an sRGB colour (Björn Ottosson's OKLab), the hue in degrees.
fn oklch(colour: [u8; 3]) -> [f64; 3] {
    let [r, g, b] = colour.map(to_linear);
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    let lightness = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s;
    let a = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
    let b = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;
    [
        lightness,
        a.hypot(b),
        (b.atan2(a) * 180.0 / PI + 360.0) % 360.0,
    ]
}

/// Linear sRGB of an OKLCH colour, which may fall outside 0..1.
fn linear(lightness: f64, chroma: f64, hue: f64) -> [f64; 3] {
    let a = chroma * (hue * PI / 180.0).cos();
    let b = chroma * (hue * PI / 180.0).sin();
    let l = (lightness + 0.3963377774 * a + 0.2158037573 * b).powf(3.0);
    let m = (lightness - 0.1055613458 * a - 0.0638541728 * b).powf(3.0);
    let s = (lightness - 0.0894841775 * a - 1.2914855480 * b).powf(3.0);
    [
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ]
}

/// An OKLCH colour in sRGB, its chroma cut back as far as it takes to fit.
fn fit(lightness: f64, chroma: f64, hue: f64) -> [u8; 3] {
    let fits = |c: f64| {
        linear(lightness, c, hue)
            .iter()
            .all(|v| (-1e-6..=1.0 + 1e-6).contains(v))
    };
    let (mut lo, mut hi) = (0.0, chroma);
    if fits(chroma) {
        lo = chroma;
    } else {
        for _ in 0..30 {
            let mid = f64::midpoint(lo, hi);
            if fits(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
    }
    linear(lightness, lo, hue).map(|v| to_byte(to_srgb(v.clamp(0.0, 1.0)) * 255.0))
}
