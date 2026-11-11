//! The shelf's tune: a slow, quiet loop made as it plays, so it costs no file on the card and
//! never repeats a recording's hiss. Eight bars at 72 beats a minute: a soft pad on each bar's
//! chord, a bass note on the first and third beats, and a music box picking out the chord in
//! eighths, a few left out at random so no two times round are quite the same.
//!
//! It plays only while nothing else does, on the shelf and in its menus, fades in over a second
//! and out over half of one as a cart goes in, and is drawn a frame's worth at a time on the
//! main thread, a handful of sines per sample.

use std::f32::consts::TAU;

/// Seconds an eighth note lasts, at 72 beats a minute.
const EIGHTH_S: f64 = 60.0 / 72.0 / 2.0;
/// Eighths in a bar.
const BAR: u64 = 8;

/// The loop's chords, a bar each, as MIDI notes from the bass up: Fmaj7, G6, Em7, Am7, Dm7,
/// G7sus, Cmaj7, Cmaj9.
const CHORDS: [[u8; 4]; 8] = [
    [53, 57, 60, 64],
    [55, 59, 62, 64],
    [52, 55, 59, 62],
    [57, 60, 64, 67],
    [50, 53, 57, 60],
    [55, 60, 62, 65],
    [48, 52, 55, 59],
    [48, 55, 59, 62],
];

/// Which of the chord's notes the music box plays on each eighth of a bar, an octave and a
/// fifth up from the pad.
const ARP: [usize; 8] = [0, 1, 2, 3, 2, 1, 3, 2];

/// The whole tune's level as a share of full scale, before the volume: under the menus' sounds.
const LEVEL: f32 = 0.2;
/// How fast it fades, in gain a second: in over a second, out over half of one.
const FADE_IN: f32 = 1.0;
const FADE_OUT: f32 = 2.0;

/// One note sounding: a sine with some of its octave, struck or swelled, then dying away.
#[derive(Clone, Debug)]
struct Voice {
    hz: f32,
    phase: f32,
    /// Seconds since it began.
    age: f32,
    /// Seconds it takes to swell to full.
    attack: f32,
    /// Seconds it takes to fall to a third.
    decay: f32,
    level: f32,
    /// Its share of the left ear, 0.5 in the middle.
    pan: f32,
    /// How much of its octave rings with it.
    bright: f32,
}

impl Voice {
    fn sample(&mut self, rate: f32) -> f32 {
        self.phase = (self.phase + self.hz / rate).fract();
        self.age += 1.0 / rate;
        let swell = if self.attack > 0.0 {
            (self.age / self.attack).min(1.0)
        } else {
            (self.age / 0.003).min(1.0)
        };
        let env = swell * (-(self.age - self.attack).max(0.0) / self.decay).exp();
        let wave = (self.phase * TAU).sin() + self.bright * (self.phase * 2.0 * TAU).sin();
        self.level * env * wave
    }

    fn done(&self) -> bool {
        self.age > self.attack + self.decay * 7.0
    }
}

/// The tune, and where it is.
#[derive(Clone, Debug)]
pub struct Music {
    voices: Vec<Voice>,
    /// Seconds played.
    clock: f64,
    /// The next eighth to strike.
    step: u64,
    /// For the notes left out.
    seed: u32,
    gain: f32,
    playing: bool,
    /// The tune's loudness against the level it was tuned to, from Settings.
    level: f32,
}

impl Default for Music {
    fn default() -> Self {
        Music::new()
    }
}

impl Music {
    pub fn new() -> Self {
        Music {
            voices: Vec::new(),
            clock: 0.0,
            step: 0,
            seed: 0x51ed_2705,
            gain: 0.0,
            playing: false,
            level: 1.0,
        }
    }

    /// Fades the tune in, or out.
    pub fn set_playing(&mut self, playing: bool) {
        self.playing = playing;
    }

    /// How loud it plays against the level it was tuned to: Settings' Music Volume.
    pub fn set_level(&mut self, level: f32) {
        self.level = level.max(0.0);
    }

    /// Whether it is faded all the way out, so nothing needs drawing.
    pub fn silent(&self) -> bool {
        !self.playing && self.gain <= 0.0
    }

