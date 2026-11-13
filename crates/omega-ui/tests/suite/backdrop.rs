use omega_store::{BackdropStyle, BACKDROP_COLOURS};
use omega_ui::{
    backdrop_colour_name, backdrop_pattern_face, backdrop_period, backdrop_rgb, backdrop_shift,
    backdrop_speed, out_h, out_w, BACKDROP_PALETTE,
};

fn patterns() -> impl Iterator<Item = BackdropStyle> {
    BackdropStyle::ALL.into_iter().filter(|s| s.moves())
}

#[test]
fn a_pattern_repeats_every_period_so_drifting_it_never_shows_a_seam() {
    for style in patterns() {
        let p = backdrop_period(style);
        let face = backdrop_pattern_face(style, 3).expect("no pattern");
        assert_eq!((face.w, face.h), (out_w() + p, out_h()));
        let px = |x: u32, y: u32| &face.rgba[((y * face.w + x) * 4) as usize..][..4];
        for y in 0..face.h {
            for x in 0..out_w() {
                assert_eq!(px(x, y), px(x + p, y), "{style:?} breaks at {x},{y}");
            }
        }
        assert!(
            face.rgba.chunks_exact(4).all(|c| c[3] == 255),
            "{style:?} lets the black show through"
        );
    }
}

#[test]
fn every_period_divides_every_canvas_so_a_still_pattern_sits_square() {
    for style in patterns() {
        let p = backdrop_period(style);
        for size in [480, 640, 720] {
            assert_eq!(size % p, 0, "{style:?}'s {p} does not divide {size}");
        }
    }
}

#[test]
fn every_pattern_is_drawn_in_whole_2x2_pixels_like_the_words_over_it() {
    for style in patterns() {
        let face = backdrop_pattern_face(style, 7).expect("no pattern");
        let px = |x: u32, y: u32| &face.rgba[((y * face.w + x) * 4) as usize..][..4];
        for y in (0..face.h).step_by(2) {
            for x in (0..face.w).step_by(2) {
                let c = px(x, y);
                assert!(
                    [px(x + 1, y), px(x, y + 1), px(x + 1, y + 1)]
                        .iter()
                        .all(|p| *p == c),
                    "{style:?} splits the pixel at {x},{y}"
                );
            }
        }
    }
}

#[test]
fn no_two_patterns_look_the_same() {
    let faces: Vec<_> = patterns()
        .map(|s| (s, backdrop_pattern_face(s, 0).expect("no pattern").rgba))
        .collect();
    for (i, (a, fa)) in faces.iter().enumerate() {
        for (b, fb) in &faces[i + 1..] {
            assert_ne!(fa, fb, "{a:?} and {b:?} are one pattern");
        }
    }
}

#[test]
fn a_plain_colour_or_a_picture_has_no_pattern_to_make() {
    assert!(backdrop_pattern_face(BackdropStyle::Solid, 0).is_none());
    assert!(backdrop_pattern_face(BackdropStyle::Wallpaper, 0).is_none());
}

#[test]
fn a_drifting_pattern_moves_slowly_by_the_clock_not_the_frame_and_starts_over() {
    for style in patterns() {
        let p = backdrop_period(style);
        assert_eq!(
            backdrop_shift(style, false, 123_456),
            0,
            "a still one moved"
        );
        let lap = (p as f64 / backdrop_speed(style) * 1000.0).round() as u64;
        assert!(lap >= 2_000, "{style:?} crosses a period in {lap} ms");
        let mut was = 0;
        for ms in (0..lap).step_by(16) {
            let s = backdrop_shift(style, true, ms);
            assert!(s < p);
            assert!(s >= was, "{style:?} went back at {ms} ms");
            was = s;
        }
        for t in [0, 777, 5_000, 61_234] {
            assert_eq!(
                backdrop_shift(style, true, t),
                backdrop_shift(style, true, t + lap),
                "{style:?} did not start over after one period"
            );
        }
    }
    assert_eq!(backdrop_shift(BackdropStyle::Solid, true, 9_999), 0);
}

/// A pattern drifts a whole art pixel at a time, never one screen pixel, so its 2x2 pixels stay
/// on the grid the shelf and the menus are drawn on, frame after frame.
#[test]
fn a_drifting_pattern_moves_a_whole_art_pixel_at_a_time() {
    for style in patterns() {
        let mut was = 0;
        let mut steps = 0;
        for ms in 0..20_000 {
            let s = backdrop_shift(style, true, ms);
            assert_eq!(s % 2, 0, "{style:?} sits half an art pixel off at {ms} ms");
            if s != was {
                let step = (s + backdrop_period(style) - was) % backdrop_period(style);
                assert_eq!(step, 2, "{style:?} jumped {step} px at {ms} ms");
                steps += 1;
            }
            was = s;
        }
        assert!(steps > 50, "{style:?} hardly moved: {steps} steps in 20 s");
    }
}

#[test]
fn every_colour_has_a_name_and_stays_dark_enough_for_a_label() {
    assert_eq!(BACKDROP_PALETTE.len(), BACKDROP_COLOURS.len());
    let mut names: Vec<&str> = (0..BACKDROP_COLOURS.len() as u8)
        .map(backdrop_colour_name)
        .collect();
    assert_eq!(names[0], "Night");
    names.sort_unstable();
    names.dedup();
    assert_eq!(
        names.len(),
        BACKDROP_COLOURS.len(),
        "two colours share a name"
    );
    for c in 0..BACKDROP_COLOURS.len() as u8 {
        let [r, g, b] = backdrop_rgb(c);
        let luma = 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32;
        assert!(
            luma < 48.0,
            "{} is too light to sit behind carts",
            backdrop_colour_name(c)
        );
    }
}
