//! A core that draws on the GPU, as the emulator's thread runs it: the context it draws in, lent
//! for the game from the one the frontend made at boot, the framebuffers it draws into, and its
//! frames, handed to the compositor as textures or as pixels.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use omega_gfx::{GpuFrame, GpuFrames, GpuThread};
use omega_retro::{GlHost, RetroCore, MAX_FRAME_H, MAX_FRAME_W};

/// The context for cores that draw on the GPU, made once at boot by the frontend; none on a
/// machine that has none to give.
static GPU: OnceLock<Arc<Mutex<Box<dyn GpuThread>>>> = OnceLock::new();

/// Gives the emulator the context a core that draws on the GPU draws in, once at boot.
pub fn provide(gpu: Box<dyn GpuThread>) {
    let _ = GPU.set(Arc::new(Mutex::new(gpu)));
}

/// What the core sees of the GPU: the framebuffer it draws into, and the GL functions.
struct CoreGl {
    thread: Arc<Mutex<Box<dyn GpuThread>>>,
    framebuffer: Arc<AtomicUsize>,
}

impl GlHost for CoreGl {
    fn framebuffer(&self) -> usize {
        self.framebuffer.load(Ordering::Relaxed)
    }

    fn proc_address(&self, name: &std::ffi::CStr) -> *const std::ffi::c_void {
        let Ok(name) = name.to_str() else {
            return std::ptr::null();
        };
        match self.thread.lock() {
            Ok(t) => t.proc_address(name),
            Err(_) => std::ptr::null(),
        }
    }
}

/// The GPU as one game's core has it on the emulator's thread.
pub struct GpuCore {
    thread: Arc<Mutex<Box<dyn GpuThread>>>,
    framebuffer: Arc<AtomicUsize>,
    /// The framebuffers, once the core has asked for the GPU and its context is current here.
    frames: Option<GpuFrames>,
    /// The size of the frame it drew last.
    last: Option<(u32, u32)>,
}

impl GpuCore {
    /// Offers `core` the GPU, before its game loads; none where there is no GPU to offer.
    pub fn offer(core: &mut dyn RetroCore) -> Option<GpuCore> {
        let thread = GPU.get()?.clone();
        let framebuffer = Arc::new(AtomicUsize::new(0));
        core.offer_gpu(Box::new(CoreGl {
            thread: thread.clone(),
            framebuffer: framebuffer.clone(),
        }));
        Some(GpuCore {
            thread,
            framebuffer,
            frames: None,
            last: None,
        })
    }

    /// Once the game has loaded: if the core asked for the GPU, its context is made current on
    /// this thread, its framebuffers made, and the core told to make its GL objects. False for
    /// a core that draws in memory, or a GPU that cannot be had.
    pub fn start(&mut self, core: &mut dyn RetroCore) -> bool {
        let Some(request) = core.gpu_request() else {
            return false;
        };
        let shares = {
            let Ok(thread) = self.thread.lock() else {
                return false;
            };
            if let Err(e) = thread.make_current() {
                crate::log!("gpu: {e}");
                return false;
            }
            thread.shares()
        };
        match GpuFrames::new((MAX_FRAME_W, MAX_FRAME_H), shares) {
            Ok(frames) => {
                self.framebuffer
                    .store(frames.framebuffer() as usize, Ordering::Relaxed);
                self.frames = Some(frames);
            }
            Err(e) => {
                crate::log!("gpu: the framebuffers: {e}");
                return false;
            }
        }
        crate::log!(
            "gpu: the core draws on OpenGL ES {}.{}{}",
            request.version.0,
            request.version.1,
            if shares {
                ", frames shared"
            } else {
                ", frames read back"
            }
        );
        core.gpu_context_reset();
        true
    }

    /// The frame the core drew in the frame that just ran, handed over; none when it drew none.
    pub fn finish(&mut self, core: &dyn RetroCore) -> Option<GpuFrame> {
        let size = core.gpu_frame()?;
        let frames = self.frames.as_mut()?;
        let frame = frames.finish(size);
        self.framebuffer
            .store(frames.framebuffer() as usize, Ordering::Relaxed);
        self.last = Some(size);
        Some(frame)
    }

    /// The last frame's pixels, XRGB8888, and its size: for a save state's screenshot.
    pub fn last_pixels(&mut self) -> Option<(Vec<u8>, (u32, u32))> {
        let size = self.last?;
        Some((self.frames.as_ref()?.read_last(size), size))
    }
}

impl Drop for GpuCore {
    fn drop(&mut self) {
        // the framebuffers go while the context is still current here, and then the context
        // lets go of this thread, which is ending
        self.frames = None;
        if let Ok(thread) = self.thread.lock() {
            thread.release();
        }
    }
}
