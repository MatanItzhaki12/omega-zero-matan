//! The opening's lines, each made again when what it says changes: a key typed, a count run
//! up. Once the opening is over their textures shrink to a pixel and give their memory back.

use omega_gfx::{Compositor, TexId};
use omega_ui::opening_line;

use crate::app::App;
use crate::frontend::faces::upload_nearest;

/// A terminal's rows in the opening's lettering, as uploaded.
#[derive(Default)]
pub(crate) struct TerminalRows {
    /// Each row's texture and what it says.
    rows: Vec<(Option<TexId>, String)>,
}

impl TerminalRows {
    /// Makes each of `lines` again whose row said something else, and hands its face to `set`.
    pub(crate) fn sync(
        &mut self,
        compositor: &mut Compositor,
        lines: Vec<(usize, String)>,
        mut set: impl FnMut(usize, TexId, u32, u32),
    ) {
        for (row, text) in lines {
            if self.rows.len() <= row {
                self.rows.resize_with(row + 1, Default::default);
            }
            let (tex, said) = &mut self.rows[row];
            if *said == text {
                continue;
            }
            let face = opening_line(&text);
            let id = upload_nearest(compositor, tex, &face);
            *said = text;
            set(row, id, face.w, face.h);
        }
    }

    /// Shrinks every row's texture to a pixel, giving its memory back.
    pub(crate) fn free(&mut self, compositor: &mut Compositor) {
        for (tex, _) in self.rows.drain(..) {
            if let Some(tex) = tex {
                compositor.update_texture(tex, 1, 1, &[0; 4]);
            }
        }
    }
}

pub(crate) fn sync_opening(app: &mut App, compositor: &mut Compositor, state: &mut TerminalRows) {
    if !app.opening_shown() {
        return state.free(compositor);
    }
    let lines = app.opening_lines();
    state.sync(compositor, lines, |row, tex, w, h| {
        app.set_opening_face(row, tex, w, h);
    });
}
