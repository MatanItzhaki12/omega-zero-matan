//! gpSP as libretro ships it, run safely from the frontend's side, the core itself untouched.
//!
//! Its recompiler writes the code it makes into a cache it maps in `retro_init`, within the
//! 128 MB an arm64 branch reaches of its own code, trying addresses a megabyte apart. In a busy
//! process none of them may be free: gpSP then goes on with no cache, and the first block it
//! translates is written through a null pointer. So [`keep_room`] loads gpSP early, while the
//! process is nearly empty, keeps it loaded so its code stays where it is, and reserves room
//! beside it at the first of the addresses gpSP tries. The room is let go just before gpSP's
//! `retro_init`, for its cache to take, and kept again once gpSP has let its cache go.
//!
//! Its link play keeps the Advance Wars and Pokémon cable protocols' buffers in `serstate`,
//! which nothing resets when a session starts while the game already drives the link alone.
//! The joiner then reads the master's buffers as its own and overflows them, writing past
//! them over gpSP's callbacks. [`SerialState`] finds `serstate` and the client ids by gpSP's
//! symbol table, and clears them as a session starts and ends, as the game would find them
//! had it started linked. A gpSP whose table does not name them is run as it is.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use libloading::Library;

/// How far apart gpSP tries addresses for its cache, and how many it tries: 128 MB either side.
const STEP: usize = 1 << 20;
const TRIES: usize = 256;
/// The room kept: more than gpSP's 10.5 MB cache, so a later gpSP's somewhat bigger one fits.
const ROOM: usize = 16 << 20;

/// Whether the core at `path` is gpSP.
pub fn is_gpsp(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with("gpsp_libretro"))
}

struct Kept {
    path: PathBuf,
    _lib: Library,
    /// An address in gpSP's code, which it tries addresses about.
    near: usize,
    room: Option<usize>,
}

static KEPT: Mutex<Option<Kept>> = Mutex::new(None);

fn kept() -> std::sync::MutexGuard<'static, Option<Kept>> {
    KEPT.lock().unwrap_or_else(|e| e.into_inner())
}

/// Loads gpSP at `path` for good and keeps room beside it for its cache: best done as early as
/// can be, before the process fills.
pub fn keep_room(path: &Path) {
    let mut kept = kept();
    if kept.as_ref().is_some_and(|k| k.path == path) {
        return;
    }
    // SAFETY: loading a libretro core runs only its constructors; retro_init is not called
    let Ok(lib) = (unsafe { Library::new(path) }) else {
        return;
    };
    // SAFETY: only the symbol's address is read, never called
    let near = unsafe { lib.get::<unsafe extern "C" fn()>(b"retro_init\0") }
        .map(|f| *f as usize)
        .ok();
    let Some(near) = near else {
        return;
    };
    let room = reserve(near);
    if room.is_none() {
        eprintln!("omega-retro: gpSP: no room beside it for its code cache");
    }
    *kept = Some(Kept {
        path: path.to_path_buf(),
        _lib: lib,
        near,
        room,
    });
}

/// Lets the room kept beside gpSP go, for its cache: just before its `retro_init`.
pub(crate) fn make_room(path: &Path) {
    if let Some(k) = kept().as_mut().filter(|k| k.path == path) {
        if let Some(room) = k.room.take() {
            unmap(room);
        }
    }
}

/// Keeps room beside gpSP again, once its cache has gone with its `retro_deinit`.
pub(crate) fn keep_room_again(path: &Path) {
    if let Some(k) = kept().as_mut().filter(|k| k.path == path) {
        if k.room.is_none() {
            k.room = reserve(k.near);
        }
    }
}

/// The addresses gpSP tries for its cache about `near`, in its order: the megabyte its code is
/// in, then a megabyte below and above, two, and so on.
fn candidates(near: usize) -> impl Iterator<Item = usize> {
    let base = near & !(STEP - 1);
    (0..TRIES)
        .map(move |i| {
            let off = (i >> 1) * STEP;
            if i & 1 == 1 {
                base.wrapping_add(off)
            } else {
                base.wrapping_sub(off)
            }
        })
        .filter(|a| *a != 0)
}

/// Lets the room at `room` go.
#[cfg(target_os = "linux")]
fn unmap(room: usize) {
    // SAFETY: the room is a mapping of ROOM bytes this module made and nothing else uses
    unsafe { munmap(room as *mut _, ROOM) };
}

