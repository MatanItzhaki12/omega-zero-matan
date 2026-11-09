use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use libloading::Library;

use super::ring::Ring;
use super::sink::{AudioError, AudioSink};

const SND_PCM_STREAM_PLAYBACK: c_int = 0;
const SND_PCM_FORMAT_S16_LE: c_int = 2;
const SND_PCM_ACCESS_RW_INTERLEAVED: c_int = 3;
const CHANNELS: c_uint = 2;

const LATENCY_US: c_uint = 40_000;

const PERIOD_FRAMES: usize = 512;

const DEVICES: [&str; 4] = ["plug:default", "default", "plughw:0,0", "hw:0,0"];

/// How far ahead Bluetooth headphones are written: as little as bluez-alsa takes without
/// running dry, as the headphones hold back a buffer of their own on top of it.
const BLUETOOTH_LATENCY_US: c_uint = 50_000;

/// How long after headphones would not open, or stopped taking sound, they are tried again.
const BLUETOOTH_RETRY: Duration = Duration::from_secs(3);

/// The Bluetooth headphones the sound should go to, by address: none for the speaker.
static BLUETOOTH: Mutex<Option<String>> = Mutex::new(None);

/// The sound is going to Bluetooth headphones now.
static ON_BLUETOOTH: AtomicBool = AtomicBool::new(false);

/// Sends the sound to the Bluetooth headphones at `mac`, through bluez-alsa, or back to the
/// speaker for none. The output thread moves over between one period and the next.
pub fn route_bluetooth(mac: Option<&str>) {
    if let Ok(mut want) = BLUETOOTH.lock() {
        if want.as_deref() != mac {
            *want = mac.map(str::to_string);
        }
    }
}

/// Whether the sound is going to Bluetooth headphones: they have their own volume, as wired
/// ones do.
pub fn on_bluetooth() -> bool {
    ON_BLUETOOTH.load(Ordering::Relaxed)
}

fn wanted_bluetooth() -> Option<String> {
    BLUETOOTH.lock().ok().and_then(|w| w.clone())
}

/// bluez-alsa's PCM for the headphones at `mac`.
pub fn bluetooth_pcm(mac: &str) -> String {
    format!("bluealsa:DEV={mac},PROFILE=a2dp")
}

type PcmOpen = unsafe extern "C" fn(*mut *mut c_void, *const c_char, c_int, c_int) -> c_int;
type PcmSetParams =
    unsafe extern "C" fn(*mut c_void, c_int, c_int, c_uint, c_uint, c_int, c_uint) -> c_int;
type PcmWritei = unsafe extern "C" fn(*mut c_void, *const c_void, u64) -> i64;
type PcmRecover = unsafe extern "C" fn(*mut c_void, c_int, c_int) -> c_int;
type PcmDrop = unsafe extern "C" fn(*mut c_void) -> c_int;
type PcmClose = unsafe extern "C" fn(*mut c_void) -> c_int;
type StrError = unsafe extern "C" fn(c_int) -> *const c_char;

struct Alsa {
    open: PcmOpen,
    set_params: PcmSetParams,
    writei: PcmWritei,
    recover: PcmRecover,
    drop: PcmDrop,
    close: PcmClose,
    strerror: StrError,
    _lib: Library,
}

impl Alsa {
    fn load() -> Result<Self, AudioError> {
        let lib = unsafe { Library::new("libasound.so.2") }
            .or_else(|_| unsafe { Library::new("libasound.so") })
            .map_err(|e| AudioError::Device(format!("libasound: {e}")))?;
        macro_rules! get {
            ($name:literal) => {
                *lib.get(concat!($name, "\0").as_bytes())
                    .map_err(|e| AudioError::Device(format!("{}: {e}", $name)))?
            };
        }
        unsafe {
            Ok(Alsa {
                open: get!("snd_pcm_open"),
                set_params: get!("snd_pcm_set_params"),
                writei: get!("snd_pcm_writei"),
                recover: get!("snd_pcm_recover"),
                drop: get!("snd_pcm_drop"),
                close: get!("snd_pcm_close"),
                strerror: get!("snd_strerror"),
                _lib: lib,
            })
        }
    }

