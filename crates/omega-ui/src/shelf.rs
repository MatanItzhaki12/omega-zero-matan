use omega_gfx::{out_w, Draw, MeshId, TexId, FOCAL};
use omega_store::{Cart, Platform};

use crate::cart::{cart_box, label_colour, label_text, model_box, model_of, CART_H, CART_W};
use crate::chrome::BAR_H;
use crate::hud::Millis;
use crate::models::disc::{self, DiscStack};
use crate::models::{snes, umd};
use crate::motion::ease;
use crate::silhouette::GbShell;
use crate::spin::{self, CartMeshes, CartModel, Pose};
use crate::stage::{stage_y, MOCKUP_H};
use crate::status_bar::TOP_BAR_H;

const SIDE_SCALE: f32 = 0.78;
const SIDE_ALPHA: f32 = 0.55;
/// How big a Game Boy pak stands on the shelf, as a share of its own size: at its own size it
/// would stand too tall beside a GBA cart. It keeps this size on its way into the handheld.
pub const GB_SHELF_SCALE: f32 = 0.8;

/// How big a cart for `platform` stands on the shelf, as a share of its own size.
pub fn shelf_scale(platform: Platform) -> f32 {
    match platform {
        Platform::Gba => 1.0,
        Platform::Gb | Platform::Gbc => GB_SHELF_SCALE,
        Platform::Snes => snes::SHELF_SCALE,
        Platform::Ps1 => disc::SHELF_SCALE,
        Platform::Psp => umd::SHELF_SCALE,
    }
}

/// How big `model` stands on the shelf, as a share of its own size.
pub fn model_scale(model: CartModel) -> f32 {
    match model {
        CartModel::Gba => 1.0,
        CartModel::Gb(_) => GB_SHELF_SCALE,
        CartModel::Snes(_) => snes::SHELF_SCALE,
        CartModel::Disc => disc::SHELF_SCALE,
        CartModel::Umd => umd::SHELF_SCALE,
    }
}

/// The size `model` stands on the shelf at, and spaces its neighbours by.
pub fn model_shelf_box(model: CartModel) -> (f32, f32) {
    let (w, h) = model_box(model);
    let scale = model_scale(model);
    (w as f32 * scale, h as f32 * scale)
}

/// The size a cart for `platform` stands on the shelf at, and spaces its neighbours by.
pub fn shelf_box(platform: Platform) -> (f32, f32) {
    let (w, h) = cart_box(platform);
    let scale = shelf_scale(platform);
    (w as f32 * scale, h as f32 * scale)
}

/// Where a cart `h` tall stands on the shelf, between the bar across the top and the bar across
/// the bottom: a pak taller than a GBA cart is centred there, and anything
/// shorter stands on the floor a GBA cart centred there would. A pak shrunk on the shelf is
/// still taller than a GBA cart, so it is centred where it would stand at its own size.
pub fn rest_y(h: f32) -> f32 {
    let (top, floor) = (TOP_BAR_H, MOCKUP_H - BAR_H);
    let y = if h > CART_H as f32 {
        top + (floor - top - h) / 2.0
    } else {
        (top + floor + CART_H as f32) / 2.0 - h
    };
    y + stage_y()
}

pub fn foot_y(h: f32) -> f32 {
    rest_y(h) + h
}

/// How far above where it stands a cart `w` by `h` can reach as it sways: it floats up, and
/// the side that turns towards the eye grows.
pub fn sway_reach(w: f32, h: f32) -> f32 {
    let grown = FOCAL / (FOCAL - w / 2.0 * SWAY.sin());
    BOB + h / 2.0 * (grown - 1.0)
}
const OMEGA: f32 = 16.0;
const PART: f32 = 130.0;

const SLOTS: i32 = 3;

