//! The DISC screen a PlayStation game on several discs opens with SELECT + MENU, where the link
//! screen opens on a GBA or a Game Boy: the player picks one of the game's discs with Left and
//! Right and A swaps it into the drive, as the console's lid opened and closed would. B, SELECT
//! or SELECT + MENU again close it and leave the disc in. A game on one disc has nothing to
//! swap, and SELECT + MENU says so with a toast.
//!
//! It is drawn in the kit over the theme's backdrop, as the link screen is: a lone panel titled
//! DISC at the top, its DISC row lit with its carets (the disc picked, of how many) and IN DRIVE
//! under it, and the game's title as a note; the game's discs side by side between the panel
//! and the bar, the picked one raised and the others faded back; and the kit's bar, B RETURN and
//! A SWAP with ◀▶ PICK between them. The game is paused under it.
//!
//! Each disc is drawn as the shelf draws it, in three dimensions, out of its stack; a game whose
//! stack is not ready yet shows the face the shelf has for it, flat.

use std::path::PathBuf;

use omega_gfx::{out_h, out_w};
use omega_input::{Action, Btn};
use omega_store::Platform;
use omega_ui::kit::action::Side;
use omega_ui::kit::bar::{BarLayout, Hint};
use omega_ui::kit::fade::stepped;
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::pixel::quad;
use omega_ui::kit::row::{panel_height, row_face, row_rect};
use omega_ui::{
    clean_label, model_box, stage_y, CartModel, Draw, TexId, Toast, UndoFace, SPRITE_PX,
};

use crate::app::App;
use crate::screens::bar::BarSet;
use crate::screens::lone::{notes, notes_height, over, rows_height, Lone, Words, NOTE_GAP};

/// How wide the DISC panel is, and how far down the 720x480 stage its top is: the LINK panel's.
pub const PANEL_W: u32 = crate::link_screen::PANEL_W;
pub const PANEL_TOP: u32 = crate::link_screen::PANEL_TOP;

/// The DISC screen's key beyond its two actions: Left and Right pick a disc.
pub const PICK: Hint<'static> = Hint {
    key: "◀▶",
    word: "PICK",
};

/// The bar's actions over the DISC screen.
pub const ACTIONS: [(&str, &str); 2] = [("RETURN", "B"), ("SWAP", "A")];

/// How far the picked disc stands above the others, in art pixels, and how far the others fade
/// back, in the kit's steps.
pub const RAISE: i32 = 6;
pub const FADED: f32 = 0.5;

/// The room between two discs, and the most a disc is drawn across or down, in art pixels.
const GAP: i32 = 8;
const MOST_W: f32 = 120.0;
const MOST_H: f32 = 96.0;

/// The DISC screen while it is open: the game's discs, in order, and the one picked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscScreen {
    pub discs: Vec<PathBuf>,
    pub at: usize,
}

/// What the DISC panel shows: the disc picked, of how many, the one in the drive (each from 0),
/// and the game's title.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscView {
    pub at: usize,
    pub count: usize,
    pub in_drive: usize,
    pub title: String,
}

/// The DISC screen's bar: its actions uploaded, and the hint between them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiscBar {
    pub set: BarSet,
    pub hints: Option<TexId>,
}

/// The DISC panel in `pal`: DISC with the disc picked of how many, lit with its carets; IN DRIVE
/// with the disc in it; and the game's title as a note under them, cut short where it is too
/// long, the panel as tall as what it holds.
pub fn disc_panel_face(view: &DiscView, pal: &Palette) -> UndoFace {
    let rows = [
        Words {
            label: "DISC".into(),
            value: Some(format!("{} OF {}", view.at + 1, view.count)),
            carets: true,
            dim: false,
        },
        Words {
            label: "IN DRIVE".into(),
            value: Some(format!("DISC {}", view.in_drive + 1)),
            carets: false,
            dim: false,
        },
    ];
    let held = rows_height(rows.len());
    let lines = [view.title.clone()];
    let mut panel = panel_face(
        PANEL_W,
        panel_height(held + notes_height(lines.len())),
        "DISC",
        true,
        pal,
    );
    for (i, row) in rows.iter().enumerate() {
        let [x, y, w, _] = row_rect(panel.well, i as u32);
        over(&mut panel.face, &row_face(w, &row.row(i == 0), pal), x, y);
    }
    notes(&mut panel, &lines, held + NOTE_GAP, pal);
    panel.face
}

