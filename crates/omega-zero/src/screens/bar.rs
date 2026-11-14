//! The bar along the bottom of the shelf and of every menu, as flow.html draws it: an action at
//! each end and, between them over whatever is behind the bar, the keys a screen has beyond its
//! two actions shown as hints. The shelf's actions are SELECT CONTROLS and START SETTINGS, a
//! menu's B RETURN and A CONFIRM. Setting the clock on first boot has nowhere to go back to, so
//! its left action is the kit's empty one: the bar keeps its shape and offers only CONFIRM. The
//! save-state switcher over a game has B RETURN and A LOAD, and while it asks whether to delete a
//! save, B CANCEL and A DELETE. Between them its X DELETE stands on a line of its own, the kit's
//! hint line, since there the bar's hints would sit bare on the game's screenshot
//! (delete_state.rs). The bar comes and goes with the switcher, at once.
//!
//! The hints never move with the screens. When a slide's two ends have different actions, the
//! old ones sink out of the screen over its first half and the new ones rise into place over
//! the second, as flow/player.js does; between two menus they stay put, and the hints change at
//! halfway. The core picker swaps them the same way as it opens and closes, its ◀▶ SWAP coming
//! in between RETURN and CONFIRM with them and leaving with them.
//!
//! A cart going in takes the whole bar down out of the screen, before the cart is pulled down
//! past it into the handheld; the bar stays away while the game is on and rises back as the cart
//! comes out.
//!
//! The power menu has a menu's actions, swapped with those of the screen POWER was held on as
//! it slides in and out. The screen after its choice has none, since nothing can be pressed:
//! the whole bar sinks away with the actions it had as that screen slides in, as it does when a
//! cart goes in.

use omega_gfx::{out_h, Draw, TexId};
use omega_ui::kit::action::Side;
use omega_ui::kit::bar::{action_swap, BarLayout, Hint, SINK};
use omega_ui::kit::pixel::quad;
use omega_ui::{ease, Millis, SPRITE_PX};

use crate::app::{App, Phase};
use crate::core_picker::CorePicker;
use crate::screens::labels::labels_hints;
use crate::screens::power::PowerScreen;

/// The core picker's one key beyond the bar's actions: Left and Right move the chip.
pub const SWAP: Hint<'static> = Hint {
    key: "◀▶",
    word: "SWAP",
};

/// Y on a saved network or a paired Bluetooth device: it forgets it.
pub const FORGET: Hint<'static> = Hint {
    key: "Y",
    word: "FORGET",
};

/// Y on one of Network's own rows, WI-FI, NETWORK TIME or SSH: it opens the row's panel.
/// The cart editor's: Left and Right change the row lit.
pub const CHANGE: Hint<'static> = Hint {
    key: "◀▶",
    word: "CHANGE",
};

pub const INFO: Hint<'static> = Hint {
    key: "Y",
    word: "INFO",
};

/// X on Network while Wi-Fi is on, and on Bluetooth while the radio is on: it looks once for
/// what is in reach, networks or devices. Neither looks by itself.
pub const SCAN: Hint<'static> = Hint {
    key: "X",
    word: "SCAN",
};

/// START on headphones connected on the Bluetooth screen: it opens their HEADPHONES menu. The
/// bar has room for two hints, so it stands in for X SCAN there, which X still does.
pub const HEADPHONES: Hint<'static> = Hint {
    key: "START",
    word: "MENU",
};

/// X on the save-state switcher: it asks whether to delete the save shown.
pub const DELETE: Hint<'static> = Hint {
    key: "X",
    word: "DELETE",
};

/// Y on Settings > Gameplay > Account, once signed in: it signs out of RetroAchievements.
pub const SIGN_OUT: Hint<'static> = Hint {
    key: "Y",
    word: "SIGN OUT",
};

/// Which pair of actions the bar shows at its ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Actions {
    /// The shelf's: SELECT CONTROLS on the left, MENU SETTINGS on the right.
    Shelf,
    /// A menu's: B RETURN and A CONFIRM.
    Menu,
    /// A menu with no way back, the clock set on first boot: A CONFIRM alone, the left action
    /// empty.
    Confirm,
    /// The save-state switcher's: B RETURN to the game, A LOAD the save shown.
    Switcher,
    /// The switcher asking whether to delete the save shown: B CANCEL, A DELETE.
    Delete,
    /// A question answered yes or no, as taking a game's star off: B NO, A YES.
    YesNo,
}

