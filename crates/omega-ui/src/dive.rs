//! The cart going into the handheld: from the middle of the shelf it rises a little, then is
//! pulled down out of the bottom of the screen, and the game's screen comes on behind it.
//! Coming out it is the same played backwards, a refused cart carrying its alert.

use omega_gfx::{out_h, out_w, Draw, MeshId, TexId};
use omega_store::Cart;

use crate::cart::{label_colour, label_text, model_box, model_of};
use crate::kit::palette::Palette;
use crate::kit::pixel::keyed;
use crate::models::disc::{self, DiscStack, StackDive, STACK_DIVE};
use crate::motion::ease;
use crate::plate::UndoFace;
use crate::shelf::{foot_y, model_scale};
use crate::spin::{self, Pose};
use crate::sprite::SPRITE_PX;

/// The alert shown on a refused cart as it comes back out: a warning sign drawn a pixel at a
/// time on the art grid, its sides a pixel out for every one and a half down, in the frame's
/// bright colour (`H`) ruled in the dark one (`k`), its mark in the dark one too. 62 by 50
/// screen pixels, as big as the sign the cart's label carried before.
#[rustfmt::skip]
const ALERT: [&str; 25] = [
    "...............k...............",
    "..............kHk..............",
    "..............kHk..............",
    ".............kHHHk.............",
    "............kHHHHHk............",
    "............kHHHHHk............",
    "...........kHHHHHHHk...........",
    "..........kHHHHHHHHHk..........",
    "..........kHHHHHHHHHk..........",
    ".........kHHHHkkkHHHHk.........",
    "........kHHHHHkkkHHHHHk........",
    "........kHHHHHkkkHHHHHk........",
    ".......kHHHHHHkkkHHHHHHk.......",
    "......kHHHHHHHkkkHHHHHHHk......",
    "......kHHHHHHHkkkHHHHHHHk......",
    ".....kHHHHHHHHkkkHHHHHHHHk.....",
    "....kHHHHHHHHHkkkHHHHHHHHHk....",
    "....kHHHHHHHHHHHHHHHHHHHHHk....",
    "...kHHHHHHHHHHHHHHHHHHHHHHHk...",
    "..kHHHHHHHHHHHkkkHHHHHHHHHHHk..",
    "..kHHHHHHHHHHHkkkHHHHHHHHHHHk..",
    ".kHHHHHHHHHHHHkkkHHHHHHHHHHHHk.",
    "kHHHHHHHHHHHHHHHHHHHHHHHHHHHHHk",
    "kHHHHHHHHHHHHHHHHHHHHHHHHHHHHHk",
    "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk",
];

/// The alert's size in art pixels.
pub const ALERT_W: u32 = ALERT[0].len() as u32;
pub const ALERT_H: u32 = ALERT.len() as u32;

/// The alert in `pal`, one texel to an art pixel, to upload NEAREST.
pub fn alert_face(pal: &Palette) -> UndoFace {
    keyed(&ALERT, pal)
}

/// One art pixel on the screen.
const ART: f32 = SPRITE_PX as f32;

/// `px` moved onto the art grid: to the nearest whole art pixel, a half rounded down.
const fn on_grid(px: f32) -> f32 {
    let art = px / ART;
    let whole = art as i32 - (art < 0.0 && art != (art as i32) as f32) as i32;
    let up = (art - whole as f32 > 0.5) as i32;
    (whole + up) as f32 * ART
}

/// The middle of the screen, on the grid.
fn cx() -> f32 {
    on_grid(out_w() as f32 / 2.0)
}

/// The cart in the middle of the shelf on its way into the handheld, or out of it.
pub struct CartDive<'a> {
    pub cart: &'a Cart,
    pub face: Option<TexId>,
    /// The model the cart is drawn with, sides and all; without one its face is spun flat.
    pub mesh: Option<MeshId>,
    /// The discs a game on discs stands as, drawn as the shelf draws them; see [`STACK_DIVE`]
    /// for how they go in.
    pub stack: Option<DiscStack>,
    pub rest: f32,
    /// How big the cart was drawn on the shelf, as a share of its own size, with `rest` its
    /// left edge there: both as `Shelf::selected_at` gives them.
    pub scale: f32,
    /// How the cart was held on the shelf: it comes level and square as it comes to the middle
    /// of the row, and goes back to this as it comes out.
    pub pose: Pose,
    /// How far in the cart is: 0 where the shelf left it, 1 out of the bottom of the screen.
    pub seat: f32,
    pub alert: Option<(TexId, f32)>,
    /// How dark the screen behind the cart has gone, as it goes in.
    pub dim: f32,
    /// How far the game's screen has come on.
    pub screen: f32,
    pub game: bool,
    /// A UMD's disc, turned this far, drawn spinning in its window while the game loads, and
    /// its hub, drawn still over it.
    pub disc: Option<(TexId, f32, TexId)>,
}

