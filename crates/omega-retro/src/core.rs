use std::fmt;
use std::path::Path;

use crate::link::Link;
use crate::rumble::Rumble;

/// The GBA's screen. A picture smaller than it, a Game Boy's, reaches the frontend centred in
/// one on black, as the handheld layouts expect; every SNES and PlayStation mode is bigger in
/// both directions and arrives at its own size.
pub const GBA_W: u32 = 240;
pub const GBA_H: u32 = 160;

/// The biggest picture the host takes from a core, room for the SNES's 512x478 and the
/// PlayStation's 640x512; anything past it is cut off.
pub const MAX_FRAME_W: u32 = 1024;
pub const MAX_FRAME_H: u32 = 576;

#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct ButtonMask(pub u16);

impl ButtonMask {
    pub const B: u16 = 1 << 0;
    pub const Y: u16 = 1 << 1;
    pub const SELECT: u16 = 1 << 2;
    pub const START: u16 = 1 << 3;
    pub const UP: u16 = 1 << 4;
    pub const DOWN: u16 = 1 << 5;
    pub const LEFT: u16 = 1 << 6;
    pub const RIGHT: u16 = 1 << 7;
    pub const A: u16 = 1 << 8;
    pub const X: u16 = 1 << 9;
    pub const L: u16 = 1 << 10;
    pub const R: u16 = 1 << 11;
    pub const L2: u16 = 1 << 12;
    pub const R2: u16 = 1 << 13;
    pub const L3: u16 = 1 << 14;
    pub const R3: u16 = 1 << 15;

