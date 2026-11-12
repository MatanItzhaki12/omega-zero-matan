//! Taking a game's star off asks first. Y on a starred game puts a FAVORITES panel up over the
//! shelf with the question, and the bar's actions turn to B NO and A YES: A takes the star off,
//! and B keeps it. Starring a game asks nothing.

use omega_input::{Action, Btn};
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::pixel::text_width;
use omega_ui::kit::row::panel_height;
use omega_ui::{Draw, UndoFace};

use crate::app::App;
use crate::screens::lone::{notes, notes_height, Lone, NOTE_GAP, NOTE_X};

/// The room either side of the question, inside the panel's frame, in art pixels.
const MARGIN: u32 = 8;

/// The panel's title.
pub const TITLE: &str = "FAVORITES";

/// The question the panel asks, about the game picked under it.
pub const QUESTION: &str = "Remove from favorites?";

/// The panel asking whether to take the picked game's star off, in `pal`: as wide as the
/// question and as tall as its one line.
pub fn unfavorite_face(pal: &Palette) -> UndoFace {
    let lines = [QUESTION.to_string()];
    let mut panel = panel_face(
        text_width(QUESTION) + NOTE_X + 2 * MARGIN,
        panel_height(notes_height(lines.len())),
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
