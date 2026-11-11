//! The clock: asked for at first boot, and opened from Settings' Date & Time.
//!
//! Both are the one lone panel, as flow.html's `#screen=datetime`: DATE & TIME, holding the
//! picker's line in the middle of its well with the field under the cursor underlined. The
//! bottom bar says what A and B do.

use omega_input::{Action, Btn};
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::pixel::LINE;
use omega_ui::kit::row::panel_height;
use omega_ui::{ClockPicker, Draw, SettingsRow, UndoFace};

use crate::app::{App, Phase};
use crate::nav::Dir;
use crate::screens::lone::{over, Lone};

/// How wide the panel is, in art pixels: flow.css's .datetime-panel.
pub const WIDTH: u32 = 240;

/// The panel's title.
pub const TITLE: &str = "DATE & TIME";

/// Where the line stands in the well: 6 under its top, and 10 clear under the line, where the
/// underline is, so its capitals have as much room above as the underline has below.
pub const LINE_TOP: u32 = 6;
const LINE_BELOW: u32 = 10;

/// The Date & Time panel holding `picker`, in `pal`.
pub fn clock_face(picker: &ClockPicker, pal: &Palette) -> UndoFace {
    let content = LINE_TOP + LINE + LINE_BELOW;
    let mut panel = panel_face(WIDTH, panel_height(content), TITLE, true, pal);
    let [x, y, w, _] = panel.well;
    let line = picker.face(pal);
    // across the well's middle by its letters, rounded down to a whole pixel
    over(
        &mut panel.face,
        &line,
        x + w.saturating_sub(line.w) / 2,
        y + LINE_TOP,
    );
    panel.face
}

impl App {
    pub(crate) fn clock_input(&mut self, action: Action) {
        let Phase::SetClock {
            picker, from_menu, ..
        } = &mut self.phase
        else {
            return;
        };
        match action {
            Action::GbaDown(Btn::Left) | Action::ShelfLeft => picker.left(),
            Action::GbaDown(Btn::Right) | Action::ShelfRight => picker.right(),
            Action::GbaDown(Btn::Up) => picker.up(),
            Action::GbaDown(Btn::Down) => picker.down(),
            Action::GbaDown(Btn::A) | Action::Insert => self.confirm_clock(),
            Action::GbaDown(Btn::B) if *from_menu => self.go(
                Phase::Settings {
                    row: SettingsRow::DateTime,
                },
                Dir::Back,
            ),
            _ => {}
        }
    }

    pub fn confirm_clock(&mut self) {
        let Phase::SetClock {
            picker,
            seed,
            from_menu,
        } = &self.phase
        else {
            return;
        };
        let (moved, offset, from_menu) = (picker.secs() - *seed, picker.offset_min(), *from_menu);
        let utc = self.utc_secs() + moved;
        if let Some(power) = &mut self.power {
            power.set_clock(utc);
        }
        self.state.utc_offset_min = offset as i16;
        self.state.clock_set = true;
        crate::time_zone::follow(self.state.utc_offset_min);
        self.persist();
        if from_menu {
            self.go(
                Phase::Settings {
                    row: SettingsRow::DateTime,
                },
                Dir::Back,
            );
        } else {
            self.start();
        }
    }

    pub fn picker(&self) -> Option<&ClockPicker> {
        match &self.phase {
            Phase::SetClock { picker, .. } => Some(picker),
            _ => None,
        }
    }

    pub fn set_clock_panel(&mut self, panel: Lone) {
        self.clock_panel = Some(panel);
    }

    /// The Date & Time panel, at first boot as from Settings.
    pub(crate) fn draw_clock(&self, out: &mut Vec<Draw>) {
        if let Some(panel) = &self.clock_panel {
            panel.draw(out);
        }
    }
}

pub(crate) fn clock_screen(utc: i64, offset_min: i16, from_menu: bool) -> Phase {
    Phase::SetClock {
        picker: ClockPicker::local(utc, offset_min),
        seed: utc - utc.rem_euclid(60),
        from_menu,
    }
}