/// Room of ROOM bytes at the first of gpSP's addresses about `near` that is free, mapped with
/// no access so it costs no memory.
#[cfg(target_os = "linux")]
fn reserve(near: usize) -> Option<usize> {
    candidates(near).find(|&at| {
        // SAFETY: a fresh anonymous mapping, at `at` only as a hint, unmapped again where the
        // kernel put it elsewhere
        unsafe {
            let p = mmap(
                at as *mut _,
                ROOM,
                PROT_NONE,
                MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE,
                -1,
                0,
            );
            if p as isize == -1 {
                return false;
            }
            if p as usize == at {
                return true;
            }
            munmap(p, ROOM);
            false
        }
    })
}

// elsewhere than Linux, where gpSP is not run on the handheld, no room is kept
#[cfg(not(target_os = "linux"))]
fn unmap(_room: usize) {}

#[cfg(not(target_os = "linux"))]
fn reserve(_near: usize) -> Option<usize> {
    None
}

#[cfg(target_os = "linux")]
const PROT_NONE: i32 = 0;
#[cfg(target_os = "linux")]
const MAP_PRIVATE: i32 = 0x02;
#[cfg(target_os = "linux")]
const MAP_ANONYMOUS: i32 = 0x20;
#[cfg(target_os = "linux")]
const MAP_NORESERVE: i32 = 0x4000;

#[cfg(target_os = "linux")]
extern "C" {
    fn mmap(
        addr: *mut std::ffi::c_void,
        len: usize,
        prot: i32,
        flags: i32,
        fd: i32,
        off: i64,
    ) -> *mut std::ffi::c_void;
    fn munmap(addr: *mut std::ffi::c_void, len: usize) -> i32;
}

/// gpSP's link state, found in the loaded library by its symbol table.
pub(crate) struct SerialState {
    serstate: usize,
    len: usize,
    client_id: usize,
    clients: usize,
}

impl SerialState {
    /// The link state of the gpSP at `path`, loaded where its `retro_init` is at `init`: none
    /// where its symbol table does not name it as this gpSP's does.
    pub(crate) fn find(path: &Path, init: usize) -> Option<SerialState> {
        let data = std::fs::read(path).ok()?;
        let syms = crate::elf::symbols(
            &data,
            &[
                "retro_init",
                "serstate",
                "netplay_client_id",
                "netplay_num_clients",
            ],
        )?;
        let at = |name: &str| syms.iter().find(|s| s.0 == name).map(|s| (s.1, s.2));
        let (init_at, _) = at("retro_init")?;
        let base = init.checked_sub(init_at as usize)?;
        let (serstate, len) = at("serstate")?;
        let (client_id, id_len) = at("netplay_client_id")?;
        let (clients, clients_len) = at("netplay_num_clients")?;
        (len > 0 && id_len == 4 && clients_len == 4).then(|| SerialState {
            serstate: base + serstate as usize,
            len: len as usize,
            client_id: base + client_id as usize,
            clients: base + clients as usize,
        })
    }

    /// The protocols' buffers cleared, as gpSP's own serialproto_reset clears them.
    ///
    /// # Safety
    /// The gpSP this was found in is loaded, and called on the thread that runs it.
    pub(crate) unsafe fn reset(&self) {
        std::ptr::write_bytes(self.serstate as *mut u8, 0, self.len);
    }

    /// gpSP alone again, as a game with no session is: client 0 of none.
    ///
    /// # Safety
    /// As [`SerialState::reset`].
    pub(crate) unsafe fn alone(&self) {
        std::ptr::write(self.client_id as *mut u32, 0);
        std::ptr::write(self.clients as *mut u32, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_tried_in_gpsps_own_order() {
        let tried: Vec<usize> = candidates(0x4000_0000 + 0x1234).take(6).collect();
        let mb = STEP;
        let base = 0x4000_0000;
        assert_eq!(
            tried,
            vec![
                base,
                base,
                base - mb,
                base + mb,
                base - 2 * mb,
                base + 2 * mb
            ]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn room_is_kept_beside_the_code_and_let_go_for_the_cache() {
        let near = room_is_kept_beside_the_code_and_let_go_for_the_cache as fn() as usize;
        let room = reserve(near).expect("no room beside the test's code");
        assert!(room.abs_diff(near) <= TRIES / 2 * STEP + STEP);
        unmap(room);
        // the cache takes it: a map at the room's address lands there
        let again = reserve(near).expect("the room was not let go");
        assert_eq!(again, room);
        unmap(again);
    }

    #[test]
    fn only_gpsp_is_gpsp() {
        assert!(is_gpsp(Path::new("/x/System/gpsp_libretro.so")));
        assert!(!is_gpsp(Path::new("/x/System/mgba_libretro.so")));
    }
}
