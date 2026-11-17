use std::ffi::{c_char, c_int, c_uint, c_void};

use libloading::Library;

use crate::core::CoreError;

pub const API_VERSION: c_uint = 1;

pub const GET_CAN_DUPE: c_uint = 3;
pub const GET_SYSTEM_DIRECTORY: c_uint = 9;
pub const SET_PIXEL_FORMAT: c_uint = 10;
pub const SET_DISK_CONTROL_INTERFACE: c_uint = 13;
pub const GET_VARIABLE: c_uint = 15;
pub const SET_VARIABLES: c_uint = 16;
pub const GET_VARIABLE_UPDATE: c_uint = 17;
pub const GET_RUMBLE_INTERFACE: c_uint = 23;
pub const GET_LOG_INTERFACE: c_uint = 27;
pub const GET_SAVE_DIRECTORY: c_uint = 31;
pub const SET_SYSTEM_AV_INFO: c_uint = 32;
pub const SET_GEOMETRY: c_uint = 37;
pub const GET_DISK_CONTROL_INTERFACE_VERSION: c_uint = 57;
pub const SET_DISK_CONTROL_EXT_INTERFACE: c_uint = 58;
pub const EXPERIMENTAL: c_uint = 0x10000;
pub const GET_AUDIO_VIDEO_ENABLE: c_uint = 47 | EXPERIMENTAL;
pub const SET_AUDIO_BUFFER_STATUS_CALLBACK: c_uint = 62;
pub const SET_NETPACKET_INTERFACE: c_uint = 78;
pub const SET_MEMORY_MAPS: c_uint = 36 | EXPERIMENTAL;
/// A core that draws on the GPU asks for a context and a framebuffer to draw into.
pub const SET_HW_RENDER: c_uint = 14;
/// A core asks which kind of context the frontend would rather give it.
pub const GET_PREFERRED_HW_RENDER: c_uint = 56;
/// A core says it shares its context with the frontend's, as PPSSPP does.
pub const SET_HW_SHARED_CONTEXT: c_uint = 44 | EXPERIMENTAL;
// libretro.h's numbering: 1 is desktop OpenGL, 2 OpenGL ES 2, 3 desktop OpenGL core, 4 OpenGL ES 3,
// 5 the ES version asked for, 6 Vulkan
pub const HW_CONTEXT_OPENGLES2: c_uint = 2;
pub const HW_CONTEXT_OPENGLES3: c_uint = 4;
/// OpenGL ES of the version the callback's `version_major` and `version_minor` name.
pub const HW_CONTEXT_OPENGLES_VERSION: c_uint = 5;
/// The picture a GPU core hands `video_refresh` in place of pixels: it drew the frame into the
/// framebuffer the frontend gave it.
pub const HW_FRAME_BUFFER_VALID: *const c_void = usize::MAX as *const c_void;
pub const DEVICE_ANALOG: c_uint = 5;
pub const DEVICE_INDEX_ANALOG_LEFT: c_uint = 0;

pub const RUMBLE_STRONG: c_uint = 0;
pub const RUMBLE_WEAK: c_uint = 1;

pub const PIXEL_FORMAT_0RGB1555: c_uint = 0;
pub const PIXEL_FORMAT_XRGB8888: c_uint = 1;
pub const PIXEL_FORMAT_RGB565: c_uint = 2;
pub const DEVICE_JOYPAD: c_uint = 1;
/// A touch screen: where it is touched, -32768 to 32767 across the picture each way, and
/// whether it is.
pub const DEVICE_POINTER: c_uint = 6;
pub const POINTER_X: c_uint = 0;
pub const POINTER_Y: c_uint = 1;
pub const POINTER_PRESSED: c_uint = 2;
pub const MEMORY_SAVE_RAM: c_uint = 0;
pub const MEMORY_SYSTEM_RAM: c_uint = 2;
pub const MEMORY_VIDEO_RAM: c_uint = 3;
pub const JOYPAD_MASK: c_uint = 256;

