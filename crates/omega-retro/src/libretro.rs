use std::cell::Cell;
use std::ffi::{c_char, c_int, c_uint, c_void, CString};
use std::marker::PhantomData;
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use libloading::Library;

use crate::core::{
    AvInfo, ButtonMask, CoreError, CoreMemory, RetroCore, GBA_H, GBA_W, MAX_FRAME_H, MAX_FRAME_W,
};
use crate::ffi::*;
use crate::link::Link;
use crate::rumble::Rumble;

static LIVE: AtomicBool = AtomicBool::new(false);

/// The framebuffer a core that draws on the GPU draws into, as each call into the core last saw
/// it: for a core that asks from a thread of its own, as PPSSPP's emulation thread does, where no
/// host is bound. Answering it 0 there sent PPSSPP's every frame to the context's own surface.
static HW_FRAMEBUFFER: AtomicUsize = AtomicUsize::new(0);

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum PixelFormat {
    /// libretro's own default, for a core that never says.
    Rgb1555,
    Xrgb8888,
    Rgb565,
}

struct Host {
    /// The last picture, XRGB8888, `video_size` pixels.
    video: Vec<u8>,
    video_size: (u32, u32),
    format: PixelFormat,
    /// The core's rate and sound, as it said them at load and since.
    av: AvInfo,
    geometry: GameGeometry,
    disk: Option<DiskControlExtCallback>,
    audio: Vec<i16>,
    inputs: [u16; 2],
    system_dir: CString,
    save_dir: CString,
    rumble: Rumble,
    asked_for_rumble: bool,
    netpacket: Option<NetpacketCallback>,
    net: Link,
    net_peer: Option<u16>,
    audio_status: Option<AudioBufferStatusFn>,
    /// The frame about to run is skipped, for a core that asks whether to draw it.
    skip_video: bool,
    options: std::collections::HashMap<String, std::ffi::CString>,
    options_dirty: bool,
    declared: std::collections::HashMap<String, Vec<String>>,
    /// The first value the core listed for each option it declared, its default: what an option
    /// omega-zero leaves alone reads as, as in any frontend, so no core falls back on defaults
    /// of its own. Supafaust forgets its own once a game is loaded.
    defaults: std::collections::HashMap<String, std::ffi::CString>,
    /// The memory map the core declared, for RetroAchievements to read the game's memory by.
    memory_map: Vec<MemoryDescriptor>,
    /// The GPU the frontend offered, and what a core that draws on it asked for: none for a
    /// core that draws in memory.
    gl: Option<Box<dyn crate::core::GlHost>>,
    hw: Option<HwRenderCallback>,
    /// The size of the frame drawn on the GPU in the frame that last ran.
    hw_frame: Option<(u32, u32)>,
    /// Each port's left stick, x and y.
    analog: [[i16; 2]; 2],
}

impl Host {
    fn new(system_dir: CString, save_dir: CString) -> Host {
        Host {
            video: vec![0; (GBA_W * GBA_H * 4) as usize],
            video_size: (GBA_W, GBA_H),
            format: PixelFormat::Rgb1555,
            av: AvInfo {
                fps: 0.0,
                sample_rate: 0.0,
            },
            geometry: GameGeometry::default(),
            disk: None,
            audio: Vec::new(),
            inputs: [0; 2],
            system_dir,
            save_dir,
            rumble: Rumble::default(),
            asked_for_rumble: false,
            netpacket: None,
            net: Link::default(),
            net_peer: None,
            audio_status: None,
            skip_video: false,
            options: std::collections::HashMap::new(),
            options_dirty: false,
            declared: std::collections::HashMap::new(),
            defaults: std::collections::HashMap::new(),
            memory_map: Vec::new(),
            gl: None,
            hw: None,
            hw_frame: None,
            analog: [[0; 2]; 2],
        }
    }

    /// Takes a frame the core drew, `pitch` bytes a row, into `video` as XRGB8888: at its own
    /// size, cut to `MAX_FRAME_W` by `MAX_FRAME_H`, or centred on black in the GBA's screen when
    /// it is smaller than that, as a Game Boy picture is.
    unsafe fn take_frame(&mut self, data: *const u8, width: c_uint, height: c_uint, pitch: usize) {
        let cols = width.min(MAX_FRAME_W) as usize;
        let rows = height.min(MAX_FRAME_H) as usize;
        let w = cols.max(GBA_W as usize);
        let h = rows.max(GBA_H as usize);
        let ox = (w - cols) / 2;
        let oy = (h - rows) / 2;
        self.video.resize(w * h * 4, 0);
        if (cols, rows) != (w, h) {
            self.video.fill(0);
        }
        self.video_size = (w as u32, h as u32);
        for y in 0..rows {
            let src = data.add(y * pitch);
            let at = ((y + oy) * w + ox) * 4;
            let out = &mut self.video[at..at + cols * 4];
            match self.format {
                PixelFormat::Xrgb8888 => {
                    ptr::copy_nonoverlapping(src, out.as_mut_ptr(), cols * 4);
                }
                PixelFormat::Rgb565 => {
                    for (x, px) in out.chunks_exact_mut(4).enumerate() {
                        let p = ptr::read_unaligned((src as *const u16).add(x));
                        let r = ((p >> 11) & 0x1f) as u8;
                        let g = ((p >> 5) & 0x3f) as u8;
                        let b = (p & 0x1f) as u8;
                        px[0] = (b << 3) | (b >> 2);
                        px[1] = (g << 2) | (g >> 4);
                        px[2] = (r << 3) | (r >> 2);
                        px[3] = 0;
                    }
                }
                PixelFormat::Rgb1555 => {
                    for (x, px) in out.chunks_exact_mut(4).enumerate() {
                        let p = ptr::read_unaligned((src as *const u16).add(x));
                        let r = ((p >> 10) & 0x1f) as u8;
                        let g = ((p >> 5) & 0x1f) as u8;
                        let b = (p & 0x1f) as u8;
                        px[0] = (b << 3) | (b >> 2);
                        px[1] = (g << 3) | (g >> 2);
                        px[2] = (r << 3) | (r >> 2);
                        px[3] = 0;
                    }
                }
            }
        }
    }
}

thread_local! {
    static ACTIVE: Cell<*mut Host> = const { Cell::new(ptr::null_mut()) };
}

struct Active<'a>(PhantomData<&'a mut Host>);

impl Active<'_> {
    fn bind(host: &mut Host) -> Active<'_> {
        if let Some(gl) = host.gl.as_ref() {
            HW_FRAMEBUFFER.store(gl.framebuffer(), Ordering::Relaxed);
        }
        ACTIVE.with(|a| a.set(host as *mut Host));
        Active(PhantomData)
    }
}

impl Drop for Active<'_> {
    fn drop(&mut self) {
        ACTIVE.with(|a| a.set(ptr::null_mut()));
    }
}

unsafe fn with_host<R>(f: impl FnOnce(&mut Host) -> R) -> Option<R> {
    let p = ACTIVE.with(std::cell::Cell::get);
    if p.is_null() {
        return None;
    }
    Some(f(&mut *p))
}

unsafe extern "C" fn log_noop(_level: c_uint, _fmt: *const c_char) {}

unsafe extern "C" fn set_rumble_state(port: c_uint, effect: c_uint, strength: u16) -> bool {
    with_host(|h| h.rumble.set(port, effect, strength)).unwrap_or(false)
}