/// The selected cart sways this far either way about its upright axis, in radians, once the
/// shelf stands still...
const SWAY: f32 = 20.0 * std::f32::consts::PI / 180.0;
const SWAY_S: f32 = 3.6;
/// ...and floats this many canvas pixels up and down, out of step with the sway so the two
/// never fall into one mechanical beat.
const BOB: f32 = 4.0;
const BOB_S: f32 = 2.4;
/// Both periods divide this, so the clock that drives them wraps here without a jump.
const LOOP_S: f32 = 7.2;
/// How long the cart picked takes to turn to the side it came in from: eased, so the turn
/// starts and ends gently rather than as fast as the row slides.
const ARRIVE_S: f32 = 0.35;
/// How fast the sway comes in once the shelf stops, and goes once it moves, per second.
const CALM_IN: f32 = 1.6;
const CALM_OUT: f32 = 8.0;
/// Carts lean into a scroll this many radians per cart a second, up to LEAN_MAX, and stand
/// straight once the scroll is slower than LEAN_FROM carts a second.
const LEAN: f32 = 0.045;
const LEAN_MAX: f32 = 0.35;
const LEAN_FROM: f32 = 0.05;
/// A turn or a lift smaller than these cannot be seen, so the cart is drawn square and level.
const STILL_YAW: f32 = 0.002;

/// How far from the middle, in carts, the holo sheen has faded out: only the cart picked has it.
const HOLO_REACH: f32 = 0.25;
const STILL_LIFT: f32 = 0.05;

const REPEAT_DELAY_MS: Millis = 400;
const REPEAT_MS: [Millis; 4] = [110, 85, 65, 50];

pub struct Shelf {
    pub carts: Vec<Cart>,
    pub index: usize,
    pub scroll: f32,
    faces: Vec<TexId>,
    shadows: [Option<TexId>; CartModel::ALL.len()],
    /// Each model's holo sheen, its frames with the light from one end of the label to the
    /// other (`cart::holo_frames`).
    holo: [Vec<TexId>; CartModel::ALL.len()],
    /// Whether the cart picked has its holo sheen: Labels > Holo Sheen.
    holo_shown: bool,
    /// Whether the cart last asked about has a clear or glitter shell, and so the sheen: kept
    /// for it by its ROM and shell, since a Game Boy cart's is read from its ROM's header.
    clear: std::cell::RefCell<Option<(std::path::PathBuf, Option<omega_store::ShellChoice>, bool)>>,
    models: Vec<CartModel>,
    /// How many discs each cart stands as: one, but for a game on several discs.
    discs: Vec<usize>,
    meshes: Option<CartMeshes>,
    ride: f32,
    vel: f32,
    held: Option<(i32, Millis, usize)>,
    calm: f32,
    /// The side the cart picked came in from, for its sway to start there: 1 from the right,
    /// whose right side it turns to the back, -1 from the left, and 0 once the sway has taken
    /// over, or while the cart is held still.
    arrive: f32,
    /// Seconds since the cart picked came in, for its turn to ease in over `ARRIVE_S`.
    arrived: f32,
    age: f32,
    /// How far the discs have turned about their own middles, in radians.
    spin: f32,
    still: bool,
    /// Which carts wear the favorite's star, by place on the row.
    marks: Vec<bool>,
    /// The star, once uploaded.
    mark: Option<TexId>,
}

impl Shelf {
    pub fn new(carts: Vec<Cart>) -> Self {
        Shelf {
            models: carts.iter().map(model_of).collect(),
            discs: carts.iter().map(disc::stack_of).collect(),
            carts,
            index: 0,
            scroll: 0.0,
            faces: Vec::new(),
            shadows: [None; CartModel::ALL.len()],
            holo: Default::default(),
            holo_shown: true,
            clear: Default::default(),
            meshes: None,
            ride: 0.0,
            vel: 0.0,
            held: None,
            calm: 0.0,
            arrive: 0.0,
            arrived: 0.0,
            age: 0.0,
            spin: 0.0,
            still: false,
            marks: Vec::new(),
            mark: None,
        }
    }

    pub fn select(&mut self, i: usize) {
        self.index = i;
        self.scroll = i as f32;
        self.ride = i as f32;
        self.vel = 0.0;
    }

    pub fn set_shadow(&mut self, face: TexId) {
        self.set_model_shadow(CartModel::Gba, face);
    }

    /// The black outline that backs a `model` cart not drawn as a model.
    pub fn set_model_shadow(&mut self, model: CartModel, face: TexId) {
        self.shadows[model.index()] = Some(face);
    }

    /// `model`'s holo sheen, its frames in order of the light along the label.
    pub fn set_model_holo(&mut self, model: CartModel, frames: Vec<TexId>) {
        self.holo[model.index()] = frames;
    }

    /// Shows the holo sheen over the cart picked, or none.
    pub fn set_holo_shown(&mut self, shown: bool) {
        self.holo_shown = shown;
    }

