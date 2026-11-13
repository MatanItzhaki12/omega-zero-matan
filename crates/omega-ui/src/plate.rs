use std::sync::OnceLock;

use crate::text;

/// How tall a line of the old text path is, from the top of its face to the bottom.
pub const HINT_H: u32 = 24;
const LABEL_PX: f32 = 18.0;

pub struct UndoFace {
    pub rgba: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

/// How far below the top of a line of the old text path its capitals and figures start.
pub fn word_cap_top() -> f32 {
    static TOP: OnceLock<f32> = OnceLock::new();
    *TOP.get_or_init(|| {
        text::label_font().map_or(0.0, |font| text::cap_top(font, HINT_H, LABEL_PX))
    })
}
