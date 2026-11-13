//! gpSP as libretro ships it, run safely from the frontend's side, the core itself untouched.
//!
//! Its recompiler writes the code it makes into a cache it maps in `retro_init`, within the
//! 128 MB an arm64 branch reaches of its own code, trying addresses a megabyte apart. Each try
//! is only a hint: where the address is taken, as its first, its own code's, always is, the
//! kernel maps the cache wherever there is room, and gpSP keeps it if its start is within
//! 128 MB of the one function it measures from. In a busy process nowhere may be: gpSP then
//! goes on with no cache and writes its first block through a null pointer. And the kernel's
//! own pick may lie at the very edge, where the cache's start is 127.9 MB from that function
//! but further from the end of gpSP's code, which a branch from the cache then cannot reach.
//!
//! So [`keep_room`] loads gpSP early, while the process is nearly empty, keeps it loaded so its
//! code stays where it is, and reserves room beside it, within 96 MB of all of its code. Just
//! before gpSP's `retro_init` every other free gap near its code is filled for the moment, and
//! the room let go: the kernel's pick then lies far off, which gpSP refuses, or in the room,
//! and gpSP's own tries come to the room. Once `retro_init` is done the gaps are let go, and
//! once gpSP has let its cache go the room is kept again.
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
#[cfg(test)]
const TRIES: usize = 256;
/// The room is kept within 96 MB of gpSP's code, so the whole cache reaches all of the code.
const ROOM_TRIES: usize = 192;
/// How far either side of gpSP's code free gaps are filled as its cache is made: further than
/// gpSP ever takes a cache.
const NEAR: usize = 140 << 20;
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
    /// The gaps filled while gpSP makes its cache.
    filled: Vec<(usize, usize)>,
}

static KEPT: Mutex<Option<Kept>> = Mutex::new(None);

fn kept() -> std::sync::MutexGuard<'static, Option<Kept>> {
    KEPT.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
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
        filled: Vec::new(),
    });
}

/// Fills the free gaps near gpSP's code but its room, and lets the room go, for its cache to be
/// made in: just before its `retro_init`.
pub(crate) fn make_room(path: &Path) {
    if let Some(k) = kept().as_mut().filter(|k| k.path == path) {
        if let Some(room) = k.room.take() {
            k.filled = fill_gaps(k.near);
            unmap(room, ROOM);
        }
    }
}

