//! The cart editor: SELECT+START on the shelf opens the cart picked in it. The row slides to the
//! left, the cart with it, and a panel slides in on the right with what the cart's system lets
//! be changed, every change shown on the cart as it is made:
//!
//! - every cart: its **Colour** (the system's own colours first, the classic shells, then plain
//!   ones), turned with **Hue** and made lighter or darker with **Shade**;
//! - a cartridge: its **Finish**, solid, clear or glitter plastic;
//! - a Game Boy pak: its **Shape**, the notched pak or the Game Boy Color's rounded one;
//! - an SNES cart: its **Shape**, the American cart or the Super Famicom's;
//! - a PlayStation disc: its colour is the disc's own, the print a label leaves bare;
//! - and **Undo**, the look it had as the editor opened, **Default**, the cart's own look again,
//!   and **Save**.
//!
//! B puts the cart back as it was. Save keeps the choice in `Labels/cart_shell.ini`, the file
//! whose choices win, in the format Slot's Cart Studio writes, and the card's watcher draws it
//! again as it would for any change to the file.

use omega_input::{Action, Btn, Millis};
use omega_store::{Outline, Platform, ShellChoice, ShellFinish};
use omega_ui::{Draw, Finish, Shell};

use crate::app::App;
use crate::options::{OptionRow, OptionsView};
use crate::screens::lone::Lone;

/// How long the row takes to slide over and the panel in, and back.
pub const OPEN_MS: Millis = 240;

/// How wide the editor's panel is, in art pixels: half the page, beside the cart.
pub const PANEL_W: u32 = 176;

/// How big the cart picked stands with the editor open, beside its panel, as a share of its
/// size on the shelf.
pub const CART_ZOOM: f32 = 0.8;

/// The row as drawn into `drawn`, moved for the editor `open` that far: shrunk to [`CART_ZOOM`]
/// about the cart picked, and that cart taken to the middle of the room the panel leaves on the
/// left, level with the panel, both in the middle of the page between the bars.
pub fn place_row(drawn: &mut [Draw], open: f32) {
    if open <= 0.0 {
        return;
    }
    let mid_x = omega_ui::out_w() as f32 / 2.0;
    // the cart picked is what is drawn across the middle of the screen
    let (mut top, mut bottom) = (f32::MAX, f32::MIN);
    for d in drawn.iter() {
        if let Some([x, y, w, h]) = rect_of(d) {
            if x <= mid_x && x + w >= mid_x {
                top = top.min(y);
                bottom = bottom.max(y + h);
            }
        }
    }
    if top > bottom {
        return;
    }
    let cart_y = f32::midpoint(top, bottom);
    let [page_w, page_h] = omega_ui::kit::layout::page();
    let room = page_w.saturating_sub(PANEL_W + EDGE);
    let (to_x, to_y) = (
        omega_ui::kit::layout::screen_px(room) / 2.0,
        omega_ui::kit::layout::screen_px(page_h) / 2.0,
    );
    let k = 1.0 - (1.0 - CART_ZOOM) * open;
    let (dx, dy) = ((to_x - mid_x) * open, (to_y - cart_y) * open);
    for d in drawn.iter_mut() {
        *d = d.zoomed(k, mid_x, cart_y).shifted(dx, dy);
    }
}

/// Where `d` is drawn, `[x, y, w, h]` in canvas pixels; none for the game's picture.
fn rect_of(d: &Draw) -> Option<[f32; 4]> {
    match *d {
        Draw::Rect { x, y, w, h, .. }
        | Draw::Tex { x, y, w, h, .. }
        | Draw::Turned { x, y, w, h, .. }
        | Draw::Spun { x, y, w, h, .. }
        | Draw::Model { x, y, w, h, .. } => Some([x, y, w, h]),
        Draw::Game | Draw::Shot { .. } => None,
    }
}

/// How far one press of Hue turns the colour, in degrees, and one of Shade lightens or darkens
/// it, as a share of its brightness.
pub const HUE_STEP: f32 = 15.0;
pub const SHADE_STEP: f32 = 0.08;

/// A row of the editor's panel.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum EditRow {
    Shape,
    Colour,
    Hue,
    Shade,
    Finish,
    /// The look the cart had as the editor opened, back, the editor still open.
    Undo,
    /// The cart's own look, whatever the label configs say.
    Default,
    Save,
}

