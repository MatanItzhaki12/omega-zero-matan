//! The menus' sounds: a tick as the cursor moves, two notes up as a choice is taken, two down
//! as a menu is left, and a soft glide as the shelf changes. They are made for the rate they
//! play at, out of sines with a little of their octave, so they sit gently over a game's sound
//! or the shelf's tune rather than cutting through it.

use std::f32::consts::TAU;

use super::Sfx;

/// How long each runs, in seconds.
pub(super) const MOVE_S: f32 = 0.05;
pub(super) const CONFIRM_S: f32 = 0.16;
pub(super) const BACK_S: f32 = 0.14;
pub(super) const SHELF_S: f32 = 0.16;

/// E6, the cursor's tick.
const MOVE_HZ: f32 = 1318.5;
/// A5 then E6, up a fifth.
const CONFIRM_HZ: [f32; 2] = [880.0, 1318.5];
/// D6 then G5, down a fifth.
const BACK_HZ: [f32; 2] = [1174.7, 784.0];
/// The glide from C5 to G5.
const SHELF_HZ: (f32, f32) = (523.3, 784.0);

/// How loud each peaks, as a share of full scale: clear over the tune and a game, still under
/// the cart sounds.
const MOVE_LEVEL: f32 = 0.36;
const CONFIRM_LEVEL: f32 = 0.43;
const BACK_LEVEL: f32 = 0.4;
const SHELF_LEVEL: f32 = 0.33;

/// `sfx`, one of the menus', as samples at `rate` scaled to i16's range.
pub(super) fn render(sfx: Sfx, rate: u32) -> Vec<f32> {
    let rate = rate.max(1) as f32;
    let out = match sfx {
        Sfx::Move => tone(rate, MOVE_S, |_| MOVE_HZ, 0.012, MOVE_LEVEL),
        Sfx::Confirm => two(rate, CONFIRM_S, CONFIRM_HZ, CONFIRM_LEVEL),
        Sfx::Back => two(rate, BACK_S, BACK_HZ, BACK_LEVEL),
        Sfx::Shelf => tone(
            rate,
            SHELF_S,
            |t| SHELF_HZ.0 * (SHELF_HZ.1 / SHELF_HZ.0).powf((t / SHELF_S).min(1.0)),
            0.06,
            SHELF_LEVEL,
        ),
        _ => Vec::new(),
    };
    out.into_iter().map(|v| v * 32_767.0).collect()
}

/// A sine at `hz(t)` with a quarter of its octave, struck at once and dying away over `decay`
/// seconds, cut off by `secs`.
fn tone(rate: f32, secs: f32, hz: impl Fn(f32) -> f32, decay: f32, level: f32) -> Vec<f32> {
    let mut phase = 0.0f32;
    (0..(secs * rate) as usize)
        .map(|i| {
            let t = i as f32 / rate;
            phase = (phase + hz(t) / rate).fract();
            let voice = (phase * TAU).sin() + 0.25 * (phase * 2.0 * TAU).sin();
            level * voice * envelope(t, secs, decay)
        })
        .collect()
}

/// `notes` one after the other, each half of `secs`, the second overlapping the first's tail.
fn two(rate: f32, secs: f32, notes: [f32; 2], level: f32) -> Vec<f32> {
    let half = secs / 2.0;
    let first = tone(rate, secs, |_| notes[0], 0.03, level);
    let second = tone(rate, half, |_| notes[1], 0.045, level);
    let start = (half * rate) as usize;
    let mut out = first;
    for (k, v) in second.into_iter().enumerate() {
        if let Some(o) = out.get_mut(start + k) {
            *o = *o * 0.4 + v;
        }
    }
    out
}

/// A 2 ms rise, then an exponential fall over `decay`, faded to nothing by `secs` so it never
/// ends on a step.
fn envelope(t: f32, secs: f32, decay: f32) -> f32 {
    let rise = (t / 0.002).min(1.0);
    let fall = (-t / decay).exp();
    let end = ((secs - t) / 0.01).clamp(0.0, 1.0);
    rise * fall * end
}