    fn message(&self, err: c_int) -> String {
        let text = unsafe { (self.strerror)(err) };
        match text.is_null() {
            true => format!("error {err}"),
            false => unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned(),
        }
    }

    /// The headphones' PCM at `rate`, or why it would not open.
    fn open_named(
        &self,
        name: &str,
        rate: u32,
        latency: c_uint,
    ) -> Result<*mut c_void, AudioError> {
        let cname = CString::new(name).map_err(|_| AudioError::NoDevice)?;
        let mut pcm: *mut c_void = std::ptr::null_mut();
        let err = unsafe { (self.open)(&mut pcm, cname.as_ptr(), SND_PCM_STREAM_PLAYBACK, 0) };
        if err < 0 || pcm.is_null() {
            return Err(AudioError::Device(format!("{name}: {}", self.message(err))));
        }
        let err = unsafe {
            (self.set_params)(
                pcm,
                SND_PCM_FORMAT_S16_LE,
                SND_PCM_ACCESS_RW_INTERLEAVED,
                CHANNELS,
                rate,
                1,
                latency,
            )
        };
        if err < 0 {
            unsafe { (self.close)(pcm) };
            return Err(AudioError::Config(format!("{name}: {}", self.message(err))));
        }
        eprintln!("omega-zero: audio {name} at {rate} Hz");
        Ok(pcm)
    }

    fn open_pcm(&self, rate: u32) -> Result<*mut c_void, AudioError> {
        let mut last = AudioError::NoDevice;
        for name in DEVICES {
            let Ok(cname) = CString::new(name) else {
                continue;
            };
            let mut pcm: *mut c_void = std::ptr::null_mut();
            let err = unsafe { (self.open)(&mut pcm, cname.as_ptr(), SND_PCM_STREAM_PLAYBACK, 0) };
            if err < 0 || pcm.is_null() {
                last = AudioError::Device(format!("{name}: {}", self.message(err)));
                continue;
            }
            let err = unsafe {
                (self.set_params)(
                    pcm,
                    SND_PCM_FORMAT_S16_LE,
                    SND_PCM_ACCESS_RW_INTERLEAVED,
                    CHANNELS,
                    rate,
                    1,
                    LATENCY_US,
                )
            };
            if err < 0 {
                unsafe { (self.close)(pcm) };
                last = AudioError::Config(format!("{name}: {}", self.message(err)));
                continue;
            }
            eprintln!("omega-zero: audio {name} at {rate} Hz");
            return Ok(pcm);
        }
        Err(last)
    }
}

pub struct AlsaSink {
    ring: Arc<Ring>,
    device: Option<Device>,
}

struct Device {
    stop: Arc<AtomicBool>,
    join: JoinHandle<()>,
}

impl AlsaSink {
    pub fn new() -> Self {
        AlsaSink {
            ring: Arc::new(Ring::new(0)),
            device: None,
        }
    }

    fn close(&mut self) {
        if let Some(d) = self.device.take() {
            d.stop.store(true, Ordering::Relaxed);
            let _ = d.join.join();
        }
    }
}

impl Default for AlsaSink {
    fn default() -> Self {
        AlsaSink::new()
    }
}

impl Drop for AlsaSink {
    fn drop(&mut self) {
        self.close();
    }
}