impl EditRow {
    pub fn label(self, platform: Platform) -> &'static str {
        match (self, platform) {
            (EditRow::Shape, _) => "Shape",
            (EditRow::Colour, Platform::Ps1) => "Disc Color",
            (EditRow::Colour, Platform::Psp) => "Case Color",
            (EditRow::Colour, _) => "Color",
            (EditRow::Hue, _) => "Hue",
            (EditRow::Shade, _) => "Shade",
            (EditRow::Finish, _) => "Finish",
            (EditRow::Undo, _) => "Undo",
            (EditRow::Default, _) => "Default",
            (EditRow::Save, _) => "Save",
        }
    }

    fn note(self, platform: Platform) -> &'static str {
        match (self, platform) {
            (EditRow::Shape, Platform::Snes) => "American or Super Famicom",
            (EditRow::Shape, _) => "Notched, or Color's rounded",
            (EditRow::Colour, Platform::Ps1) => "The disc's own print",
            (EditRow::Colour, _) => "Its system's, then plain",
            (EditRow::Hue, _) => "Turns the color round",
            (EditRow::Shade, _) => "Lighter or darker",
            (EditRow::Finish, _) => "Solid, clear or glitter",
            (EditRow::Undo, _) => "A: back to how it was",
            (EditRow::Default, _) => "A: the cart's own look",
            (EditRow::Save, _) => "A keeps it, B undoes",
        }
    }

    /// Left and Right change it.
    fn steps(self) -> bool {
        !matches!(self, EditRow::Undo | EditRow::Default | EditRow::Save)
    }
}

/// The rows `platform`'s cart has, in order.
pub fn rows(platform: Platform) -> &'static [EditRow] {
    use EditRow::{Colour, Default, Finish, Hue, Save, Shade, Shape, Undo};
    match platform {
        Platform::Gb | Platform::Gbc | Platform::Snes => {
            &[Shape, Colour, Hue, Shade, Finish, Undo, Default, Save]
        }
        Platform::Gba | Platform::Psp => &[Colour, Hue, Shade, Finish, Undo, Default, Save],
        // a disc has no shell: its colour is its print's
        Platform::Ps1 => &[Colour, Hue, Shade, Undo, Default, Save],
    }
}

/// The panel's title for `platform`'s cart.
pub fn title(platform: Platform) -> &'static str {
    match platform {
        Platform::Gba => "GBA CART",
        Platform::Gb | Platform::Gbc => "GAME PAK",
        Platform::Snes => "SNES CART",
        Platform::Psp => "UMD",
        Platform::Ps1 => "DISC",
    }
}

/// The shapes `platform`'s cart comes in, `Auto` first: the ROM's own.
fn shapes(platform: Platform) -> &'static [Outline] {
    match platform {
        Platform::Gb | Platform::Gbc => &[Outline::Auto, Outline::Notched, Outline::Rounded],
        Platform::Snes => &[Outline::Auto, Outline::Ntsc, Outline::Sfc],
        _ => &[Outline::Auto],
    }
}

fn shape_word(shape: Outline) -> &'static str {
    match shape {
        Outline::Auto => "AUTO",
        Outline::Notched => "NOTCHED",
        Outline::Rounded => "ROUNDED",
        Outline::Ntsc => "AMERICAN",
        Outline::Sfc => "SUPER FAMICOM",
    }
}

fn finish_word(finish: ShellFinish) -> &'static str {
    match finish {
        ShellFinish::Solid => "SOLID",
        ShellFinish::Clear => "CLEAR",
        ShellFinish::Glitter => "GLITTER",
    }
}

const FINISHES: [ShellFinish; 3] = [ShellFinish::Solid, ShellFinish::Clear, ShellFinish::Glitter];