unsafe extern "C" fn environment(cmd: c_uint, data: *mut c_void) -> bool {
    match cmd {
        GET_AUDIO_VIDEO_ENABLE => {
            if data.is_null() {
                return false;
            }
            // bit 0 draws the frame, bit 1 plays its sound; a skipped frame is still heard
            with_host(|h| *(data as *mut c_int) = if h.skip_video { 2 } else { 3 }).is_some()
        }
        GET_CAN_DUPE => {
            if data.is_null() {
                return false;
            }
            *(data as *mut bool) = true;
            true
        }
        SET_PIXEL_FORMAT => {
            if data.is_null() {
                return false;
            }
            let want = match *(data as *const c_uint) {
                PIXEL_FORMAT_0RGB1555 => PixelFormat::Rgb1555,
                PIXEL_FORMAT_XRGB8888 => PixelFormat::Xrgb8888,
                PIXEL_FORMAT_RGB565 => PixelFormat::Rgb565,
                _ => return false,
            };
            with_host(|h| h.format = want).is_some()
        }
        SET_SYSTEM_AV_INFO => {
            if data.is_null() {
                return false;
            }
            let av = std::ptr::read(data as *const SystemAvInfo);
            with_host(|h| {
                h.av = AvInfo {
                    fps: av.timing.fps,
                    sample_rate: av.timing.sample_rate,
                };
                h.geometry = av.geometry;
            })
            .is_some()
        }
        SET_GEOMETRY => {
            if data.is_null() {
                return false;
            }
            // the frames carry their own size; the most a core may change here is its picture's
            let g = std::ptr::read(data as *const GameGeometry);
            with_host(|h| {
                h.geometry.base_width = g.base_width;
                h.geometry.base_height = g.base_height;
                h.geometry.aspect_ratio = g.aspect_ratio;
            })
            .is_some()
        }
        SET_HW_RENDER => {
            if data.is_null() {
                return false;
            }
            let cb = &mut *(data as *mut HwRenderCallback);
            let es = matches!(
                cb.context_type,
                HW_CONTEXT_OPENGLES2 | HW_CONTEXT_OPENGLES3 | HW_CONTEXT_OPENGLES_VERSION
            );
            // only OpenGL ES, and only for a frontend that offered a GPU
            if !es || with_host(|h| h.gl.is_some()) != Some(true) {
                return false;
            }
            cb.get_current_framebuffer = Some(hw_framebuffer);
            cb.get_proc_address = Some(hw_proc_address);
            let kept = *cb;
            with_host(|h| h.hw = Some(kept)).is_some()
        }
        GET_PREFERRED_HW_RENDER => {
            if data.is_null() || with_host(|h| h.gl.is_some()) != Some(true) {
                return false;
            }
            *(data as *mut c_uint) = HW_CONTEXT_OPENGLES3;
            true
        }
        SET_HW_SHARED_CONTEXT => with_host(|h| h.gl.is_some()).unwrap_or(false),
        GET_DISK_CONTROL_INTERFACE_VERSION => {
            if data.is_null() {
                return false;
            }
            *(data as *mut c_uint) = 1;
            true
        }
        SET_DISK_CONTROL_INTERFACE => {
            if data.is_null() {
                return with_host(|h| h.disk = None).is_some();
            }
            let cb = std::ptr::read(data as *const DiskControlCallback);
            with_host(|h| h.disk = Some(cb.into())).is_some()
        }
        SET_DISK_CONTROL_EXT_INTERFACE => {
            if data.is_null() {
                return with_host(|h| h.disk = None).is_some();
            }
            let cb = std::ptr::read(data as *const DiskControlExtCallback);
            with_host(|h| h.disk = Some(cb)).is_some()
        }
        GET_SYSTEM_DIRECTORY | GET_SAVE_DIRECTORY => {
            if data.is_null() {
                return false;
            }
            with_host(|h| {
                let dir = if cmd == GET_SYSTEM_DIRECTORY {
                    &h.system_dir
                } else {
                    &h.save_dir
                };
                *(data as *mut *const c_char) = dir.as_ptr();
                true
            })
            .unwrap_or(false)
        }
        GET_VARIABLE => {
            if data.is_null() {
                return false;
            }
            let var = &mut *(data as *mut Variable);
            if var.key.is_null() {
                return false;
            }
            let Ok(key) = std::ffi::CStr::from_ptr(var.key).to_str() else {
                return false;
            };
            with_host(
                |h| match h.options.get(key).or_else(|| h.defaults.get(key)) {
                    Some(value) => {
                        var.value = value.as_ptr();
                        true
                    }
                    None => {
                        var.value = ptr::null();
                        false
                    }
                },
            )
            .unwrap_or(false)
        }
        GET_VARIABLE_UPDATE => {
            if data.is_null() {
                return false;
            }
            with_host(|h| {
                *(data as *mut bool) = h.options_dirty;
                h.options_dirty = false;
                true
            })
            .unwrap_or(false)
        }
        SET_VARIABLES => {
            if data.is_null() {
                return with_host(|h| {
                    h.declared.clear();
                    h.defaults.clear();
                })
                .is_some();
            }
            let mut list = std::collections::HashMap::new();
            let mut p = data as *const Variable;
            for _ in 0..4096 {
                let var = &*p;
                if var.key.is_null() {
                    break;
                }
                let Ok(key) = std::ffi::CStr::from_ptr(var.key).to_str() else {
                    break;
                };
                let values = if var.value.is_null() {
                    Vec::new()
                } else {
                    std::ffi::CStr::from_ptr(var.value)
                        .to_str()
                        .ok()
                        .and_then(|v| v.split_once(';'))
                        .map(|(_, values)| {
                            values
                                .trim()
                                .split('|')
                                .map(|s| s.trim().to_string())
                                .collect()
                        })
                        .unwrap_or_default()
                };
                list.insert(key.to_string(), values);
                p = p.add(1);
            }
            let defaults = list
                .iter()
                .filter_map(|(key, values)| {
                    Some((key.clone(), CString::new(values.first()?.as_str()).ok()?))
                })
                .collect();
            with_host(|h| {
                h.declared = list;
                h.defaults = defaults;
            })
            .is_some()
        }
        GET_RUMBLE_INTERFACE => {
            if data.is_null() {
                return false;
            }
            with_host(|h| {
                h.asked_for_rumble = true;
                (*(data as *mut RumbleInterface)).set_rumble_state = set_rumble_state;
            })
            .is_some()
        }
        GET_LOG_INTERFACE => {
            if data.is_null() {
                return false;
            }
            (*(data as *mut LogCallback)).log = log_noop as *const c_void;
            true
        }
        SET_AUDIO_BUFFER_STATUS_CALLBACK => {
            if data.is_null() {
                return with_host(|h| h.audio_status = None).is_some();
            }
            let cb = std::ptr::read(data as *const AudioBufferStatusCallback);
            with_host(|h| h.audio_status = cb.callback).is_some()
        }
        SET_NETPACKET_INTERFACE => {
            if data.is_null() {
                return with_host(|h| h.netpacket = None).is_some();
            }
            with_host(|h| {
                h.netpacket = Some(std::ptr::read(data as *const NetpacketCallback));
            })
            .is_some()
        }
        SET_MEMORY_MAPS => {
            if data.is_null() {
                return false;
            }
            let map = &*(data as *const MemoryMap);
            if map.descriptors.is_null() {
                return false;
            }
            let all = std::slice::from_raw_parts(map.descriptors, map.num_descriptors as usize);
            with_host(|h| h.memory_map = all.to_vec()).is_some()
        }
        _ => false,
    }
}

unsafe extern "C" fn netpacket_send(
    _flags: c_int,
    buf: *const c_void,
    len: usize,
    _client_id: u16,
) {
    if buf.is_null() || len == 0 {
        return;
    }
    let bytes = std::slice::from_raw_parts(buf as *const u8, len).to_vec();
    with_host(|h| h.net.push_outbound(bytes));
}

unsafe extern "C" fn netpacket_poll_receive() {
    let Some((receive, net, client_id)) = with_host(|h| {
        let receive = h.netpacket.as_ref().and_then(|cb| cb.receive)?;
        let client_id = h.net_peer?;
        Some((receive, h.net.clone(), client_id))
    })
    .flatten() else {
        return;
    };
    if !net.is_active() {
        return;
    }
    while let Some(packet) = net.take_inbound() {
        receive(packet.as_ptr() as *const c_void, packet.len(), client_id);
    }
}

unsafe fn begin_link(client_id: u16, started: impl FnOnce()) {
    let Some(start) = with_host(|h| h.netpacket.as_ref().and_then(|cb| cb.start)).flatten() else {
        return;
    };
    start(client_id, netpacket_send, netpacket_poll_receive);
    started();

    let peer = 1u16.wrapping_sub(client_id);
    let connected = with_host(|h| {
        h.net.set_active(true);
        h.net_peer = Some(peer);
        h.netpacket.as_ref().and_then(|cb| cb.connected)
    })
    .flatten();
    if let Some(connected) = connected {
        connected(peer);
    }
}

unsafe fn halt_link() {
    let Some((stop, peer, disconnected)) = with_host(|h| {
        let stop = h.netpacket.as_ref().and_then(|cb| cb.stop);
        let peer = h.net_peer.take();
        let disconnected = h.netpacket.as_ref().and_then(|cb| cb.disconnected);
        (stop, peer, disconnected)
    }) else {
        return;
    };
    if let (Some(peer), Some(disconnected)) = (peer, disconnected) {
        disconnected(peer);
    }
    if let Some(stop) = stop {
        stop();
    }
}

unsafe fn drain_link() {
    let Some((receive, poll, net, client_id)) = with_host(|h| {
        let cb = h.netpacket.as_ref()?;
        let client_id = h.net_peer?;
        Some((cb.receive, cb.poll, h.net.clone(), client_id))
    })
    .flatten() else {
        return;
    };
    if !net.is_active() {
        return;
    }
    if let Some(receive) = receive {
        while let Some(packet) = net.take_inbound() {
            receive(packet.as_ptr() as *const c_void, packet.len(), client_id);
        }
    }
    if let Some(poll) = poll {
        poll();
    }
}

unsafe fn report_audio_status(skip: bool) {
    let Some(status) = with_host(|h| h.audio_status).flatten() else {
        return;
    };
    status(true, if skip { 0 } else { 100 }, skip);
}

unsafe extern "C" fn video_refresh(
    data: *const c_void,
    width: c_uint,
    height: c_uint,
    pitch: usize,
) {
    // drawn on the GPU, into the framebuffer the frontend gave
    if data == HW_FRAME_BUFFER_VALID {
        with_host(|h| h.hw_frame = Some((width, height)));
        return;
    }
    // a null frame is a duplicate: the last picture stands
    if data.is_null() || width == 0 || height == 0 {
        return;
    }
    with_host(|h| h.take_frame(data as *const u8, width, height, pitch));
}

unsafe extern "C" fn audio_sample(left: i16, right: i16) {
    with_host(|h| h.audio.extend_from_slice(&[left, right]));
}

unsafe extern "C" fn audio_batch(data: *const i16, frames: usize) -> usize {
    if !data.is_null() {
        with_host(|h| {
            h.audio
                .extend_from_slice(std::slice::from_raw_parts(data, frames * 2));
        });
    }
    frames
}

unsafe extern "C" fn input_poll() {}

/// The framebuffer a core that draws on the GPU draws the frame into.
unsafe extern "C" fn hw_framebuffer() -> usize {
    with_host(|h| h.gl.as_ref().map_or(0, |gl| gl.framebuffer()))
        .unwrap_or_else(|| HW_FRAMEBUFFER.load(Ordering::Relaxed))
}

/// A GL function a core that draws on the GPU looks up.
unsafe extern "C" fn hw_proc_address(sym: *const c_char) -> Option<HwProc> {
    if sym.is_null() {
        return None;
    }
    let name = std::ffi::CStr::from_ptr(sym);
    let at = with_host(|h| {
        h.gl.as_ref()
            .map_or(ptr::null(), |gl| gl.proc_address(name))
    })
    .unwrap_or(ptr::null());
    // SAFETY: a non-null address the GL loader gave for `name`, a function of no arguments as
    // libretro types every GL function it hands over
    (!at.is_null()).then(|| std::mem::transmute::<*const c_void, HwProc>(at))
}

