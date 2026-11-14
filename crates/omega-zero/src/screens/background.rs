//! The THEME screen, opened from Settings: a lone panel, as flow.html's `#screen=background`,
//! its rows BACKGROUND, MOTION and COLOUR drawn over the background they are choosing, and FONT
//! over the bar it is choosing for. A new colour re-tints the panel, and every screen after it,
//! the next frame; a new font redraws the bar's actions under it in that frame too.

use omega_input::Action;
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::row::panel_height;
use omega_ui::{Draw, UndoFace};

use crate::app::{App, Phase};
use crate::background_screen::Outcome as BackgroundOutcome;
use crate::list_view::ListView;
use crate::nav::Dir;
use crate::screens::lone::{hold, rows_height, Lone};

/// How wide the panel is, in art pixels: flow.css's .lone.small.
pub const WIDTH: u32 = 200;

/// The panel's title.
pub const TITLE: &str = "THEME";

/// The THEME panel holding `view`'s rows, in `pal`.
pub fn background_face(view: &ListView, pal: &Palette) -> UndoFace {
    let content = rows_height(view.rows.len());
    let mut panel = panel_face(WIDTH, panel_height(content), TITLE, true, pal);
    hold(&mut panel, &view.rows, view.lit, pal);
    panel.face
}

impl App {
    pub(crate) fn background_input(&mut self, action: Action) {
        let (look, font, wallpaper) = (self.backdrop(), self.state.font, self.wallpaper.is_some());
        let Phase::Background(screen) = &mut self.phase else {
            return;
        };
        match screen.input(look, font, wallpaper, action) {
            BackgroundOutcome::Close => self.go(
                Phase::Options(crate::options::OptionsScreen {
                    row: crate::options::OptionRow::Theme,
                    ..crate::options::OptionsScreen::new(crate::options::OptionsPage::Shelf)
                }),
                Dir::Back,
            ),
            BackgroundOutcome::Changed(b) => {
                self.state.backdrop = Some(b.style);
                self.state.backdrop_moving = b.moving;
                self.state.backdrop_colour = b.colour;
                self.persist();
            }
            BackgroundOutcome::Font(font) => {
                self.state.font = font;
                self.persist();
            }
            BackgroundOutcome::Stay => {}
        }
    }

    /// The THEME screen's list, while it is open.
    pub fn background_view(&self) -> Option<ListView> {
        match &self.phase {
            Phase::Background(screen) => Some(screen.view(self.backdrop(), self.state.font)),
            _ => None,
        }
    }

    pub fn set_background_panel(&mut self, panel: Lone) {
        self.background_panel = Some(panel);
    }

    /// The panel, over the background drawn under every screen, with nothing between.
    pub(crate) fn draw_background_screen(&self, out: &mut Vec<Draw>) {
        if let Some(panel) = &self.background_panel {
            panel.draw(out);
        }
    }
}
