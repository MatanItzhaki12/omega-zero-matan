//! A one-screen view made of pieces of the two DS screens, after Pokémon Fulldex's merged view
//! (mamesj/pokemon-fulldex, frontend/merged.h, GPL-3.0).
//!
//! A [`Scene`] is a list of [`Piece`]s drawn in order, each a rectangle of one screen (or a plain
//! colour) drawn somewhere in the view at some scale. Positions are in the view's own pixels, the
//! 256 by 192 a whole DS screen fills; the view is drawn [`SCALE`] times that, so a piece drawn
//! smaller keeps its detail. A piece can leave out some of its screen's layers (the touch screen's
//! background, so only its buttons stand over the top screen), and pixels of given colours.

use crate::screen::{Screens, H, W};

/// How many of the drawn view's pixels a view pixel is, each way.
pub const SCALE: usize = 2;
/// The drawn view's size.
pub const OUT_W: usize = W * SCALE;
pub const OUT_H: usize = H * SCALE;

/// Layer classes, from the 2D renderer's own flags for the layer that drew a pixel.
pub const LAYER_BG0: u8 = 0x01;
pub const LAYER_BG1: u8 = 0x02;
pub const LAYER_BG2: u8 = 0x04;
pub const LAYER_BG3: u8 = 0x08;
pub const LAYER_OBJ: u8 = 0x10;
pub const LAYER_BACKDROP: u8 = 0x20;
pub const LAYER_3D: u8 = 0x40;

/// The class of a pixel the renderer flagged `flag`: a background, a sprite, the backdrop or
/// the 3D layer.
pub fn layer_class(flag: u8) -> u8 {
    if flag & 0x40 != 0 {
        return if flag & 0x80 != 0 {
            LAYER_OBJ
        } else {
            LAYER_3D
        };
    }
    if flag & 0x90 != 0 {
        return LAYER_OBJ;
    }
    if flag & 0x0F != 0 {
        return flag & 0x0F;
    }
    LAYER_BACKDROP
}

/// Where a piece's pixels come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The top screen (0) or the touch screen (1).
    Screen(usize),
    /// A plain colour, 0xRRGGBB.
    Fill(u32),
}

/// One piece of a scene: a rectangle of a screen drawn somewhere in the view.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub source: Source,
    /// The rectangle on the screen: x, y, width, height.
    pub sx: i32,
    pub sy: i32,
    pub sw: i32,
    pub sh: i32,
    /// Where its top left corner goes in the view, and how big a screen pixel is drawn.
    pub dx: f32,
    pub dy: f32,
    pub scale: f32,
    /// Drawn this many times wider than `scale` makes it.
    pub stretch_x: f32,
    /// Layer classes not drawn.
    pub drop: u8,
    /// How opaque what is drawn is, of 255.
    pub alpha: u8,
    /// Mirrored left to right.
    pub flip_x: bool,
    /// An outline round it, outside it, its colour and width in view pixels: as a panel floating
    /// over the picture has, or the battle button chosen.
    pub border: Option<(u32, f32)>,
    /// Colours of its screen not drawn (0xRRGGBB): the outline round a sprite.
    pub keys: [u32; 2],
}

impl Piece {
    /// The whole of `screen`, filling the view.
    pub fn full(screen: usize) -> Piece {
        Piece::crop(screen, 0, 0, W as i32, H as i32)
    }

    /// A rectangle of `screen`, at its own place and size until moved.
    pub fn crop(screen: usize, sx: i32, sy: i32, sw: i32, sh: i32) -> Piece {
        Piece {
            source: Source::Screen(screen),
            sx,
            sy,
            sw,
            sh,
            dx: sx as f32,
            dy: sy as f32,
            scale: 1.0,
            stretch_x: 1.0,
            drop: 0,
            alpha: 255,
            flip_x: false,
            border: None,
            keys: [NO_KEY; 2],
        }
    }

    /// A plain rectangle of `rgb`, `w` by `h` view pixels at `dx, dy`.
    pub fn fill(dx: f32, dy: f32, w: f32, h: f32, rgb: u32) -> Piece {
        Piece {
            source: Source::Fill(rgb),
            sx: 0,
            sy: 0,
            sw: 1,
            sh: 1,
            dx,
            dy,
            scale: 1.0,
            stretch_x: 1.0,
            drop: 0,
            alpha: 255,
            flip_x: false,
            border: None,
            keys: [NO_KEY; 2],
        }
        .sized(w, h)
    }