unsafe extern "C" fn input_state(port: c_uint, device: c_uint, index: c_uint, id: c_uint) -> i16 {
    if device == DEVICE_ANALOG {
        if index != DEVICE_INDEX_ANALOG_LEFT {
            return 0;
        }
        return with_host(|h| {
            h.analog
                .get(port as usize)
                .and_then(|stick| stick.get(id as usize))
                .copied()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    }
    if device != DEVICE_JOYPAD {
        return 0;
    }
    with_host(|h| {
        let Some(&input) = h.inputs.get(port as usize) else {
            return 0;
        };
        match id {
            JOYPAD_MASK => input as i16,
            _ if id < 16 => ((input >> id) & 1) as i16,
            _ => 0,
        }
    })
    .unwrap_or(0)
}

pub struct LibretroCore {
    api: Api,
    host: Box<Host>,
    rom: Vec<u8>,
    rom_path: Option<CString>,
    /// The disc to start from, and the file it is read from, handed to the core before it loads.
    initial_disc: Option<(c_uint, CString)>,
    loaded: bool,
    /// gpSP's library, and its link state where its symbol table names it: none for another core.
    gpsp: Option<(std::path::PathBuf, Option<crate::gpsp::SerialState>)>,
    _lib: Library,
}

fn cdir(path: &Path) -> Result<CString, CoreError> {
    CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| CoreError::Load(format!("{} contains a nul", path.display())))
}

/// The game as the core is handed it: its bytes, or nothing for a core that reads the file
/// itself from its path. A PlayStation disc runs to hundreds of megabytes, which a device with
/// 1 GB must never read into memory whole.
fn game_bytes(rom: &Path, need_fullpath: bool) -> Result<Vec<u8>, CoreError> {
    if need_fullpath {
        return Ok(Vec::new());
    }
    Ok(std::fs::read(rom)?)
}

/// A string a core wrote into `buf`, up to its terminator.
fn written(buf: &[u8]) -> Option<String> {
    let end = buf.iter().position(|&b| b == 0)?;
    std::str::from_utf8(&buf[..end]).ok().map(str::to_string)
}

/// How many discs are in the core's drive: 1 for a core with no drive, a game on one disc.
unsafe fn discs_in(disk: Option<DiskControlExtCallback>) -> usize {
    match disk.and_then(|d| d.get_num_images) {
        Some(count) => count() as usize,
        None => 1,
    }
}

/// Which disc is in the drive, counting from 0.
unsafe fn disc_in(disk: Option<DiskControlExtCallback>) -> usize {
    match disk.and_then(|d| d.get_image_index) {
        Some(index) => index() as usize,
        None => 0,
    }
}

/// Opens the drive, puts disc `index` in and closes it again, with the host bound for the
/// core's calls back. A core keeps the lid reading open a while after it closes, long enough for
/// the game to see the swap, so the three come one after the other.
unsafe fn swap_disc(disk: Option<DiskControlExtCallback>, index: usize) -> bool {
    let Some(d) = disk else {
        return false;
    };
    let (Some(eject), Some(set)) = (d.set_eject_state, d.set_image_index) else {
        return false;
    };
    if index >= discs_in(disk) {
        return false;
    }
    if !eject(true) {
        return false;
    }
    let swapped = set(index as c_uint);
    let closed = eject(false);
    swapped && closed
}

/// The file disc `index` is read from, through the extended interface.
unsafe fn disc_file(disk: Option<DiskControlExtCallback>, index: usize) -> Option<String> {
    let path = disk.and_then(|d| d.get_image_path)?;
    let mut buf = vec![0u8; 4096];
    let ok = path(index as c_uint, buf.as_mut_ptr() as *mut c_char, buf.len());
    ok.then(|| written(&buf)).flatten()
}

impl LibretroCore {
    pub fn open(dylib: &Path) -> Result<Self, CoreError> {
        let dir = dylib
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        Self::open_with(dylib, &dir, &dir)
    }

    pub fn open_with(dylib: &Path, system_dir: &Path, save_dir: &Path) -> Result<Self, CoreError> {
        Self::open_with_options(dylib, system_dir, save_dir, &[])
    }

    /// Opens the core with `options` already set, for a core that reads some as it starts,
    /// as pcsx_rearmed reads which BIOS to run.
    pub fn open_with_options(
        dylib: &Path,
        system_dir: &Path,
        save_dir: &Path,
        options: &[(&str, &str)],
    ) -> Result<Self, CoreError> {
        if LIVE.swap(true, Ordering::SeqCst) {
            return Err(CoreError::Unsupported("a core is already open".into()));
        }
        Self::open_inner(dylib, system_dir, save_dir, options)
            .inspect_err(|_| LIVE.store(false, Ordering::SeqCst))
    }

    pub fn reported_system_dir(&self) -> String {
        self.host.system_dir.to_string_lossy().into_owned()
    }

    pub fn reported_save_dir(&self) -> String {
        self.host.save_dir.to_string_lossy().into_owned()
    }

    pub fn asked_for_rumble(&self) -> bool {
        self.host.asked_for_rumble
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        if !self.host.declared.is_empty() {
            match self.host.declared.get(key) {
                None => {
                    eprintln!("omega-retro: core declares no option {key:?}, setting it anyway");
                }
                Some(values) if !values.is_empty() && !values.iter().any(|v| v == value) => {
                    eprintln!("omega-retro: core declares {key:?} as {values:?}, not {value:?}");
                }
                Some(_) => {}
            }
        }
        let Ok(value) = CString::new(value) else {
            return;
        };
        self.host.options.insert(key.to_string(), value);
        self.host.options_dirty = true;
    }

    pub fn options(&self) -> Vec<(String, String)> {
        self.host
            .options
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), v.to_str().ok()?.to_string())))
            .collect()
    }

    pub fn declared_options(&self) -> &std::collections::HashMap<String, Vec<String>> {
        &self.host.declared
    }

    pub fn option(&self, key: &str) -> Option<String> {
        self.host
            .options
            .get(key)
            .and_then(|v| v.to_str().ok())
            .map(std::string::ToString::to_string)
    }

    fn system_info(&self) -> SystemInfo {
        let mut info = SystemInfo::default();
        unsafe { (self.api.get_system_info)(&mut info) };
        info
    }

    /// The name the core goes by, such as "Snes9x".
    pub fn library_name(&self) -> String {
        let name = self.system_info().library_name;
        if name.is_null() {
            return String::new();
        }
        unsafe { std::ffi::CStr::from_ptr(name) }
            .to_string_lossy()
            .into_owned()
    }

    /// Whether the core reads its games from their paths itself rather than from memory.
    pub fn needs_fullpath(&self) -> bool {
        self.system_info().need_fullpath
    }

    /// Whether the core handed over a drive whose discs can be swapped.
    pub fn has_disk_control(&self) -> bool {
        self.host.disk.is_some()
    }

    /// The picture's size and shape as the core last described them: base width, base height
    /// and aspect ratio, 0 where it never said.
    pub fn geometry(&self) -> (u32, u32, f32) {
        let g = self.host.geometry;
        (g.base_width, g.base_height, g.aspect_ratio)
    }

    fn open_inner(
        dylib: &Path,
        system_dir: &Path,
        save_dir: &Path,
        options: &[(&str, &str)],
    ) -> Result<Self, CoreError> {
        let lib = unsafe { Library::new(dylib) }.map_err(|e| CoreError::Load(e.to_string()))?;
        let api = unsafe { Api::load(&lib) }?;
        let version = unsafe { (api.api_version)() };
        if version != API_VERSION {
            return Err(CoreError::Unsupported(format!("libretro api {version}")));
        }
        let mut host = Box::new(Host::new(cdir(system_dir)?, cdir(save_dir)?));
        for (key, value) in options {
            if let Ok(value) = CString::new(*value) {
                host.options.insert(key.to_string(), value);
            }
        }
        unsafe {
            let _a = Active::bind(&mut host);
            (api.set_environment)(environment);
            (api.set_video_refresh)(video_refresh);
            (api.set_audio_sample)(audio_sample);
            (api.set_audio_sample_batch)(audio_batch);
            (api.set_input_poll)(input_poll);
            (api.set_input_state)(input_state);
            // gpSP's code cache takes the room kept beside it as it is made
            let gpsp = crate::gpsp::is_gpsp(dylib);
            if gpsp {
                crate::gpsp::make_room(dylib);
            }
            (api.init)();
            if gpsp {
                crate::gpsp::made_room(dylib);
            }
        }
        let gpsp = crate::gpsp::is_gpsp(dylib).then(|| {
            let serial = crate::gpsp::SerialState::find(dylib, api.init as usize);
            if let Some(serial) = &serial {
                // a gpSP kept loaded starts each game from a clean link, as one loaded afresh
                // SAFETY: gpSP is loaded, and on this thread, which runs it
                unsafe {
                    serial.reset();
                    serial.alone();
                }
            }
            (dylib.to_path_buf(), serial)
        });
        Ok(LibretroCore {
            api,
            host,
            rom: Vec::new(),
            rom_path: None,
            initial_disc: None,
            loaded: false,
            gpsp,
            _lib: lib,
        })
    }

    fn unload(&mut self) {
        if !self.loaded {
            return;
        }
        let _a = Active::bind(&mut self.host);
        unsafe { (self.api.unload_game)() };
        self.loaded = false;
    }
}

impl Drop for LibretroCore {
    fn drop(&mut self) {
        self.unload();
        unsafe {
            let _a = Active::bind(&mut self.host);
            (self.api.deinit)();
        }
        // gpSP's cache has gone with its deinit: its room is kept again
        if let Some((path, _)) = &self.gpsp {
            crate::gpsp::keep_room_again(path);
        }
        LIVE.store(false, Ordering::SeqCst);
    }
}

impl RetroCore for LibretroCore {
    fn set_option(&mut self, key: &str, value: &str) {
        LibretroCore::set_option(self, key, value);
    }

    fn load(&mut self, rom: &Path) -> Result<(), CoreError> {
        self.unload();
        self.rom = game_bytes(rom, self.needs_fullpath())?;
        self.rom_path = Some(
            CString::new(rom.as_os_str().as_encoded_bytes())
                .map_err(|_| CoreError::Load("rom path contains a nul".into()))?,
        );
        let info = GameInfo {
            path: self.rom_path.as_ref().map_or(ptr::null(), |p| p.as_ptr()),
            data: if self.rom.is_empty() {
                ptr::null()
            } else {
                self.rom.as_ptr() as *const c_void
            },
            size: self.rom.len(),
            meta: ptr::null(),
        };
        if let Some((index, path)) = &self.initial_disc {
            if let Some(set) = self.host.disk.and_then(|d| d.set_initial_image) {
                let _a = Active::bind(&mut self.host);
                unsafe { set(*index, path.as_ptr()) };
            }
        }
        let ok = {
            let _a = Active::bind(&mut self.host);
            unsafe { (self.api.load_game)(&info) }
        };
        if !ok {
            self.rom = Vec::new();
            self.rom_path = None;
            return Err(CoreError::Load(format!("core refused {}", rom.display())));
        }
        self.loaded = true;
        let mut av = SystemAvInfo::default();
        unsafe { (self.api.get_system_av_info)(&mut av) };
        self.host.av = AvInfo {
            fps: av.timing.fps,
            sample_rate: av.timing.sample_rate,
        };
        self.host.geometry = av.geometry;
        unsafe { (self.api.set_controller_port_device)(0, DEVICE_JOYPAD) };
        Ok(())
    }

    fn run_frame(&mut self, input: ButtonMask) {
        self.run_frame_linked(input, ButtonMask::default());
    }

    fn run_frame_linked(&mut self, p1: ButtonMask, p2: ButtonMask) {
        if !self.loaded {
            return;
        }
        self.host.inputs = [p1.0, p2.0];
        self.host.hw_frame = None;
        let _a = Active::bind(&mut self.host);
        unsafe { (self.api.run)() };
    }

    /// Asks the core to skip drawing the next frame both ways a core may listen: an audio
    /// underrun for a core with frame skipping, such as pcsx_rearmed, and video turned off for
    /// one that asks whether to draw, such as Snes9x.
    fn set_frame_skip(&mut self, skip: bool) {
        self.host.skip_video = skip;
        let _a = Active::bind(&mut self.host);
        unsafe { report_audio_status(skip) };
    }

    fn video_xrgb8888(&self) -> &[u8] {
        &self.host.video
    }

    fn offer_gpu(&mut self, host: Box<dyn crate::core::GlHost>) {
        self.host.gl = Some(host);
    }

    fn gpu_request(&self) -> Option<crate::core::GpuRequest> {
        let hw = self.host.hw?;
        let version = match hw.context_type {
            HW_CONTEXT_OPENGLES2 => (2, 0),
            HW_CONTEXT_OPENGLES3 => (3, 0),
            _ => (hw.version_major.max(2), hw.version_minor),
        };
        Some(crate::core::GpuRequest {
            version,
            depth: hw.depth,
            stencil: hw.stencil,
            bottom_left: hw.bottom_left_origin,
        })
    }

