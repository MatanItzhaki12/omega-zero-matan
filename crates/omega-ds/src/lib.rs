//! omega-zero's DS screens: the DS's two screens shown on the handheld's one.
//!
//! The core draws the top screen above the touch screen in one frame. [`Ds`] turns that into the
//! picture shown, as the player's [`Layout`] says: both screens stacked or side by side, one
//! screen with the other a press away or small in its corner, or, for a game with layouts of its
//! own, One Screen: one view made of both, laid out for what the game is doing, after Pokémon
//! Fulldex's merged view (mamesj/pokemon-fulldex, GPL-3.0). It also answers for the touch screen
//! the handheld has none of: L2 held makes the D-pad a cursor on it and A a touch, and One Screen
//! touches the buttons it lays over the picture when they are chosen with the D-pad.

pub mod gen4;
pub mod layout;
pub mod rom;
pub mod scene;
pub mod screen;
pub mod watch;

use std::path::Path;

use gen4::Gen4;
pub use layout::Layout;
use scene::{Scene, OUT_H, OUT_W};
use screen::{Screens, BOTTOM, H, PIXELS, TOP, W};
use watch::Watch;

/// The DS's buttons as libretro numbers them in a joypad's mask, and the two the handheld adds.
pub mod keys {
    pub const B: u16 = 1 << 0;
    pub const Y: u16 = 1 << 1;
    pub const SELECT: u16 = 1 << 2;
    pub const START: u16 = 1 << 3;
    pub const UP: u16 = 1 << 4;
    pub const DOWN: u16 = 1 << 5;
    pub const LEFT: u16 = 1 << 6;
    pub const RIGHT: u16 = 1 << 7;
    pub const A: u16 = 1 << 8;
    pub const X: u16 = 1 << 9;
    pub const L: u16 = 1 << 10;
    pub const R: u16 = 1 << 11;
    /// The touch cursor, while it is on.
    pub const L2: u16 = 1 << 12;
    /// Never the game's: melonDS's core swaps its screens on it.
    pub const R2: u16 = 1 << 13;
}

/// A game with layouts of its own, and the functions it is told apart by.
struct Game {
    watch: Watch,
    gen4: Gen4,
}

/// The cursor on the touch screen, in its pixels.
struct Cursor {
    x: f32,
    y: f32,
    /// How many frames the D-pad has been held moving it.
    moving: u32,
    /// Whether it is on (L2), and whether A is touching where it is.
    on: bool,
    touching: bool,
}

/// How fast the cursor moves, in touch-screen pixels a frame: slow at first, for aiming, then
/// faster the longer the D-pad is held.
const CURSOR_SLOW: f32 = 1.0;
const CURSOR_FAST: f32 = 4.0;
const CURSOR_RAMP: f32 = 24.0;

/// How long a screen must stay blank before Focus shows the other, in frames: a flash or a fade
/// is shorter.
const FOCUS_HOLD: u32 = 45;

/// A DS game's screens, from the frames its core draws to the picture shown.
pub struct Ds {
    layout: Layout,
    swapped: bool,
    game: Option<Game>,
    cursor: Cursor,
    frame: u32,
    /// Focus's choice: the touch screen while only it has anything on it, and how long the
    /// other choice has stood.
    focus_bottom: bool,
    focus_pending: u32,
    /// The frame shown last, for the input of the next; and the one drawn into.
    work: Vec<u8>,
    view: Vec<u32>,
    scene: &'static str,
}

impl Ds {
    /// The screens of the DS game at `rom`, whose header's game code is `code`, laid out `layout`.
    pub fn new(rom: &Path, code: &str, layout: Layout) -> Ds {
        let game = gen4::Game::of(code).map(|game| {
            let watch = Watch::new(watch::load(game.table(), rom));
            let gen4 = Gen4::new(game, &watch);
            Game { watch, gen4 }
        });
        Ds {
            layout,
            swapped: false,
            game,
            cursor: Cursor {
                x: W as f32 / 2.0,
                y: H as f32 / 2.0,
                moving: 0,
                on: false,
                touching: false,
            },
            frame: 0,
            focus_bottom: false,
            focus_pending: 0,
            work: Vec::new(),
            view: vec![0; OUT_W * OUT_H],
            scene: "",
        }
    }

    /// Whether the game has layouts of its own for One Screen.
    pub fn has_layouts(&self) -> bool {
        self.game.is_some()
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    pub fn set_layout(&mut self, layout: Layout) {
        self.layout = layout;
    }

    /// The other screen made the large one (SELECT+Y), or back.
    pub fn swap(&mut self) {
        self.swapped = !self.swapped;
    }

    pub fn swapped(&self) -> bool {
        self.swapped
    }

    /// What the view shows, as its layouts name it: `battle-moves`, `top`, `stacked`...
    pub fn scene(&self) -> &'static str {
        self.scene
    }