    /// The frame of `model`'s holo sheen a cart turned `yaw`, `offset` from the middle, shows,
    /// and how much of it: the light along its label as far as the cart is turned, end to end
    /// over the sway's reach, so it slides over the label as the cart turns one way and back.
    /// Only the cart picked has it, fading in as it comes to the middle: one more face drawn
    /// over one cart, not over every cart in sight, and none while the row scrolls past.
    fn holo_at(
        &self,
        cart: &Cart,
        model: CartModel,
        yaw: f32,
        offset: f32,
    ) -> Option<(TexId, f32)> {
        let fade = 1.0 - offset.abs() / HOLO_REACH;
        if !self.holo_shown || fade <= 0.0 || !self.shines(cart, model) {
            return None;
        }
        let frames = &self.holo[model.index()];
        let last = frames.len().checked_sub(1)?;
        let t = (0.5 + yaw / (2.0 * SWAY)).clamp(0.0, 1.0);
        let tex = frames.get((t * last as f32).round() as usize).copied()?;
        Some((tex, fade))
    }

    /// Whether `cart`'s label has the holo sheen: a UMD's always, as its label is under the
    /// disc's clear window whatever its shell; a cartridge's in a clear or glitter shell, a
    /// solid one's having none.
    fn shines(&self, cart: &Cart, model: CartModel) -> bool {
        model == CartModel::Umd || self.clear_shell(cart)
    }

    /// Whether `cart`'s shell is clear or glitter plastic.
    fn clear_shell(&self, cart: &Cart) -> bool {
        let mut kept = self.clear.borrow_mut();
        if let Some((rom, shell, clear)) = kept.as_ref() {
            if *rom == cart.rom && *shell == cart.shell {
                return *clear;
            }
        }
        let clear = crate::shell::shell_for(cart).finish != crate::shell::Finish::Solid;
        *kept = Some((cart.rom.clone(), cart.shell, clear));
        clear
    }

    /// The models carts are drawn with from now on, sides and all.
    pub fn set_meshes(&mut self, meshes: CartMeshes) {
        self.meshes = Some(meshes);
    }

    pub fn set_gb_shadow(&mut self, shell: GbShell, face: TexId) {
        self.set_model_shadow(CartModel::Gb(shell), face);
    }

    /// Gives the cart `stem` the shell `shell` (none for its own), and the model that shell's
    /// shape is drawn with: the cart editor's choice, shown as it is made. Its face is drawn
    /// again apart from this, as it is for a label. False where the shelf has no such cart.
    pub fn set_shell(&mut self, stem: &str, shell: Option<omega_store::ShellChoice>) -> bool {
        let Some(i) = self.carts.iter().position(|c| c.stem == stem) else {
            return false;
        };
        self.carts[i].shell = shell;
        if let Some(model) = self.models.get_mut(i) {
            *model = model_of(&self.carts[i]);
        }
        true
    }

    pub fn set_faces(&mut self, faces: Vec<TexId>) {
        self.faces = faces;
    }

    /// The carts that wear the favorite's star, by place on the row.
    pub fn set_marks(&mut self, marks: Vec<bool>) {
        self.marks = marks;
    }

    /// The star favorites wear, uploaded.
    pub fn set_mark(&mut self, mark: TexId) {
        self.mark = Some(mark);
    }

    /// Puts `carts` on the row in place of what was there, each drawn on its face, the first
    /// picked: a RECENTS shelf as a game goes in. Faces are kept only when every cart has one.
    pub fn replace(&mut self, carts: Vec<(Cart, Option<TexId>)>) {
        let faces: Option<Vec<TexId>> = carts.iter().map(|(_, f)| *f).collect();
        self.faces = faces.unwrap_or_default();
        self.models = carts.iter().map(|(c, _)| model_of(c)).collect();
        self.discs = carts.iter().map(|(c, _)| disc::stack_of(c)).collect();
        self.carts = carts.into_iter().map(|(c, _)| c).collect();
        self.marks.clear();
        self.held = None;
        self.select(0);
    }

    /// The favorite's star over the cart at `i`, drawn `(x, y, w, h)`, at its top right corner.
    fn push_mark(&self, i: usize, at: (f32, f32, f32, f32), alpha: f32, out: &mut Vec<Draw>) {
        let (Some(tex), true) = (self.mark, self.marks.get(i).copied().unwrap_or(false)) else {
            return;
        };
        let (x, y, w, _) = at;
        let side = crate::STAR_PX * (w / CART_W as f32).clamp(0.5, 1.0);
        out.push(Draw::Tex {
            x: (x + w - side * 0.8).round(),
            y: (y - side * 0.3).round(),
            w: side.round(),
            h: side.round(),
            tex,
            alpha,
        });
    }

