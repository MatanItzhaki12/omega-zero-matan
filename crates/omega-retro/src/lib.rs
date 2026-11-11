//! The cores: a libretro frontend that loads a core's library, gives it a game, runs it a frame
//! at a time, and takes its picture, sound, saves and states. It also holds what one core needs
//! of its own, gpSP's code cache room and link reset in [`gpsp`], and the link cable's
//! exchange for the GBA's.

mod core;
mod elf;
mod ffi;
pub mod gpsp;
mod libretro;
mod link;
mod mock;
mod rumble;

pub use core::{
    AvInfo, ButtonMask, CoreError, CoreMemory, GlHost, GpuRequest, RetroCore, GBA_H, GBA_W,
    MAX_FRAME_H, MAX_FRAME_W,
};
pub use ffi::MemoryDescriptor;
pub use ffi::{
    NETPACKET_BROADCAST, NETPACKET_FLUSH_HINT, NETPACKET_RELIABLE, NETPACKET_UNRELIABLE,
    NETPACKET_UNSEQUENCED,
};
pub use libretro::{host_core, LibretroCore};
pub use link::{Link, LinkChannel, LoopbackLink};
pub use mock::MockCore;
pub use rumble::Rumble;
