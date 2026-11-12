mod alsa;
pub mod bt_volume;
mod chime;
#[cfg(feature = "host")]
mod host;
mod menu_sound;
pub mod music;
mod ring;
mod sfx;
mod sink;
mod stub;
pub mod volume;

pub use alsa::{bluetooth_pcm, hush, on_bluetooth, route_bluetooth, AlsaSink};
#[cfg(feature = "host")]
pub use host::HostAudio;
pub use music::Music;
pub use ring::{ring_capacity, Ring};
pub use sfx::Sfx;
pub use sink::{AudioError, AudioSink};
pub use stub::StubSink;

pub const GBA_HZ: u32 = 32_768;

#[cfg(feature = "host")]
pub fn open_sink() -> Box<dyn AudioSink> {
    if std::env::var_os("OMEGA_SILENT").is_some() {
        return Box::new(StubSink::draining());
    }
    Box::new(HostAudio::new())
}

#[cfg(not(feature = "host"))]
pub fn open_sink() -> Box<dyn AudioSink> {
    Box::new(AlsaSink::new())
}