/// Where the DISC panel goes, `[x, y]` in art pixels: where the LINK panel goes.
pub fn panel_at(panel: &Lone) -> [i32; 2] {
    let stage_top = (stage_y().max(0.0) as u32 / SPRITE_PX) as i32;
    let page_w = (out_w() / SPRITE_PX) as i32;
    [
        (page_w - panel.w as i32).div_euclid(2),
        stage_top + PANEL_TOP as i32,
    ]
}

/// Where each of `count` discs is drawn between `top` and `bottom` (art pixels down the screen),
/// `[x, y, w, h]` in art pixels, the disc at `at` raised: side by side across the middle, each
/// its face's shape, as big as they all fit.
pub fn disc_places(count: usize, at: usize, top: i32, bottom: i32) -> Vec<[i32; 4]> {
    if count == 0 {
        return Vec::new();
    }
    let (fw, fh) = model_box(CartModel::Disc);
    let page_w = (out_w() / SPRITE_PX) as i32;
    let room_w = (page_w - 2 * GAP - GAP * (count as i32 - 1)) as f32 / count as f32;
    let room_h = (bottom - top - RAISE - 2 * GAP) as f32;
    let scale = (room_w.min(MOST_W) / fw as f32).min(room_h.min(MOST_H) / fh as f32);
    let (w, h) = (
        (fw as f32 * scale).floor() as i32,
        (fh as f32 * scale).floor() as i32,
    );
    let across = count as i32 * w + (count as i32 - 1) * GAP;
    let left = (page_w - across).div_euclid(2);
    let y = top + RAISE + (bottom - top - RAISE - h).div_euclid(2);
    (0..count)
        .map(|i| {
            let raise = if i == at { RAISE } else { 0 };
            [left + i as i32 * (w + GAP), y - raise, w, h]
        })
        .collect()
}

/// An art pixel position as the screen pixels a Draw takes: always even.
fn px(art: i32) -> f32 {
    (art * SPRITE_PX as i32) as f32
}

impl App {
    /// SELECT + MENU on a PlayStation game: the DISC screen, or a toast for a game on one disc.
    pub(crate) fn open_disc_screen(&mut self) {
        let Some(rom) = self.seated_cart().map(|c| c.rom.clone()) else {
            return;
        };
        let discs = omega_store::ps1::discs(&rom);
        if discs.len() < 2 {
            self.hud.toast(Toast::OneDisc, self.now());
            return;
        }
        let at = self.disc_in().min(discs.len() - 1);
        self.disc_screen = Some(DiscScreen { discs, at });
    }

    /// The disc in the drive of the game in the handheld, counting from 0: the first until the
    /// DISC screen swaps another in.
    pub fn disc_in(&self) -> usize {
        match (&self.disc_in, self.seated()) {
            (Some((stem, i)), Some(seated)) if stem == seated => *i,
            _ => 0,
        }
    }

    /// Says disc `index` of the game `stem` is in the drive, as when a game put away on that disc
    /// comes back on it.
    pub(crate) fn set_disc_in(&mut self, stem: &str, index: usize) {
        self.disc_in = Some((stem.to_string(), index));
    }

    /// The disc the DISC screen swapped in, for the session to hand the core, once.
    pub fn take_disc_swap(&mut self) -> Option<usize> {
        self.disc_swap.take()
    }

    pub fn disc_screen_open(&self) -> bool {
        self.disc_screen.is_some()
    }

    /// What the DISC panel shows, while the DISC screen is open.
    pub fn disc_view(&self) -> Option<DiscView> {
        let screen = self.disc_screen.as_ref()?;
        Some(DiscView {
            at: screen.at,
            count: screen.discs.len(),
            in_drive: self.disc_in(),
            title: self.seated().map(clean_label).unwrap_or_default(),
        })
    }

    pub(crate) fn disc_input(&mut self, action: Action) {
        let Some(screen) = &mut self.disc_screen else {
            return;
        };
        match action {
            Action::GbaDown(Btn::Left) => screen.at = screen.at.saturating_sub(1),
            Action::GbaDown(Btn::Right) => {
                screen.at = (screen.at + 1).min(screen.discs.len().saturating_sub(1));
            }
            Action::GbaDown(Btn::A) => {
                let at = screen.at;
                self.disc_screen = None;
                self.swap_disc(at);
            }
            Action::GbaDown(Btn::B | Btn::Select) | Action::GameMenu => {
                self.disc_screen = None;
            }
            _ => {}
        }
    }