impl Actions {
    pub const ALL: [Actions; 6] = [
        Actions::Shelf,
        Actions::Menu,
        Actions::Confirm,
        Actions::Switcher,
        Actions::Delete,
        Actions::YesNo,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// The big word and the button in its pill of the action at `side`.
    pub fn action(self, side: Side) -> (&'static str, &'static str) {
        match (self, side) {
            (Actions::Shelf, Side::Left) => ("CONTROLS", "SELECT"),
            (Actions::Shelf, Side::Right) => ("SETTINGS", "MENU"),
            (Actions::Menu | Actions::Switcher, Side::Left) => ("RETURN", "B"),
            (Actions::Menu | Actions::Confirm, Side::Right) => ("CONFIRM", "A"),
            (Actions::Confirm, Side::Left) => ("", ""),
            (Actions::Switcher, Side::Right) => ("LOAD", "A"),
            (Actions::Delete, Side::Left) => ("CANCEL", "B"),
            (Actions::Delete, Side::Right) => ("DELETE", "A"),
            (Actions::YesNo, Side::Left) => ("NO", "B"),
            (Actions::YesNo, Side::Right) => ("YES", "A"),
        }
    }
}

/// Where the bar is in a frame: whose actions it shows, how far they are sunk below their place
/// in a swap, and how far the whole bar is sunk below the screen's bottom edge as a cart goes
/// in or comes out, both in art pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BarPlace {
    pub actions: Actions,
    pub sink: u32,
    pub drop: u32,
}

/// One pair of actions uploaded, each face with its size in art pixels, and where they and the
/// hints between them go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarSet {
    pub layout: BarLayout,
    pub left: (TexId, u32, u32),
    pub right: (TexId, u32, u32),
}

/// The bar's faces: each pair of actions, and the hints laid out between each, in
/// `Actions::ALL`'s order. The hints are those of the screen they are shown on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarFaces {
    pub sets: [BarSet; Actions::ALL.len()],
    pub hints: [Option<TexId>; Actions::ALL.len()],
}

/// The actions the bar shows on `phase` at rest, or None where there is no bar: in a game and
/// its menus, and asleep. A cart going in or coming out still has the shelf's, and the save
/// states over a game have their own.
pub fn actions_on(phase: &Phase) -> Option<Actions> {
    match phase {
        Phase::Shelf | Phase::Inserting { .. } | Phase::Ejecting { .. } => Some(Actions::Shelf),
        Phase::SetClock {
            from_menu: false, ..
        } => Some(Actions::Confirm),
        Phase::Settings { .. }
        | Phase::Controls
        | Phase::Network(_)
        | Phase::Bluetooth(_)
        | Phase::Info(_)
        | Phase::Labels(_)
        | Phase::Background(_)
        | Phase::Options(_)
        | Phase::SetClock { .. }
        | Phase::About => Some(Actions::Menu),
        Phase::Polaroids { .. } => Some(Actions::Switcher),
        Phase::Playing { .. } | Phase::Doze { .. } => None,
    }
}

/// How far into its way in (`CartDive::seat`) a cart is when the bar has sunk out of the
/// screen: before the cart, pulled down, reaches it.
const SUNK_AT: f32 = 0.62;

/// How far the bar has sunk `seat` of the way through a cart's way in (0 on the shelf, 1 in
/// the handheld): gone, eased, by `SUNK_AT`, and back as the cart comes out.
fn sunk_for(seat: f32) -> u32 {
    (ease((seat / SUNK_AT).clamp(0.0, 1.0)) * SINK as f32).round() as u32
}

/// How far through the swap to its actions the core picker is, eased: the bar follows the
/// picker opening and closing.
fn picker_swap(picker: &CorePicker, now: Millis) -> f32 {
    ease(picker.openness(now))
}

/// The actions shown, and how far sunk, `p` of the way (eased) from `from`'s to `to`'s.
fn swapped(from: Option<Actions>, to: Option<Actions>, p: f32) -> Option<(Actions, u32)> {
    if from == to {
        return to.map(|a| (a, 0));
    }
    let swap = action_swap(p);
    match (swap.new, from, to) {
        (false, Some(a), _) | (true, _, Some(a)) => Some((a, swap.sink)),
        // one end has no bar: the hints stay and the other end's actions keep out of sight
        (_, Some(a), None) | (_, None, Some(a)) => Some((a, SINK)),
        (_, None, None) => None,
    }
}