    /// The functions the core is to watch, and where it marks their entries: none for a game
    /// with no layouts.
    pub fn watch(&mut self) -> Option<(&[u8], &mut [u8], u32, u32)> {
        self.game.as_mut().map(|g| g.watch.for_core())
    }

    /// Forgets what ran before: after a state is loaded or the game reset.
    pub fn forget(&mut self) {
        if let Some(g) = &mut self.game {
            g.watch.forget();
        }
    }

    /// The keys the game is handed for the next frame, from the buttons held (libretro's joypad
    /// mask, L2 the cursor), and where the touch screen is touched, as libretro's pointer has it
    /// across the core's frame of both screens. `shown` is the frame the core drew last, of
    /// `size` (both screens, XRGB8888, still the core's own until it runs the next), which a
    /// game's layout reads its buttons from.
    pub fn before_frame(
        &mut self,
        held: u16,
        shown: &[u8],
        size: (u32, u32),
    ) -> (u16, Option<(i16, i16)>) {
        let mut keys = held & !(keys::L2 | keys::R2);
        self.cursor.on = held & keys::L2 != 0;
        let mut touch = None;
        if self.cursor.on {
            let dpad = keys::LEFT | keys::RIGHT | keys::UP | keys::DOWN;
            let c = &mut self.cursor;
            c.moving = if held & dpad != 0 { c.moving + 1 } else { 0 };
            let speed = CURSOR_SLOW
                + (CURSOR_FAST - CURSOR_SLOW) * (c.moving as f32 / CURSOR_RAMP).min(1.0);
            let dir = |neg: u16, pos: u16| {
                (held & pos != 0) as i32 as f32 - (held & neg != 0) as i32 as f32
            };
            c.x = (c.x + dir(keys::LEFT, keys::RIGHT) * speed).clamp(0.0, W as f32 - 1.0);
            c.y = (c.y + dir(keys::UP, keys::DOWN) * speed).clamp(0.0, H as f32 - 1.0);
            c.touching = held & keys::A != 0;
            if c.touching {
                touch = Some((c.x as i32, c.y as i32));
            }
            keys &= !(dpad | keys::A);
        } else {
            self.cursor.touching = false;
            if self.layout == Layout::OneScreen && !self.swapped {
                if let Some(g) = &mut self.game {
                    let shown = Screens::new(shown, size, [&[], &[]]);
                    let (k, t) = g.gen4.input(self.frame, keys, &g.watch, shown.as_ref());
                    keys = k;
                    touch = t;
                }
            }
        }
        (keys, touch.map(pointer))
    }

    /// Counts a frame the core ran, `main_ram` the DS's main memory after it: every frame run,
    /// shown or not, as the functions the game entered in it are read here.
    pub fn ran(&mut self, main_ram: &[u8]) {
        self.frame += 1;
        if let Some(g) = &mut self.game {
            g.watch.frame_done(main_ram);
        }
    }

    /// The picture for the frame the core drew last, `frame` of `size` (both screens,
    /// XRGB8888), with `layers` the layer each pixel came from, written into `out` as XRGB8888:
    /// its size, or none for a frame that is not both screens.
    pub fn view(
        &mut self,
        frame: &[u8],
        size: (u32, u32),
        layers: [&[u8]; 2],
        out: &mut Vec<u8>,
    ) -> Option<(u32, u32)> {
        Screens::new(frame, size, layers)?;
        // without the cursor the frame is laid out as it is; with it, from a copy it is drawn in
        if !self.cursor.on {
            return Screens::new(frame, size, layers).map(|s| self.show(&s, out));
        }
        let mut work = std::mem::take(&mut self.work);
        work.clear();
        work.extend_from_slice(frame);
        draw_cursor(
            &mut work,
            self.cursor.x,
            self.cursor.y,
            self.cursor.touching,
        );
        let made = Screens::new(&work, size, layers).map(|s| self.show(&s, out));
        self.work = work;
        made
    }

