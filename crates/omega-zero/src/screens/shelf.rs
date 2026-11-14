//! The shelf: the carts in a row over the background and the bar across the top. Each is a
//! layer of its own as the shelf slides away to a menu and back. The bar across the bottom, with
//! the shelf's system in its pill, is drawn over every screen (bar.rs).

use omega_ui::kit::layout::screen_px;
use omega_ui::kit::pixel::quad;
use omega_ui::{ease, Draw, TexId, TopBar, SPRITE_PX};

use crate::app::App;
use crate::nav::Offsets;

const CORE_PICKER_RECEDE: f32 = 0.26;
const CORE_PICKER_DIM: f32 = 0.614;
/// How far under the top bar the cart's name stands, in art pixels.
const TITLE_GAP: u32 = 5;

impl App {
    /// The shelf, its carts moved `off.carts_dx` across and `off.carts_dy` down, its top bar
    /// `off.bar_dy` down.
    pub(crate) fn draw_shelf(&self, off: Offsets, out: &mut Vec<Draw>) {
        let carts = out.len();
        match (self.core_picker_shown(), self.selected_stem()) {
            (Some(picker), Some(stem)) => {
                let open = ease(picker.openness(self.now()));
                let dim = 1.0 + (CORE_PICKER_DIM - 1.0) * open;
                self.shelf()
                    .draw_row(Some(stem), 0.0, CORE_PICKER_RECEDE * open, dim, out);
                shift(&mut out[carts..], off.carts_dx, off.carts_dy);
            }
            _ => {
                self.shelf()
                    .draw_row(None, self.shelf_shake(), 0.0, 1.0, out);
                // the cart editor open: the row shrinks and slides to the left, the cart picked
                // level with the panel, its name out of the way
                let edit = self.cart_editor_openness();
                if edit > 0.0 {
                    crate::cart_editor::place_row(&mut out[carts..], edit);
                } else {
                    self.draw_title(out);
                }
                shift(&mut out[carts..], off.carts_dx, off.carts_dy);
            }
        }
        let bar = out.len();
        self.draw_top_bar(out);
        shift(&mut out[bar..], 0.0, off.bar_dy);
        self.draw_unfavorite(out);
    }

    /// The picked cart's name, while Settings > Shelf shows Game Titles: centred under the top
    /// bar, above the carts.
    fn draw_title(&self, out: &mut Vec<Draw>) {
        let (Some(_), Some((tex, w, h))) = (self.shelf_title(), self.title_face) else {
            return;
        };
        let x = (omega_gfx::out_w() / SPRITE_PX).saturating_sub(w) / 2;
        let y = (omega_ui::TOP_BAR_H as u32) / SPRITE_PX + TITLE_GAP;
        out.push(quad(tex, w, h, screen_px(x), screen_px(y)));
    }

    /// The picked cart's name, for the shelf to show over it while Game Titles is on.
    pub fn shelf_title(&self) -> Option<String> {
        if !self.state.show_titles || self.core_picker_shown().is_some() {
            return None;
        }
        self.selected_stem().map(omega_ui::clean_label)
    }

    pub fn set_title_face(&mut self, tex: TexId, w: u32, h: u32) {
        self.title_face = Some((tex, w, h));
    }

    /// The bar across the top, in the chrome's colours, its faces baked in its ink.
    pub(crate) fn draw_top_bar(&self, out: &mut Vec<Draw>) {
        let r = self.top_readouts;
        TopBar {
            colours: self.chrome_colours(),
            links: self.links(),
            faces: self.link_faces.as_ref(),
            clock: r.clock,
            battery: self.battery,
            percent: r.percent,
            bolt: r.bolt,
        }
        .draw(out);
    }
}

/// Moves each of `drawn` `dx` across and `dy` down.
fn shift(drawn: &mut [Draw], dx: f32, dy: f32) {
    for d in drawn {
        *d = d.shifted(dx, dy);
    }
}