/// The colours plain to every system, after its own.
const PLAIN: &[(&str, [u8; 3])] = &[
    ("Black", [0x26, 0x26, 0x2a]),
    ("White", [0xec, 0xec, 0xe8]),
    ("Red", [0xc8, 0x2a, 0x30]),
    ("Orange", [0xec, 0x7a, 0x22]),
    ("Yellow", [0xf2, 0xc8, 0x2c]),
    ("Green", [0x2f, 0x9a, 0x52]),
    ("Teal", [0x1f, 0x9a, 0x9c]),
    ("Blue", [0x2c, 0x5c, 0xc8]),
    ("Indigo", [0x45, 0x3e, 0x9a]),
    ("Purple", [0x7c, 0x3c, 0xa8]),
    ("Pink", [0xe2, 0x6a, 0xa8]),
    ("Gold", [0xc9, 0xa2, 0x3c]),
    ("Silver", [0xb4, 0xb8, 0xc0]),
];

/// `platform`'s own colours: the classic shells its carts came in, by name, with the finish
/// each had.
fn own_colours(platform: Platform) -> Vec<(&'static str, Shell)> {
    let gba = |code: &str| omega_ui::gba_shell_for(code);
    match platform {
        Platform::Gba => vec![
            ("Charcoal", omega_ui::DEFAULT_SHELL),
            ("Ruby", gba("AXV")),
            ("Sapphire", gba("AXP")),
            ("Emerald", gba("BPE")),
            ("FireRed", gba("BPR")),
            ("LeafGreen", gba("BPG")),
            ("Boktai Clear", gba("U3I")),
            ("Drill Dozer Red", gba("V49")),
        ],
        Platform::Gb | Platform::Gbc => vec![
            ("Game Boy Grey", omega_ui::DMG_SHELL),
            ("Game Boy Black", omega_ui::DUAL_MODE_SHELL),
            ("Color Clear", omega_ui::GB_CLEAR_SHELL),
        ],
        Platform::Snes => vec![
            ("SNES Grey", omega_ui::SNES_SHELL),
            ("Famicom Grey", omega_ui::SFC_SHELL),
        ],
        Platform::Psp => vec![("Smoke", omega_ui::UMD_SHELL)],
        Platform::Ps1 => vec![(
            "Black",
            Shell {
                colour: [0x1a, 0x19, 0x21],
                finish: Finish::Solid,
            },
        )],
    }
}

/// Every colour the Colour row steps through for `platform`: its own, then the plain ones, each
/// with the finish it brings (a plain colour keeps the finish there is).
fn colours(platform: Platform) -> Vec<(&'static str, [u8; 3], Option<ShellFinish>)> {
    own_colours(platform)
        .into_iter()
        .map(|(name, s)| (name, s.colour, Some(finish_of(s.finish))))
        .chain(PLAIN.iter().map(|(name, c)| (*name, *c, None)))
        .collect()
}

fn finish_of(finish: Finish) -> ShellFinish {
    match finish {
        Finish::Solid => ShellFinish::Solid,
        Finish::Translucent => ShellFinish::Clear,
        Finish::Glitter => ShellFinish::Glitter,
    }
}

/// `rgb` as hue (degrees), saturation and value, each of the last two 0 to 1.
pub fn hsv(rgb: [u8; 3]) -> (f32, f32, f32) {
    let [r, g, b] = rgb.map(|c| f32::from(c) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    (h, s, max)
}

/// The colour of hue `h` (degrees), saturation `s` and value `v`.
pub fn rgb((h, s, v): (f32, f32, f32)) -> [u8; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h.rem_euclid(360.0) / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r, g, b].map(|c| ((c + m) * 255.0).round().clamp(0.0, 255.0) as u8)
}

/// The editor while it is open: the cart, the look it had as it opened, the one shown now, the
/// row the cursor is on, and how far it is through opening or closing.
#[derive(Clone, Debug, PartialEq)]
pub struct CartEditor {
    pub platform: Platform,
    pub stem: String,
    /// The look the cart had as the editor opened, put back by B.
    pub was: Option<ShellChoice>,
    /// The look shown now: none for the cart's own.
    pub choice: Option<ShellChoice>,
    /// The cart's own look, which a first change starts from.
    pub own: ShellChoice,
    pub row: EditRow,
    pub opened: Millis,
    /// When it began to close, and whether what it shows was kept.
    pub closing: Option<Millis>,
}