impl App {
    /// Where the bar is now, or None when it is out of sight.
    pub fn bar_place(&self) -> Option<BarPlace> {
        if self.power_shown() {
            return self.power_bar_place();
        }
        let now = self.now();
        let (actions, sink) = match (&self.slide, self.core_picker_shown()) {
            (Some(slide), _) => swapped(
                actions_on(&slide.from),
                actions_on(&self.phase),
                slide.progress(now),
            )?,
            // the picker's B and A are the bar's: it swaps to them as the picker opens
            (None, Some(picker)) if matches!(self.phase, Phase::Shelf) => swapped(
                Some(Actions::Shelf),
                Some(Actions::Menu),
                picker_swap(&picker, now),
            )?,
            // the cart editor over the shelf: a menu's B RETURN and A CONFIRM
            (None, _) if self.cart_editor.is_some() && matches!(self.phase, Phase::Shelf) => {
                (Actions::Menu, 0)
            }
            // the MENU panel over a game has a menu's B RETURN and A CONFIRM
            (None, _) if self.pause_menu_shown().is_some() => (Actions::Menu, 0),
            // the switcher asking whether to delete a save: B CANCEL and A DELETE
            (None, _) if self.delete_asked() && matches!(self.phase, Phase::Polaroids { .. }) => {
                (Actions::Delete, 0)
            }
            // the shelf asking whether to take a game's star off, or Saves whether to restore
            // or delete: B NO and A YES
            (None, _) if self.unfavorite_asked() && matches!(self.phase, Phase::Shelf) => {
                (Actions::YesNo, 0)
            }
            (None, _)
                if self.saves_asking().is_some() && matches!(self.phase, Phase::Options(_)) =>
            {
                (Actions::YesNo, 0)
            }
            (None, _) => (actions_on(&self.phase)?, 0),
        };
        let drop = match self.phase {
            Phase::Inserting { .. } | Phase::Ejecting { .. } => sunk_for(self.seat()),
            _ => 0,
        };
        (drop < SINK).then_some(BarPlace {
            actions,
            sink: (sink + drop).min(SINK),
            drop,
        })
    }

    /// Where the bar is over a power screen, or through a slide to or from one: the menu's
    /// actions swap with those of the screen POWER was held on, and the screen after the
    /// choice, which has none, takes the whole bar down with the actions it had as it slides in.
    fn power_bar_place(&self) -> Option<BarPlace> {
        let now = self.now();
        let slide = self.power_slide().map(|s| (s.from, s.progress(now)));
        if let Some(PowerScreen::Going(_)) = self.power_screen() {
            let (from, p) = slide?;
            let actions = self.actions_over(from)?;
            let drop = (p * SINK as f32).round() as u32;
            return (drop < SINK).then_some(BarPlace {
                actions,
                sink: drop,
                drop,
            });
        }
        let to = self.actions_over(self.power_screen());
        let (actions, sink) = match slide {
            Some((from, p)) => swapped(self.actions_over(from), to, p)?,
            None => (to?, 0),
        };
        Some(BarPlace {
            actions,
            sink,
            drop: 0,
        })
    }

    /// The actions the bar shows on `screen`, or with None on the screen POWER was held on.
    fn actions_over(&self, screen: Option<PowerScreen>) -> Option<Actions> {
        match screen {
            None => actions_on(&self.phase),
            Some(PowerScreen::Menu(_)) => Some(Actions::Menu),
            Some(PowerScreen::Going(_)) => None,
        }
    }