    /// Lays the screens `s` out into `out` as the layout says, the cursor drawn in them.
    fn show(&mut self, s: &Screens, out: &mut Vec<u8>) -> (u32, u32) {
        self.update_focus(s);
        let shown = if self.swapped {
            1 - self.focused()
        } else {
            self.focused()
        };
        let cursor_shows_touch = self.cursor.on
            && matches!(
                self.layout,
                Layout::OneScreen | Layout::Focus | Layout::Picture
            );
        out.clear();
        match self.layout {
            _ if cursor_shows_touch => {
                self.scene = "cursor";
                one(s, BOTTOM, out)
            }
            Layout::OneScreen if self.game.is_some() && !self.swapped => {
                let scene = match &mut self.game {
                    Some(g) => g.gen4.scene(self.frame, &g.watch, s),
                    None => Scene::only("top", TOP),
                };
                self.scene = scene.name;
                self.draw(&scene, s, out)
            }
            Layout::OneScreen | Layout::Focus => {
                self.scene = if shown == TOP { "top" } else { "bottom" };
                one(s, shown, out)
            }
            Layout::Picture => {
                self.scene = "picture";
                let scene = layout::picture(if self.swapped { BOTTOM } else { TOP });
                self.draw(&scene, s, out)
            }
            Layout::Stacked => {
                self.scene = "stacked";
                let (first, second) = if self.swapped {
                    (BOTTOM, TOP)
                } else {
                    (TOP, BOTTOM)
                };
                out.extend_from_slice(s.rows(first));
                out.extend_from_slice(s.rows(second));
                (W as u32, 2 * H as u32)
            }
            Layout::SideBySide => {
                self.scene = "side-by-side";
                let (left, right) = if self.swapped {
                    (BOTTOM, TOP)
                } else {
                    (TOP, BOTTOM)
                };
                let (l, r) = (s.rows(left), s.rows(right));
                for y in 0..H {
                    out.extend_from_slice(&l[y * W * 4..(y + 1) * W * 4]);
                    out.extend_from_slice(&r[y * W * 4..(y + 1) * W * 4]);
                }
                (2 * W as u32, H as u32)
            }
        }
    }

    /// The screen Focus shows: the top one, but while it has stayed blank and the touch screen
    /// has not.
    fn focused(&self) -> usize {
        if self.focus_bottom {
            BOTTOM
        } else {
            TOP
        }
    }

    fn update_focus(&mut self, s: &Screens) {
        let (top_blank, bottom_blank) = (s.blank(TOP), s.blank(BOTTOM));
        let want = match (top_blank, bottom_blank) {
            (true, true) => self.focus_bottom,
            (true, false) => true,
            _ => false,
        };
        if want == self.focus_bottom {
            self.focus_pending = 0;
            return;
        }
        self.focus_pending += 1;
        let hold = if want { FOCUS_HOLD } else { 1 };
        if self.focus_pending >= hold {
            self.focus_bottom = want;
            self.focus_pending = 0;
        }
    }

    fn draw(&mut self, scene: &Scene, s: &Screens, out: &mut Vec<u8>) -> (u32, u32) {
        // one screen whole, as most of a game is: that screen at its own size, copied
        if let Some(screen) = scene.whole_screen() {
            return one(s, screen, out);
        }
        scene.draw(s, &mut self.view);
        out.reserve(OUT_W * OUT_H * 4);
        for &c in &self.view {
            out.extend_from_slice(&c.to_le_bytes());
        }
        (OUT_W as u32, OUT_H as u32)
    }
}

/// `screen` alone, at its own size.
fn one(s: &Screens, screen: usize, out: &mut Vec<u8>) -> (u32, u32) {
    out.extend_from_slice(s.rows(screen));
    (W as u32, H as u32)
}

/// A touch at `x, y` of the touch screen, as libretro's pointer has it across the core's frame,
/// the touch screen its lower half.
pub fn pointer((x, y): (i32, i32)) -> (i16, i16) {
    let across =
        |v: i32, len: i32| ((v * 2 + 1) * 0x8000 / len - 0x8000).clamp(-0x8000, 0x7FFF) as i16;
    (
        across(x.clamp(0, W as i32 - 1), W as i32),
        across(H as i32 + y.clamp(0, H as i32 - 1), 2 * H as i32),
    )
}

/// The cursor, an arrow with a dark rim, its tip at `x, y` of the touch screen in `frame`; its
/// fill pale while A touches.
const ARROW: [&str; 12] = [
    "X..........",
    "XX.........",
    "XOX........",
    "XOOX.......",
    "XOOOX......",
    "XOOOOX.....",
    "XOOOOOX....",
    "XOOOOOOX...",
    "XOOOOXXXX..",
    "XOOXOX.....",
    "XOX.XOX....",
    "XX...XX....",
];

