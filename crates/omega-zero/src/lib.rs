//! omega-zero itself: the app, its screens and its session. [`app::App`] holds what is on the
//! screen and answers each press; [`session::Session`] ties it to the core, the sound and the
//! card; `frontend` draws it each frame. `main.rs` runs it on the handheld or in a window.
//!
//! The map of the code, and how to add a setting, a screen, a toast or a sound, is in
//! `DEVELOPING.md`.

pub mod achievements;
pub mod app;
pub mod audio;
pub mod background_screen;
pub mod backup_settings;
pub mod bluetooth;
pub mod bluetooth_screen;
pub mod boot_fade;
pub mod bootlogo;
pub mod build_info;
pub mod cable;
pub mod card_watch;
pub mod card_writer;
pub mod cart_editor;
pub mod core;
pub mod core_picker;
pub mod disc_screen;
pub mod drc;
pub mod emu;
pub mod face_builder;
pub mod favorites;
pub mod frames;
pub mod frontend;
pub mod gpu_core;
pub mod headphones;
pub mod headphones_menu;
pub mod input;
pub mod json;
pub mod label_cutter;
pub mod label_screen;
pub mod label_sync;
pub mod latency;
pub mod link_kind;
pub mod link_net;
pub mod link_radio;
pub mod link_screen;
pub mod link_start;
pub mod link_state;
pub mod list_view;
pub mod log;
mod menu_sounds;
pub mod nas;
pub mod nav;
pub mod netplay;
pub mod netplay_lan;
pub mod netplay_lobby;
pub mod opening;
pub mod options;
pub mod pause_menu;
pub mod persist;
pub mod play_log;
pub mod resample;
pub mod rewind;
pub mod root;
pub mod save_backup;
pub mod save_manager;
pub mod saves_settings;
pub mod screens;
mod screenshot;
pub mod session;
pub mod spawn;
pub mod ssh;
pub mod start_notice;
pub mod stopwatch;
pub mod thumb;
pub mod time_zone;
pub mod update;
pub mod video_mode;
pub mod wallpaper;
pub mod wifi;
pub mod wifi_screen;
