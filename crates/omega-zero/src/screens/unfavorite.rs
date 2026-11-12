//! Taking a game's star off asks first. Y on a starred game puts a FAVORITES panel up over the
//! shelf, the question over the game's name, and the bar's actions turn to B NO and A YES: A
//! takes the star off, and B keeps it. Starring a game asks nothing.

use omega_input::{Action, Btn};
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::pixel::LINE;
use omega_ui::kit::row::panel_height;
use omega_ui::{Draw, UndoFace};

use crate::app::App;
use crate::screens::lone::{notes, Lone, NOTE_GAP};
use crate::screens::power::WIDTH;

/// The panel's title.
pub const TITLE: &str = "FAVORITES";

/// The question the panel asks, over the name of the game it is about.
pub const QUESTION: &str = "Remove from favorites?";

/// The panel asking whether to take the star off the game called `name`, in `pal`: the
/// question, and the name under it in the dim colour, cut short where it is too long.
pub fn unfavorite_face(name: &str, pal: &Palette) -> UndoFace {
    let lines = [QUESTION.to_string(), name.to_string()];
    let mut panel = panel_face(
        WIDTH,
        panel_height(NOTE_GAP + lines.len() as u32 * LINE + 5),
        TITLE,
        true,
        pal,
    );
    notes(&mut panel, &lines, NOTE_GAP, pal);
    panel.face
}

impl App {
    /// Whether the shelf is asking to take the picked game's star off.
    pub fn unfavorite_asked(&self) -> bool {
        self.unfavorite_asked
    }

    pub fn set_unfavorite_panel(&mut self, panel: Lone) {
        self.unfavorite_panel = Some(panel);
    }

    /// The name of the game the question is about, as the shelf titles it.
    pub fn unfavorite_name(&self) -> Option<String> {
        self.selected_stem().map(omega_ui::clean_label)
    }

    /// Y on the shelf: a game with no star gets one at once, and one with a star asks first.
    pub(crate) fn favorite_pressed(&mut self, now: omega_input::Millis) {
        let starred = self
            .shelf()
            .carts
            .get(self.shelf().index)
            .is_some_and(|c| self.is_favorite(c));
        match starred {
            true => self.unfavorite_asked = true,
            false => self.toggle_favorite(now),
        }
    }

    /// A press while the question is up: A takes the star off, B keeps it, and the rest wait.
    pub(crate) fn unfavorite_input(&mut self, action: Action) {
        match action {
            Action::GbaDown(Btn::A) => {
                self.unfavorite_asked = false;
                let now = self.now();
                self.toggle_favorite(now);
            }
            Action::GbaDown(Btn::B) => self.unfavorite_asked = false,
            _ => {}
        }
    }

    /// The question over the shelf, while it is up.
    pub(crate) fn draw_unfavorite(&self, out: &mut Vec<Draw>) {
        if let (true, Some(panel)) = (self.unfavorite_asked, &self.unfavorite_panel) {
            panel.draw(out);
        }
    }
}