impl CartEditor {
    pub fn new(cart: &omega_store::Cart, now: Millis) -> CartEditor {
        let shell = omega_ui::shell_for(&omega_store::Cart {
            shell: None,
            ..cart.clone()
        });
        CartEditor {
            platform: cart.platform,
            stem: cart.stem.clone(),
            was: cart.shell,
            choice: cart.shell,
            own: ShellChoice {
                outline: Outline::Auto,
                colour: shell.colour,
                finish: finish_of(shell.finish),
            },
            row: rows(cart.platform)[0],
            opened: now,
            closing: None,
        }
    }

    /// How far open it is, 0 to 1, eased: the row's slide and the panel's.
    pub fn openness(&self, now: Millis) -> f32 {
        let t = |since: Millis| (now.saturating_sub(since) as f32 / OPEN_MS as f32).min(1.0);
        let open = match self.closing {
            Some(at) => 1.0 - t(at),
            None => t(self.opened),
        };
        omega_ui::ease(open)
    }

    /// Closed all the way, to be dropped.
    pub fn closed(&self, now: Millis) -> bool {
        self.closing
            .is_some_and(|at| now.saturating_sub(at) >= OPEN_MS)
    }

    /// The look being changed: the one shown, or the cart's own where none is.
    fn current(&self) -> ShellChoice {
        self.choice.unwrap_or(self.own)
    }

    /// The Colour row's value: the name of the colour shown, where it is one of them.
    fn colour_name(&self) -> String {
        let colour = self.current().colour;
        colours(self.platform)
            .into_iter()
            .find(|(_, c, _)| *c == colour)
            .map_or_else(|| "CUSTOM".to_string(), |(name, ..)| name.to_uppercase())
    }

    /// What `row` shows on its right.
    pub fn value(&self, row: EditRow) -> String {
        let now = self.current();
        match row {
            EditRow::Shape => shape_word(now.outline).to_string(),
            EditRow::Colour => self.colour_name(),
            EditRow::Hue => format!("{:.0}°", hsv(now.colour).0),
            EditRow::Shade => format!("{:.0}%", hsv(now.colour).2 * 100.0),
            EditRow::Finish => finish_word(now.finish).to_string(),
            // where it stands: the look as it opened, or the cart's own
            EditRow::Undo if self.choice == self.was => "AS IT WAS".to_string(),
            EditRow::Default if self.choice.is_none() => "DEFAULT".to_string(),
            EditRow::Undo | EditRow::Default | EditRow::Save => String::new(),
        }
    }

    /// One step of `row` to the right, or the left: the look shown changed.
    pub fn step(&mut self, row: EditRow, right: bool) {
        let mut c = self.current();
        let turn = |list_len: usize, at: Option<usize>| {
            let n = list_len as isize;
            let at = at.map_or(if right { -1 } else { 0 }, |i| i as isize);
            (at + if right { 1 } else { -1 }).rem_euclid(n) as usize
        };
        match row {
            EditRow::Shape => {
                let list = shapes(self.platform);
                let at = list.iter().position(|s| *s == c.outline);
                c.outline = list[turn(list.len(), at)];
            }
            EditRow::Colour => {
                let list = colours(self.platform);
                let at = list.iter().position(|(_, rgb, _)| *rgb == c.colour);
                let (_, rgb, finish) = list[turn(list.len(), at)];
                c.colour = rgb;
                if let Some(f) = finish.filter(|_| self.platform != Platform::Ps1) {
                    c.finish = f;
                }
            }
            EditRow::Hue => {
                let (h, s, v) = hsv(c.colour);
                // a grey has no hue to turn: it is given a little colour to turn
                let s = if s < 0.05 { 0.35 } else { s };
                let h = h + if right { HUE_STEP } else { -HUE_STEP };
                c.colour = rgb((h.rem_euclid(360.0), s, v));
            }
            EditRow::Shade => {
                let (h, s, v) = hsv(c.colour);
                let v = (v + if right { SHADE_STEP } else { -SHADE_STEP }).clamp(0.06, 1.0);
                c.colour = rgb((h, s, v));
            }
            EditRow::Finish => {
                let at = FINISHES.iter().position(|f| *f == c.finish);
                c.finish = FINISHES[turn(FINISHES.len(), at)];
            }
            EditRow::Undo | EditRow::Default | EditRow::Save => return,
        }
        self.choice = Some(c);
    }