impl AudioSink for AlsaSink {
    fn open(&mut self, sample_rate: u32) -> Result<(), AudioError> {
        self.close();
        let ring = self.ring.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let (ready_tx, ready_rx) = mpsc::channel();
        let join = std::thread::Builder::new()
            .name("omega-audio".into())
            .spawn(move || match play(&ring, sample_rate) {
                Ok(mut device) => {
                    let _ = ready_tx.send(Ok(()));
                    device.run(&ring, &flag);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            })
            .map_err(|e| AudioError::Device(e.to_string()))?;
        match ready_rx.recv() {
            Ok(Ok(())) => {
                self.device = Some(Device { stop, join });
                Ok(())
            }
            Ok(Err(e)) => {
                let _ = join.join();
                Err(e)
            }
            Err(_) => {
                let _ = join.join();
                Err(AudioError::Device("output thread stopped".into()))
            }
        }
    }

    fn ring(&self) -> Arc<Ring> {
        self.ring.clone()
    }
}

struct Playback {
    alsa: Alsa,
    pcm: *mut c_void,
    rate: u32,
    /// The Bluetooth headphones `pcm` is, by address: none for the speaker.
    bluetooth: Option<String>,
    /// The headphones that last would not open or stopped taking sound, and when.
    refused: Option<(String, Instant)>,
}

// SAFETY: the PCM is only ever used from the output thread that opened it
unsafe impl Send for Playback {}

fn play(ring: &Arc<Ring>, sample_rate: u32) -> Result<Playback, AudioError> {
    let alsa = Alsa::load()?;
    let pcm = alsa.open_pcm(sample_rate)?;
    ring.reopen(sample_rate);
    Ok(Playback {
        alsa,
        pcm,
        rate: sample_rate,
        bluetooth: None,
        refused: None,
    })
}

impl Playback {
    fn close_pcm(&mut self) {
        unsafe {
            (self.alsa.drop)(self.pcm);
            (self.alsa.close)(self.pcm);
        }
        self.pcm = std::ptr::null_mut();
    }

    /// Moves the sound to where it is wanted, between one period and the next: to the
    /// Bluetooth headphones while they are connected and take it, else to the speaker.
    fn follow(&mut self) {
        let want = wanted_bluetooth();
        if want.is_none() {
            self.refused = None;
        }
        let refused = |mac: &str, r: &Option<(String, Instant)>| {
            r.as_ref()
                .is_some_and(|(m, at)| m == mac && at.elapsed() < BLUETOOTH_RETRY)
        };
        let target = want.filter(|mac| !refused(mac, &self.refused));
        if target == self.bluetooth && !self.pcm.is_null() {
            return;
        }
        if !self.pcm.is_null() {
            self.close_pcm();
        }
        if let Some(mac) = &target {
            let name = bluetooth_pcm(mac);
            match self.alsa.open_named(&name, self.rate, BLUETOOTH_LATENCY_US) {
                Ok(pcm) => {
                    self.pcm = pcm;
                    self.bluetooth = target;
                    ON_BLUETOOTH.store(true, Ordering::Relaxed);
                    return;
                }
                Err(e) => {
                    eprintln!("omega-zero: audio: {e}");
                    self.refused = Some((mac.clone(), Instant::now()));
                }
            }
        }
        ON_BLUETOOTH.store(false, Ordering::Relaxed);
        self.bluetooth = None;
        match self.alsa.open_pcm(self.rate) {
            Ok(pcm) => self.pcm = pcm,
            Err(e) => eprintln!("omega-zero: audio: {e}"),
        }
    }

    fn run(&mut self, ring: &Ring, stop: &AtomicBool) {
        let mut buf = vec![0i16; PERIOD_FRAMES * CHANNELS as usize];
        while !stop.load(Ordering::Relaxed) {
            self.follow();
            ring.fill(&mut buf);
            if self.pcm.is_null() {
                // no output at all: keep the ring draining at its pace
                std::thread::sleep(Duration::from_micros(
                    PERIOD_FRAMES as u64 * 1_000_000 / u64::from(self.rate.max(1)),
                ));
                continue;
            }
            let mut written = 0;
            while written < PERIOD_FRAMES {
                let at = written * CHANNELS as usize;
                let frames = unsafe {
                    (self.alsa.writei)(
                        self.pcm,
                        buf[at..].as_ptr() as *const c_void,
                        (PERIOD_FRAMES - written) as u64,
                    )
                };
                if frames < 0 {
                    let err = unsafe { (self.alsa.recover)(self.pcm, frames as c_int, 1) };
                    if err < 0 {
                        eprintln!("omega-zero: audio: {}", self.alsa.message(err));
                        // headphones gone: back to the speaker, and they are tried again
                        // in a moment
                        if let Some(mac) = self.bluetooth.take() {
                            self.refused = Some((mac, Instant::now()));
                            self.close_pcm();
                            break;
                        }
                        return;
                    }
                    continue;
                }
                written += frames as usize;
            }
        }
        ON_BLUETOOTH.store(false, Ordering::Relaxed);
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        if !self.pcm.is_null() {
            self.close_pcm();
        }
    }
}
