//! A yes-or-no question over a screen: a panel with its title and the question, as small as the
//! question, and the bar's actions turned to B NO and A YES. Taking a game's star off asks one
//! over the shelf (`unfavorite`), and Settings › Saves asks one over its panels before putting
//! a backup back or deleting anything.

use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::pixel::text_width;
use omega_ui::kit::row::panel_height;
use omega_ui::UndoFace;

use crate::screens::lone::{notes, notes_height, NOTE_GAP, NOTE_X};

/// The room either side of the question, inside the panel's frame, in art pixels.
const MARGIN: u32 = 8;

/// The panel asking `question` under the title `title`, in `pal`: as wide as the question, or
/// as its title's tab, and as tall as its one line.
pub fn question_face(title: &str, question: &str, pal: &Palette) -> UndoFace {
    let lines = [question.to_string()];
    let tab = text_width(title) + 2 * MARGIN + 12;
    let mut panel = panel_face(
        (text_width(question) + NOTE_X + 2 * MARGIN).max(tab),
        panel_height(notes_height(lines.len())),
        title,
        true,
        pal,
    );
    notes(&mut panel, &lines, NOTE_GAP, pal);
    panel.face
}