    pub fn turbo(self, frame: u32) -> ButtonMask {
        let mut mask = self.0 & !(Self::X | Self::Y);
        if (frame / 3).is_multiple_of(2) {
            if self.0 & Self::X != 0 {
                mask |= Self::A;
            }
            if self.0 & Self::Y != 0 {
                mask |= Self::B;
            }
        }
        ButtonMask(mask)
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct AvInfo {
    pub fps: f64,
    pub sample_rate: f64,
}

#[derive(Debug)]
pub enum CoreError {
    Io(std::io::Error),
    Load(String),
    Unsupported(String),
    State(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Io(e) => write!(f, "io: {e}"),
            CoreError::Load(m) => write!(f, "load: {m}"),
            CoreError::Unsupported(m) => write!(f, "unsupported: {m}"),
            CoreError::State(m) => write!(f, "state: {m}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::Io(e)
    }
}

/// The frontend's side of a core that draws on the GPU: the framebuffer the core draws the next
/// frame into, and the GL functions it looks up. The frontend makes the context current on the
/// core's thread before the core runs.
pub trait GlHost: Send {
    /// The framebuffer object the core draws the frame about to run into.
    fn framebuffer(&self) -> usize;
    /// The address of GL function `name`, or null.
    fn proc_address(&self, name: &std::ffi::CStr) -> *const std::ffi::c_void;
}

/// What a core that draws on the GPU asked for as its game loaded.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GpuRequest {
    /// The OpenGL ES version: (2, 0), (3, 0) and on.
    pub version: (u32, u32),
    pub depth: bool,
    pub stencil: bool,
    /// It draws with GL's origin at the bottom left, as GL does: its frames are upside down.
    pub bottom_left: bool,
}

pub trait RetroCore: Send {
    fn load(&mut self, rom: &Path) -> Result<(), CoreError>;
    fn run_frame(&mut self, input: ButtonMask);
    fn run_frame_linked(&mut self, p1: ButtonMask, _p2: ButtonMask) {
        self.run_frame(p1);
    }
    fn set_option(&mut self, _key: &str, _value: &str) {}
    fn set_frame_skip(&mut self, _skip: bool) {}
    /// The last picture the core drew, XRGB8888 in rows `video_size().0` pixels long.
    fn video_xrgb8888(&self) -> &[u8];
    /// How big that picture is, width by height. A core that never says draws the GBA's screen.
    fn video_size(&self) -> (u32, u32) {
        (GBA_W, GBA_H)
    }
    fn take_audio(&mut self) -> Vec<i16>;
    /// The sound since the last call, into `out` in place of what it held: a core that keeps its
    /// own buffer hands it over and keeps `out`'s, so neither is made again every frame.
    fn take_audio_into(&mut self, out: &mut Vec<i16>) {
        *out = self.take_audio();
    }
    fn serialize(&mut self) -> Result<Vec<u8>, CoreError>;
    /// The state, as [`RetroCore::serialize`] gives it, written into `buf` in place of what it
    /// held, so a buffer handed back can be written again rather than a new one made.
    fn serialize_into(&mut self, buf: &mut Vec<u8>) -> Result<(), CoreError> {
        *buf = self.serialize()?;
        Ok(())
    }
    fn unserialize(&mut self, data: &[u8]) -> Result<(), CoreError>;
    /// Starts the game over, as the console's reset does; the cart's save memory stays.
    fn reset(&mut self) {}
    fn save_ram(&self) -> Option<Vec<u8>>;
    /// The loaded game's memory, for RetroAchievements; none for a core that shows none.
    fn memory(&self) -> Option<CoreMemory> {
        None
    }
    fn load_save_ram(&mut self, data: &[u8]) -> Result<(), CoreError>;
    fn av_info(&self) -> AvInfo;
    fn rumble(&self) -> Rumble {
        Rumble::default()
    }
    fn net(&self) -> Link {
        Link::default()
    }
    fn start_link(&mut self, _client_id: u16) {}
    fn pump_link(&mut self) {}
    fn stop_link(&mut self) {}
    /// How many discs the game in the core is on, through libretro's disk control.
    fn disc_count(&self) -> usize {
        1
    }
    /// Which of them is in the drive, counting from 0.
    fn disc_index(&self) -> usize {
        0
    }
    /// Opens the drive, puts disc `index` in and closes it, as a player swapping discs would;
    /// false if the core has no such disc or no disk control.
    fn set_disc(&mut self, _index: usize) -> bool {
        false
    }
    /// The file disc `index` is read from, as the core names it, when it can say.
    fn disc_path(&self, _index: usize) -> Option<String> {
        None
    }
    /// Asks for disc `index`, read from `path`, to be the one in the drive when the game loads,
    /// as it was when the game was last put away; called before `load`. A core that checks finds
    /// `path` at that index or starts from the first disc.
    fn set_initial_disc(&mut self, _index: usize, _path: &str) {}
    /// Offers the core a GPU to draw on, before `load`: a core that asks for one gets it. A core
    /// never offered one draws in memory, as every core did before.
    fn offer_gpu(&mut self, _host: Box<dyn GlHost>) {}
    /// What the core asked for, once its game is loaded, if it draws on the GPU.
    fn gpu_request(&self) -> Option<GpuRequest> {
        None
    }
    /// Tells the core its context is ready, or has been made again: it makes its GL objects.
    fn gpu_context_reset(&mut self) {}
    /// Tells the core its context is going: it lets go of its GL objects.
    fn gpu_context_destroy(&mut self) {}
    /// The size of the frame the core drew on the GPU in the last `run_frame`; none when it drew
    /// none, a frame it repeats.
    fn gpu_frame(&self) -> Option<(u32, u32)> {
        None
    }
    /// The left analog stick of `port`, each axis -32768 to 32767, right and down positive.
    fn set_analog(&mut self, _port: usize, _x: i16, _y: i16) {}
    /// Where the picture is touched, each axis -32768 to 32767 across it, right and down
    /// positive; none, not touched.
    fn set_pointer(&mut self, _at: Option<(i16, i16)>) {}
    /// The one-screen hooks omega-zero's melonDS core exports, where the core is that one.
    fn ds_hooks(&self) -> Option<crate::ds::DsHooks> {
        None
    }
}

/// A loaded game's memory as RetroAchievements reads it: the map the core declared, and the
/// regions it answers for by number, each its bytes and its length. The pointers are the core's
/// and stay good while the game is loaded.
pub struct CoreMemory {
    pub map: Vec<crate::ffi::MemoryDescriptor>,
    pub system: (*mut u8, usize),
    pub save: (*mut u8, usize),
    pub video: (*mut u8, usize),
}

// SAFETY: the pointers are only read on the thread that runs the core, while it is loaded
unsafe impl Send for CoreMemory {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder(Vec<ButtonMask>);

    impl RetroCore for Recorder {
        fn load(&mut self, _rom: &Path) -> Result<(), CoreError> {
            Ok(())
        }
        fn run_frame(&mut self, input: ButtonMask) {
            self.0.push(input);
        }
        fn video_xrgb8888(&self) -> &[u8] {
            &[]
        }
        fn take_audio(&mut self) -> Vec<i16> {
            Vec::new()
        }
        fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
            Ok(Vec::new())
        }
        fn unserialize(&mut self, _data: &[u8]) -> Result<(), CoreError> {
            Ok(())
        }
        fn save_ram(&self) -> Option<Vec<u8>> {
            None
        }
        fn load_save_ram(&mut self, _data: &[u8]) -> Result<(), CoreError> {
            Ok(())
        }
        fn av_info(&self) -> AvInfo {
            AvInfo {
                fps: 60.0,
                sample_rate: 48_000.0,
            }
        }
    }

    #[test]
    fn a_core_without_link_mode_runs_player_1_alone() {
        let mut core = Recorder::default();
        core.run_frame_linked(ButtonMask(ButtonMask::A), ButtonMask(ButtonMask::B));
        assert_eq!(core.0, vec![ButtonMask(ButtonMask::A)]);
    }
}
