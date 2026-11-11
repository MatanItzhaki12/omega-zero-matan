//! The buttons, from raw presses to what they mean: [`Gestures`] turns the handheld's events
//! into [`Action`]s, taps, holds and SELECT chords, with the timings omega-zero and Slot share,
//! and [`Keys`] says which buttons the game being played keeps for itself.

mod gesture;
mod raw;

pub use gesture::{
    Action, Gestures, Keys, MENU_DOUBLE_TAP_MS, MENU_HOLD_MS, MENU_TAP_MS, MUTE_CHORD_MS,
    POWER_HOLD_MS, SELECT_CHORD_MS, SELECT_TAP_MS, VOLUME_REPEAT_DELAY_MS, VOLUME_REPEAT_MS,
};
pub use raw::{Btn, InputSource, Millis, RawEvent};