impl CartDive<'_> {
    pub fn draw(&self, out: &mut Vec<Draw>) {
        let seat = self.seat.clamp(0.0, 1.0);
        let dim = self.dim.clamp(0.0, 1.0);
        if dim > 0.0 {
            out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: out_w() as f32,
                h: out_h() as f32,
                colour: [0.0, 0.0, 0.0, dim],
            });
        }

        let ((x, y, cw, ch), pose) = self.placed(seat, true);
        if self.game && self.screen > 0.0 {
            out.push(Draw::Game);
        }
        // all the way in, the cart is out of the screen
        if seat >= 1.0 {
            return;
        }
        match (self.face, self.stack, self.mesh) {
            (Some(_), Some(stack), _) => match STACK_DIVE {
                StackDive::Whole => {
                    disc::push_stack(out, (x, y, cw, ch), &stack, [1.0; 4], 0.0, pose.yaw)
                }
                // the discs behind the first stay where the stack came to, at the top of its
                // rise, and go dark with the shelf behind them
                StackDive::FrontDisc => {
                    let (rect, held) = self.placed(seat, false);
                    let k = 1.0 - dim;
                    for disc in (1..stack.discs).rev() {
                        disc::push_disc(out, rect, &stack, disc, [k, k, k, 1.0], 0.0, held.yaw);
                    }
                    disc::push_disc(out, (x, y, cw, ch), &stack, 0, [1.0; 4], 0.0, pose.yaw);
                }
            },
            (Some(tex), None, Some(mesh)) => out.push(spin::posed_cart(
                (x, y, cw, ch),
                mesh,
                tex,
                [1.0, 1.0, 1.0, 1.0],
                0.0,
                pose.yaw,
            )),
            (Some(tex), None, None) => {
                spin::push_cart_side(out, (x, y, cw, ch), tex, 0.0, pose.yaw, 1.0, 1.0);
                out.push(spin::posed_face((x, y, cw, ch), tex, 1.0, 0.0, pose.yaw));
            }
            (None, _, _) => {
                let c = label_colour(&label_text(self.cart));
                out.push(Draw::Rect {
                    x,
                    y,
                    w: cw,
                    h: ch,
                    colour: [
                        c[0] as f32 / 255.0,
                        c[1] as f32 / 255.0,
                        c[2] as f32 / 255.0,
                        1.0,
                    ],
                });
            }
        }

        // a UMD's disc turning in its window, the cart held square to the screen
        if let (Some((tex, turn, hub)), true) = (self.disc, pose.yaw == 0.0) {
            let (wx, wy, ww, wh) = crate::cart::umd_window();
            let (dx, dy, dw, dh) = (x + wx * cw, y + wy * ch, ww * cw, wh * ch);
            out.push(Draw::Turned {
                x: dx,
                y: dy,
                w: dw,
                h: dh,
                tex,
                alpha: 1.0,
                turn,
            });
            let k = crate::cart::umd_hub_share();
            out.push(Draw::Tex {
                x: dx + dw * (1.0 - k) / 2.0,
                y: dy + dh * (1.0 - k) / 2.0,
                w: dw * k,
                h: dh * k,
                tex: hub,
                alpha: 1.0,
            });
        }

        // in the middle of the cart, to the nearest art pixel, as the cart moves between them
        if let Some((tex, alpha)) = self.alert {
            let (w, h) = ((ALERT_W * SPRITE_PX) as f32, (ALERT_H * SPRITE_PX) as f32);
            out.push(Draw::Tex {
                x: on_grid(x + (cw - w) / 2.0),
                y: on_grid(y + (ch - h) / 2.0),
                w,
                h,
                tex,
                alpha,
            });
        }
    }

    /// Where the cart is drawn `seat` of the way in, and how it is held: it comes to the middle
    /// of the row, square and level, as it rises a little; then, if it `drops`, it is pulled
    /// down out of the bottom of the screen, into the handheld, and otherwise it stays at the
    /// top of its rise.
    fn placed(&self, seat: f32, drops: bool) -> ((f32, f32, f32, f32), Pose) {
        let model = model_of(self.cart);
        let (cw, ch) = model_box(model);
        let (cw, ch) = (cw as f32, ch as f32);
        let scale = self.scale.clamp(0.0, 1.0);
        let shelf = model_scale(model);
        let row = (scale / shelf).min(1.0);
        let w0 = cw * scale;
        let (sw, sh) = (cw * shelf, ch * shelf);
        let k = ease((seat / SETTLE).min(1.0));
        let grown = row + (1.0 - row) * k;
        let (cw, ch) = (sw * grown, sh * grown);
        let centre = self.rest + w0 / 2.0;
        let x = centre + (cx() - centre) * k - cw / 2.0;
        let pose = self.pose.toward_rest(k);
        let top = foot_y(sh) - ch;
        let gone = out_h() as f32 - top;
        let fall = match drops {
            true => dive(seat, gone),
            false => dive(seat.min(RISE), gone),
        };
        ((x, top + fall - pose.lift, cw, ch), pose)
    }
}

/// How far into its way in a cart has come to the middle of the row, square and level.
const SETTLE: f32 = 0.35;
/// How far the cart rises before it is pulled down, in screen pixels, and how much of its way
/// in that takes: the quick lift of a hand letting go.
const LIFT: f32 = 8.0;
const RISE: f32 = 0.2;
/// How the pull down gathers speed: slow off the top, faster and faster out of the screen.
const PULL: f32 = 2.5;

/// How far below where it stands a cart is `seat` of the way in, `gone` being where it is out
/// of the screen: a quick rise, then a long pull down. Coming out it is the same played
/// backwards: up fast, slowing to a stop a little over its place, then down onto it.
fn dive(seat: f32, gone: f32) -> f32 {
    let u = seat.clamp(0.0, 1.0);
    if u < RISE {
        let k = 1.0 - u / RISE;
        -LIFT * (1.0 - k * k)
    } else {
        -LIFT + (gone + LIFT) * ((u - RISE) / (1.0 - RISE)).powf(PULL)
    }
}