    /// Placed at `dx, dy`.
    pub fn at(mut self, dx: f32, dy: f32) -> Piece {
        self.dx = dx;
        self.dy = dy;
        self
    }

    /// Drawn at `scale`.
    pub fn scaled(mut self, scale: f32) -> Piece {
        self.scale = scale;
        self
    }

    /// Without the layer classes `drop`.
    pub fn dropping(mut self, drop: u8) -> Piece {
        self.drop = drop;
        self
    }

    /// With the dark outline a floating panel has.
    pub fn bordered(mut self) -> Piece {
        self.border = Some((BORDER, BORDER_W));
        self
    }

    /// A fill stretched to `w` by `h` view pixels.
    fn sized(mut self, w: f32, h: f32) -> Piece {
        self.scale = h;
        self.stretch_x = w / h;
        self
    }

    /// Its size in the view.
    pub fn size(&self) -> (f32, f32) {
        (
            self.sw as f32 * self.scale * self.stretch_x,
            self.sh as f32 * self.scale,
        )
    }
}

/// The outline drawn round a floating panel, and how wide it is in view pixels.
pub const BORDER: u32 = 0x282830;
pub const BORDER_W: f32 = 0.75;
/// A colour no pixel has: no key.
pub const NO_KEY: u32 = u32::MAX;

/// A one-screen view: its pieces, drawn in order on a plain colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// What it shows, for the log and the tests: `battle-moves`, `top`...
    pub name: &'static str,
    pub pieces: Vec<Piece>,
    /// The colour behind the pieces, 0xRRGGBB, or a screen's pixel to take it from.
    pub back: Back,
}

/// What stands behind a scene's pieces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Back {
    Colour(u32),
    /// The colour of a screen's pixel: `screen, x, y`.
    Sample(usize, usize, usize),
}

impl Scene {
    /// `screen` alone, filling the view.
    pub fn only(name: &'static str, screen: usize) -> Scene {
        Scene {
            name,
            pieces: vec![Piece::full(screen)],
            back: Back::Colour(0),
        }
    }

    /// `pieces` side by side (`across`) or one above the other, at one scale, as large as the
    /// view holds them, in its middle.
    pub fn stack(name: &'static str, pieces: Vec<Piece>, across: bool) -> Scene {
        let (mut w, mut h) = (0.0f32, 0.0f32);
        for p in &pieces {
            if across {
                w += p.sw as f32;
                h = h.max(p.sh as f32);
            } else {
                h += p.sh as f32;
                w = w.max(p.sw as f32);
            }
        }
        let scale = (H as f32 / h).min(W as f32 / w);
        let (mut x, mut y) = ((W as f32 - w * scale) / 2.0, (H as f32 - h * scale) / 2.0);
        let mut out = Vec::with_capacity(pieces.len());
        for mut p in pieces {
            p.scale = scale;
            if across {
                p.dx = x;
                p.dy = y + (h - p.sh as f32) * scale / 2.0;
                x += p.sw as f32 * scale;
            } else {
                p.dx = x + (w - p.sw as f32) * scale / 2.0;
                p.dy = y;
                y += p.sh as f32 * scale;
            }
            out.push(p);
        }
        Scene {
            name,
            pieces: out,
            back: Back::Colour(0),
        }
    }

    /// Whether the scene is one screen whole at its own size, on nothing: drawn by copying that
    /// screen, at the screen's own size.
    pub fn whole_screen(&self) -> Option<usize> {
        match self.pieces.as_slice() {
            [p] if p.sx == 0
                && p.sy == 0
                && p.sw == W as i32
                && p.sh == H as i32
                && p.dx == 0.0
                && p.dy == 0.0
                && p.scale == 1.0
                && p.stretch_x == 1.0
                && p.drop == 0
                && p.alpha == 255
                && !p.flip_x
                && p.border.is_none()
                && p.keys == [NO_KEY; 2] =>
            {
                match p.source {
                    Source::Screen(s) => Some(s),
                    Source::Fill(_) => None,
                }
            }
            _ => None,
        }
    }

