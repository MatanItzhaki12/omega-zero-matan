//! The core picker's board and lid, asked of the face builder for the cart the shelf is on and
//! uploaded once they come back: the board, pixel art at a texel to an art pixel, with nearest
//! filtering.

use omega_gfx::{Compositor, TexId};
use omega_ui::UndoFace;

use crate::app::App;
use crate::face_builder::FaceBuilder;
use crate::frontend::faces::{upload_nearest, upload_rgba};

pub(crate) fn sync_core_picker(
    app: &mut App,
    compositor: &mut Compositor,
    builder: &FaceBuilder,
    asked: &mut Option<String>,
    board: &mut Option<TexId>,
    lid: &mut Option<TexId>,
    built: &mut Option<String>,
) {
    let highlighted = app.selected_stem().map(str::to_string);
    if highlighted.is_some() && *asked != highlighted {
        if let Some(cart) = app
            .carts()
            .find(|c| highlighted.as_deref() == Some(c.stem.as_str()))
        {
            builder.request(cart.clone());
        }
        asked.clone_from(&highlighted);
    }
    let Some(faces) = builder.take() else {
        return;
    };
    if highlighted.as_deref() != Some(faces.stem.as_str()) || *built == highlighted {
        return;
    }
    let board_face = UndoFace {
        rgba: faces.board.rgba,
        w: faces.board.w,
        h: faces.board.h,
    };
    let board_id = upload_nearest(compositor, board, &board_face);
    let lid_id = upload_rgba(compositor, lid, faces.lid.w, faces.lid.h, &faces.lid.rgba);
    app.set_core_board_faces(board_id, lid_id);
    *built = Some(faces.stem);
}