pub const NETPACKET_UNRELIABLE: i32 = 0;
pub const NETPACKET_RELIABLE: i32 = 1 << 0;
pub const NETPACKET_UNSEQUENCED: i32 = 1 << 1;
pub const NETPACKET_FLUSH_HINT: i32 = 1 << 2;
pub const NETPACKET_BROADCAST: u16 = 0xFFFF;

#[repr(C)]
pub struct GameInfo {
    pub path: *const c_char,
    pub data: *const c_void,
    pub size: usize,
    pub meta: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct GameGeometry {
    pub base_width: c_uint,
    pub base_height: c_uint,
    pub max_width: c_uint,
    pub max_height: c_uint,
    pub aspect_ratio: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct SystemTiming {
    pub fps: f64,
    pub sample_rate: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct SystemAvInfo {
    pub geometry: GameGeometry,
    pub timing: SystemTiming,
}

/// What a core says of itself before it loads anything.
#[repr(C)]
pub struct SystemInfo {
    pub library_name: *const c_char,
    pub library_version: *const c_char,
    pub valid_extensions: *const c_char,
    /// The core reads the game from its path itself and is handed no copy of it in memory.
    pub need_fullpath: bool,
    pub block_extract: bool,
}

impl Default for SystemInfo {
    fn default() -> Self {
        SystemInfo {
            library_name: std::ptr::null(),
            library_version: std::ptr::null(),
            valid_extensions: std::ptr::null(),
            need_fullpath: false,
            block_extract: false,
        }
    }
}

pub type SetEjectStateFn = unsafe extern "C" fn(ejected: bool) -> bool;
pub type GetEjectStateFn = unsafe extern "C" fn() -> bool;
pub type GetImageIndexFn = unsafe extern "C" fn() -> c_uint;
pub type SetImageIndexFn = unsafe extern "C" fn(index: c_uint) -> bool;
pub type GetNumImagesFn = unsafe extern "C" fn() -> c_uint;
pub type ReplaceImageIndexFn = unsafe extern "C" fn(index: c_uint, info: *const GameInfo) -> bool;
pub type AddImageIndexFn = unsafe extern "C" fn() -> bool;
pub type SetInitialImageFn = unsafe extern "C" fn(index: c_uint, path: *const c_char) -> bool;
pub type GetImagePathFn =
    unsafe extern "C" fn(index: c_uint, path: *mut c_char, len: usize) -> bool;
pub type GetImageLabelFn =
    unsafe extern "C" fn(index: c_uint, label: *mut c_char, len: usize) -> bool;

/// libretro's disk control, the drive of a game on several discs, as a core hands it over
/// through `SET_DISK_CONTROL_INTERFACE`.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct DiskControlCallback {
    pub set_eject_state: Option<SetEjectStateFn>,
    pub get_eject_state: Option<GetEjectStateFn>,
    pub get_image_index: Option<GetImageIndexFn>,
    pub set_image_index: Option<SetImageIndexFn>,
    pub get_num_images: Option<GetNumImagesFn>,
    pub replace_image_index: Option<ReplaceImageIndexFn>,
    pub add_image_index: Option<AddImageIndexFn>,
}

/// The same drive through `SET_DISK_CONTROL_EXT_INTERFACE`, which can also say which disc to
/// start from and where each disc is read from.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct DiskControlExtCallback {
    pub set_eject_state: Option<SetEjectStateFn>,
    pub get_eject_state: Option<GetEjectStateFn>,
    pub get_image_index: Option<GetImageIndexFn>,
    pub set_image_index: Option<SetImageIndexFn>,
    pub get_num_images: Option<GetNumImagesFn>,
    pub replace_image_index: Option<ReplaceImageIndexFn>,
    pub add_image_index: Option<AddImageIndexFn>,
    pub set_initial_image: Option<SetInitialImageFn>,
    pub get_image_path: Option<GetImagePathFn>,
    pub get_image_label: Option<GetImageLabelFn>,
}

impl From<DiskControlCallback> for DiskControlExtCallback {
    fn from(d: DiskControlCallback) -> Self {
        DiskControlExtCallback {
            set_eject_state: d.set_eject_state,
            get_eject_state: d.get_eject_state,
            get_image_index: d.get_image_index,
            set_image_index: d.set_image_index,
            get_num_images: d.get_num_images,
            replace_image_index: d.replace_image_index,
            add_image_index: d.add_image_index,
            set_initial_image: None,
            get_image_path: None,
            get_image_label: None,
        }
    }
}

#[repr(C)]
pub struct Variable {
    pub key: *const c_char,
    pub value: *const c_char,
}

#[repr(C)]
pub struct LogCallback {
    pub log: *const c_void,
}

pub type SetRumbleStateFn = unsafe extern "C" fn(c_uint, c_uint, u16) -> bool;

#[repr(C)]
pub struct RumbleInterface {
    pub set_rumble_state: SetRumbleStateFn,
}

pub type AudioBufferStatusFn =
    unsafe extern "C" fn(active: bool, occupancy: c_uint, underrun_likely: bool);

#[repr(C)]
pub struct AudioBufferStatusCallback {
    pub callback: Option<AudioBufferStatusFn>,
}

pub type NetpacketSend =
    unsafe extern "C" fn(flags: c_int, buf: *const c_void, len: usize, client_id: u16);
pub type NetpacketPollReceive = unsafe extern "C" fn();

pub type NetpacketStart =
    unsafe extern "C" fn(client_id: u16, send: NetpacketSend, poll_receive: NetpacketPollReceive);
pub type NetpacketReceive = unsafe extern "C" fn(buf: *const c_void, len: usize, client_id: u16);
pub type NetpacketStop = unsafe extern "C" fn();
pub type NetpacketPoll = unsafe extern "C" fn();
pub type NetpacketConnected = unsafe extern "C" fn(client_id: u16) -> bool;
pub type NetpacketDisconnected = unsafe extern "C" fn(client_id: u16);

#[repr(C)]
pub struct NetpacketCallback {
    pub start: Option<NetpacketStart>,
    pub receive: Option<NetpacketReceive>,
    pub stop: Option<NetpacketStop>,
    pub poll: Option<NetpacketPoll>,
    pub connected: Option<NetpacketConnected>,
    pub disconnected: Option<NetpacketDisconnected>,
    pub protocol_version: *const c_char,
}

// SAFETY: besides function pointers, `protocol_version` points at a string literal in the
unsafe impl Send for NetpacketCallback {}

pub type EnvironmentFn = unsafe extern "C" fn(c_uint, *mut c_void) -> bool;
pub type VideoRefreshFn = unsafe extern "C" fn(*const c_void, c_uint, c_uint, usize);
pub type AudioSampleFn = unsafe extern "C" fn(i16, i16);
pub type AudioBatchFn = unsafe extern "C" fn(*const i16, usize) -> usize;
pub type InputPollFn = unsafe extern "C" fn();
pub type InputStateFn = unsafe extern "C" fn(c_uint, c_uint, c_uint, c_uint) -> i16;

pub struct Api {
    pub init: unsafe extern "C" fn(),
    pub deinit: unsafe extern "C" fn(),
    pub api_version: unsafe extern "C" fn() -> c_uint,
    pub get_system_info: unsafe extern "C" fn(*mut SystemInfo),
    pub get_system_av_info: unsafe extern "C" fn(*mut SystemAvInfo),
    pub set_environment: unsafe extern "C" fn(EnvironmentFn),
    pub set_video_refresh: unsafe extern "C" fn(VideoRefreshFn),
    pub set_audio_sample: unsafe extern "C" fn(AudioSampleFn),
    pub set_audio_sample_batch: unsafe extern "C" fn(AudioBatchFn),
    pub set_input_poll: unsafe extern "C" fn(InputPollFn),
    pub set_input_state: unsafe extern "C" fn(InputStateFn),
    pub set_controller_port_device: unsafe extern "C" fn(c_uint, c_uint),
    pub load_game: unsafe extern "C" fn(*const GameInfo) -> bool,
    pub unload_game: unsafe extern "C" fn(),
    pub run: unsafe extern "C" fn(),
    pub reset: unsafe extern "C" fn(),
    pub serialize_size: unsafe extern "C" fn() -> usize,
    pub serialize: unsafe extern "C" fn(*mut c_void, usize) -> bool,
    pub unserialize: unsafe extern "C" fn(*const c_void, usize) -> bool,
    pub get_memory_data: unsafe extern "C" fn(c_uint) -> *mut c_void,
    pub get_memory_size: unsafe extern "C" fn(c_uint) -> usize,
}

impl Api {
    pub unsafe fn load(lib: &Library) -> Result<Self, CoreError> {
        macro_rules! get {
            ($name:literal) => {
                *lib.get(concat!($name, "\0").as_bytes())
                    .map_err(|e| CoreError::Load(format!("{}: {e}", $name)))?
            };
        }
        Ok(Api {
            init: get!("retro_init"),
            deinit: get!("retro_deinit"),
            api_version: get!("retro_api_version"),
            get_system_info: get!("retro_get_system_info"),
            get_system_av_info: get!("retro_get_system_av_info"),
            set_environment: get!("retro_set_environment"),
            set_video_refresh: get!("retro_set_video_refresh"),
            set_audio_sample: get!("retro_set_audio_sample"),
            set_audio_sample_batch: get!("retro_set_audio_sample_batch"),
            set_input_poll: get!("retro_set_input_poll"),
            set_input_state: get!("retro_set_input_state"),
            set_controller_port_device: get!("retro_set_controller_port_device"),
            load_game: get!("retro_load_game"),
            unload_game: get!("retro_unload_game"),
            run: get!("retro_run"),
            reset: get!("retro_reset"),
            serialize_size: get!("retro_serialize_size"),
            serialize: get!("retro_serialize"),
            unserialize: get!("retro_unserialize"),
            get_memory_data: get!("retro_get_memory_data"),
            get_memory_size: get!("retro_get_memory_size"),
        })
    }
}

/// One region of a core's memory as `retro_memory_descriptor` lays it out: where it is in the
/// system's address space and where its bytes are.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MemoryDescriptor {
    pub flags: u64,
    pub ptr: *mut c_void,
    pub offset: usize,
    pub start: usize,
    pub select: usize,
    pub disconnect: usize,
    pub len: usize,
    pub addrspace: *const c_char,
}