    /// Puts `cart` on the row at `at`, drawn on `face`, as a favorite starred on another shelf
    /// is. The cart picked stays the one picked.
    pub fn insert(&mut self, at: usize, cart: Cart, face: Option<TexId>) {
        let at = at.min(self.carts.len());
        if let Some(face) = face.filter(|_| self.faces.len() >= at) {
            self.faces.insert(at, face);
        }
        if self.marks.len() >= at {
            self.marks.insert(at, false);
        }
        self.models
            .insert(at.min(self.models.len()), model_of(&cart));
        self.discs
            .insert(at.min(self.discs.len()), disc::stack_of(&cart));
        self.carts.insert(at, cart);
        if self.carts.len() > 1 && at <= self.index {
            self.select(self.index + 1);
        }
    }

    /// Takes the cart at `at` off the row, as a favorite unstarred is. The cart after it, or
    /// before it at the end, is picked in its place where it was the one picked.
    pub fn remove(&mut self, at: usize) -> Option<Cart> {
        if at >= self.carts.len() {
            return None;
        }
        if at < self.faces.len() {
            self.faces.remove(at);
        }
        if at < self.models.len() {
            self.models.remove(at);
        }
        if at < self.discs.len() {
            self.discs.remove(at);
        }
        if at < self.marks.len() {
            self.marks.remove(at);
        }
        let cart = self.carts.remove(at);
        self.held = None;
        let index = match self.index {
            i if i > at => i - 1,
            i => i.min(self.carts.len().saturating_sub(1)),
        };
        self.select(index);
        Some(cart)
    }

    /// The model the cart `stem` is drawn with, once models are set: a game on several discs
    /// all its discs in one model.
    pub fn mesh_for(&self, stem: &str) -> Option<MeshId> {
        let i = self.carts.iter().position(|c| c.stem == stem)?;
        let meshes = self.meshes?;
        let model = *self.models.get(i)?;
        match (model, meshes.stacks()) {
            (CartModel::Disc, Some(stacks)) => Some(stacks.whole(self.discs_at(i))),
            _ => Some(meshes.of(model)),
        }
    }

    /// The discs the game `stem` stands as, once its face and the models are set; `None` for
    /// a cart.
    pub fn stack_for(&self, stem: &str) -> Option<DiscStack> {
        let i = self.carts.iter().position(|c| c.stem == stem)?;
        self.stack_at(i)
    }

    /// The discs the game at `i` stands as, once its face and the models are set.
    pub fn stack_at(&self, i: usize) -> Option<DiscStack> {
        if self.model_at(i)? != CartModel::Disc {
            return None;
        }
        Some(DiscStack {
            tex: *self.faces.get(i)?,
            discs: self.discs_at(i),
            meshes: self.meshes?.stacks()?,
        })
    }

    /// How many discs the cart at `i` stands as.
    fn discs_at(&self, i: usize) -> usize {
        match self.discs.get(i) {
            Some(n) => *n,
            // a cart pushed onto the row after it was made has not been counted
            None => self.carts.get(i).map_or(1, disc::stack_of),
        }
    }

    /// The model the cart at `i` stands as.
    pub fn model_at(&self, i: usize) -> Option<CartModel> {
        self.models.get(i).copied()
    }

    pub fn find(&self, stem: &str) -> Option<(&Cart, Option<TexId>)> {
        let i = self.carts.iter().position(|c| c.stem == stem)?;
        Some((&self.carts[i], self.faces.get(i).copied()))
    }

    pub fn left(&mut self) {
        self.step(-1);
    }

    pub fn right(&mut self) {
        self.step(1);
    }

    pub fn jump_next_letter(&mut self) {
        self.jump(1);
    }

    pub fn jump_prev_letter(&mut self) {
        self.jump(-1);
    }

    fn start_of_letter(&self, from: usize) -> usize {
        let n = self.carts.len();
        let letter = omega_store::initial(&self.carts[from].stem);
        let mut at = from;
        for _ in 0..n {
            let before = (at as i32 - 1).rem_euclid(n as i32) as usize;
            if omega_store::initial(&self.carts[before].stem) != letter {
                break;
            }
            at = before;
        }
        at
    }