    /// Puts disc `at` in the drive, and says so; the disc already in it stays.
    fn swap_disc(&mut self, at: usize) {
        if at == self.disc_in() {
            return;
        }
        let Some(stem) = self.seated().map(str::to_string) else {
            return;
        };
        self.disc_in = Some((stem, at));
        self.disc_swap = Some(at);
        let n = u8::try_from(at + 1).unwrap_or(u8::MAX);
        self.hud.toast(Toast::Disc(n), self.now());
    }

    /// Whether SELECT + MENU opens the DISC screen rather than the link screen.
    pub(crate) fn swaps_discs(&self) -> bool {
        self.platform == Platform::Ps1
    }

    pub fn set_disc_panel(&mut self, panel: Lone) {
        self.disc_panel = Some(panel);
    }

    pub fn set_disc_bar(&mut self, bar: DiscBar) {
        self.disc_bar = Some(bar);
    }

    /// The face of each of the game's discs, in order. Each is the game's own face on the shelf
    /// for now: a face for each disc comes with the disc stack.
    pub fn disc_faces(&self) -> Vec<Option<TexId>> {
        let count = self.disc_screen.as_ref().map_or(0, |s| s.discs.len());
        let face = self
            .seated()
            .and_then(|stem| self.shelf().find(stem))
            .and_then(|(_, face)| face);
        vec![face; count]
    }

    /// The DISC screen over the game: the theme's backdrop, the discs, the DISC panel and the
    /// bar.
    pub(crate) fn draw_disc_screen(&self, screen: &DiscScreen, out: &mut Vec<Draw>) {
        self.draw_background(out);
        let panel = self.disc_panel.map(|p| (p, panel_at(&p)));
        let top = panel.map_or(0, |(p, [_, y])| y + p.h as i32);
        let bottom = (BarLayout::top(out_h()) / SPRITE_PX as f32) as i32;
        self.draw_disc_art(screen, top, bottom, out);
        if let Some((p, [x, y])) = panel {
            out.push(quad(p.tex, p.w, p.h, px(x), px(y)));
        }
        self.draw_disc_bar(out);
    }

    /// The game's discs side by side between `top` and `bottom`, each drawn as its stack on the
    /// shelf draws it, the picked one raised and the rest faded back in the kit's steps.
    fn draw_disc_art(&self, screen: &DiscScreen, top: i32, bottom: i32, out: &mut Vec<Draw>) {
        let places = disc_places(screen.discs.len(), screen.at, top, bottom);
        let stack = self.seated().and_then(|stem| self.shelf().stack_for(stem));
        if let Some(stack) = stack.filter(|s| s.discs == screen.discs.len()) {
            for (i, [x, y, w, h]) in places.into_iter().enumerate() {
                let k = if i == screen.at { 1.0 } else { stepped(FADED) };
                omega_ui::push_disc_alone(
                    out,
                    (px(x), px(y), px(w), px(h)),
                    &stack,
                    i,
                    [k, k, k, 1.0],
                    0.0,
                    0.0,
                );
            }
            return;
        }
        let faces = self.disc_faces();
        for (i, (face, [x, y, w, h])) in faces.into_iter().zip(places).enumerate() {
            let Some(tex) = face else {
                continue;
            };
            out.push(Draw::Tex {
                x: px(x),
                y: px(y),
                w: px(w),
                h: px(h),
                tex,
                alpha: if i == screen.at { 1.0 } else { stepped(FADED) },
            });
        }
    }

    fn draw_disc_bar(&self, out: &mut Vec<Draw>) {
        let Some(bar) = &self.disc_bar else {
            return;
        };
        let h = out_h();
        if let Some(tex) = bar.hints {
            out.push(bar.set.layout.hints_quad(tex, h));
        }
        for ((tex, w, th), [x, y]) in [
            (bar.set.left, bar.set.layout.left),
            (bar.set.right, bar.set.layout.right),
        ] {
            out.push(quad(tex, w, th, px(x), BarLayout::top(h) + px(y)));
        }
    }
}

/// The DISC screen's actions as the bar draws them at `side`.
pub fn disc_action(side: Side) -> (&'static str, &'static str) {
    match side {
        Side::Left => ACTIONS[0],
        Side::Right => ACTIONS[1],
    }
}