    fn gpu_context_reset(&mut self) {
        let Some(reset) = self.host.hw.and_then(|hw| hw.context_reset) else {
            return;
        };
        let _a = Active::bind(&mut self.host);
        unsafe { reset() };
    }

    fn gpu_context_destroy(&mut self) {
        let Some(destroy) = self.host.hw.and_then(|hw| hw.context_destroy) else {
            return;
        };
        let _a = Active::bind(&mut self.host);
        unsafe { destroy() };
    }

    fn gpu_frame(&self) -> Option<(u32, u32)> {
        self.host.hw_frame
    }

    fn set_analog(&mut self, port: usize, x: i16, y: i16) {
        if let Some(stick) = self.host.analog.get_mut(port) {
            *stick = [x, y];
        }
    }

    fn video_size(&self) -> (u32, u32) {
        self.host.video_size
    }

    fn take_audio(&mut self) -> Vec<i16> {
        std::mem::take(&mut self.host.audio)
    }

    fn take_audio_into(&mut self, out: &mut Vec<i16>) {
        out.clear();
        std::mem::swap(out, &mut self.host.audio);
    }

    fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
        let mut buf = Vec::new();
        self.serialize_into(&mut buf)?;
        Ok(buf)
    }

    fn serialize_into(&mut self, buf: &mut Vec<u8>) -> Result<(), CoreError> {
        let size = unsafe { (self.api.serialize_size)() };
        if size == 0 {
            return Err(CoreError::State("core reports no state".into()));
        }
        // a buffer handed back is zeroed and written over, as a new one would be, without
        // asking the system for its memory again
        buf.clear();
        buf.resize(size, 0);
        let ok = {
            let _a = Active::bind(&mut self.host);
            unsafe { (self.api.serialize)(buf.as_mut_ptr() as *mut c_void, size) }
        };
        if ok {
            Ok(())
        } else {
            Err(CoreError::State("serialize refused".into()))
        }
    }

    fn unserialize(&mut self, data: &[u8]) -> Result<(), CoreError> {
        let ok = {
            let _a = Active::bind(&mut self.host);
            unsafe { (self.api.unserialize)(data.as_ptr() as *const c_void, data.len()) }
        };
        if ok {
            Ok(())
        } else {
            Err(CoreError::State("unserialize refused".into()))
        }
    }

    fn reset(&mut self) {
        if !self.loaded {
            return;
        }
        let _a = Active::bind(&mut self.host);
        unsafe { (self.api.reset)() };
    }

    fn memory(&self) -> Option<CoreMemory> {
        if !self.loaded {
            return None;
        }
        let region = |id| {
            let data = unsafe { (self.api.get_memory_data)(id) } as *mut u8;
            let len = unsafe { (self.api.get_memory_size)(id) };
            match data.is_null() {
                true => (ptr::null_mut(), 0),
                false => (data, len),
            }
        };
        Some(CoreMemory {
            map: self.host.memory_map.clone(),
            system: region(MEMORY_SYSTEM_RAM),
            save: region(MEMORY_SAVE_RAM),
            video: region(MEMORY_VIDEO_RAM),
        })
    }

    fn save_ram(&self) -> Option<Vec<u8>> {
        let data = unsafe { (self.api.get_memory_data)(MEMORY_SAVE_RAM) };
        let len = unsafe { (self.api.get_memory_size)(MEMORY_SAVE_RAM) };
        if data.is_null() || len == 0 {
            return None;
        }
        Some(unsafe { std::slice::from_raw_parts(data as *const u8, len) }.to_vec())
    }

    fn load_save_ram(&mut self, data: &[u8]) -> Result<(), CoreError> {
        let dst = unsafe { (self.api.get_memory_data)(MEMORY_SAVE_RAM) };
        let len = unsafe { (self.api.get_memory_size)(MEMORY_SAVE_RAM) };
        if dst.is_null() || len == 0 {
            return Err(CoreError::Unsupported("core exposes no save ram".into()));
        }
        let n = len.min(data.len());
        unsafe { ptr::copy_nonoverlapping(data.as_ptr(), dst as *mut u8, n) };
        Ok(())
    }

    fn av_info(&self) -> AvInfo {
        self.host.av
    }

    fn disc_count(&self) -> usize {
        unsafe { discs_in(self.host.disk) }
    }

    fn disc_index(&self) -> usize {
        unsafe { disc_in(self.host.disk) }
    }

    fn set_disc(&mut self, index: usize) -> bool {
        let disk = self.host.disk;
        let _a = Active::bind(&mut self.host);
        unsafe { swap_disc(disk, index) }
    }

    fn disc_path(&self, index: usize) -> Option<String> {
        unsafe { disc_file(self.host.disk, index) }
    }

    fn set_initial_disc(&mut self, index: usize, path: &str) {
        self.initial_disc = CString::new(path).ok().map(|p| (index as c_uint, p));
    }

    fn rumble(&self) -> Rumble {
        self.host.rumble.clone()
    }

    fn net(&self) -> Link {
        self.host.net.clone()
    }

    fn start_link(&mut self, client_id: u16) {
        let serial = self.gpsp.as_ref().and_then(|(_, s)| s.as_ref());
        let _a = Active::bind(&mut self.host);
        // gpSP starts the session from a clean link, whatever the game did with it alone
        // SAFETY: gpSP is loaded, and on this thread, which runs it
        unsafe {
            begin_link(client_id, || {
                if let Some(serial) = serial {
                    serial.reset();
                }
            });
        };
    }

    fn pump_link(&mut self) {
        let _a = Active::bind(&mut self.host);
        unsafe { drain_link() };
    }

    fn stop_link(&mut self) {
        let _a = Active::bind(&mut self.host);
        unsafe { halt_link() };
        // and leaves it alone again, with no peer's buffers left behind
        if let Some(serial) = self.gpsp.as_ref().and_then(|(_, s)| s.as_ref()) {
            // SAFETY: gpSP is loaded, and on this thread, which runs it
            unsafe {
                serial.alone();
                serial.reset();
            }
        }
    }
}