    fn jump(&mut self, dir: i32) {
        let n = self.carts.len();
        if n < 2 {
            return;
        }
        let wrap = |i: i32| i.rem_euclid(n as i32) as usize;
        let here = omega_store::initial(&self.carts[self.index].stem);
        if self
            .carts
            .iter()
            .all(|c| omega_store::initial(&c.stem) == here)
        {
            return;
        }
        let target = match dir > 0 {
            true => {
                let mut at = self.index;
                for _ in 0..n {
                    at = wrap(at as i32 + 1);
                    if omega_store::initial(&self.carts[at].stem) != here {
                        break;
                    }
                }
                at
            }
            false => {
                let start = self.start_of_letter(self.index);
                match start == self.index {
                    true => self.start_of_letter(wrap(start as i32 - 1)),
                    false => start,
                }
            }
        };
        let ahead = (target as i32 - self.index as i32).rem_euclid(n as i32);
        let delta = match dir > 0 {
            true => ahead,
            false => ahead - n as i32,
        };
        self.index = target;
        self.ride += delta as f32;
    }

    /// Picks the cart at `target`, the shelf sliding to it the shorter way round, as a jump
    /// to a letter does.
    pub fn glide_to(&mut self, target: usize) {
        let n = self.carts.len() as i32;
        if target as i32 >= n || target == self.index {
            return;
        }
        let ahead = (target as i32 - self.index as i32).rem_euclid(n);
        let delta = if ahead <= n / 2 { ahead } else { ahead - n };
        self.index = target;
        self.ride += delta as f32;
    }

    pub fn hold_left(&mut self, now: Millis) {
        self.hold(-1, now);
    }

    pub fn hold_right(&mut self, now: Millis) {
        self.hold(1, now);
    }

    fn hold(&mut self, by: i32, now: Millis) {
        self.step(by);
        self.held = Some((by, now + REPEAT_DELAY_MS, 0));
    }

    pub fn release_left(&mut self) {
        self.release(-1);
    }

    pub fn release_right(&mut self) {
        self.release(1);
    }

    fn release(&mut self, by: i32) {
        if matches!(self.held, Some((held, _, _)) if held == by) {
            self.held = None;
        }
    }

    pub fn release_hold(&mut self) {
        self.held = None;
    }

    pub fn tick(&mut self, now: Millis) {
        let Some((by, due, fired)) = self.held else {
            return;
        };
        if now < due {
            return;
        }
        self.step(by);
        let rate = REPEAT_MS[fired.min(REPEAT_MS.len() - 1)];
        self.held = Some((by, now + rate, fired + 1));
    }

    fn step(&mut self, by: i32) {
        let n = self.carts.len();
        if n < 2 {
            return;
        }
        self.index = (self.index as i32 + by).rem_euclid(n as i32) as usize;
        self.ride += by as f32;
        self.arrive = by.signum() as f32;
        self.arrived = 0.0;
    }

    pub fn scroll_target(&self) -> f32 {
        let n = self.carts.len();
        if n == 0 {
            return 0.0;
        }
        let from = self.ride;
        let n = n as f32;
        from + (self.index as f32 - from + n / 2.0).rem_euclid(n) - n / 2.0
    }

    /// How far apart the carts in view stand: the widest of them, so none reaches into the
    /// next. A row of one system's carts, all one width, stands as it always did.
    fn pitch(&self) -> f32 {
        (-SLOTS..=SLOTS)
            .filter_map(|slot| self.cart_at_offset(slot))
            .map(|i| {
                let model = self
                    .models
                    .get(i)
                    .copied()
                    .unwrap_or_else(|| model_of(&self.carts[i]));
                model_shelf_box(model).0
            })
            .fold(0.0, f32::max)
    }

    pub fn cart_at_offset(&self, off: i32) -> Option<usize> {
        let n = self.carts.len() as i32;
        if n == 0 {
            return None;
        }
        if n == 1 {
            return (off == 0).then_some(self.index);
        }
        Some((self.index as i32 + off).rem_euclid(n) as usize)
    }

    /// Where the selected cart's left edge is on the row right now, and how big it is drawn
    /// there as a share of its own size.
    pub fn selected_at(&self) -> (f32, f32) {
        let model = self.model_at(self.index).unwrap_or(CartModel::Gba);
        let w = model_shelf_box(model).0;
        let offset = self.scroll_target() - self.scroll;
        let scale = shrink(offset);
        (
            out_w() as f32 / 2.0 + offset * self.pitch() - w * scale / 2.0,
            scale * model_scale(model),
        )
    }

