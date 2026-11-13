//! The SNES and PlayStation cores, run for real when their libraries are in vendor/ (task
//! core:snes9x:host, core:supafaust:host and core:pcsx:host put them there), on games written
//! here byte by byte: a SNES cart that paints its screen one colour in the mode it is asked for,
//! and a PlayStation disc whose program does the same at one resolution and then another. They
//! are ours, so nothing commercial is needed and nothing is committed but this file.

use std::path::{Path, PathBuf};

use omega_retro::{ButtonMask, LibretroCore, RetroCore};

use crate::CORE_LOCK;

fn lock() -> std::sync::MutexGuard<'static, ()> {
    CORE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn vendored(library: &str) -> Option<PathBuf> {
    let path = omega_retro::host_core(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor"),
        library,
    );
    if path.exists() {
        Some(path)
    } else {
        eprintln!("no {library} core in vendor/, skipping");
        None
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("console-cores");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir.join(name)
}

/// A 32 KiB LoROM SNES cart: it sets the backdrop to `colour` (BGR555), puts the PPU in BG
/// `mode` with SETINI `setini` (bit 3 pseudo hi-res, bit 0 interlace), turns the screen on and
/// waits. `pal` makes it a European cart, which a core runs at 50 Hz.
fn snes_cart(colour: u16, mode: u8, setini: u8, pal: bool) -> Vec<u8> {
    let [lo, hi] = colour.to_le_bytes();
    #[rustfmt::skip]
    let code = [
        0x78, 0x18, 0xfb,             // sei; clc; xce
        0xe2, 0x20,                   // sep #$20
        0xa9, 0x8f, 0x8d, 0x00, 0x21, // lda #$8f; sta INIDISP: blank
        0x9c, 0x21, 0x21,             // stz CGADD
        0xa9, lo, 0x8d, 0x22, 0x21,   // lda #lo; sta CGDATA
        0xa9, hi, 0x8d, 0x22, 0x21,   // lda #hi; sta CGDATA
        0xa9, mode, 0x8d, 0x05, 0x21, // lda #mode; sta BGMODE
        0xa9, setini, 0x8d, 0x33, 0x21, // lda #setini; sta SETINI
        0xa9, 0x0f, 0x8d, 0x00, 0x21, // lda #$0f; sta INIDISP: on, full brightness
        0x80, 0xfe,                   // bra *
    ];
    let mut rom = vec![0u8; 0x8000];
    rom[..code.len()].copy_from_slice(&code);
    rom[0x7fc0..0x7fd5].copy_from_slice(b"OMEGA ZERO TEST      ");
    rom[0x7fd5] = 0x20; // LoROM
    rom[0x7fd6] = 0x00; // ROM only
    rom[0x7fd7] = 0x05; // 32 KiB
    rom[0x7fd8] = 0x00; // no RAM
    rom[0x7fd9] = if pal { 0x02 } else { 0x01 };
    rom[0x7fda] = 0x33;
    // every vector to the start; the emulation-mode reset is the one that runs
    for v in (0x7fe4..0x7ffe).step_by(2) {
        rom[v..v + 2].copy_from_slice(&0x8000u16.to_le_bytes());
    }
    rom[0x7fdc..0x7fe0].copy_from_slice(&[0xff, 0xff, 0x00, 0x00]);
    let sum = rom.iter().fold(0u16, |a, &b| a.wrapping_add(u16::from(b)));
    rom[0x7fdc..0x7fde].copy_from_slice(&(!sum).to_le_bytes());
    rom[0x7fde..0x7fe0].copy_from_slice(&sum.to_le_bytes());
    rom
}

fn run(core: &mut LibretroCore, frames: usize) {
    for _ in 0..frames {
        core.run_frame(ButtonMask::default());
        let _ = core.take_audio();
    }
}

/// The colour in the middle of the picture, as red, green, blue.
fn middle(core: &LibretroCore) -> [u8; 3] {
    let (w, h) = core.video_size();
    let o = ((h / 2 * w + w / 2) * 4) as usize;
    let px = &core.video_xrgb8888()[o..o + 4];
    [px[2], px[1], px[0]]
}

const RED: u16 = 0x001f;
const BLUE: u16 = 0x7c00;

fn snes_core(library: &str, cart: Vec<u8>, name: &str) -> Option<LibretroCore> {
    let dylib = vendored(library)?;
    let path = scratch(name);
    std::fs::write(&path, cart).expect("write the cart");
    let mut core = LibretroCore::open(&dylib).expect("the core would not open");
    assert!(!core.needs_fullpath(), "{library} reads a cart from memory");
    core.load(&path).expect("the core refused our cart");
    run(&mut core, 30);
    Some(core)
}

#[test]
fn snes9x_draws_a_256x224_picture_in_our_colour() {
    let _g = lock();
    let Some(core) = snes_core("snes9x", snes_cart(RED, 1, 0, false), "red.sfc") else {
        return;
    };
    assert_eq!(core.video_size(), (256, 224));
    let [r, g, b] = middle(&core);
    assert!(r > 200 && g < 40 && b < 40, "the backdrop is {r},{g},{b}");
    let fps = core.av_info().fps;
    assert!((fps - 60.0).abs() < 0.2, "an NTSC cart runs at {fps}");
}

#[test]
fn snes9x_doubles_its_width_in_hi_res() {
    let _g = lock();
    let Some(core) = snes_core("snes9x", snes_cart(BLUE, 5, 0, false), "hires.sfc") else {
        return;
    };
    let (w, h) = core.video_size();
    assert_eq!(w, 512, "a mode 5 picture is {w}x{h}");
    let [r, _, b] = middle(&core);
    assert!(b > 200 && r < 40, "the backdrop lost its colour");
}

#[test]
fn snes9x_runs_a_european_cart_at_50() {
    let _g = lock();
    let Some(core) = snes_core("snes9x", snes_cart(RED, 1, 0, true), "pal.sfc") else {
        return;
    };
    let fps = core.av_info().fps;
    assert!((fps - 50.0).abs() < 0.2, "a PAL cart runs at {fps}");
}

#[test]
fn a_snes9x_state_fits_the_rewind_buffer_many_times_over() {
    let _g = lock();
    let Some(mut core) = snes_core("snes9x", snes_cart(RED, 1, 0, false), "state.sfc") else {
        return;
    };
    let state = core.serialize().expect("no state");
    eprintln!("snes9x state: {} bytes", state.len());
    assert!(
        state.len() < 1024 * 1024,
        "a {} byte state leaves the 20 MB rewind buffer too few",
        state.len()
    );
}

#[test]
fn supafaust_draws_a_256x224_picture_in_our_colour() {
    let _g = lock();
    let Some(mut core) = snes_core(
        "mednafen_supafaust",
        snes_cart(RED, 1, 0, false),
        "faust.sfc",
    ) else {
        return;
    };
    let (w, h) = core.video_size();
    assert_eq!(h, 224, "Supafaust drew {w}x{h}");
    assert!(w == 256 || w == 512, "Supafaust drew {w}x{h}");
    let [r, g, b] = middle(&core);
    assert!(r > 200 && g < 40 && b < 40, "the backdrop is {r},{g},{b}");
    let state = core.serialize().expect("no state");
    eprintln!("supafaust state: {} bytes", state.len());
}

/// Supafaust forgets the defaults of its options once a game has loaded and reads every option
/// again on a frame after the frontend says one changed: one omega-zero sets, as it sets where
/// Supafaust's threads run, had it read an empty value it could not take and abort the console.
#[test]
fn supafaust_runs_with_an_option_set_as_omega_zero_sets_them() {
    let _g = lock();
    let Some(dylib) = vendored("mednafen_supafaust") else {
        return;
    };
    let path = scratch("faust-options.sfc");
    std::fs::write(&path, snes_cart(BLUE, 1, 0, false)).expect("write the cart");
    let mut core = LibretroCore::open(&dylib).expect("the core would not open");
    core.set_option("supafaust_thread_affinity_emu", "0x0");
    core.set_option("supafaust_thread_affinity_ppu", "0x0");
    core.load(&path).expect("the core refused our cart");
    run(&mut core, 30);
    core.set_option("supafaust_h_filter", "512");
    run(&mut core, 30);
    let [r, _, b] = middle(&core);
    assert!(b > 200 && r < 40, "the backdrop lost its colour");
}

/// MIPS: `reg = value`, as lui and ori.
fn li(reg: u32, value: u32) -> [u32; 2] {
    [
        0x3c00_0000 | reg << 16 | value >> 16,
        0x3400_0000 | reg << 21 | reg << 16 | (value & 0xffff),
    ]
}

const T0: u32 = 8;
const T1: u32 = 9;
const T2: u32 = 10;
/// sw t1, 0x10(t0): to GP0, the GPU's drawing port, t0 holding 0x1f80_0000.
const TO_GP0: u32 = 0xad09_1810;
/// sw t1, 0x14(t0): to GP1, its display port.
const TO_GP1: u32 = 0xad09_1814;

/// A PlayStation program: it shows 320x240 of the GPU's memory filled with `rgb`, waits about
/// a dozen frames and switches the display to 640x480 interlaced, then waits forever.
fn ps1_program(rgb: [u8; 3]) -> Vec<u32> {
    let mut code = Vec::new();
    code.extend(li(T0, 0x1f80_0000));
    let to = |port: u32, value: u32, code: &mut Vec<u32>| {
        code.extend(li(T1, value));
        code.push(port);
    };
    for v in [
        0x0000_0000, // reset
        0x0500_0000, // display from 0,0
        0x06c6_0260, // the usual horizontal range
        0x0704_2018, // and vertical
        0x0800_0001, // 320x240, NTSC
        0x0300_0000, // display on
    ] {
        to(TO_GP1, v, &mut code);
    }
    let [r, g, b] = rgb.map(u32::from);
    for v in [
        0xe100_0400,                        // draw where the display is
        0xe300_0000,                        // drawing area from 0,0
        0xe400_0000 | (479 << 10) | 639,    // to 639,479
        0xe500_0000,                        // no offset
        0x0200_0000 | b << 16 | g << 8 | r, // fill, in this colour,
        0x0000_0000,                        // from 0,0
        480 << 16 | 640,                    // 640x480 of it
    ] {
        to(TO_GP0, v, &mut code);
    }
    code.extend(li(T2, 0x0040_0000));
    code.push(0x254a_ffff); // addiu t2, t2, -1
    code.push(0x1540_fffe); // bne t2, zero, back
    code.push(0);
    to(TO_GP1, 0x0800_0027, &mut code); // 640x480, interlaced
    code.push(0x1000_ffff); // b .
    code.push(0);
    code
}

/// The program as a PS-X EXE, loaded and started at 0x80010000.
fn ps1_exe(code: &[u32]) -> Vec<u8> {
    let text = code.len() * 4;
    let t_size = text.div_ceil(2048) * 2048;
    let mut exe = vec![0u8; 2048 + t_size];
    exe[..8].copy_from_slice(b"PS-X EXE");
    let mut put = |at: usize, v: u32| exe[at..at + 4].copy_from_slice(&v.to_le_bytes());
    put(0x10, 0x8001_0000); // pc0
    put(0x18, 0x8001_0000); // t_addr
    put(0x1c, t_size as u32);
    put(0x30, 0x801f_ff00); // stack
    let region = b"Sony Computer Entertainment Inc. for North America area";
    exe[0x4c..0x4c + region.len()].copy_from_slice(region);
    for (i, word) in code.iter().enumerate() {
        exe[2048 + i * 4..2048 + i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    exe
}

fn both_ways32(v: u32) -> [u8; 8] {
    let mut b = [0u8; 8];
    b[..4].copy_from_slice(&v.to_le_bytes());
    b[4..].copy_from_slice(&v.to_be_bytes());
    b
}

fn both_ways16(v: u16) -> [u8; 4] {
    let mut b = [0u8; 4];
    b[..2].copy_from_slice(&v.to_le_bytes());
    b[2..].copy_from_slice(&v.to_be_bytes());
    b
}

/// An ISO 9660 directory record.
fn record(extent: u32, size: u32, dir: bool, name: &[u8]) -> Vec<u8> {
    let len = (33 + name.len()).next_multiple_of(2);
    let mut r = vec![0u8; len];
    r[0] = len as u8;
    r[2..10].copy_from_slice(&both_ways32(extent));
    r[10..18].copy_from_slice(&both_ways32(size));
    r[25] = if dir { 2 } else { 0 };
    r[28..32].copy_from_slice(&both_ways16(1));
    r[32] = name.len() as u8;
    r[33..33 + name.len()].copy_from_slice(name);
    r
}

/// A PlayStation disc of 2048-byte sectors that boots `exe`: its SYSTEM.CNF says to.
fn ps1_disc(exe: &[u8]) -> Vec<u8> {
    const ROOT: u32 = 20;
    const CNF: u32 = 22;
    const EXE: u32 = 23;
    let cnf = b"BOOT = cdrom:\\MAIN.EXE;1\r\nTCB = 4\r\nEVENT = 10\r\nSTACK = 801FFF00\r\n";
    let sectors = EXE as usize + exe.len().div_ceil(2048);
    let mut disc = vec![0u8; sectors * 2048];
    let at = |sector: u32| sector as usize * 2048;

    let pvd = at(16);
    disc[pvd] = 1;
    disc[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
    disc[pvd + 6] = 1;
    disc[pvd + 8..pvd + 72].copy_from_slice(&[b' '; 64]);
    disc[pvd + 8..pvd + 19].copy_from_slice(b"PLAYSTATION");
    disc[pvd + 40..pvd + 50].copy_from_slice(b"OMEGA_ZERO");
    disc[pvd + 80..pvd + 88].copy_from_slice(&both_ways32(sectors as u32));
    disc[pvd + 120..pvd + 124].copy_from_slice(&both_ways16(1));
    disc[pvd + 124..pvd + 128].copy_from_slice(&both_ways16(1));
    disc[pvd + 128..pvd + 132].copy_from_slice(&both_ways16(2048));
    disc[pvd + 132..pvd + 140].copy_from_slice(&both_ways32(10));
    disc[pvd + 140..pvd + 144].copy_from_slice(&18u32.to_le_bytes());
    disc[pvd + 148..pvd + 152].copy_from_slice(&19u32.to_be_bytes());
    let root = record(ROOT, 4096, true, &[0]);
    disc[pvd + 156..pvd + 156 + root.len()].copy_from_slice(&root);
    disc[pvd + 881] = 1;

    let end = at(17);
    disc[end] = 255;
    disc[end + 1..end + 6].copy_from_slice(b"CD001");
    disc[end + 6] = 1;

    for (sector, extent) in [(18, ROOT.to_le_bytes()), (19, ROOT.to_be_bytes())] {
        let p = at(sector);
        disc[p] = 1;
        disc[p + 2..p + 6].copy_from_slice(&extent);
        let parent: u16 = 1;
        let parent = if sector == 18 {
            parent.to_le_bytes()
        } else {
            parent.to_be_bytes()
        };
        disc[p + 6..p + 8].copy_from_slice(&parent);
    }

    let mut dir = Vec::new();
    dir.extend(record(ROOT, 4096, true, &[0]));
    dir.extend(record(ROOT, 4096, true, &[1]));
    dir.extend(record(CNF, cnf.len() as u32, false, b"SYSTEM.CNF;1"));
    dir.extend(record(EXE, exe.len() as u32, false, b"MAIN.EXE;1"));
    disc[at(ROOT)..at(ROOT) + dir.len()].copy_from_slice(&dir);
    disc[at(CNF)..at(CNF) + cnf.len()].copy_from_slice(cnf);
    disc[at(EXE)..at(EXE) + exe.len()].copy_from_slice(exe);
    disc
}

fn write_disc(name: &str, rgb: [u8; 3]) -> PathBuf {
    let path = scratch(name);
    std::fs::write(&path, ps1_disc(&ps1_exe(&ps1_program(rgb)))).expect("write the disc");
    path
}

/// pcsx_rearmed as omega-zero opens it, its options there before it starts: it picks its BIOS
/// in retro_init, and has no memory card 1 at all unless it is told to keep it for us.
fn pcsx() -> Option<LibretroCore> {
    pcsx_in("auto")
}

/// pcsx_rearmed made to run every disc as `region`'s.
fn pcsx_in(region: &str) -> Option<LibretroCore> {
    let dylib = vendored("pcsx_rearmed")?;
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let options = [
        ("pcsx_rearmed_bios", "HLE"),
        ("pcsx_rearmed_memcard1", "libretro"),
        ("pcsx_rearmed_memcard2", "none"),
        ("pcsx_rearmed_region", region),
    ];
    Some(
        LibretroCore::open_with_options(&dylib, dir, dir, &options)
            .expect("pcsx_rearmed would not open"),
    )
}

/// Runs the core until `until` holds of its picture, at most `frames` frames.
fn run_until(core: &mut LibretroCore, frames: usize, until: impl Fn(&LibretroCore) -> bool) {
    for _ in 0..frames {
        if until(core) {
            return;
        }
        run(core, 1);
    }
    panic!(
        "the picture never came: {:?}, middle {:?}",
        core.video_size(),
        middle(core)
    );
}

#[test]
fn pcsx_rearmed_reads_its_disc_from_its_path_and_has_a_drive() {
    let _g = lock();
    let Some(core) = pcsx() else {
        return;
    };
    assert!(
        core.needs_fullpath(),
        "a disc would be read into memory whole"
    );
    assert!(core.has_disk_control(), "pcsx_rearmed handed over no drive");
}

#[test]
fn pcsx_rearmed_boots_our_disc_and_follows_its_resolution() {
    let _g = lock();
    let Some(mut core) = pcsx() else {
        return;
    };
    let disc = write_disc("red.iso", [0xff, 0, 0]);
    core.load(&disc).expect("pcsx_rearmed refused our disc");
    let fps = core.av_info().fps;
    assert!((fps - 60.0).abs() < 0.5, "an NTSC disc runs at {fps}");
    let red = |c: &LibretroCore| {
        let [r, g, b] = middle(c);
        r > 200 && g < 40 && b < 40
    };
    run_until(&mut core, 300, |c| c.video_size() == (320, 240) && red(c));
    run_until(&mut core, 600, |c| c.video_size().0 == 640 && red(c));
    let (w, h) = core.video_size();
    eprintln!("pcsx_rearmed went from 320x240 to {w}x{h}");
    assert!(h >= 480, "a 640x480 interlaced picture came out {w}x{h}");
    assert_eq!(
        core.save_ram().map(|card| card.len()),
        Some(128 * 1024),
        "memory card 1 is not the game's save"
    );
    let state = core.serialize().expect("no state");
    eprintln!("pcsx_rearmed state: {} bytes", state.len());
}

fn playlist() -> (PathBuf, PathBuf) {
    write_disc("disc1.iso", [0xff, 0, 0]);
    let second = write_disc("disc2.iso", [0, 0, 0xff]);
    let m3u = scratch("game.m3u");
    std::fs::write(&m3u, "# two discs\ndisc1.iso\ndisc2.iso\n").expect("write the playlist");
    (m3u, second)
}

#[test]
fn a_playlist_s_discs_swap_in_pcsx_rearmed_s_drive() {
    let _g = lock();
    let Some(mut core) = pcsx() else {
        return;
    };
    let (m3u, second) = playlist();
    core.load(&m3u).expect("pcsx_rearmed refused the playlist");
    run(&mut core, 5);
    assert_eq!(core.disc_count(), 2);
    assert_eq!(core.disc_index(), 0);
    let path = core.disc_path(1).expect("the second disc has no file");
    assert!(
        Path::new(&path).ends_with("disc2.iso") || path.ends_with("disc2.iso"),
        "the second disc is read from {path}, not {}",
        second.display()
    );
    assert!(core.set_disc(1), "the swap failed");
    assert_eq!(core.disc_index(), 1);
    run(&mut core, 5);
    assert!(!core.set_disc(2), "a third disc of two went in");
    assert_eq!(core.disc_index(), 1);
}

#[test]
fn pcsx_rearmed_starts_on_the_disc_it_was_left_on() {
    let _g = lock();
    let (m3u, _) = playlist();
    let path = {
        let Some(mut core) = pcsx() else {
            return;
        };
        core.load(&m3u).expect("pcsx_rearmed refused the playlist");
        core.disc_path(1).expect("the second disc has no file")
    };
    let Some(mut core) = pcsx() else {
        return;
    };
    core.set_initial_disc(1, &path);
    core.load(&m3u).expect("pcsx_rearmed refused the playlist");
    assert_eq!(core.disc_index(), 1, "the game started on its first disc");
    let blue = |c: &LibretroCore| {
        let [r, _, b] = middle(c);
        b > 200 && r < 40
    };
    run_until(&mut core, 300, blue);
}

#[test]
fn pcsx_rearmed_runs_a_european_disc_at_50() {
    let _g = lock();
    let Some(mut core) = pcsx_in("PAL") else {
        return;
    };
    core.load(&write_disc("pal.iso", [0xff, 0, 0]))
        .expect("pcsx_rearmed refused our disc");
    let fps = core.av_info().fps;
    assert!((fps - 50.0).abs() < 0.5, "a PAL disc runs at {fps}");
    run_until(&mut core, 300, |c| {
        let [r, g, _] = middle(c);
        r > 200 && g < 40
    });
}