/// Where this computer keeps its own build of `library`'s core under `vendor`, the repo's
/// vendor/: in vendor/host/, apart from the console's, which on Linux have the same names. A
/// Mac's were kept in vendor/ itself, as .dylib files no console core is ever named, and are
/// still found there. The path is where it would be when there is none, for the caller to
/// find nothing at.
pub fn host_core(vendor: &Path, library: &str) -> std::path::PathBuf {
    let name = format!("{library}_libretro.{}", std::env::consts::DLL_EXTENSION);
    let own = vendor.join("host").join(&name);
    let mac = vendor.join(&name);
    if !own.exists() && cfg!(target_os = "macos") && mac.exists() {
        return mac;
    }
    own
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::ffi::CStr;

    fn host_with(options: HashMap<String, CString>, options_dirty: bool) -> Box<Host> {
        let mut host = Host::new(CString::new(".").unwrap(), CString::new(".").unwrap());
        host.format = PixelFormat::Xrgb8888;
        host.options = options;
        host.options_dirty = options_dirty;
        Box::new(host)
    }

    /// A GPU that hands out framebuffer 7, and `glClear` at a made-up address.
    struct FakeGpu;

    impl crate::core::GlHost for FakeGpu {
        fn framebuffer(&self) -> usize {
            7
        }
        fn proc_address(&self, name: &std::ffi::CStr) -> *const c_void {
            match name.to_bytes() {
                b"glClear" => 0x1234 as *const c_void,
                _ => ptr::null(),
            }
        }
    }

    fn hw_request(context_type: c_uint) -> HwRenderCallback {
        HwRenderCallback {
            context_type,
            context_reset: None,
            get_current_framebuffer: None,
            get_proc_address: None,
            depth: true,
            stencil: true,
            bottom_left_origin: true,
            version_major: 3,
            version_minor: 0,
            cache_context: false,
            context_destroy: None,
            debug_context: false,
        }
    }

    #[test]
    fn a_gpu_core_is_given_a_context_only_where_one_was_offered() {
        let mut host = host_with(HashMap::new(), false);
        let mut cb = hw_request(HW_CONTEXT_OPENGLES3);
        {
            let _a = Active::bind(&mut host);
            assert!(
                !unsafe { environment(SET_HW_RENDER, &mut cb as *mut _ as *mut c_void) },
                "a frontend that offered no GPU took a GPU core"
            );
        }
        host.gl = Some(Box::new(FakeGpu));
        let _a = Active::bind(&mut host);
        assert!(unsafe { environment(SET_HW_RENDER, &mut cb as *mut _ as *mut c_void) });
        let fb = cb
            .get_current_framebuffer
            .expect("the framebuffer callback");
        assert_eq!(unsafe { fb() }, 7);
        let lookup = cb.get_proc_address.expect("the lookup callback");
        let clear = CString::new("glClear").unwrap();
        let none = CString::new("glNotAThing").unwrap();
        assert!(unsafe { lookup(clear.as_ptr()) }.is_some());
        assert!(unsafe { lookup(none.as_ptr()) }.is_none());
        let mut preferred: c_uint = 0;
        assert!(unsafe {
            environment(
                GET_PREFERRED_HW_RENDER,
                &mut preferred as *mut _ as *mut c_void,
            )
        });
        assert_eq!(preferred, HW_CONTEXT_OPENGLES3);
    }

    #[test]
    fn only_opengl_es_is_taken() {
        let mut host = host_with(HashMap::new(), false);
        host.gl = Some(Box::new(FakeGpu));
        let _a = Active::bind(&mut host);
        // RETRO_HW_CONTEXT_OPENGL, _OPENGL_CORE and _VULKAN: none is the handheld's
        for kind in [1, 3, 6] {
            let mut cb = hw_request(kind);
            assert!(!unsafe { environment(SET_HW_RENDER, &mut cb as *mut _ as *mut c_void) });
        }
    }

    /// PPSSPP asks for the framebuffer from its own emulation thread, where no host is bound:
    /// it is told the one the call into the core was given, not 0.
    #[test]
    fn the_framebuffer_is_told_to_a_core_s_own_thread_too() {
        let mut host = host_with(HashMap::new(), false);
        host.gl = Some(Box::new(FakeGpu));
        let _a = Active::bind(&mut host);
        let here = unsafe { hw_framebuffer() };
        assert_ne!(here, 0);
        let there = std::thread::spawn(|| unsafe { hw_framebuffer() })
            .join()
            .unwrap();
        assert_eq!(
            there, here,
            "the core's own thread was told another framebuffer"
        );
    }

    /// PPSSPP built for OpenGL ES asks for RETRO_HW_CONTEXT_OPENGLES2, 2 in libretro.h: refusing it
    /// left PPSSPP drawing every frame on the CPU.
    #[test]
    fn opengl_es_is_taken_by_libretro_s_own_numbers() {
        let mut host = host_with(HashMap::new(), false);
        host.gl = Some(Box::new(FakeGpu));
        let _a = Active::bind(&mut host);
        for kind in [2, 4, 5] {
            let mut cb = hw_request(kind);
            assert!(
                unsafe { environment(SET_HW_RENDER, &mut cb as *mut _ as *mut c_void) },
                "context type {kind} refused"
            );
        }
    }

    #[test]
    fn a_frame_drawn_on_the_gpu_is_its_size_and_leaves_the_pixels_alone() {
        let mut host = host_with(HashMap::new(), false);
        let before = host.video.clone();
        let _a = Active::bind(&mut host);
        unsafe { video_refresh(HW_FRAME_BUFFER_VALID, 480, 272, 0) };
        drop(_a);
        assert_eq!(host.hw_frame, Some((480, 272)));
        assert_eq!(host.video, before);
    }

    #[test]
    fn the_left_stick_is_read_as_the_core_asks_for_it() {
        let mut host = host_with(HashMap::new(), false);
        host.analog[0] = [-32768, 12000];
        let _a = Active::bind(&mut host);
        unsafe {
            assert_eq!(
                input_state(0, DEVICE_ANALOG, DEVICE_INDEX_ANALOG_LEFT, 0),
                -32768
            );
            assert_eq!(
                input_state(0, DEVICE_ANALOG, DEVICE_INDEX_ANALOG_LEFT, 1),
                12000
            );
            assert_eq!(
                input_state(0, DEVICE_ANALOG, 1, 0),
                0,
                "the right stick is not there"
            );
            assert_eq!(
                input_state(1, DEVICE_ANALOG, DEVICE_INDEX_ANALOG_LEFT, 0),
                0
            );
        }
    }

    fn declaration(key: &str, value: &str) -> (Variable, CString, CString) {
        let key = CString::new(key).unwrap();
        let value = CString::new(value).unwrap();
        let var = Variable {
            key: key.as_ptr(),
            value: value.as_ptr(),
        };
        (var, key, value)
    }

    fn end_of_list() -> Variable {
        Variable {
            key: ptr::null(),
            value: ptr::null(),
        }
    }

    #[test]
    fn set_variables_records_what_the_core_declared() {
        let mut host = host_with(HashMap::new(), false);
        let (a, _ak, _av) = declaration("mgba_sgb_borders", "Use Super Game Boy Borders; ON|OFF");
        let (b, _bk, _bv) =
            declaration("mgba_gb_colors_preset", "Game Boy Palette Preset; 0|1|2|3");
        let list = [a, b, end_of_list()];
        let ok = {
            let _active = Active::bind(&mut host);
            unsafe { environment(SET_VARIABLES, list.as_ptr() as *mut c_void) }
        };

        assert!(ok);
        assert_eq!(
            host.declared.get("mgba_sgb_borders").map(Vec::as_slice),
            Some(["ON".to_string(), "OFF".to_string()].as_slice())
        );
        assert_eq!(
            host.declared
                .get("mgba_gb_colors_preset")
                .map(Vec::as_slice),
            Some(["0", "1", "2", "3"].map(str::to_string).as_slice())
        );
        assert_eq!(
            host.declared.len(),
            2,
            "the walk ran past the terminator and read whatever was after it"
        );
        assert!(
            !host.declared.contains_key("mgba_sgb_border"),
            "a key the core never declared came back declared"
        );
    }

    #[test]
    fn set_variables_with_no_list_declares_nothing() {
        let mut host = host_with(HashMap::new(), false);
        host.declared
            .insert("stale".to_string(), vec!["x".to_string()]);

        let ok = {
            let _active = Active::bind(&mut host);
            unsafe { environment(SET_VARIABLES, ptr::null_mut()) }
        };

        assert!(ok);
        assert!(host.declared.is_empty(), "a stale declaration survived");
    }

    #[test]
    fn get_variable_writes_the_options_pointer_for_the_core_to_read() {
        let mut options = HashMap::new();
        options.insert("gpsp_serial".to_string(), CString::new("rfu").unwrap());
        let mut host = host_with(options, false);
        let _active = Active::bind(&mut host);

        let key = CString::new("gpsp_serial").unwrap();
        let mut var = Variable {
            key: key.as_ptr(),
            value: ptr::null(),
        };
        let ok = unsafe { environment(GET_VARIABLE, &mut var as *mut Variable as *mut c_void) };

        assert!(ok);
        let value = unsafe { CStr::from_ptr(var.value) };
        assert_eq!(value.to_str().unwrap(), "rfu");
    }

    #[test]
    fn get_variable_reports_false_for_an_unknown_key() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);

        let key = CString::new("nope").unwrap();
        let mut var = Variable {
            key: key.as_ptr(),
            value: ptr::null(),
        };
        let ok = unsafe { environment(GET_VARIABLE, &mut var as *mut Variable as *mut c_void) };

        assert!(!ok);
        assert!(var.value.is_null());
    }

    /// What `key` reads as to the core, if it reads as anything.
    fn read_variable(key: &str) -> Option<String> {
        let key = CString::new(key).unwrap();
        let mut var = Variable {
            key: key.as_ptr(),
            value: ptr::null(),
        };
        let ok = unsafe { environment(GET_VARIABLE, &mut var as *mut Variable as *mut c_void) };
        (ok && !var.value.is_null()).then(|| {
            unsafe { CStr::from_ptr(var.value) }
                .to_string_lossy()
                .into_owned()
        })
    }

    #[test]
    fn an_option_left_alone_reads_as_the_first_value_the_core_listed() {
        let mut options = HashMap::new();
        options.insert("faust_slend".to_string(), CString::new("200").unwrap());
        let mut host = host_with(options, true);
        let (a, _ak, _av) = declaration("faust_slstart", "First line; 0|1|2");
        let (b, _bk, _bv) = declaration("faust_slend", "Last line; 223|222");
        let list = [a, b, end_of_list()];
        let _active = Active::bind(&mut host);
        assert!(unsafe { environment(SET_VARIABLES, list.as_ptr() as *mut c_void) });

        assert_eq!(read_variable("faust_slstart").as_deref(), Some("0"));
        assert_eq!(
            read_variable("faust_slend").as_deref(),
            Some("200"),
            "the default won over the value omega-zero set"
        );
        assert_eq!(read_variable("faust_never_declared"), None);

        assert!(unsafe { environment(SET_VARIABLES, ptr::null_mut()) });
        assert_eq!(
            read_variable("faust_slstart"),
            None,
            "a default outlived its declaration"
        );
    }

    #[test]
    fn get_variable_update_reports_and_clears_the_dirty_flag() {
        let mut host = host_with(HashMap::new(), true);
        let _active = Active::bind(&mut host);

        let mut dirty = false;
        let ok =
            unsafe { environment(GET_VARIABLE_UPDATE, &mut dirty as *mut bool as *mut c_void) };
        assert!(ok);
        assert!(dirty, "first call must report the pending change");

        let mut dirty_again = true;
        let ok = unsafe {
            environment(
                GET_VARIABLE_UPDATE,
                &mut dirty_again as *mut bool as *mut c_void,
            )
        };
        assert!(ok);
        assert!(!dirty_again, "flag must be cleared after being read once");
    }

    #[test]
    fn get_variable_refuses_a_null_key() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);

        let mut var = Variable {
            key: ptr::null(),
            value: ptr::null(),
        };
        let ok = unsafe { environment(GET_VARIABLE, &mut var as *mut Variable as *mut c_void) };
        assert!(!ok);
    }

    thread_local! {
        static TEST_AUDIO_STATUS: RefCell<Vec<(bool, c_uint, bool)>> =
            const { RefCell::new(Vec::new()) };
    }

    unsafe extern "C" fn test_audio_status(active: bool, occupancy: c_uint, underrun: bool) {
        TEST_AUDIO_STATUS.with(|r| r.borrow_mut().push((active, occupancy, underrun)));
    }

    #[test]
    fn set_audio_buffer_status_callback_stores_what_the_core_hands_over() {
        let mut host = host_with(HashMap::new(), false);
        let mut cb = AudioBufferStatusCallback {
            callback: Some(test_audio_status),
        };
        let ok = {
            let _active = Active::bind(&mut host);
            unsafe {
                environment(
                    SET_AUDIO_BUFFER_STATUS_CALLBACK,
                    &mut cb as *mut AudioBufferStatusCallback as *mut c_void,
                )
            }
        };

        assert!(ok, "refusing this turns the cores' frameskip off entirely");
        assert!(host.audio_status.is_some(), "the callback was never stored");
    }

    #[test]
    fn set_audio_buffer_status_callback_null_withdraws_it() {
        let mut host = host_with(HashMap::new(), false);
        host.audio_status = Some(test_audio_status);
        let ok = {
            let _active = Active::bind(&mut host);
            unsafe { environment(SET_AUDIO_BUFFER_STATUS_CALLBACK, ptr::null_mut()) }
        };

        assert!(ok, "withdrawing is a legal call and must be answered true");
        assert!(host.audio_status.is_none());
    }

    #[test]
    fn set_frame_skip_tells_the_core_to_skip_by_reporting_an_underrun() {
        TEST_AUDIO_STATUS.with(|r| r.borrow_mut().clear());
        let mut host = host_with(HashMap::new(), false);
        host.audio_status = Some(test_audio_status);
        {
            let _active = Active::bind(&mut host);
            unsafe {
                report_audio_status(true);
                report_audio_status(false);
            }
        }

        TEST_AUDIO_STATUS.with(|r| {
            assert_eq!(
                r.borrow().as_slice(),
                &[(true, 0, true), (true, 100, false)],
                "the skip and the draw did not read as an underrun and a healthy buffer"
            );
        });
    }

    #[test]
    fn set_frame_skip_is_a_noop_when_the_core_registered_no_callback() {
        TEST_AUDIO_STATUS.with(|r| r.borrow_mut().clear());
        let mut host = host_with(HashMap::new(), false);
        {
            let _active = Active::bind(&mut host);
            unsafe { report_audio_status(true) };
        }

        TEST_AUDIO_STATUS.with(|r| assert!(r.borrow().is_empty()));
    }

    thread_local! {
        static TEST_RECEIVED: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
        static TEST_RECEIVED_CLIENT_IDS: RefCell<Vec<u16>> = const { RefCell::new(Vec::new()) };
        static TEST_POLLS: Cell<u32> = const { Cell::new(0) };
        static TEST_START_CLIENT: Cell<Option<u16>> = const { Cell::new(None) };
        static TEST_STOP_CALLS: Cell<u32> = const { Cell::new(0) };
        static TEST_CONNECTED_CLIENT: Cell<Option<u16>> = const { Cell::new(None) };
        static TEST_DISCONNECTED_CLIENT: Cell<Option<u16>> = const { Cell::new(None) };
    }

    fn reset_test_netpacket_recorders() {
        TEST_RECEIVED.with(|r| r.borrow_mut().clear());
        TEST_RECEIVED_CLIENT_IDS.with(|c| c.borrow_mut().clear());
        TEST_POLLS.with(|p| p.set(0));
        TEST_START_CLIENT.with(|c| c.set(None));
        TEST_STOP_CALLS.with(|c| c.set(0));
        TEST_CONNECTED_CLIENT.with(|c| c.set(None));
        TEST_DISCONNECTED_CLIENT.with(|c| c.set(None));
    }

    unsafe extern "C" fn test_receive(buf: *const c_void, len: usize, client_id: u16) {
        let bytes = std::slice::from_raw_parts(buf as *const u8, len).to_vec();
        TEST_RECEIVED.with(|r| r.borrow_mut().push(bytes));
        TEST_RECEIVED_CLIENT_IDS.with(|c| c.borrow_mut().push(client_id));
    }

    unsafe extern "C" fn test_receive_reentrant(buf: *const c_void, len: usize, client_id: u16) {
        test_receive(buf, len, client_id);
        let reentrant = b"reentrant";
        netpacket_send(0, reentrant.as_ptr() as *const c_void, reentrant.len(), 0);
    }

    unsafe extern "C" fn test_start_reentrant(
        client_id: u16,
        _send: NetpacketSend,
        poll_receive: NetpacketPollReceive,
    ) {
        TEST_START_CLIENT.with(|c| c.set(Some(client_id)));
        poll_receive();
    }

    unsafe extern "C" fn test_poll() {
        TEST_POLLS.with(|p| p.set(p.get() + 1));
    }

    unsafe extern "C" fn test_start(
        client_id: u16,
        _send: NetpacketSend,
        _poll_receive: NetpacketPollReceive,
    ) {
        TEST_START_CLIENT.with(|c| c.set(Some(client_id)));
    }

    unsafe extern "C" fn test_stop() {
        TEST_STOP_CALLS.with(|c| c.set(c.get() + 1));
    }

    unsafe extern "C" fn test_connected(client_id: u16) -> bool {
        TEST_CONNECTED_CLIENT.with(|c| c.set(Some(client_id)));
        true
    }

    unsafe extern "C" fn test_disconnected(client_id: u16) {
        TEST_DISCONNECTED_CLIENT.with(|c| c.set(Some(client_id)));
    }

    fn test_netpacket_callback() -> NetpacketCallback {
        NetpacketCallback {
            start: Some(test_start),
            receive: Some(test_receive),
            stop: None,
            poll: Some(test_poll),
            connected: None,
            disconnected: None,
            protocol_version: ptr::null(),
        }
    }

    #[test]
    fn set_netpacket_interface_stores_the_callback_the_core_hands_over() {
        let mut host = host_with(HashMap::new(), false);
        let mut cb = test_netpacket_callback();
        let ok = {
            let _active = Active::bind(&mut host);
            unsafe {
                environment(
                    SET_NETPACKET_INTERFACE,
                    &mut cb as *mut NetpacketCallback as *mut c_void,
                )
            }
        };

        assert!(ok);
        assert!(host.netpacket.is_some(), "the callback was never stored");
    }

    #[test]
    fn set_netpacket_interface_null_data_withdraws_the_interface() {
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        let ok = {
            let _active = Active::bind(&mut host);
            unsafe { environment(SET_NETPACKET_INTERFACE, ptr::null_mut()) }
        };

        assert!(ok, "withdrawing is a legal call and must be answered true");
        assert!(
            host.netpacket.is_none(),
            "a NULL data pointer must clear a previously registered callback"
        );
    }

    #[test]
    fn netpacket_send_pushes_the_cores_packet_onto_outbound() {
        let mut host = host_with(HashMap::new(), false);
        let packet = b"link cable byte";
        {
            let _active = Active::bind(&mut host);
            unsafe {
                netpacket_send(
                    NETPACKET_RELIABLE,
                    packet.as_ptr() as *const c_void,
                    packet.len(),
                    0,
                );
            };
        }

        assert_eq!(host.net.take_outbound().as_deref(), Some(&packet[..]));
        assert_eq!(host.net.take_outbound(), None, "only one packet was sent");
    }

    #[test]
    fn netpacket_send_ignores_a_null_or_empty_packet() {
        let mut host = host_with(HashMap::new(), false);
        {
            let _active = Active::bind(&mut host);
            unsafe { netpacket_send(0, ptr::null(), 4, 0) };
            unsafe { netpacket_send(0, [1u8].as_ptr() as *const c_void, 0, 0) };
        }

        assert_eq!(
            host.net.take_outbound(),
            None,
            "a null buffer or zero length must not enqueue a phantom packet"
        );
    }

    #[test]
    fn netpacket_poll_receive_drains_inbound_into_the_cores_receive_in_order() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        host.net.set_active(true);
        host.net_peer = Some(1);
        host.net.push_inbound(b"first".to_vec());
        host.net.push_inbound(b"second".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { netpacket_poll_receive() };
        }

        TEST_RECEIVED.with(|r| {
            assert_eq!(
                r.borrow().as_slice(),
                &[b"first".to_vec(), b"second".to_vec()],
                "order was not kept"
            );
        });
        assert_eq!(host.net.take_inbound(), None, "queue must be drained");
    }

    #[test]
    fn netpacket_poll_receive_does_nothing_without_a_receive_callback() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.net.set_active(true);
        host.net.push_inbound(b"stranded".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { netpacket_poll_receive() };
        }

        TEST_RECEIVED.with(|r| assert!(r.borrow().is_empty()));
        assert_eq!(
            host.net.take_inbound().as_deref(),
            Some(&b"stranded"[..]),
            "with nowhere to hand the packet it must be left queued, not dropped"
        );
    }

    #[test]
    fn netpacket_poll_receive_does_nothing_once_the_session_is_no_longer_active() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        host.net.push_inbound(b"stale".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { netpacket_poll_receive() };
        }

        TEST_RECEIVED.with(|r| assert!(r.borrow().is_empty(), "an ended session reached the core"));
        assert_eq!(
            host.net.take_inbound().as_deref(),
            Some(&b"stale"[..]),
            "the packet must be left queued, not delivered to a session that already ended"
        );
    }

    #[test]
    fn netpacket_poll_receive_does_nothing_without_a_recorded_peer() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        host.net.set_active(true);
        host.net.push_inbound(b"orphaned".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { netpacket_poll_receive() };
        }

        TEST_RECEIVED.with(|r| {
            assert!(
                r.borrow().is_empty(),
                "a packet must not be delivered with no peer to tag it"
            );
        });
        assert_eq!(
            host.net.take_inbound().as_deref(),
            Some(&b"orphaned"[..]),
            "the packet must be left queued, not delivered mislabelled as our own id"
        );
    }

    #[test]
    fn netpacket_poll_receive_tags_packets_with_the_peers_client_id() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(0, || {}) };
        }
        reset_test_netpacket_recorders();
        host.net.push_inbound(b"from the peer".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { netpacket_poll_receive() };
        }

        TEST_RECEIVED_CLIENT_IDS.with(|c| {
            assert_eq!(
                c.borrow().as_slice(),
                &[1],
                "packets must be tagged with the peer's id, not the host's own"
            );
        });
    }

    #[test]
    fn drain_link_hands_the_core_everything_waiting_then_polls() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        host.net.set_active(true);
        host.net_peer = Some(1);
        host.net.push_inbound(b"queued".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_RECEIVED.with(|r| assert_eq!(r.borrow().as_slice(), &[b"queued".to_vec()]));
        TEST_POLLS.with(|p| assert_eq!(p.get(), 1, "poll must run once a frame"));
    }

    #[test]
    fn drain_link_polls_even_with_nothing_inbound() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        host.net.set_active(true);
        host.net_peer = Some(1);
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_POLLS.with(|p| assert_eq!(p.get(), 1));
    }

    #[test]
    fn drain_link_skips_poll_when_the_core_offered_none() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            poll: None,
            ..test_netpacket_callback()
        });
        host.net.set_active(true);
        host.net_peer = Some(1);
        host.net.push_inbound(b"still delivered".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_RECEIVED.with(|r| {
            assert_eq!(r.borrow().as_slice(), &[b"still delivered".to_vec()]);
        });
        TEST_POLLS.with(|p| assert_eq!(p.get(), 0, "poll is optional and was not offered"));
    }

    #[test]
    fn drain_link_is_a_noop_when_the_core_never_registered_netpacket() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.net.set_active(true);
        host.net.push_inbound(b"nobody asked".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_RECEIVED.with(|r| assert!(r.borrow().is_empty()));
        TEST_POLLS.with(|p| assert_eq!(p.get(), 0));
        assert_eq!(
            host.net.take_inbound().as_deref(),
            Some(&b"nobody asked"[..]),
            "with no registered core the packet must be left queued, not lost"
        );
    }

    #[test]
    fn drain_link_does_nothing_once_the_session_is_no_longer_active() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        host.net.push_inbound(b"stale".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_RECEIVED.with(|r| assert!(r.borrow().is_empty(), "an ended session reached the core"));
        TEST_POLLS.with(|p| assert_eq!(p.get(), 0, "an ended session must not be polled either"));
        assert_eq!(
            host.net.take_inbound().as_deref(),
            Some(&b"stale"[..]),
            "the packet must be left queued, not delivered to a session that already ended"
        );
    }

    #[test]
    fn drain_link_does_nothing_without_a_recorded_peer() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        host.net.set_active(true);
        host.net.push_inbound(b"orphaned".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_RECEIVED.with(|r| {
            assert!(
                r.borrow().is_empty(),
                "a packet must not be delivered with no peer to tag it"
            );
        });
        TEST_POLLS.with(|p| {
            assert_eq!(
                p.get(),
                0,
                "must not even poll with no peer to tag a receive"
            );
        });
        assert_eq!(
            host.net.take_inbound().as_deref(),
            Some(&b"orphaned"[..]),
            "the packet must be left queued, not delivered mislabelled as our own id"
        );
    }

    #[test]
    fn drain_link_tags_packets_with_the_peers_client_id() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(0, || {}) };
        }
        reset_test_netpacket_recorders();
        host.net.push_inbound(b"from the peer".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_RECEIVED_CLIENT_IDS.with(|c| {
            assert_eq!(
                c.borrow().as_slice(),
                &[1],
                "packets must be tagged with the peer's id, not our own"
            );
        });
    }

    #[test]
    fn drain_link_survives_the_reentrancy_gpsp_documents() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            receive: Some(test_receive_reentrant),
            ..test_netpacket_callback()
        });
        host.net.set_active(true);
        host.net_peer = Some(1);
        host.net.push_inbound(b"queued".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { drain_link() };
        }

        TEST_RECEIVED.with(|r| assert_eq!(r.borrow().as_slice(), &[b"queued".to_vec()]));
        assert_eq!(
            host.net.take_outbound().as_deref(),
            Some(&b"reentrant"[..]),
            "the reentrant netpacket_send call must still reach outbound"
        );
    }

    #[test]
    fn netpacket_poll_receive_survives_the_reentrancy_gpsp_documents() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            receive: Some(test_receive_reentrant),
            ..test_netpacket_callback()
        });
        host.net.set_active(true);
        host.net_peer = Some(1);
        host.net.push_inbound(b"queued".to_vec());
        {
            let _active = Active::bind(&mut host);
            unsafe { netpacket_poll_receive() };
        }

        assert_eq!(host.net.take_outbound().as_deref(), Some(&b"reentrant"[..]));
    }

    #[test]
    fn begin_link_survives_a_core_that_reenters_from_start() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            start: Some(test_start_reentrant),
            ..test_netpacket_callback()
        });
        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(0, || {}) };
        }

        TEST_START_CLIENT.with(|c| assert_eq!(c.get(), Some(0)));
        assert!(host.net.is_active());
    }

    #[test]
    fn begin_link_hands_the_core_its_client_id_and_marks_the_session_active() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());
        assert!(
            !host.net.is_active(),
            "a fresh link is not backing a session"
        );
        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(1, || {}) };
        }

        TEST_START_CLIENT.with(|c| assert_eq!(c.get(), Some(1), "client id was not forwarded"));
        assert!(
            host.net.is_active(),
            "starting a session must mark the link active"
        );
    }

    #[test]
    fn begin_link_is_a_noop_when_the_core_never_registered_netpacket() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(0, || {}) };
        }

        TEST_START_CLIENT.with(|c| assert_eq!(c.get(), None, "nothing to start, nothing called"));
        assert!(!host.net.is_active());
    }

    #[test]
    fn begin_link_calls_connected_with_the_peers_client_id() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            connected: Some(test_connected),
            ..test_netpacket_callback()
        });
        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(0, || {}) };
        }

        TEST_CONNECTED_CLIENT.with(|c| {
            assert_eq!(
                c.get(),
                Some(1),
                "connected was not called with the peer's id"
            );
        });
    }

    #[test]
    fn begin_link_is_fine_with_no_connected_callback() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());

        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(0, || {}) };
        }

        assert!(
            host.net.is_active(),
            "a missing connected callback must not stop the session from starting"
        );
    }

    #[test]
    fn halt_link_calls_the_cores_stop_when_it_registered_one() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            stop: Some(test_stop),
            ..test_netpacket_callback()
        });
        {
            let _active = Active::bind(&mut host);
            unsafe { halt_link() };
        }

        TEST_STOP_CALLS.with(|c| assert_eq!(c.get(), 1, "stop was never called"));
    }

    #[test]
    fn halt_link_is_a_noop_when_the_core_never_offered_a_stop() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(test_netpacket_callback());

        {
            let _active = Active::bind(&mut host);
            unsafe { halt_link() };
        }

        TEST_STOP_CALLS.with(|c| assert_eq!(c.get(), 0));
    }

    #[test]
    fn halt_link_calls_disconnected_with_the_peers_client_id() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            disconnected: Some(test_disconnected),
            ..test_netpacket_callback()
        });
        {
            let _active = Active::bind(&mut host);
            unsafe { begin_link(1, || {}) };
        }
        {
            let _active = Active::bind(&mut host);
            unsafe { halt_link() };
        }

        TEST_DISCONNECTED_CLIENT.with(|c| {
            assert_eq!(
                c.get(),
                Some(0),
                "disconnected was not called with the peer's id"
            );
        });
    }

    #[test]
    fn halt_link_does_not_call_disconnected_when_no_session_ever_started() {
        reset_test_netpacket_recorders();
        let mut host = host_with(HashMap::new(), false);
        host.netpacket = Some(NetpacketCallback {
            disconnected: Some(test_disconnected),
            ..test_netpacket_callback()
        });

        {
            let _active = Active::bind(&mut host);
            unsafe { halt_link() };
        }

        TEST_DISCONNECTED_CLIENT.with(|c| assert_eq!(c.get(), None, "nothing was ever connected"));
    }

    #[test]
    fn input_state_answers_each_port_from_its_own_mask() {
        let mut host = host_with(HashMap::new(), false);
        host.inputs = [ButtonMask::A, ButtonMask::B | ButtonMask::START];
        let _active = Active::bind(&mut host);

        let mask = |port| unsafe { input_state(port, DEVICE_JOYPAD, 0, JOYPAD_MASK) } as u16;
        assert_eq!(mask(0), ButtonMask::A);
        assert_eq!(mask(1), ButtonMask::B | ButtonMask::START);
    }

    #[test]
    fn input_state_answers_single_buttons_on_port_1() {
        let mut host = host_with(HashMap::new(), false);
        host.inputs = [0, ButtonMask::B];
        let _active = Active::bind(&mut host);

        let button = |id| unsafe { input_state(1, DEVICE_JOYPAD, 0, id) };
        assert_eq!(button(0), 1, "B is bit 0 of the libretro joypad");
        assert_eq!(button(8), 0, "A is not held on port 1");
    }

    #[test]
    fn input_state_answers_nothing_past_port_1() {
        let mut host = host_with(HashMap::new(), false);
        host.inputs = [u16::MAX, u16::MAX];
        let _active = Active::bind(&mut host);

        assert_eq!(unsafe { input_state(2, DEVICE_JOYPAD, 0, JOYPAD_MASK) }, 0);
    }

    #[test]
    fn a_small_picture_is_centred_in_the_buffer() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        let frame = vec![0xffu8; 160 * 144 * 4];
        unsafe { video_refresh(frame.as_ptr() as *const c_void, 160, 144, 160 * 4) };

        let lit =
            |x: usize, y: usize| unsafe { with_host(|h| h.video[(y * 240 + x) * 4]) }.unwrap() != 0;
        assert!(lit(40, 8), "the top left of the picture is not at (40, 8)");
        assert!(lit(199, 151), "the bottom right is not at (199, 151)");
        assert!(!lit(39, 8), "the picture starts one column too early");
        assert!(!lit(40, 7), "the picture starts one row too early");
    }

    #[test]
    fn the_margin_is_cleared_when_the_picture_shrinks() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        unsafe {
            let full = vec![0xffu8; 240 * 160 * 4];
            video_refresh(full.as_ptr() as *const c_void, 240, 160, 240 * 4);
            let small = vec![0x11u8; 160 * 144 * 4];
            video_refresh(small.as_ptr() as *const c_void, 160, 144, 160 * 4);
        }
        let corner = unsafe { with_host(|h| h.video[0]) }.unwrap();
        assert_eq!(corner, 0, "the previous picture is still in the margin");
    }

    fn gradient(w: usize, h: usize) -> Vec<u8> {
        let mut px = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let o = (y * w + x) * 4;
                px[o] = x as u8;
                px[o + 1] = y as u8;
                px[o + 2] = (x ^ y) as u8;
            }
        }
        px
    }

    #[test]
    fn a_full_size_picture_is_unmoved() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        let frame = gradient(GBA_W as usize, GBA_H as usize);
        unsafe {
            video_refresh(
                frame.as_ptr() as *const c_void,
                GBA_W,
                GBA_H,
                GBA_W as usize * 4,
            );
        };

        assert_eq!(unsafe { with_host(|h| h.video.clone()) }.unwrap(), frame);
    }

    #[test]
    fn get_can_dupe_tells_a_core_the_frontend_keeps_the_last_frame() {
        assert_eq!(GET_CAN_DUPE, 3);

        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);

        let mut can_dupe = false;
        let ok = unsafe { environment(GET_CAN_DUPE, &mut can_dupe as *mut bool as *mut c_void) };

        assert!(ok, "a core that asks has to be answered, not ignored");
        assert!(
            can_dupe,
            "the frontend does keep the last frame, so it must say so"
        );
    }

    #[test]
    fn get_can_dupe_refuses_a_null_pointer() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);

        assert!(!unsafe { environment(GET_CAN_DUPE, ptr::null_mut()) });
    }

    #[test]
    fn a_duplicate_frame_leaves_the_previous_picture_exactly_as_it_was() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        let frame = gradient(GBA_W as usize, GBA_H as usize);
        unsafe {
            video_refresh(
                frame.as_ptr() as *const c_void,
                GBA_W,
                GBA_H,
                GBA_W as usize * 4,
            );
            video_refresh(ptr::null(), GBA_W, GBA_H, GBA_W as usize * 4);
        }

        assert_eq!(
            unsafe { with_host(|h| h.video.clone()) }.unwrap(),
            frame,
            "a duplicate frame changed the picture instead of keeping it"
        );
    }

    #[test]
    fn a_duplicate_frame_keeps_a_centred_game_boy_picture_where_it_is() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        let frame = gradient(160, 144);
        unsafe { video_refresh(frame.as_ptr() as *const c_void, 160, 144, 160 * 4) };
        let drawn = unsafe { with_host(|h| h.video.clone()) }.unwrap();

        unsafe {
            video_refresh(ptr::null(), 160, 144, 160 * 4);
            video_refresh(ptr::null(), 160, 144, 160 * 4);
        }

        let kept = unsafe { with_host(|h| h.video.clone()) }.unwrap();
        assert_eq!(kept, drawn, "the picture did not survive two duplicates");
        let lit = (58 * GBA_W as usize + 140) * 4;
        assert_eq!(
            &kept[lit..lit + 3],
            &[100u8, 50, 100 ^ 50],
            "the picture was blanked or moved where a duplicate should have kept it"
        );
    }

    fn shown(host: &Host) -> (u32, u32, Vec<u8>) {
        (host.video_size.0, host.video_size.1, host.video.clone())
    }

    #[test]
    fn a_snes_picture_arrives_at_its_own_size() {
        let mut host = host_with(HashMap::new(), false);
        let frame = gradient(256, 224);
        {
            let _active = Active::bind(&mut host);
            unsafe { video_refresh(frame.as_ptr() as *const c_void, 256, 224, 256 * 4) };
        }
        assert_eq!(shown(&host), (256, 224, frame));
    }

    #[test]
    fn the_picture_follows_the_core_as_it_changes_resolution() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        for (w, h) in [(320, 240), (640, 480), (512, 448), (256, 224), (368, 240)] {
            let frame = gradient(w, h);
            unsafe {
                video_refresh(
                    frame.as_ptr() as *const c_void,
                    w as c_uint,
                    h as c_uint,
                    w * 4,
                );
            };
            let (sw, sh, px) = unsafe { with_host(|h| shown(h)) }.unwrap();
            assert_eq!((sw as usize, sh as usize), (w, h));
            assert_eq!(px, frame, "the {w}x{h} picture came out changed");
        }
    }

    #[test]
    fn a_frame_with_padding_past_each_row_is_read_row_by_row() {
        let mut host = host_with(HashMap::new(), false);
        let (w, h, pitch) = (320usize, 240usize, 1024 * 4);
        let picture = gradient(w, h);
        let mut padded = vec![0xeeu8; pitch * h];
        for y in 0..h {
            padded[y * pitch..y * pitch + w * 4].copy_from_slice(&picture[y * w * 4..][..w * 4]);
        }
        {
            let _active = Active::bind(&mut host);
            unsafe { video_refresh(padded.as_ptr() as *const c_void, 320, 240, pitch) };
        }
        assert_eq!(shown(&host), (320, 240, picture));
    }

    #[test]
    fn a_picture_past_the_largest_is_cut_to_it() {
        let mut host = host_with(HashMap::new(), false);
        let (w, h) = (MAX_FRAME_W as usize + 16, MAX_FRAME_H as usize + 8);
        let frame = vec![0x40u8; w * h * 4];
        {
            let _active = Active::bind(&mut host);
            unsafe {
                video_refresh(
                    frame.as_ptr() as *const c_void,
                    w as c_uint,
                    h as c_uint,
                    w * 4,
                );
            };
        }
        assert_eq!(host.video_size, (MAX_FRAME_W, MAX_FRAME_H));
        assert_eq!(host.video.len(), (MAX_FRAME_W * MAX_FRAME_H * 4) as usize);
    }

    #[test]
    fn a_core_that_never_names_a_pixel_format_draws_in_0rgb1555() {
        let mut host = Box::new(Host::new(
            CString::new(".").unwrap(),
            CString::new(".").unwrap(),
        ));
        // pure red, green and blue, then white, in 0RGB1555
        let frame: Vec<u16> = [0x7c00u16, 0x03e0, 0x001f, 0x7fff]
            .into_iter()
            .cycle()
            .take(256 * 224)
            .collect();
        {
            let _active = Active::bind(&mut host);
            unsafe { video_refresh(frame.as_ptr() as *const c_void, 256, 224, 256 * 2) };
        }
        assert_eq!(host.format, PixelFormat::Rgb1555);
        assert_eq!(
            &host.video[..16],
            &[0, 0, 255, 0, 0, 255, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0],
            "BGRX of red, green, blue and white"
        );
    }

    #[test]
    fn every_pixel_format_libretro_has_is_taken() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        for (code, want) in [
            (PIXEL_FORMAT_0RGB1555, PixelFormat::Rgb1555),
            (PIXEL_FORMAT_RGB565, PixelFormat::Rgb565),
            (PIXEL_FORMAT_XRGB8888, PixelFormat::Xrgb8888),
        ] {
            let mut c = code;
            let ok = unsafe { environment(SET_PIXEL_FORMAT, &mut c as *mut c_uint as *mut c_void) };
            assert!(ok, "format {code} refused");
            assert_eq!(unsafe { with_host(|h| h.format) }, Some(want));
        }
        let mut unknown: c_uint = 7;
        assert!(!unsafe {
            environment(SET_PIXEL_FORMAT, &mut unknown as *mut c_uint as *mut c_void)
        });
    }

    #[test]
    fn a_core_can_change_its_rate_and_picture_while_it_runs() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        let mut av = SystemAvInfo {
            geometry: GameGeometry {
                base_width: 320,
                base_height: 240,
                max_width: 1024,
                max_height: 512,
                aspect_ratio: 4.0 / 3.0,
            },
            timing: SystemTiming {
                fps: 50.0,
                sample_rate: 44_100.0,
            },
        };
        let ok = unsafe {
            environment(
                SET_SYSTEM_AV_INFO,
                &mut av as *mut SystemAvInfo as *mut c_void,
            )
        };
        assert!(ok);
        assert_eq!(
            unsafe { with_host(|h| h.av) },
            Some(AvInfo {
                fps: 50.0,
                sample_rate: 44_100.0
            })
        );

        let mut g = GameGeometry {
            base_width: 640,
            base_height: 480,
            max_width: 1,
            max_height: 1,
            aspect_ratio: 4.0 / 3.0,
        };
        let ok = unsafe { environment(SET_GEOMETRY, &mut g as *mut GameGeometry as *mut c_void) };
        assert!(ok);
        let geometry = unsafe { with_host(|h| h.geometry) }.unwrap();
        assert_eq!(
            (geometry.base_width, geometry.base_height),
            (640, 480),
            "the new picture size was not taken"
        );
        assert_eq!(
            (geometry.max_width, geometry.max_height),
            (1024, 512),
            "SET_GEOMETRY may not change the largest picture"
        );
    }

    #[test]
    fn a_core_that_reads_its_own_files_is_handed_no_bytes() {
        let missing = Path::new("/nonexistent/omega-zero/game.chd");
        assert!(
            game_bytes(missing, true).is_ok_and(|b| b.is_empty()),
            "the file was read for a core that reads it itself"
        );
        assert!(
            game_bytes(missing, false).is_err(),
            "a core that wants the bytes must have them read"
        );
    }

    thread_local! {
        static DRIVE: RefCell<Drive> = const { RefCell::new(Drive { discs: 3, index: 0, open: false, log: Vec::new() }) };
    }

    struct Drive {
        discs: c_uint,
        index: c_uint,
        open: bool,
        log: Vec<String>,
    }

    unsafe extern "C" fn drive_set_eject(open: bool) -> bool {
        DRIVE.with(|d| {
            let mut d = d.borrow_mut();
            d.open = open;
            d.log.push(if open { "open" } else { "close" }.to_string());
        });
        true
    }

    unsafe extern "C" fn drive_get_eject() -> bool {
        DRIVE.with(|d| d.borrow().open)
    }

    unsafe extern "C" fn drive_get_index() -> c_uint {
        DRIVE.with(|d| d.borrow().index)
    }

    unsafe extern "C" fn drive_set_index(index: c_uint) -> bool {
        DRIVE.with(|d| {
            let mut d = d.borrow_mut();
            if !d.open || index >= d.discs {
                return false;
            }
            d.index = index;
            d.log.push(format!("disc {index}"));
            true
        })
    }

    unsafe extern "C" fn drive_num_images() -> c_uint {
        DRIVE.with(|d| d.borrow().discs)
    }

    unsafe extern "C" fn drive_image_path(index: c_uint, buf: *mut c_char, len: usize) -> bool {
        let path = format!("/Games/PS1/Game (Disc {}).chd\0", index + 1);
        if path.len() > len {
            return false;
        }
        ptr::copy_nonoverlapping(path.as_ptr(), buf as *mut u8, path.len());
        true
    }

    fn reset_drive() {
        DRIVE.with(|d| {
            *d.borrow_mut() = Drive {
                discs: 3,
                index: 0,
                open: false,
                log: Vec::new(),
            }
        });
    }

    fn drive_v0() -> DiskControlCallback {
        DiskControlCallback {
            set_eject_state: Some(drive_set_eject),
            get_eject_state: Some(drive_get_eject),
            get_image_index: Some(drive_get_index),
            set_image_index: Some(drive_set_index),
            get_num_images: Some(drive_num_images),
            replace_image_index: None,
            add_image_index: None,
        }
    }

    #[test]
    fn the_frontend_says_it_speaks_the_extended_disk_control() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        let mut version: c_uint = 0;
        let ok = unsafe {
            environment(
                GET_DISK_CONTROL_INTERFACE_VERSION,
                &mut version as *mut c_uint as *mut c_void,
            )
        };
        assert!(ok);
        assert_eq!(version, 1);
    }

    #[test]
    fn a_disc_swap_opens_the_drive_changes_the_disc_and_closes_it() {
        reset_drive();
        let mut host = host_with(HashMap::new(), false);
        let mut cb = drive_v0();
        let ok = {
            let _active = Active::bind(&mut host);
            unsafe {
                environment(
                    SET_DISK_CONTROL_INTERFACE,
                    &mut cb as *mut DiskControlCallback as *mut c_void,
                )
            }
        };
        assert!(ok, "the drive was refused");
        let disk = host.disk;
        assert_eq!(unsafe { discs_in(disk) }, 3);
        assert_eq!(unsafe { disc_in(disk) }, 0);

        let _active = Active::bind(&mut host);
        assert!(unsafe { swap_disc(disk, 2) }, "the swap failed");
        assert_eq!(unsafe { disc_in(disk) }, 2);
        DRIVE.with(|d| {
            let d = d.borrow();
            assert_eq!(d.log, ["open", "disc 2", "close"]);
            assert!(!d.open, "the drive was left open");
        });
        assert!(
            !unsafe { swap_disc(disk, 3) },
            "a fourth disc of three went in"
        );
        assert_eq!(unsafe { disc_in(disk) }, 2);
        assert_eq!(
            unsafe { disc_file(disk, 0) },
            None,
            "the first interface cannot name a disc's file"
        );
    }

    #[test]
    fn the_extended_drive_names_each_disc_s_file() {
        reset_drive();
        let mut host = host_with(HashMap::new(), false);
        let mut cb = DiskControlExtCallback {
            get_image_path: Some(drive_image_path),
            ..DiskControlExtCallback::from(drive_v0())
        };
        let ok = {
            let _active = Active::bind(&mut host);
            unsafe {
                environment(
                    SET_DISK_CONTROL_EXT_INTERFACE,
                    &mut cb as *mut DiskControlExtCallback as *mut c_void,
                )
            }
        };
        assert!(ok);
        assert_eq!(
            unsafe { disc_file(host.disk, 1) }.as_deref(),
            Some("/Games/PS1/Game (Disc 2).chd")
        );
    }

    #[test]
    fn with_no_drive_a_game_is_on_one_disc_and_cannot_swap() {
        let mut host = host_with(HashMap::new(), false);
        assert_eq!(unsafe { discs_in(None) }, 1);
        assert_eq!(unsafe { disc_in(None) }, 0);
        let _active = Active::bind(&mut host);
        assert!(!unsafe { swap_disc(None, 0) });
        drop(_active);
        let mut cb = drive_v0();
        {
            let _active = Active::bind(&mut host);
            unsafe {
                environment(
                    SET_DISK_CONTROL_INTERFACE,
                    &mut cb as *mut DiskControlCallback as *mut c_void,
                );
                environment(SET_DISK_CONTROL_INTERFACE, ptr::null_mut());
            }
        }
        assert!(host.disk.is_none(), "a withdrawn drive stayed");
    }

    #[test]
    fn a_core_that_asks_whether_to_draw_is_told_to_skip_a_skipped_frame() {
        let mut host = host_with(HashMap::new(), false);
        let _active = Active::bind(&mut host);
        let ask = || {
            let mut enable: c_int = -1;
            let ok = unsafe {
                environment(
                    GET_AUDIO_VIDEO_ENABLE,
                    &mut enable as *mut c_int as *mut c_void,
                )
            };
            assert!(ok, "the question went unanswered");
            enable
        };
        assert_eq!(ask(), 3, "a frame that is shown is drawn and heard");
        unsafe { with_host(|h| h.skip_video = true) };
        assert_eq!(ask(), 2, "a skipped frame is heard but not drawn");
    }
}