    /// How the selected cart is held right now, wherever it is in the row.
    pub fn pose(&self) -> Pose {
        self.pose_at(self.scroll_target() - self.scroll, true)
    }

    /// How a cart `offset` places from the middle of the row is held: the one in the middle
    /// sways and floats while the shelf stands still, and every cart leans into a scroll.
    ///
    /// The cart picked turns as it comes to the middle, to where its sway reaches on the side it
    /// came in from, and its sway starts there (`calm_down`): from the right, its right side to
    /// the back; from the left, its left. It does not lean into the scroll as the others do, so it
    /// turns one way, smoothly, from the scroll into the sway.
    fn pose_at(&self, offset: f32, picked: bool) -> Pose {
        let near = 1.0 - offset.abs().min(1.0);
        let calm = ease(self.calm);
        let lean = match self.vel.abs() < LEAN_FROM {
            true => 0.0,
            false => (-self.vel * LEAN).clamp(-LEAN_MAX, LEAN_MAX),
        };
        let wave = |period: f32| (self.age * std::f32::consts::TAU / period).sin();
        let seen = |v: f32, least: f32| if v.abs() < least { 0.0 } else { v };
        // the cart picked: turned to the sway's reach on the side it came in from, until the
        // sway takes over; one leaving the middle only lets its sway go
        let arrive = match picked {
            true => self.arrive * ease((self.arrived / ARRIVE_S).min(1.0)),
            false => 0.0,
        };
        let turned = (1.0 - calm) * arrive + calm * wave(SWAY_S);
        // it turns into its sway rather than leaning into the scroll first, which would set it the other way
        let lean = lean * (1.0 - self.arrive.abs() * picked as u8 as f32);
        Pose {
            yaw: seen(SWAY * near * turned + lean, STILL_YAW),
            lift: seen(BOB * calm * near * wave(BOB_S), STILL_LIFT),
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.arrived += dt;
        self.calm_down(dt);
        let accel = -2.0 * OMEGA * self.vel - OMEGA * OMEGA * (self.scroll - self.scroll_target());
        self.vel += accel * dt;
        self.scroll += self.vel * dt;
    }

    /// Keeps the selected cart from swaying while something else has it, such as the core
    /// picker: it settles square and level, and starts again once let go.
    pub fn hold_still(&mut self, still: bool) {
        self.still = still;
    }

    fn calm_down(&mut self, dt: f32) {
        let moving = (self.scroll - self.scroll_target()).abs() > 0.01 || self.vel.abs() > 0.2;
        if self.still {
            // held still, it settles square, not turned to a side
            self.arrive = 0.0;
        }
        if moving || self.still {
            self.calm = (self.calm - CALM_OUT * dt).max(0.0);
        } else {
            if self.calm == 0.0 {
                // the sway starts where the cart came in turned to: at its reach on that side
                self.age = match self.arrive {
                    a if a > 0.0 => SWAY_S / 4.0,
                    a if a < 0.0 => SWAY_S * 3.0 / 4.0,
                    _ => 0.0,
                };
            }
            self.calm = (self.calm + CALM_IN * dt).min(1.0);
            if self.calm == 1.0 {
                self.arrive = 0.0;
            }
        }
        self.age = (self.age + dt).rem_euclid(LOOP_S);
        self.spin = (self.spin + disc::SPIN * std::f32::consts::TAU * ease(self.calm) * dt)
            .rem_euclid(std::f32::consts::TAU);
    }

    /// The row, none of its carts held.
    pub fn draw(&self, shake: f32, out: &mut Vec<Draw>) {
        self.draw_row(None, shake, 0.0, 1.0, out);
    }

    pub fn draw_row(
        &self,
        hidden: Option<&str>,
        shake: f32,
        recede: f32,
        dim: f32,
        out: &mut Vec<Draw>,
    ) {
        let recede = recede.clamp(0.0, 1.0);
        let dim = dim.clamp(0.0, 1.0);
        let target = self.scroll_target();
        let pitch = self.pitch();
        // from the outside in, so each cart stands in front of the ones further out and the
        // picked one in front of all: on a row of every system's carts, as FAVORITES is, a
        // cart beside a wider one would otherwise cover it
        let outside_in = (1..=SLOTS).rev().flat_map(|d| [-d, d]).chain([0]);
        for slot in outside_in {
            let Some(i) = self.cart_at_offset(slot) else {
                continue;
            };
            let cart = &self.carts[i];
            if hidden == Some(cart.stem.as_str()) {
                continue;
            }
            let offset = target + slot as f32 - self.scroll;
            let t = offset.abs().min(1.0);
            let scale = shrink(offset);
            let alpha = (1.0 + (SIDE_ALPHA - 1.0) * t) * (1.0 - recede);
            // a cart pushed onto the row after it was made has no model looked up yet
            let model = self
                .models
                .get(i)
                .copied()
                .unwrap_or_else(|| model_of(cart));
            let (cw, ch) = model_shelf_box(model);
            let (w, h) = (cw * scale, ch * scale);
            let away = offset.signum() * (1.0 + offset.abs());
            let x = out_w() as f32 / 2.0 + offset * pitch - w / 2.0 + away * PART * recede;
            if x + w <= 0.0 || x >= out_w() as f32 || alpha <= 0.0 {
                continue;
            }
            let x = x + shake;
            let y = foot_y(ch) - h;
            let face = self.faces.get(i).copied();
            // A cart still waiting for its face stands still: only a face can turn.
            let pose = match face {
                Some(_) => self.pose_at(offset, i == self.index).toward_rest(recede),
                None => Pose::REST,
            };
            let y = y - pose.lift;
            if let (Some(tex), Some(meshes)) = (face, self.meshes) {
                // a side cart is the face dimmed over its black backing, as one opaque model
                let colour = match alpha < 1.0 {
                    true => {
                        let k = alpha * dim;
                        [k, k, k, recede_alpha(alpha)]
                    }
                    false => [1.0, 1.0, 1.0, dim],
                };
                match self.stack_at(i) {
                    // a game on several discs is each of them, one behind the other
                    Some(stack) => {
                        disc::push_stack(out, (x, y, w, h), &stack, colour, self.spin, pose.yaw);
                    }
                    None => {
                        out.push(spin::posed_cart(
                            (x, y, w, h),
                            meshes.of(model),
                            tex,
                            colour,
                            0.0,
                            pose.yaw,
                        ));
                        // the label's holo sheen over the model's face, as far along as the
                        // cart is turned
                        if let Some((holo, fade)) = self.holo_at(cart, model, pose.yaw, offset) {
                            out.push(spin::posed_face(
                                (x, y, w, h),
                                holo,
                                alpha * dim * fade,
                                0.0,
                                pose.yaw,
                            ));
                        }
                    }
                }
                self.push_mark(i, (x, y, w, h), alpha * dim, out);
                continue;
            }
            if let Some(tex) = face {
                spin::push_cart_side(
                    out,
                    (x, y, w, h),
                    tex,
                    0.0,
                    pose.yaw,
                    alpha * dim,
                    recede_alpha(alpha),
                );
            }
            if alpha < 1.0 {
                if let Some(tex) = self.shadows[model.index()] {
                    out.push(spin::posed_face(
                        (x, y, w, h),
                        tex,
                        recede_alpha(alpha),
                        0.0,
                        pose.yaw,
                    ));
                }
            }
            // the label's holo sheen over the face, as far along as the cart is turned
            let holo = face.and_then(|_| self.holo_at(cart, model, pose.yaw, offset));
            out.push(match face {
                Some(tex) => spin::posed_face((x, y, w, h), tex, alpha * dim, 0.0, pose.yaw),
                None => {
                    let c = label_colour(&label_text(cart));
                    Draw::Rect {
                        x,
                        y,
                        w,
                        h,
                        colour: [
                            c[0] as f32 / 255.0,
                            c[1] as f32 / 255.0,
                            c[2] as f32 / 255.0,
                            alpha * dim,
                        ],
                    }
                }
            });
            if let Some((tex, fade)) = holo {
                out.push(spin::posed_face(
                    (x, y, w, h),
                    tex,
                    alpha * dim * fade,
                    0.0,
                    pose.yaw,
                ));
            }
        }
    }
}

fn shrink(offset: f32) -> f32 {
    1.0 + (SIDE_SCALE - 1.0) * offset.abs().min(1.0)
}

fn recede_alpha(face_alpha: f32) -> f32 {
    (face_alpha / SIDE_ALPHA).clamp(0.0, 1.0)
}