    /// The panel as the options panels are drawn: its title, each row's name and value, the
    /// lit one and its note.
    pub fn view(&self) -> OptionsView {
        let rows = rows(self.platform);
        let lit = rows.iter().position(|r| *r == self.row).unwrap_or(0);
        OptionsView {
            title: title(self.platform),
            lines: rows
                .iter()
                .map(|r| {
                    (
                        OptionRow::CartEdit(*r),
                        r.label(self.platform).to_string(),
                        self.value(*r),
                    )
                })
                .collect(),
            lit,
            note: self.row.note(self.platform).to_string(),
            above: false,
            below: false,
        }
    }
}

/// The OptionRow a row of the editor's panel stands as: left and right change it, but Undo, Default
/// and Save, which A does.
pub fn row_carets(row: EditRow) -> bool {
    row.steps()
}

impl App {
    /// SELECT+START on the shelf: the cart picked opened in the editor.
    pub(crate) fn open_cart_editor(&mut self) {
        let Some(cart) = self.shelf().carts.get(self.shelf().index).cloned() else {
            return self.refuse();
        };
        let now = self.now();
        crate::log!("cart editor: {}", cart.stem);
        self.cart_editor = Some(CartEditor::new(&cart, now));
        self.cart_editor_panel = None;
    }

    /// The editor while it is open, as far as it shows.
    pub fn cart_editor(&self) -> Option<&CartEditor> {
        self.cart_editor.as_ref()
    }

    /// The editor's panel to bake, while it is open.
    pub fn cart_editor_view(&self) -> Option<OptionsView> {
        self.cart_editor.as_ref().map(CartEditor::view)
    }

    pub fn set_cart_editor_panel(&mut self, panel: Lone) {
        self.cart_editor_panel = Some(panel);
    }

    /// A press with the editor open: Up and Down move between its rows, Left and Right change
    /// the one lit, A does Undo, Default and Save (and steps the rest), and B, or MENU, puts the cart
    /// back as it was. Nothing reaches the shelf beneath.
    pub(crate) fn cart_editor_input(&mut self, action: Action) {
        let now = self.now();
        let Some(ed) = &mut self.cart_editor else {
            return;
        };
        if ed.closing.is_some() {
            return;
        }
        let list = rows(ed.platform);
        let at = list.iter().position(|r| *r == ed.row).unwrap_or(0);
        // what the press does to the editor, and then to the cart and the card
        let then = match action {
            Action::GbaDown(Btn::Up) => {
                ed.row = list[(at + list.len() - 1) % list.len()];
                Then::Nothing
            }
            Action::GbaDown(Btn::Down) => {
                ed.row = list[(at + 1) % list.len()];
                Then::Nothing
            }
            Action::GbaDown(b @ (Btn::Left | Btn::Right)) if ed.row.steps() => {
                ed.step(ed.row, b == Btn::Right);
                Then::Show
            }
            Action::GbaDown(Btn::A) => match ed.row {
                EditRow::Undo => {
                    ed.choice = ed.was;
                    Then::Show
                }
                EditRow::Default => {
                    ed.choice = None;
                    Then::Show
                }
                EditRow::Save => Then::Close(true),
                row => {
                    ed.step(row, true);
                    Then::Show
                }
            },
            Action::GbaDown(Btn::B) | Action::QuickMenu => Then::Close(false),
            _ => Then::Nothing,
        };
        match then {
            Then::Nothing => {}
            Then::Show => self.show_cart_editor_choice(),
            Then::Close(keep) => self.close_cart_editor(keep, now),
        }
    }

    /// The editor's look on the cart: on every shelf it stands on, and its face drawn again.
    fn show_cart_editor_choice(&mut self) {
        let Some(ed) = &self.cart_editor else {
            return;
        };
        let (platform, stem, choice) = (ed.platform, ed.stem.clone(), ed.choice);
        self.set_cart_shell(platform, &stem, choice);
    }

    /// Gives the cart `stem` of `platform` the look `choice` on every shelf, and has its face
    /// drawn again.
    fn set_cart_shell(&mut self, platform: Platform, stem: &str, choice: Option<ShellChoice>) {
        for (kind, shelf) in &mut self.shelves {
            let mine = match kind.platform() {
                Some(p) => p == platform,
                // FAVORITES and RECENTS hold carts of every system
                None => shelf
                    .carts
                    .iter()
                    .any(|c| c.platform == platform && c.stem == stem),
            };
            if mine {
                shelf.set_shell(stem, choice);
            }
        }
        self.redraw_faces(vec![(platform, stem.to_string())]);
    }