/// Lets the gaps filled for gpSP's cache go, once its `retro_init` has made it.
pub(crate) fn made_room(path: &Path) {
    if let Some(k) = kept().as_mut().filter(|k| k.path == path) {
        for (at, len) in k.filled.drain(..) {
            unmap(at, len);
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
fn candidates(near: usize, tries: usize) -> impl Iterator<Item = usize> {
    let base = near & !(STEP - 1);
    (0..tries)
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

/// Lets a mapping this module made, of `len` bytes at `at`, go.
#[cfg(target_os = "linux")]
fn unmap(at: usize, len: usize) {
    // SAFETY: the mapping is one this module made, which nothing else uses
    unsafe { munmap(at as *mut _, len) };
}

/// The free gaps within NEAR of `near`, as /proc/self/maps has the process's mappings, each
/// filled with a mapping of no access: those the kernel mapped where asked.
#[cfg(target_os = "linux")]
fn fill_gaps(near: usize) -> Vec<(usize, usize)> {
    let Ok(maps) = std::fs::read_to_string("/proc/self/maps") else {
        return Vec::new();
    };
    let (lo, hi) = (near.saturating_sub(NEAR), near.saturating_add(NEAR));
    gaps(&maps, lo, hi)
        .into_iter()
        .filter(|&(at, len)| map_at(at, len))
        .collect()
}

/// The gaps between the mappings `maps` lists, clipped to `lo` to `hi`.
fn gaps(maps: &str, lo: usize, hi: usize) -> Vec<(usize, usize)> {
    let mut ranges: Vec<(usize, usize)> = maps
        .lines()
        .filter_map(|l| {
            let (a, b) = l.split_whitespace().next()?.split_once('-')?;
            Some((
                usize::from_str_radix(a, 16).ok()?,
                usize::from_str_radix(b, 16).ok()?,
            ))
        })
        .collect();
    ranges.sort_unstable();
    let mut out = Vec::new();
    let mut from = lo;
    for (start, end) in ranges {
        if start > from && from < hi {
            let to = start.min(hi);
            out.push((from, to - from));
        }
        from = from.max(end);
    }
    if from < hi {
        out.push((from, hi - from));
    }
    out
}

/// A mapping of no access, `len` bytes at `at` and only there.
#[cfg(target_os = "linux")]
fn map_at(at: usize, len: usize) -> bool {
    // SAFETY: a fresh anonymous mapping, at `at` only as a hint, unmapped again where the
    // kernel put it elsewhere
    unsafe {
        let p = mmap(
            at as *mut _,
            len,
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
        munmap(p, len);
        false
    }
}

/// Room of ROOM bytes at the first of gpSP's addresses about `near`, within 96 MB, that is
/// free, mapped with no access so it costs no memory.
#[cfg(target_os = "linux")]
fn reserve(near: usize) -> Option<usize> {
    candidates(near, ROOM_TRIES).find(|&at| map_at(at, ROOM))
}

// elsewhere than Linux, where gpSP is not run on the handheld, no room is kept
#[cfg(not(target_os = "linux"))]
fn unmap(_at: usize, _len: usize) {}

#[cfg(not(target_os = "linux"))]
fn fill_gaps(_near: usize) -> Vec<(usize, usize)> {
    Vec::new()
}

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

    /// The tests that map beside the test's own code, one at a time, as each fills or takes
    /// the same gaps.
    static MAPPING: Mutex<()> = Mutex::new(());

    #[test]
    fn addresses_are_tried_in_gpsps_own_order() {
        let tried: Vec<usize> = candidates(0x4000_0000 + 0x1234, TRIES).take(6).collect();
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
        let _turn = MAPPING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let near = room_is_kept_beside_the_code_and_let_go_for_the_cache as fn() as usize;
        let room = reserve(near).expect("no room beside the test's code");
        assert!(room.abs_diff(near) <= ROOM_TRIES / 2 * STEP + STEP);
        unmap(room, ROOM);
        // the cache takes it: a map at the room's address lands there
        let again = reserve(near).expect("the room was not let go");
        assert_eq!(again, room);
        unmap(again, ROOM);
    }

    #[test]
    fn the_gaps_are_between_the_mappings_and_within_the_bounds() {
        let maps = "1000-2000 r-xp 0 0:0 0 /lib\n3000-4000 rw-p 0 0:0 0\n6000-7000 ---p 0 0:0 0\n";
        assert_eq!(
            gaps(maps, 0x1800, 0x6800),
            vec![(0x2000, 0x1000), (0x4000, 0x2000)]
        );
        assert_eq!(gaps(maps, 0x0, 0x1000), vec![(0x0, 0x1000)]);
        assert_eq!(gaps(maps, 0x7000, 0x8000), vec![(0x7000, 0x1000)]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn with_the_gaps_filled_a_hint_near_the_code_lands_in_the_room_or_far_off() {
        let _turn = MAPPING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let near =
            with_the_gaps_filled_a_hint_near_the_code_lands_in_the_room_or_far_off as fn() as usize;
        let room = reserve(near).expect("no room beside the test's code");
        let filled = fill_gaps(near);
        unmap(room, ROOM);
        // what gpSP does first: a hint at its own code, which is taken, so the kernel picks
        // SAFETY: a fresh anonymous mapping, let go at once
        let picked = unsafe {
            let p = mmap(
                near as *mut _,
                10 << 20,
                PROT_NONE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            );
            munmap(p, 10 << 20);
            p as usize
        };
        for (at, len) in filled {
            unmap(at, len);
        }
        assert!(
            picked == room || picked.abs_diff(near) > 128 << 20,
            "the kernel picked {picked:#x}, {} MB from the code, outside the room at {room:#x}",
            picked.abs_diff(near) >> 20
        );
    }

    #[test]
    fn only_gpsp_is_gpsp() {
        assert!(is_gpsp(Path::new("/x/System/gpsp_libretro.so")));
        assert!(!is_gpsp(Path::new("/x/System/mgba_libretro.so")));
    }
}
