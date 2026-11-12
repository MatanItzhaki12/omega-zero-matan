//! Each menu screen's input and drawing, as `impl App` blocks: `App::apply` hands a press to
//! the screen it is on and `App::draw` draws the screen it shows, and what each screen does
//! with them lives in its own file here.

pub mod about;
pub mod background;
pub mod bar;
pub mod bluetooth;
pub mod clock;
pub mod controls;
pub mod delete_state;
pub mod info;
pub mod keyboard;
pub mod labels;
pub mod lone;
pub mod network;
pub mod power;
pub mod settings;
pub mod shelf;
pub mod ssh;
pub mod unfavorite;
