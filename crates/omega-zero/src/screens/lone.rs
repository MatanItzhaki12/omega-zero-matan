//! A panel on its own in the middle of the page, with no modal around it, as flow.html's
//! CONTROLS, THEME, DATE & TIME and LABELS are. Each is one face at art resolution, its
//! chrome and everything it holds baked in, and drawn as one quad; it is baked again only when
//! what it shows, or the palette, changes.

use omega_gfx::{Draw, TexId};
use omega_ui::kit::layout::{centred, screen_px};
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::PanelFace;
use omega_ui::kit::pixel::{quad, text_width, LINE};
use omega_ui::kit::row::{row_face, row_rect, Row, ROW_H};
use omega_ui::kit::text::text_face;
use omega_ui::UndoFace;

use crate::list_view::{RowLabel, RowValue, ViewRow};

/// A lone panel uploaded: its texture, and its size in art pixels.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Lone {
    pub tex: TexId,
    pub w: u32,
    pub h: u32,
}

impl Lone {
    /// Where the panel goes, `[x, y, w, h]` in art pixels: in the middle of the page above the
    /// bottom bar, rounded down to a whole art pixel.
    pub fn rect(&self) -> [u32; 4] {
        centred(self.w, self.h)
    }

    /// The panel where it goes, at two screen pixels to an art pixel.
    pub fn draw(&self, out: &mut Vec<Draw>) {
        let [x, y, w, h] = self.rect();
        out.push(quad(self.tex, w, h, screen_px(x), screen_px(y)));
    }
}

/// How far a note stands in from the well's inside, and below the rows over it.
pub const NOTE_X: u32 = 3;
pub const NOTE_GAP: u32 = 4;

/// What a note is cut short with when it is too long for its panel.
const ELLIPSIS: char = '…';

/// `top` laid over `under` with its top left at (`x`, `y`), where it is not clear; what falls
/// outside `under` is dropped.
pub fn over(under: &mut UndoFace, top: &UndoFace, x: u32, y: u32) {
    for ty in 0..top.h.min(under.h.saturating_sub(y)) {
        for tx in 0..top.w.min(under.w.saturating_sub(x)) {
            let from = ((ty * top.w + tx) * 4) as usize;
            if top.rgba[from + 3] == 0 {
                continue;
            }
            let to = (((y + ty) * under.w + x + tx) * 4) as usize;
            under.rgba[to..to + 4].copy_from_slice(&top.rgba[from..from + 4]);
        }
    }
}

/// A list's row in the words a kit row shows: its name, its value (in capitals, but for a name
/// that is not omega-zero's to set, such as an account's) and whether Left and Right change it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Words {
    pub label: String,
    pub value: Option<String>,
    pub carets: bool,
    pub dim: bool,
}

impl Words {
    pub fn of(row: &ViewRow) -> Words {
        let label = match &row.label {
            RowLabel::Setting(name) => name.to_string(),
            RowLabel::Name(name) => name.clone(),
        };
        let value = row.value.as_ref().map(|value| match value {
            RowValue::Setting(true) => "ON".to_string(),
            RowValue::Setting(false) => "OFF".to_string(),
            RowValue::Choice(text) | RowValue::Said(text) => text.to_uppercase(),
            RowValue::Name(name) => name.clone(),
        });
        let carets = matches!(row.value, Some(RowValue::Setting(_) | RowValue::Choice(_)));
        Words {
            label,
            value,
            carets,
            dim: row.dim,
        }
    }

    /// The kit row showing these words, lit when the cursor is on it.
    pub fn row(&self, lit: bool) -> Row<'_> {
        Row {
            label: &self.label,
            value: self.value.as_deref(),
            lit,
            carets: self.carets,
            dim: self.dim,
            ..Row::default()
        }
    }
}

/// A list's rows in `panel`'s well, one under the other from its top, as omega-panel lays them
/// out, the one at `lit` under the cursor.
pub fn hold(panel: &mut PanelFace, rows: &[ViewRow], lit: Option<usize>, pal: &Palette) {
    hold_rows(panel, rows, lit, false, pal);
}

/// As [`hold`], every value kept clear of the carets' room, so those without carets line up
/// with those that have them.
pub fn hold_aligned(panel: &mut PanelFace, rows: &[ViewRow], lit: Option<usize>, pal: &Palette) {
    hold_rows(panel, rows, lit, true, pal);
}

fn hold_rows(
    panel: &mut PanelFace,
    rows: &[ViewRow],
    lit: Option<usize>,
    aligned: bool,
    pal: &Palette,
) {
    for (i, row) in rows.iter().enumerate() {
        let [x, y, w, _] = row_rect(panel.well, i as u32);
        let words = Words::of(row);
        let row = Row {
            caret_room: aligned,
            ..words.row(lit == Some(i))
        };
        let face = row_face(w, &row, pal);
        over(&mut panel.face, &face, x, y);
    }
}

/// How tall the rows of a well are.
pub fn rows_height(rows: usize) -> u32 {
    rows as u32 * ROW_H
}

/// How tall `lines` notes are under rows: `NOTE_GAP` above them, a line each, and 2 under the
/// last, so its capitals stand as far from the well's bottom as the first row's from its top.
pub fn notes_height(lines: usize) -> u32 {
    NOTE_GAP + lines as u32 * LINE + 2
}

/// The notes `lines` in `panel`'s well, `below` its top, in the dim colour as flow.css's .note
/// draws them, a line each; one too long for the well is cut short with an ellipsis.
pub fn notes(panel: &mut PanelFace, lines: &[String], below: u32, pal: &Palette) {
    let [x, y, w, _] = panel.well;
    let room = w.saturating_sub(NOTE_X);
    for (i, line) in lines.iter().enumerate() {
        let face = text_face(&fitted(line, room), pal.dim);
        over(
            &mut panel.face,
            &face,
            x + NOTE_X,
            y + below + i as u32 * LINE,
        );
    }
}

/// `text` as it fits `room` art pixels across: whole, or cut short with an ellipsis.
pub fn fitted(text: &str, room: u32) -> String {
    if text_width(text) <= room {
        return text.to_string();
    }
    let mut shown: String = text.to_string();
    while !shown.is_empty() && text_width(&format!("{shown}{ELLIPSIS}")) > room {
        shown.pop();
    }
    format!("{}{ELLIPSIS}", shown.trim_end())
}