fn draw_cursor(frame: &mut [u8], x: f32, y: f32, touching: bool) {
    let fill: [u8; 3] = if touching {
        [0x60, 0xd0, 0xff]
    } else {
        [0xff, 0xff, 0xff]
    };
    let (cx, cy) = (x as i32, y as i32);
    for (dy, row) in ARROW.iter().enumerate() {
        for (dx, ch) in row.bytes().enumerate() {
            let c = match ch {
                b'X' => [0x10, 0x10, 0x18],
                b'O' => fill,
                _ => continue,
            };
            let (px, py) = (cx + dx as i32, cy + dy as i32);
            if !(0..W as i32).contains(&px) || !(0..H as i32).contains(&py) {
                continue;
            }
            let i = (PIXELS + py as usize * W + px as usize) * 4;
            frame[i..i + 3].copy_from_slice(&[c[2], c[1], c[0]]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::tests::frame;

    fn ds(layout: Layout) -> Ds {
        Ds::new(Path::new("/nowhere.nds"), "ABCD", layout)
    }

    fn run(d: &mut Ds, f: &[u8]) -> ((u32, u32), Vec<u8>) {
        let mut out = Vec::new();
        d.ran(&[]);
        let size = d.view(f, (256, 384), [&[], &[]], &mut out).unwrap();
        (size, out)
    }

    #[test]
    fn a_game_with_no_layouts_shows_the_top_screen_but_while_only_the_touch_screen_is_lit() {
        let mut d = ds(Layout::OneScreen);
        assert!(!d.has_layouts());
        let lit = frame(0x123456, 0x654321);
        let (size, out) = run(&mut d, &lit);
        assert_eq!(size, (256, 192));
        assert_eq!(&out[..3], &[0x56, 0x34, 0x12], "the top screen");
        // the top screen dark: the touch screen once that has lasted
        let mut dark_top = frame(0, 0x654321);
        // (one pixel apart: a touch screen with something on it, not one colour)
        dark_top[PIXELS * 4] = 0xff;
        for _ in 0..FOCUS_HOLD {
            run(&mut d, &dark_top);
        }
        assert_eq!(d.scene(), "bottom");
        d.swap();
        run(&mut d, &dark_top);
        assert_eq!(d.scene(), "top", "SELECT+Y shows the other");
    }

    #[test]
    fn stacked_and_side_by_side_show_both_screens() {
        let f = frame(0x111111, 0x222222);
        let mut d = ds(Layout::Stacked);
        let (size, out) = run(&mut d, &f);
        assert_eq!(size, (256, 384));
        assert_eq!(out, f);
        d.set_layout(Layout::SideBySide);
        let (size, out) = run(&mut d, &f);
        assert_eq!(size, (512, 192));
        assert_eq!(
            (out[0], out[256 * 4]),
            (0x11, 0x22),
            "the top screen left, the touch screen right"
        );
    }

    #[test]
    fn picture_in_picture_puts_the_other_screen_in_the_corner() {
        let f = frame(0x111111, 0x222222);
        let mut d = ds(Layout::Picture);
        let (size, out) = run(&mut d, &f);
        assert_eq!(size, (OUT_W as u32, OUT_H as u32));
        let at = |x: usize, y: usize| out[(y * OUT_W + x) * 4];
        assert_eq!(at(10, 10), 0x11);
        assert_eq!(at(OUT_W - 20, OUT_H - 20), 0x22);
    }

    #[test]
    fn l2_makes_the_dpad_a_cursor_and_a_a_touch() {
        let mut d = ds(Layout::OneScreen);
        let (keys, touch) = d.before_frame(keys::L2 | keys::RIGHT | keys::A | keys::B, &[], (0, 0));
        assert_eq!(keys, keys::B, "the D-pad, A and L2 are the cursor's");
        let (x, y) = touch.unwrap();
        assert!(x > 0 && y > 0x4000, "on the lower half: {x},{y}");
        let f = frame(0x111111, 0x222222);
        run(&mut d, &f);
        assert_eq!(
            d.scene(),
            "cursor",
            "the touch screen shows while the cursor is on"
        );
        let (keys, touch) = d.before_frame(keys::A | keys::R2, &f, (256, 384));
        assert_eq!((keys, touch), (keys::A, None), "R2 never reaches the game");
    }

    #[test]
    fn a_touch_is_handed_over_as_libretro_points_at_the_lower_half() {
        let (x, y) = pointer((0, 0));
        assert_eq!(x, -0x8000 + 0x80, "the middle of the first column");
        assert!(
            (0..0x100).contains(&y),
            "the touch screen's top row, just under the middle: {y}"
        );
        let (x, y) = pointer((255, 191));
        assert!(x > 0x7F00 && y > 0x7F00);
    }
}