    /// Draws the scene from `screens` into `out`, [`OUT_W`] by [`OUT_H`] pixels of 0xRRGGBB.
    pub fn draw(&self, screens: &Screens, out: &mut [u32]) {
        debug_assert_eq!(out.len(), OUT_W * OUT_H);
        let back = match self.back {
            Back::Colour(c) => c,
            Back::Sample(s, x, y) => screens.pixel(s, x.min(W - 1), y.min(H - 1)),
        };
        out.fill(back);
        for piece in &self.pieces {
            if let Some((colour, width)) = piece.border {
                outline(piece, colour, width, out);
            }
            draw_piece(piece, screens, out);
        }
    }
}

/// The view's pixels, `x0..x1` and `y0..y1` of the drawn view, a piece covers.
fn cover(piece: &Piece, pad: f32) -> (usize, usize, usize, usize) {
    let (w, h) = piece.size();
    let k = SCALE as f32;
    let clamp = |v: f32, max: usize| (v.max(0.0) as usize).min(max);
    (
        clamp(((piece.dx - pad) * k).round(), OUT_W),
        clamp(((piece.dx + w + pad) * k).round(), OUT_W),
        clamp(((piece.dy - pad) * k).round(), OUT_H),
        clamp(((piece.dy + h + pad) * k).round(), OUT_H),
    )
}

fn outline(piece: &Piece, colour: u32, width: f32, out: &mut [u32]) {
    let (x0, x1, y0, y1) = cover(piece, width);
    for y in y0..y1 {
        out[y * OUT_W + x0..y * OUT_W + x1].fill(colour);
    }
}

fn draw_piece(piece: &Piece, screens: &Screens, out: &mut [u32]) {
    let (x0, x1, y0, y1) = cover(piece, 0.0);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let k = SCALE as f32;
    let alpha = piece.alpha as u32;
    let put = |px: &mut u32, colour: u32| {
        *px = match alpha {
            255 => colour,
            a => blend(*px, colour, a),
        };
    };
    let screen = match piece.source {
        Source::Fill(rgb) => {
            for y in y0..y1 {
                out[y * OUT_W + x0..y * OUT_W + x1]
                    .iter_mut()
                    .for_each(|px| put(px, rgb));
            }
            return;
        }
        Source::Screen(s) => s,
    };
    // each column's source pixel, worked out once for the piece rather than at every pixel
    let sx_scale = piece.scale * piece.stretch_x * k;
    let cols: Vec<usize> = (x0..x1)
        .map(|x| {
            let u = ((x as f32 + 0.5) / k - piece.dx) * k / sx_scale;
            let mut sx = (u.max(0.0) as i32).min(piece.sw - 1);
            if piece.flip_x {
                sx = piece.sw - 1 - sx;
            }
            (piece.sx + sx).clamp(0, W as i32 - 1) as usize
        })
        .collect();
    let sy_scale = piece.scale * k;
    let rows = screens.rows(screen);
    let keyed = piece.keys != [NO_KEY; 2];
    for y in y0..y1 {
        let v = ((y as f32 + 0.5) / k - piece.dy) * k / sy_scale;
        let sy = (piece.sy + (v.max(0.0) as i32).min(piece.sh - 1)).clamp(0, H as i32 - 1) as usize;
        let src = &rows[sy * W * 4..(sy + 1) * W * 4];
        let row = &mut out[y * OUT_W + x0..y * OUT_W + x1];
        for (px, &sx) in row.iter_mut().zip(&cols) {
            if piece.drop != 0 && layer_class(screens.layer(screen, sx, sy)) & piece.drop != 0 {
                continue;
            }
            let c = u32::from_le_bytes([src[sx * 4], src[sx * 4 + 1], src[sx * 4 + 2], 0]);
            if keyed && piece.keys.contains(&c) {
                continue;
            }
            put(px, c);
        }
    }
}

/// `over` drawn `a` of 255 opaque on `under`.
pub fn blend(under: u32, over: u32, a: u32) -> u32 {
    let mut out = 0;
    for shift in [0, 8, 16] {
        let (u, o) = ((under >> shift) & 0xFF, (over >> shift) & 0xFF);
        out |= ((o * a + u * (255 - a)) / 255) << shift;
    }
    out
}