    /// Closes the editor: `keep` writes what it shows to the card, otherwise the cart is put back
    /// as it was.
    fn close_cart_editor(&mut self, keep: bool, now: Millis) {
        let Some(ed) = self.cart_editor.clone() else {
            return;
        };
        if keep {
            match &self.root {
                Some(root) => match omega_store::write_choice(root, &ed.stem, ed.choice) {
                    Ok(()) => {
                        crate::log!(
                            "cart editor: {} kept as {}",
                            ed.stem,
                            ed.choice
                                .map_or_else(|| "auto".to_string(), |c| c.to_value())
                        );
                    }
                    Err(e) => {
                        crate::log!("cart editor: {}: {e}", ed.stem);
                        self.refuse();
                    }
                },
                None => self.refuse(),
            }
        } else if ed.choice != ed.was {
            self.set_cart_shell(ed.platform, &ed.stem, ed.was);
        }
        if let Some(ed) = &mut self.cart_editor {
            ed.closing = Some(now);
        }
    }

    /// The editor gone once closed all the way.
    pub(crate) fn tick_cart_editor(&mut self, now: Millis) {
        if self.cart_editor.as_ref().is_some_and(|e| e.closed(now)) {
            self.cart_editor = None;
        }
    }

    /// How far the editor is open, for the shelf's slide: 0 with none.
    pub fn cart_editor_openness(&self) -> f32 {
        self.cart_editor
            .as_ref()
            .map_or(0.0, |e| e.openness(self.now()))
    }

    /// The editor's panel, sliding in from the right as it opens: at the right of the page,
    /// level with the middle.
    pub(crate) fn draw_cart_editor(&self, out: &mut Vec<Draw>) {
        let (Some(panel), open) = (&self.cart_editor_panel, self.cart_editor_openness()) else {
            return;
        };
        if open <= 0.0 {
            return;
        }
        let page_w = omega_ui::out_w() / omega_ui::SPRITE_PX;
        // level with the cart, in the middle of the page between the bars
        let [_, y, w, h] = panel.rect();
        let rest = page_w.saturating_sub(w + EDGE);
        let away = page_w as f32 + 2.0;
        let x = away + (rest as f32 - away) * open;
        out.push(omega_ui::kit::pixel::quad(
            panel.tex,
            w,
            h,
            omega_ui::kit::layout::screen_px(x.round().max(0.0) as u32),
            omega_ui::kit::layout::screen_px(y),
        ));
    }
}

/// How far the panel stands in from the right of the page, in art pixels.
const EDGE: u32 = 8;

/// What a press in the editor comes to, past the editor itself.
enum Then {
    Nothing,
    /// The look changed: shown on the cart.
    Show,
    /// Closed, the look kept or put back.
    Close(bool),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_turned_all_the_way_round_comes_back() {
        for c in [[0xc8, 0x2a, 0x30], [0x45, 0x3e, 0x9a], [0x2f, 0x9a, 0x52]] {
            let back = rgb(hsv(c));
            for i in 0..3 {
                assert!(back[i].abs_diff(c[i]) <= 1, "{c:?} came back {back:?}");
            }
        }
        let (h, ..) = hsv([0xff, 0, 0]);
        assert!(h.abs() < 0.01);
    }

    #[test]
    fn each_system_has_its_own_rows_and_title() {
        assert_eq!(rows(Platform::Gb)[0], EditRow::Shape);
        assert_eq!(rows(Platform::Snes)[0], EditRow::Shape);
        assert!(!rows(Platform::Gba).contains(&EditRow::Shape));
        assert!(
            !rows(Platform::Ps1).contains(&EditRow::Finish),
            "a disc has no plastic"
        );
        for p in Platform::ALL {
            assert_eq!(rows(p).last(), Some(&EditRow::Save), "{p:?}");
            assert!(!title(p).is_empty());
            assert_eq!(colours(p).len(), own_colours(p).len() + PLAIN.len());
        }
        assert_eq!(EditRow::Colour.label(Platform::Ps1), "Disc Color");
    }
}