// SAFETY: a descriptor points into the core's own memory, which only the thread running the
// core reads, and its address space names are the core's static strings
unsafe impl Send for MemoryDescriptor {}

/// `retro_memory_map`: the descriptors a core hands over with SET_MEMORY_MAPS.
#[repr(C)]
pub struct MemoryMap {
    pub descriptors: *const MemoryDescriptor,
    pub num_descriptors: c_uint,
}

pub type HwContextReset = unsafe extern "C" fn();
pub type HwGetFramebuffer = unsafe extern "C" fn() -> usize;
pub type HwProc = unsafe extern "C" fn();
pub type HwGetProcAddress = unsafe extern "C" fn(sym: *const c_char) -> Option<HwProc>;

/// libretro's `retro_hw_render_callback`: what a core that draws on the GPU asks for, and the
/// frontend's functions it draws through, which the frontend fills in.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct HwRenderCallback {
    pub context_type: c_uint,
    pub context_reset: Option<HwContextReset>,
    pub get_current_framebuffer: Option<HwGetFramebuffer>,
    pub get_proc_address: Option<HwGetProcAddress>,
    pub depth: bool,
    pub stencil: bool,
    pub bottom_left_origin: bool,
    pub version_major: c_uint,
    pub version_minor: c_uint,
    pub cache_context: bool,
    pub context_destroy: Option<HwContextReset>,
    pub debug_context: bool,
}
