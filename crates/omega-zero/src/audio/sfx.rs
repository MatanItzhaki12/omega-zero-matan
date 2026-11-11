use super::chime::{self, BLIP_S, KEY_S, OMEGA_S, READING_S};
use super::menu_sound::{self, BACK_S, CONFIRM_S, MOVE_S, SHELF_S};

const INSERT: &[u8] = include_bytes!("../../assets/insert.pcm");
const EJECT: &[u8] = include_bytes!("../../assets/eject.pcm");
const ASSET_HZ: f32 = 48_000.0;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Sfx {
    Insert,
    Eject,
    /// The opening's power-on: a square wave swept up from 150 Hz to 1 kHz, as Mega Man X5's.
    Blip,
    /// A key typed in the opening: 440 Hz with every harmonic, gone in a breath.
    Key,
    /// A file read in the opening: a fast warble of A5, C6 and F5 that dies away.
    Reading,
    /// The opening's last file, OMEGA: the same warble an octave down, held longer.
    Omega,
    /// A menu's cursor, or the shelf's pick, moving on: a short tick.
    Move,
    /// A choice taken, a row stepped or a menu opened: two notes up.
    Confirm,
    /// A menu left: two notes down.
    Back,
    /// The shelf changed for another: a soft glide up.
    Shelf,
}

impl Sfx {
    /// Whether it is one of the menus' sounds, which Settings › Shelf › Menu Sounds silences.
    pub fn is_menu(self) -> bool {
        matches!(self, Sfx::Move | Sfx::Confirm | Sfx::Back | Sfx::Shelf)
    }
}

impl Sfx {
    pub fn lead(self) -> f32 {
        match self {
            Sfx::Insert => 0.097,
            Sfx::Eject => 0.021,
            _ => 0.0,
        }
    }

    pub fn tail(self) -> f32 {
        let secs = match self {
            Sfx::Insert => INSERT.len() as f32 / 2.0 / ASSET_HZ,
            Sfx::Eject => EJECT.len() as f32 / 2.0 / ASSET_HZ,
            Sfx::Blip => BLIP_S,
            Sfx::Key => KEY_S,
            Sfx::Reading => READING_S,
            Sfx::Omega => OMEGA_S,
            Sfx::Move => MOVE_S,
            Sfx::Confirm => CONFIRM_S,
            Sfx::Back => BACK_S,
            Sfx::Shelf => SHELF_S,
        };
        secs - self.lead()
    }

    pub fn render(self, sample_rate: u32) -> Vec<i16> {
        let pcm = match self {
            Sfx::Insert => INSERT,
            Sfx::Eject => EJECT,
            Sfx::Blip | Sfx::Key | Sfx::Reading | Sfx::Omega => {
                return stereo(chime::render(self, sample_rate))
            }
            Sfx::Move | Sfx::Confirm | Sfx::Back | Sfx::Shelf => {
                return stereo(menu_sound::render(self, sample_rate))
            }
        };
        stereo(resampled(pcm, sample_rate))
    }
}

/// `mono` as the frames the sink takes, the same sample to both ears.
fn stereo(mono: Vec<f32>) -> Vec<i16> {
    let mut out = Vec::with_capacity(mono.len() * 2);
    for v in mono {
        let s = v.clamp(-32768.0, 32767.0) as i16;
        out.push(s);
        out.push(s);
    }
    out
}

fn resampled(pcm: &[u8], sample_rate: u32) -> Vec<f32> {
    let src: Vec<f32> = pcm
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32)
        .collect();
    if sample_rate == ASSET_HZ as u32 || src.is_empty() {
        return src;
    }
    let ratio = sample_rate as f32 / ASSET_HZ;
    let n = (src.len() as f32 * ratio) as usize;
    (0..n)
        .map(|i| {
            let x = i as f32 / ratio;
            let a = x as usize;
            let f = x - a as f32;
            let b = (a + 1).min(src.len() - 1);
            src[a] * (1.0 - f) + src[b] * f
        })
        .collect()
}
