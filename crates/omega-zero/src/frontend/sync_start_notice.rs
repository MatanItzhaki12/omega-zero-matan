//! The start notice's lines, made as the opening's are, each again when what it says changes:
//! the cursor blinking after its prompt. Once the notice is put away their textures shrink to
//! a pixel and give their memory back.

use omega_gfx::Compositor;

use crate::app::App;
use crate::frontend::sync_opening::TerminalRows;

pub(crate) fn sync_start_notice(
    app: &mut App,
    compositor: &mut Compositor,
    state: &mut TerminalRows,
) {
    if !app.start_notice_shown() {
        return state.free(compositor);
    }
    let lines = app.start_notice_lines();
    state.sync(compositor, lines, |row, tex, w, h| {
        app.set_start_notice_face(row, tex, w, h);
    });
}
