//! What omega-zero's melonDS core adds for the one-screen layouts (cores/melonds's
//! one-screen.patch): found by name in the core's library, and absent from any other.
//!
//! - `omega_ds_watch(bitmap, hits, base, end)`: watch the ARM9's entries into the functions
//!   marked in `bitmap` (a bit per halfword from `base` to `end`), each setting its bit in `hits`.
//! - `omega_ds_layers(screen)`: which 2D layer drew each pixel of a screen in the frame drawn
//!   last, 256 by 192 bytes.
//! - `omega_ds_flush_save()`: the save the game wrote since melonDS last wrote it to its file,
//!   written now (cores/melonds's sram-flush.patch; a core from before it has none).

use std::os::raw::c_uint;

use libloading::Library;

/// The pixels of one DS screen.
pub const SCREEN_PIXELS: usize = 256 * 192;

type WatchFn = unsafe extern "C" fn(*const u8, *mut u8, u32, u32);
type LayersFn = unsafe extern "C" fn(c_uint) -> *const u8;
type FlushSaveFn = unsafe extern "C" fn();

/// The core's one-screen hooks, where its library has them.
#[derive(Clone, Copy)]
pub struct DsHooks {
    watch: WatchFn,
    layers: LayersFn,
    flush_save: Option<FlushSaveFn>,
}

impl DsHooks {
    /// The hooks in `lib`, or none for a core without them.
    pub(crate) fn find(lib: &Library) -> Option<DsHooks> {
        // SAFETY: the symbols are the patch's, with these signatures
        unsafe {
            let watch = *lib.get::<WatchFn>(b"omega_ds_watch\0").ok()?;
            let layers = *lib.get::<LayersFn>(b"omega_ds_layers\0").ok()?;
            let flush_save = lib
                .get::<FlushSaveFn>(b"omega_ds_flush_save\0")
                .ok()
                .map(|f| *f);
            Some(DsHooks {
                watch,
                layers,
                flush_save,
            })
        }
    }

    /// Watches the functions marked in `bitmap`, from `base` to `end`, each entry setting its bit
    /// in `hits`; none watches nothing.
    ///
    /// # Safety
    /// The core must be loaded and called on the thread that runs it, and `bitmap` and `hits`,
    /// each long enough for `base..end`, must stay where they are until the watch is replaced.
    pub unsafe fn watch(&self, watch: Option<(&[u8], &mut [u8], u32, u32)>) {
        match watch {
            Some((bitmap, hits, base, end)) => {
                let bits = (end.saturating_sub(base) / 2) as usize;
                assert!(bitmap.len() * 8 >= bits && hits.len() * 8 >= bits);
                (self.watch)(bitmap.as_ptr(), hits.as_mut_ptr(), base, end);
            }
            None => (self.watch)(std::ptr::null(), std::ptr::null_mut(), 0, 0),
        }
    }

    /// Writes the save the game wrote since the core last wrote it to its file, where the core
    /// can.
    ///
    /// # Safety
    /// The core must be loaded and called on the thread that runs it.
    pub unsafe fn flush_save(&self) {
        if let Some(flush) = self.flush_save {
            flush();
        }
    }

    /// Which layer drew each pixel of `screen` (0 the top one, 1 the touch screen) in the frame
    /// drawn last.
    ///
    /// # Safety
    /// The core must be loaded and called on the thread that runs it; the slice is the core's,
    /// good until it next runs a frame.
    pub unsafe fn layers(&self, screen: usize) -> &[u8] {
        let p = (self.layers)(screen as c_uint);
        if p.is_null() {
            return &[];
        }
        std::slice::from_raw_parts(p, SCREEN_PIXELS)
    }
}
