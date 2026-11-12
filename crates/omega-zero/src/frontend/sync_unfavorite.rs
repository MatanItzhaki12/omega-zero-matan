//! The FAVORITES panel asking whether to take a game's star off, baked while the shelf asks:
//! again for another game's name, or the palette.

use omega_gfx::Compositor;

use crate::app::App;
use crate::frontend::lone::Baked;
use crate::screens::unfavorite::unfavorite_face;

pub(crate) fn sync_unfavorite(
    app: &mut App,
    compositor: &mut Compositor,
    state: &mut Baked<(String, u8)>,
) {
    if !app.unfavorite_asked() {
        return;
    }
    let name = app.unfavorite_name().unwrap_or_default();
    let pal = app.palette();
    if let Some(panel) = state.bake(compositor, (name, app.palette_colour()), |(name, _)| {
        unfavorite_face(name, pal)
    }) {
        app.set_unfavorite_panel(panel);
    }
}
