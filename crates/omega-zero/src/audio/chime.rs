//! The opening's sounds, after Mega Man X5's boot screen, measured off it: a power-on sweep, a
//! beep for each key and a warble for each file read. They are square waves made for the rate
//! they play at, softened above 8 kHz as the PlayStation's were.

use super::Sfx;

/// How long each sound runs, in seconds.
pub(super) const BLIP_S: f32 = 0.36;
pub(super) const KEY_S: f32 = 0.16;
pub(super) const READING_S: f32 = 0.55;
pub(super) const OMEGA_S: f32 = 0.9;

/// How loud each one peaks, as a share of full scale.
const BLIP_LEVEL: f32 = 0.22;
const KEY_LEVEL: f32 = 0.2;
const READING_LEVEL: f32 = 0.2;
const OMEGA_LEVEL: f32 = 0.24;

/// X5's file warble: A5, C6, F5, each held 45 ms.
const READING_NOTES: [f32; 3] = [877.0, 1046.5, 698.5];
/// OMEGA's, an octave down and with C5 twice, to sit longer.
const OMEGA_NOTES: [f32; 4] = [438.5, 523.3, 349.2, 523.3];
const NOTE_S: f32 = 0.045;

/// Where the softening starts, in Hz.
const SOFT_HZ: f32 = 8_000.0;

/// `sfx`, one of the opening's, as samples at `rate` scaled to i16's range.
pub(super) fn render(sfx: Sfx, rate: u32) -> Vec<f32> {
    let rate = rate.max(1) as f32;
    let mut out = match sfx {
        Sfx::Blip => blip(rate),
        Sfx::Key => key(rate),
        Sfx::Reading => warble(rate, READING_S, &READING_NOTES, READING_LEVEL),
        Sfx::Omega => warble(rate, OMEGA_S, &OMEGA_NOTES, OMEGA_LEVEL),
        _ => Vec::new(),
    };
    soften(&mut out, rate);
    soften(&mut out, rate);
    for v in &mut out {
        *v *= 32_767.0;
    }
    out
}

fn samples(rate: f32, secs: f32) -> impl Iterator<Item = f32> {
    (0..(secs * rate) as usize).map(move |i| i as f32 / rate)
}

/// A square wave's sign at `phase`, in turns, `duty` of each turn high.
fn pulse(phase: f32, duty: f32) -> f32 {
    if phase.fract() < duty {
        1.0
    } else {
        -1.0
    }
}

/// 150 Hz up to 1 kHz in 0.32 s, cut off by 0.36 s.
fn blip(rate: f32) -> Vec<f32> {
    let mut phase = 0.0;
    samples(rate, BLIP_S)
        .map(|t| {
            phase += 150.0 * (1000.0f32 / 150.0).powf(t / 0.32) / rate;
            let gate = (t / 0.003).min(1.0) * ((BLIP_S - t) / 0.12).clamp(0.0, 1.0);
            BLIP_LEVEL * pulse(phase, 0.5) * gate
        })
        .collect()
}

/// A 25% pulse at 440 Hz, which keeps every harmonic but the fourth's, dying in 45 ms.
fn key(rate: f32) -> Vec<f32> {
    samples(rate, KEY_S)
        .map(|t| KEY_LEVEL * pulse(440.0 * t, 0.25) * (t / 0.002).min(1.0) * (-t / 0.045).exp())
        .collect()
}

/// `notes` in turn, `NOTE_S` each, with a breath of noise, fading out over `secs`.
fn warble(rate: f32, secs: f32, notes: &[f32], level: f32) -> Vec<f32> {
    let mut phase = 0.0;
    let mut seed = 0x9204_u32;
    samples(rate, secs)
        .map(|t| {
            phase += notes[(t / NOTE_S) as usize % notes.len()] / rate;
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0;
            let gate = (t / 0.004).min(1.0) * (1.0 - t / secs).max(0.0).powf(1.5);
            level * (pulse(phase, 0.5) * 0.85 + noise * 0.08) * gate
        })
        .collect()
}

/// One pole of low-pass at `SOFT_HZ`, run over `v` in place.
fn soften(v: &mut [f32], rate: f32) {
    let k = 1.0 - (-2.0 * std::f32::consts::PI * SOFT_HZ / rate).exp();
    let mut y = 0.0;
    for s in v.iter_mut() {
        y += k * (*s - y);
        *s = y;
    }
}
