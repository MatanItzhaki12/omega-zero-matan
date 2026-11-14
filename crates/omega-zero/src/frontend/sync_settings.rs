//! Settings' faces, baked as little and as seldom as they can be: the modal with its panels and
//! the two lit titles once per canvas and palette, and a row only when what it says, whether it
//! is lit or the palette changes. Moving the cursor bakes the two rows it leaves and lands on;
//! the clock's minute bakes the Date & Time row alone.

use omega_gfx::{Compositor, TexId};
use omega_ui::{
    settings_frame, settings_row_face, settings_row_width, settings_title_face, SettingsBaked,
    SettingsFaces, SettingsGroup, SettingsRow,
};

use crate::app::App;
use crate::frontend::faces::upload_nearest;

/// The textures Settings is drawn with, each filled again in place, and what they were baked
/// from.
#[derive(Default)]
pub(crate) struct SettingsTexes {
    baked: SettingsBaked,
    frame: Option<TexId>,
    titles: [Option<TexId>; 2],
    rows: [Option<TexId>; SettingsRow::ALL.len()],
    faces: Option<SettingsFaces>,
}

pub(crate) fn sync_settings(app: &mut App, compositor: &mut Compositor, state: &mut SettingsTexes) {
    if app.settings_row().is_none() {
        return;
    }
    let shown = app.settings_shown();
    let plan = state.baked.rebake(app.palette_colour(), &shown);
    if !plan.frame && plan.rows.is_empty() && plan.gone.is_empty() {
        return;
    }
    let pal = app.palette();
    if plan.frame || state.faces.is_none() {
        let rows: Vec<SettingsRow> = shown.iter().map(|s| s.row).collect();
        let frame = settings_frame(omega_ui::top_in(&rows), pal);
        let frame_tex = upload_nearest(compositor, &mut state.frame, &frame.face);
        let titles = SettingsGroup::ALL.map(|g| {
            let face = settings_title_face(g, pal);
            let tex = upload_nearest(compositor, &mut state.titles[g.index()], &face);
            (tex, [face.w, face.h])
        });
        state.faces = Some(SettingsFaces::new(&frame, frame_tex, titles));
    }
    let Some(faces) = state.faces.as_mut() else {
        return;
    };
    for row in plan.gone {
        faces.rows[row.index()] = None;
    }
    for shown in plan.rows {
        let w = settings_row_width(faces.wells[shown.row.group().index()][2]);
        let face = settings_row_face(w, &shown, pal);
        let i = shown.row.index();
        let tex = upload_nearest(compositor, &mut state.rows[i], &face);
        faces.rows[i] = Some((tex, [face.w, face.h]));
    }
    app.set_settings_faces(faces.clone());
}