    /// The keys `phase` has beyond its two actions, for between them: on Network X SCAN while
    /// Wi-Fi is on and Y FORGET on a saved network (`network_hints`), on Bluetooth X SCAN while
    /// the radio is on and Y FORGET on a paired device (`bluetooth_hints`), on Labels START FETCH
    /// or START STOP and Y SIGN OUT (`labels_bar_hints`'), and on the shelf, while the core
    /// picker over it shows RETURN and CONFIRM, ◀▶ SWAP. They are asked of `phase` rather than
    /// the screen on show so a slide can show the hints of the screen it leaves. A screen with
    /// more keys adds them here; Settings and SSH have none, the keyboard keeps its own line in
    /// its modal, and the save states their own line over the game (delete_state.rs).
    pub fn bar_hints(&self, phase: &Phase) -> Vec<Hint<'static>> {
        match phase {
            Phase::Shelf if self.picker_has_bar() => vec![SWAP],
            Phase::Shelf if self.cart_editor.is_some() => vec![CHANGE],
            Phase::Network(screen) => self.network_hints(screen),
            Phase::Bluetooth(screen) => self.bluetooth_hints(screen),
            Phase::Options(screen)
                if screen.typing.is_none()
                    && screen.row == crate::options::OptionRow::Account
                    && self.achievements_account() =>
            {
                vec![SIGN_OUT]
            }
            Phase::Labels(screen) if screen.typing.is_none() => labels_hints(
                &screen
                    .view(&self.labels, self.online(), self.state.holo)
                    .legend,
            ),
            _ => Vec::new(),
        }
    }

    /// Whether the core picker is open far enough to have the bar: past halfway through the
    /// swap to its actions, where they and its hint come in.
    fn picker_has_bar(&self) -> bool {
        self.core_picker_shown()
            .is_some_and(|picker| action_swap(picker_swap(&picker, self.now())).new)
    }

    /// The hints the bar shows now: the screen's on show, and through a slide the leaving
    /// screen's until halfway, the arriving one's after. The power screens have none of their
    /// own.
    pub fn bar_hints_shown(&self) -> Vec<Hint<'static>> {
        if self.power_shown() {
            let shown = match self.power_slide() {
                Some(slide) if slide.progress(self.now()) < 0.5 => slide.from,
                _ => self.power_screen(),
            };
            return match shown {
                None => self.bar_hints(&self.phase),
                Some(_) => Vec::new(),
            };
        }
        match &self.slide {
            Some(slide) if slide.progress(self.now()) < 0.5 => self.bar_hints(&slide.from),
            _ => self.bar_hints(&self.phase),
        }
    }

    /// Each pair of actions, uploaded: what the room for the hints between them is laid out
    /// from.
    pub fn set_bar_sets(&mut self, sets: [BarSet; Actions::ALL.len()]) {
        self.bar_faces = Some(BarFaces {
            sets,
            hints: [None; Actions::ALL.len()],
        });
    }

    /// The bar's faces once its actions are uploaded.
    pub fn bar_faces(&self) -> Option<&BarFaces> {
        self.bar_faces.as_ref()
    }

    /// The face of the hints laid out between `actions`, those of the screen they are shown on.
    pub fn set_bar_hints_face(&mut self, actions: Actions, tex: TexId) {
        if let Some(faces) = &mut self.bar_faces {
            faces.hints[actions.index()] = Some(tex);
        }
    }

    /// The bar, over whatever screen is shown or sliding and still as they slide: the hints,
    /// then the two actions. Nothing else is drawn between them.
    pub(crate) fn draw_bar(&self, out: &mut Vec<Draw>) {
        let (Some(place), Some(faces)) = (self.bar_place(), &self.bar_faces) else {
            return;
        };
        let set = &faces.sets[place.actions.index()];
        let (h, px) = (out_h(), |art: i32| (art * SPRITE_PX as i32) as f32);
        if let Some(tex) = faces.hints[place.actions.index()] {
            out.push(
                set.layout
                    .hints_quad(tex, h)
                    .shifted(0.0, px(place.drop as i32)),
            );
        }
        // the shelf's system in its pill between the actions, going with them: the shelf's
        // actions are shown only with the shelf at one end of a slide, or a cart going in or out
        if let (Actions::Shelf, None, None, Some(tex)) = (
            place.actions,
            self.core_picker_shown(),
            self.cart_editor.as_ref(),
            self.shelf_platform.face,
        ) {
            // sunk all the way, it is out of the screen
            let sink = place.sink + self.pill_sink();
            if sink < SINK {
                let w = self.shelf_platform.w;
                let x = (omega_gfx::out_w() / SPRITE_PX / 2).saturating_sub(w / 2);
                let y = BarLayout::top(h) + px(sink as i32);
                out.push(quad(tex, w, omega_ui::kit::action::BAR, px(x as i32), y));
            }
        }
        if place.sink >= SINK {
            return;
        }
        for ((tex, w, th), [x, y]) in [(set.left, set.layout.left), (set.right, set.layout.right)] {
            let y = BarLayout::top(h) + px(y + place.sink as i32);
            out.push(quad(tex, w, th, px(x), y));
        }
    }
}
