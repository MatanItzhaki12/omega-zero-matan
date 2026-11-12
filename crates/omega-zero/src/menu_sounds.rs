//! Which of the menus' sounds a press makes. Rather than each screen choosing its own, the app
//! looks at what a press changed: a menu opened chimes up, one closed chimes down, a cursor or a
//! value that moved ticks, and a press that changed nothing makes no sound, so the top of a
//! list is felt by its silence. A screen added later sounds right with nothing written for it.
//!
//! The shelf's pick moving ticks too, from [`App::tick_shelf_sound`], since a held direction
//! moves it on its own between presses; a shelf changed for another glides.
//!
//! In a game the buttons are the game's: nothing sounds until a menu is opened over it.

use omega_input::{Action, Btn};
use omega_store::AppState;

use crate::app::{App, Phase};
use crate::audio::Sfx;
use crate::screens::power::PowerScreen;

/// What a press can change that the ear follows.
#[derive(Clone, PartialEq)]
pub(crate) struct Mark {
    /// A menu, a panel or a picker is up.
    menu: bool,
    /// Everything about the screens and menus up, as their own record prints it.
    screens: String,
    state: AppState,
    shelf: usize,
    favorites: usize,
}

impl App {
    pub(crate) fn sound_mark(&self) -> Mark {
        Mark {
            menu: self.menu_up(),
            screens: format!(
                "{:?}{:?}{:?}{:?}{:?}{}{}{:?}",
                self.phase,
                self.pause_menu,
                self.game_menu,
                self.disc_screen,
                self.core_picker.as_ref().map(|p| (p.seat(), p.closing())),
                self.delete_asked,
                self.unfavorite_asked,
                self.saves_ui.asking,
            ),
            state: self.state.clone(),
            shelf: self.shelf_at,
            favorites: self.favorites.len(),
        }
    }

    /// Whether the buttons are a menu's now, rather than the shelf's or a game's.
    fn menu_up(&self) -> bool {
        let screen = !matches!(
            self.phase,
            Phase::Shelf
                | Phase::Playing { .. }
                | Phase::Inserting { .. }
                | Phase::Ejecting { .. }
                | Phase::Doze { .. }
        );
        screen
            || self.pause_menu.is_some()
            || self.game_menu.is_some()
            || self.disc_screen.is_some()
            || self.core_picker.is_some()
            || matches!(self.power_screen, Some(PowerScreen::Menu(_)))
    }

    /// The sound `action` makes, given what it changed from `before`.
    pub(crate) fn sound_for_press(&mut self, before: &Mark, action: Action) {
        if matches!(action, Action::GbaUp(_)) {
            return;
        }
        let after = self.sound_mark();
        if after == *before {
            return;
        }
        let sfx = if after.shelf != before.shelf {
            Some(Sfx::Shelf)
        } else if before.menu && !after.menu {
            Some(Sfx::Back)
        } else if !before.menu && after.menu {
            Some(Sfx::Confirm)
        } else if after.menu {
            Some(match action {
                Action::GbaDown(Btn::B) => Sfx::Back,
                Action::GbaDown(Btn::A | Btn::Start | Btn::X | Btn::Y) => Sfx::Confirm,
                _ => Sfx::Move,
            })
        } else if matches!(self.phase, Phase::Shelf) && after.favorites != before.favorites {
            // a star put on a cart or taken off
            Some(Sfx::Confirm)
        } else {
            None
        };
        if let Some(sfx) = sfx {
            self.play_menu_sound(sfx);
        }
    }

    /// Ticks as the shelf's pick moves, held or pressed; called once a frame.
    pub(crate) fn tick_shelf_sound(&mut self) {
        let pick = (self.shelf_at, self.shelf().index);
        let moved = self.sound_pick.is_some_and(|was| was != pick);
        let same_shelf = self.sound_pick.is_some_and(|(shelf, _)| shelf == pick.0);
        self.sound_pick = Some(pick);
        if moved && same_shelf && matches!(self.phase, Phase::Shelf) && !self.menu_up() {
            self.play_menu_sound(Sfx::Move);
        }
    }

    /// Plays one of the menus' sounds, unless Menu Sounds is off or a cart's own sound is due
    /// this frame.
    fn play_menu_sound(&mut self, sfx: Sfx) {
        if self.state.menu_sounds && self.sfx.is_none_or(Sfx::is_menu) {
            self.sfx = Some(sfx);
        }
    }
}