    /// The next `frames` of the tune at `rate`, as interleaved stereo at `volume` out of 100.
    pub fn render(&mut self, rate: u32, frames: usize, volume: u8) -> Vec<i16> {
        let rate = rate.max(1) as f32;
        let loud = LEVEL * self.level * crate::audio::volume::gain(volume) * 32_767.0;
        let mut out = Vec::with_capacity(frames * 2);
        for _ in 0..frames {
            while self.clock >= self.step as f64 * EIGHTH_S {
                self.strike();
            }
            let fade = if self.playing { FADE_IN } else { -FADE_OUT };
            self.gain = (self.gain + fade / rate).clamp(0.0, 1.0);
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for v in &mut self.voices {
                let s = v.sample(rate);
                l += s * v.pan;
                r += s * (1.0 - v.pan);
            }
            // a gentle curve on the fade, so it neither starts nor stops on a jump
            let g = self.gain * self.gain * loud;
            out.push((l * g).clamp(-32_768.0, 32_767.0) as i16);
            out.push((r * g).clamp(-32_768.0, 32_767.0) as i16);
            self.clock += 1.0 / rate as f64;
        }
        self.voices.retain(|v| !v.done());
        out
    }

    /// Starts the notes of the eighth due now.
    fn strike(&mut self) {
        let bar = (self.step / BAR) as usize % CHORDS.len();
        let eighth = (self.step % BAR) as usize;
        let chord = CHORDS[bar];
        if eighth == 0 {
            for (k, &note) in chord.iter().enumerate().skip(1) {
                self.voices.push(Voice {
                    hz: hz(note),
                    phase: 0.0,
                    age: 0.0,
                    attack: 0.6,
                    decay: 2.4,
                    level: 0.16,
                    pan: 0.35 + 0.15 * k as f32,
                    bright: 0.1,
                });
            }
        }
        if eighth == 0 || eighth == 4 {
            self.voices.push(Voice {
                hz: hz(chord[0] - 12),
                phase: 0.0,
                age: 0.0,
                attack: 0.0,
                decay: 0.9,
                level: 0.3,
                pan: 0.5,
                bright: 0.3,
            });
        }
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        // one eighth in five left out, never the bar's first
        let rest = eighth != 0 && (self.seed >> 16).is_multiple_of(5);
        if !rest {
            let note = chord[ARP[eighth]] + 19;
            self.voices.push(Voice {
                hz: hz(note),
                phase: 0.0,
                age: 0.0,
                attack: 0.0,
                decay: 0.45,
                level: 0.12,
                pan: if eighth.is_multiple_of(2) { 0.62 } else { 0.38 },
                bright: 0.35,
            });
        }
        self.step += 1;
    }
}

/// MIDI note `note`'s pitch in Hz.
fn hz(note: u8) -> f32 {
    440.0 * 2f32.powf((note as f32 - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_silent_until_it_plays_and_fades_back_to_silence() {
        let mut m = Music::new();
        assert!(m.silent());
        assert!(m.render(48_000, 4_800, 60).iter().all(|&s| s == 0));
        m.set_playing(true);
        let heard = m.render(48_000, 96_000, 60);
        assert!(
            heard.iter().any(|&s| s.abs() > 200),
            "the tune never sounded"
        );
        m.set_playing(false);
        m.render(48_000, 48_000, 60);
        assert!(m.silent(), "the tune never faded out");
    }

    #[test]
    fn it_stays_quiet_and_never_clips() {
        let mut m = Music::new();
        m.set_playing(true);
        let peak = m
            .render(32_768, 32_768 * 30, 100)
            .iter()
            .map(|s| s.unsigned_abs())
            .max()
            .unwrap();
        assert!(
            peak < 16_000,
            "the tune peaks at {peak}, too loud for a backdrop"
        );
        assert!(peak > 1_000, "the tune peaks at {peak}, too quiet to hear");
    }

    #[test]
    fn its_notes_die_away_rather_than_pile_up() {
        let mut m = Music::new();
        m.set_playing(true);
        for _ in 0..60 {
            m.render(48_000, 48_000, 60);
        }
        assert!(
            m.voices.len() < 40,
            "{} notes still sounding",
            m.voices.len()
        );
    }

    #[test]
    fn a_muted_handheld_hears_none_of_it() {
        let mut m = Music::new();
        m.set_playing(true);
        assert!(m.render(48_000, 96_000, 0).iter().all(|&s| s == 0));
    }
}
