//! The questions over a screen, baked while one is up: the FAVORITES panel asking whether to
//! take a game's star off, and the SAVES panel asking whether to restore or delete. Each is
//! baked again for another question or palette.

use omega_gfx::Compositor;

use crate::app::App;
use crate::frontend::lone::Baked;
use crate::saves_settings::QUESTION_TITLE;
use crate::screens::question::question_face;
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

pub(crate) fn sync_saves_question(
    app: &mut App,
    compositor: &mut Compositor,
    state: &mut Baked<(&'static str, u8)>,
) {
    let Some(answer) = app.saves_asking() else {
        return;
    };
    let pal = app.palette();
    let baked = state.bake(
        compositor,
        (answer.question(), app.palette_colour()),
        |(question, _)| question_face(QUESTION_TITLE, question, pal),
    );
    if let Some(panel) = baked {
        app.set_saves_question(panel);
    }
}
