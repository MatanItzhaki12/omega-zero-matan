//! The Labels screen, opened from Settings: the ScreenScraper account, and a sync that fetches
//! the real labels of the carts that have none.
//!
//! It is a lone panel, as flow.html's `#screen=labels`: SCREENSCRAPER (SIGN IN, or the account)
//! and FETCH LABELS, then two lines saying how the sync stands. The keys it has beyond B and A
//! (Y SIGN OUT, START FETCH) are hints on the bottom bar, between its actions. Its account is
//! typed on the keyboard screen.

use omega_input::Action;
use omega_ui::kit::bar::Hint;
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::row::panel_height;
use omega_ui::{Draw, UndoFace};

use crate::app::{App, Phase};
use crate::label_screen::Outcome as LabelsOutcome;
use crate::list_view::{LegendHint, ListView};
use crate::nav::Dir;
use crate::screens::lone::{hold, notes, notes_height, rows_height, Lone, NOTE_GAP};

/// How wide the panel is, in art pixels: flow.css's .labels-panel.
pub const WIDTH: u32 = 280;

/// The panel's title.
pub const TITLE: &str = "LABELS";

/// The note lines the panel keeps room for, whatever it says, so it holds still as the sync
/// goes on.
pub const NOTE_LINES: usize = 2;

/// The LABELS panel holding `view`'s rows and its notes under them, in `pal`.
pub fn labels_face(view: &ListView, pal: &Palette) -> UndoFace {
    let rows = rows_height(view.rows.len());
    let content = rows + notes_height(NOTE_LINES);
    let mut panel = panel_face(WIDTH, panel_height(content), TITLE, true, pal);
    hold(&mut panel, &view.rows, view.lit, pal);
    let shown = &view.status[..view.status.len().min(NOTE_LINES)];
    notes(&mut panel, shown, rows + NOTE_GAP, pal);
    panel.face
}

/// What the bottom bar shows between its actions for a Labels screen whose legend is `legend`:
/// the keys beyond B (RETURN) and A (CONFIRM), Y to sign out and START to fetch, or to stop a
/// fetch.
pub fn labels_hints(legend: &[LegendHint]) -> Vec<Hint<'static>> {
    legend
        .iter()
        .filter_map(|hint| match hint {
            LegendHint::SignOut => Some(Hint {
                key: "Y",
                word: "SIGN OUT",
            }),
            LegendHint::Start => Some(Hint {
                key: "START",
                word: "FETCH",
            }),
            LegendHint::Stop => Some(Hint {
                key: "START",
                word: "STOP",
            }),
            _ => None,
        })
        .collect()
}

impl App {
    pub(crate) fn labels_input(&mut self, action: Action) {
        let root = self.root.clone();
        let Phase::Labels(screen) = &mut self.phase else {
            return;
        };
        let before = screen.clone();
        let outcome = screen.input(&mut self.labels, action, root.as_deref());
        // the keyboard slides in over the list and away again; from the user name to the
        // password it stays where it is
        match (before.typing.is_some(), screen.typing.is_some()) {
            (false, true) => self.slide_from(Phase::Labels(before), Dir::Deeper),
            (true, false) => self.slide_from(Phase::Labels(before), Dir::Back),
            _ => {}
        }
        match outcome {
            LabelsOutcome::Close => self.go(
                Phase::Options(crate::options::OptionsScreen {
                    row: crate::options::OptionRow::Labels,
                    ..crate::options::OptionsScreen::new(crate::options::OptionsPage::Shelf)
                }),
                Dir::Back,
            ),
            LabelsOutcome::Sync if self.online() => {
                let wants = self.label_wants();
                self.labels.start(wants);
            }
            LabelsOutcome::Holo => {
                self.state.holo = !self.state.holo;
                self.show_holo();
                self.persist();
            }
            LabelsOutcome::Sync | LabelsOutcome::Stay => {}
        }
    }

    /// The Labels screen's list, while it is open and not taking an account.
    pub fn labels_view(&self) -> Option<ListView> {
        match &self.phase {
            Phase::Labels(screen) if screen.typing.is_none() => {
                Some(screen.view(&self.labels, self.online(), self.state.holo))
            }
            _ => None,
        }
    }

    /// The hints the bottom bar shows on the Labels screen (see [`labels_hints`]); none while
    /// it is not open, or is taking an account.
    pub fn labels_bar_hints(&self) -> Vec<Hint<'static>> {
        self.labels_view()
            .map(|view| labels_hints(&view.legend))
            .unwrap_or_default()
    }

    pub fn set_labels_panel(&mut self, panel: Lone) {
        self.labels_panel = Some(panel);
    }

    pub(crate) fn draw_labels(&self, out: &mut Vec<Draw>) {
        if let Some(panel) = &self.labels_panel {
            panel.draw(out);
        }
    }
}
