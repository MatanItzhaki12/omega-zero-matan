//! The FAVORITES panel asking whether to take a game's star off, baked while the shelf asks:
//! again for another palette.

use omega_gfx::Compositor;

use crate::app::App;
use crate::frontend::lone::Baked;
use crate::screens::unfavorite::unfavorite_face;

pub(crate) fn sync_unfavorite(app: &mut App, compositor: &mut Compositor, state: &mut Baked<u8>) {
    if !app.unfavorite_asked() {
        return;
    }
    let pal = app.palette();
    if let Some(panel) = state.bake(compositor, app.palette_colour(), |_| unfavorite_face(pal)) {
        app.set_unfavorite_panel(panel);
    }
}
