#![cfg(any(target_os = "macos", target_os = "linux"))]
//! The start notice as the frontend draws it: the opening's mint on black, where the opening's
//! terminal stands and in the very letters the opening drew, with no shelf between the two, and
//! the shelf once a press puts it away.

use crate::common;

use common::tmp_root_with_carts;
use omega_gfx::{out_h, out_w, Compositor, HeadlessSurface};
use omega_input::{Btn, InputSource, Millis, RawEvent};
use omega_power::SimPlatform;
use omega_store::{write_app_state, write_selected_core, AppState, Core, Platform};
use omega_zero::frontend::Frontend;
use omega_zero::opening::Facts;
use tempfile::TempDir;

const MMX: &str = "Mega Man X (U) (V1.1) [!]";

/// The terminal's size on the screen: 32 letters of 18 pixels, 22 lines of 18.
const TERMINAL_W: u32 = 576;
const TERMINAL_H: u32 = 396;
const PITCH: u32 = 18;

#[derive(Default)]
struct Presses(Vec<RawEvent>);

impl InputSource for Presses {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        std::mem::take(&mut self.0)
    }
}

fn compositor() -> Option<(HeadlessSurface, Compositor)> {
    let surface = HeadlessSurface::new().ok()?;
    let c = Compositor::new(&surface).ok()?;
    Some((surface, c))
}

/// Boots on a card whose last start, Mega Man X's on Supafaust, took omega-zero down, the
/// opening playing as the handheld plays it.
fn boot(c: &mut Compositor) -> (TempDir, Frontend) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    std::fs::write(
        d.path().join(format!("Games/SNES/{MMX}.sfc")),
        vec![0u8; 0x8000],
    )
    .unwrap();
    write_selected_core(d.path(), MMX, Core::Supafaust).unwrap();
    let state = AppState {
        clock_set: true,
        boot_animation: true,
        cart: Some(MMX.into()),
        cart_platform: Some(Platform::Snes),
        ..AppState::default()
    };
    write_app_state(d.path(), &state).unwrap();
    std::fs::write(d.path().join("Config/.starting"), format!("SNES/{MMX}")).unwrap();
    let platform = SimPlatform::at(d.path().to_path_buf()).stopped_at(1_790_784_000);
    let mut f = Frontend::boot(Box::new(platform));
    f.upload_faces(c);
    f.start_opening(&Facts::default());
    (d, f)
}

/// Frame `n` at 60 fps.
fn frame(f: &mut Frontend, c: &mut Compositor, input: &mut Presses, n: u64) -> Vec<u8> {
    f.advance_at(input, n * 1000 / 60, 1.0 / 60.0);
    f.compose(c);
    c.read_frame()
}

fn rgb(frame: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = ((y * out_w() + x) * 4) as usize;
    [frame[i], frame[i + 1], frame[i + 2]]
}

fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 2)
}

/// Where the terminal's first letter stands: the top of its capitals.
fn origin() -> (u32, u32) {
    let x = ((out_w() - TERMINAL_W) / 2) & !1;
    let y = ((out_h() - TERMINAL_H) / 2) & !1;
    (x, y)
}

/// The pixels of the terminal's rows `rows`, from the top of the first's capitals to the top
/// of the next's, the whole terminal across.
fn band(frame: &[u8], rows: std::ops::Range<u32>) -> Vec<[u8; 3]> {
    let (x, y) = origin();
    let mut px = Vec::new();
    for yy in y + rows.start * PITCH..y + rows.end * PITCH {
        for xx in x..=(x + TERMINAL_W) {
            px.push(rgb(frame, xx, yy));
        }
    }
    px
}

fn inked(px: &[[u8; 3]]) -> usize {
    px.iter().filter(|p| **p != [0, 0, 0]).count()
}

#[test]
fn the_notice_is_the_openings_terminal_and_then_the_shelf() {
    let Some((_s, mut c)) = compositor() else {
        eprintln!("no GL on this host, skipping");
        return;
    };
    let (_d, mut f) = boot(&mut c);
    let mut input = Presses::default();
    let all = (out_w() * out_h()) as usize;
    let mut n = 1;
    let mut opening = Vec::new();
    // the opening with all its lines up, then its fade out and the notice in, with no shelf
    let notice = loop {
        let lit = frame(&mut f, &mut c, &mut input, n);
        if n == 600 {
            assert_eq!(f.app().opening_levels(), Some((1.0, 1.0)));
            opening = lit.clone();
        }
        if n > 600 {
            let black = lit.chunks_exact(4).filter(|p| p[..3] == [0, 0, 0]).count();
            assert!(black > all * 3 / 4, "frame {n}: the shelf showed through");
        }
        if f.app().start_notice_prompted() {
            break lit;
        }
        n += 1;
        assert!(n < 1200, "the notice's prompt never came up");
    };
    assert!(!f.app().opening_shown());
    assert_eq!(f.app().start_notice_levels(), Some((1.0, 1.0)));
    let (x, y) = origin();
    // the O of OMEGA, as the opening draws it: mint at the capitals' top, pale lower down
    assert!(
        near(rgb(&notice, x + 1, y + 2), [161, 228, 208]),
        "{:?}",
        rgb(&notice, x + 1, y + 2)
    );
    assert!(
        near(rgb(&notice, x + 1, y + 6), [214, 238, 232]),
        "{:?}",
        rgb(&notice, x + 1, y + 6)
    );
    assert_eq!(rgb(&notice, x + 6, y + 6), [0, 0, 0], "the O is not hollow");
    assert_eq!(
        band(&notice, 0..2),
        band(&opening, 0..2),
        "the header is not the opening's, letter for letter"
    );
    assert!(inked(&band(&notice, 3..4)) > 0, "no warning");
    assert_eq!(inked(&band(&notice, 2..3)), 0, "the blank line is inked");
    assert!(inked(&band(&notice, 16..17)) > 0, "no prompt");
    assert_eq!(inked(&band(&notice, 17..22)), 0, "lines past the prompt");
    // nothing stands right of the terminal's 32 letters
    for yy in y - 2..y + TERMINAL_H {
        for xx in x + TERMINAL_W + 1..out_w() {
            assert_eq!(rgb(&notice, xx, yy), [0, 0, 0], "ink at {xx},{yy}");
        }
    }
    input.0.push(RawEvent::Down(Btn::A));
    frame(&mut f, &mut c, &mut input, n + 1);
    input.0.push(RawEvent::Up(Btn::A));
    let mut after = Vec::new();
    for m in n + 2..n + 60 {
        after = frame(&mut f, &mut c, &mut input, m);
    }
    assert!(
        !f.app().start_notice_shown(),
        "still shown a second after A"
    );
    assert_eq!(f.app().selected_stem(), Some(MMX));
    let black = after
        .chunks_exact(4)
        .filter(|p| p[..3] == [0, 0, 0])
        .count();
    assert!(
        black < all / 2,
        "{black} of {all} pixels still black: no shelf"
    );
}
