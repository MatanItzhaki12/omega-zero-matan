//! A UMD going in, drawn frame by frame. A jump in the cart, or a disc spun faster than the
//! screen can show it turning, reads as a stutter however steady the frames are, so each frame
//! drawn is checked against the one before.

use crate::common;

use common::{boot, tmp_root_with_carts};
use omega_gfx::{Draw, TexId};
use omega_input::Action;

const DISC: TexId = TexId::from_raw(900);
const BLUR: TexId = TexId::from_raw(901);
const HUB: TexId = TexId::from_raw(902);

/// What one frame drew of the disc.
#[derive(Debug)]
struct Frame {
    /// Where the disc was drawn, top edge.
    y: f32,
    /// How far the sharp disc was turned, and in how many copies.
    turn: Option<f32>,
    copies: usize,
    /// How much of the ring blur showed.
    blurred: f32,
}

fn frame_of(out: &[Draw]) -> Option<Frame> {
    let mut f = Frame {
        y: f32::NAN,
        turn: None,
        copies: 0,
        blurred: 0.0,
    };
    for d in out {
        match *d {
            Draw::Turned { y, tex, turn, .. } if tex == DISC => {
                f.turn.get_or_insert(turn);
                f.y = y;
                f.copies += 1;
            }
            Draw::Tex { y, tex, alpha, .. } if tex == BLUR => {
                f.y = y;
                f.blurred = alpha;
            }
            _ => {}
        }
    }
    (!f.y.is_nan()).then_some(f)
}

/// A UMD put in, `frames` drawn at 60 a second, its core ready after `ready`.
fn going_in(frames: usize, ready: usize) -> Vec<Option<Frame>> {
    let d = tmp_root_with_carts(&[]);
    let dir = d.path().join("Games/PSP");
    std::fs::create_dir_all(&dir).unwrap();
    for stem in ["Patapon", "Patapon 2"] {
        std::fs::write(dir.join(format!("{stem}.iso")), vec![0u8; 64 * 1024]).unwrap();
    }
    let mut a = boot(d.path());
    a.set_spin_disc("Patapon".into(), DISC, BLUR, HUB);
    a.apply(Action::Insert);
    (0..frames)
        .map(|f| {
            if f == ready {
                a.on_core_ready();
            }
            a.update(1.0 / 60.0);
            let mut out = Vec::new();
            a.draw(&mut out);
            frame_of(&out)
        })
        .collect()
}

#[test]
fn a_disc_too_fast_to_be_seen_turning_is_drawn_as_its_blur_and_never_strobes() {
    let frames = going_in(110, 1_000);
    let last = frames.last().unwrap().as_ref().expect("no disc drawn");
    assert_eq!(last.blurred, 1.0, "the fast disc is not all blur: {last:?}");
    assert_eq!(
        last.copies, 0,
        "the sharp disc is still drawn under the blur"
    );
    // while the sharp disc shows at all, it never turns so far in a frame that the eye loses
    // which way it went
    for w in frames.windows(2) {
        if let (Some(a), Some(b)) = (&w[0], &w[1]) {
            if let (Some(ta), Some(tb), true) = (a.turn, b.turn, b.blurred < 0.5) {
                let step = (tb - ta).rem_euclid(std::f32::consts::TAU);
                assert!(
                    step < std::f32::consts::PI / 3.0,
                    "the sharp disc jumped {:.0}° in a frame: {a:?} to {b:?}",
                    step.to_degrees()
                );
            }
        }
    }
}

#[test]
fn a_umd_spins_up_at_the_top_and_goes_in_before_its_core_opens() {
    let d = tmp_root_with_carts(&[]);
    let dir = d.path().join("Games/PSP");
    std::fs::create_dir_all(&dir).unwrap();
    for stem in ["Patapon", "Patapon 2"] {
        std::fs::write(dir.join(format!("{stem}.iso")), vec![0u8; 64 * 1024]).unwrap();
    }
    let mut a = boot(d.path());
    a.set_spin_disc("Patapon".into(), DISC, BLUR, HUB);
    a.apply(Action::Insert);
    let mut opened_at = None;
    for f in 0..240 {
        a.update(1.0 / 60.0);
        if a.core_may_open() {
            opened_at.get_or_insert(f);
            assert!(a.seat() >= 1.0, "the core opened with the UMD in sight");
        } else if f < 100 {
            assert!(a.seat() < 0.5, "the UMD went in before it spun up");
        }
    }
    let at = opened_at.expect("the core never opened");
    assert!(at < 200, "the core waited {at} frames");
}

#[test]
fn the_umd_never_jerks_on_its_way_in() {
    let frames = going_in(240, 90);
    let ys: Vec<f32> = frames.iter().flatten().map(|f| f.y).collect();
    assert!(
        ys.len() > 100,
        "the disc was drawn on only {} frames",
        ys.len()
    );
    // the cart speeds up and slows down smoothly: from one frame to the next its speed changes
    // by a few pixels a frame at most, so nothing starts or stops with a jerk
    for w in ys.windows(3) {
        let (v0, v1) = (w[1] - w[0], w[2] - w[1]);
        assert!(
            (v1 - v0).abs() < 6.0,
            "the UMD's speed jumped from {v0:.1} to {v1:.1} px a frame"
        );
    }
}
