use omega_gfx::{Canvas, Filter, WHOLE_TEXTURE};
use omega_store::Platform;
use omega_zero::video_mode::{screen_on, source_rect, VideoMode};

const PLATFORMS: [Platform; 3] = [Platform::Gba, Platform::Gb, Platform::Gbc];
const MODES: [VideoMode; 2] = [VideoMode::Actual, VideoMode::Stretch];

fn area(r: [f32; 4], c: Canvas) -> f32 {
    let w = (r[0] + r[2]).min(c.w as f32) - r[0].max(0.0);
    let h = (r[1] + r[3]).min(c.h as f32) - r[1].max(0.0);
    w.max(0.0) * h.max(0.0)
}

#[test]
fn on_720x480_every_picture_is_the_whole_canvas_as_it_always_was() {
    for platform in PLATFORMS {
        for mode in MODES {
            let s = screen_on(Canvas::WIDE, platform, mode);
            assert_eq!(
                s.view.rect,
                [0.0, 0.0, 720.0, 480.0],
                "{platform:?} {mode:?}"
            );
            assert_eq!(s.view.uv, source_rect(platform, mode));
            assert_eq!(s.view.filter, Filter::Lcd3x);
            assert_eq!(s.still.uv, WHOLE_TEXTURE);
            assert!(
                s.bands.is_empty(),
                "{platform:?} {mode:?} left {:?}",
                s.bands
            );
        }
    }
}

#[test]
fn on_640x480_a_gba_game_fills_the_width_from_the_top_over_a_bezel() {
    for mode in MODES {
        let s = screen_on(Canvas::FOUR_THREE, Platform::Gba, mode);
        assert_eq!(s.view.rect, [0.0, 0.0, 640.0, 427.0]);
        assert_eq!(s.view.uv, WHOLE_TEXTURE);
        assert_eq!(
            s.view.filter,
            Filter::Sharp,
            "8/3 is no scale for the 3x mask"
        );
        assert_eq!(s.still, s.view);
        assert_eq!(s.bands, vec![[0.0, 427.0, 640.0, 53.0]]);
    }
}

#[test]
fn on_640x480_a_game_boy_game_stays_at_3x_through_the_mask() {
    for platform in [Platform::Gb, Platform::Gbc] {
        let s = screen_on(Canvas::FOUR_THREE, platform, VideoMode::Actual);
        assert_eq!(s.view.filter, Filter::Lcd3x);
        assert_eq!(s.view.uv, WHOLE_TEXTURE);
        let [x, y, w, h] = s.view.rect;
        assert_eq!((w, h), (720.0, 480.0), "not 3x");
        let picture = (x + 40.0 * 3.0, y + 8.0 * 3.0);
        assert_eq!(picture, (80.0, 24.0), "the 160x144 picture is not centred");
        assert!(
            s.bands.is_empty(),
            "the frame's own border covers the canvas"
        );

        let stretched = screen_on(Canvas::FOUR_THREE, platform, VideoMode::Stretch);
        assert_eq!(stretched.view.rect, [0.0, 0.0, 640.0, 480.0]);
        assert_eq!(stretched.view.uv, source_rect(platform, VideoMode::Stretch));
        assert_eq!(
            stretched.still, s.view,
            "a screenshot is shown at its own size"
        );
    }
}

#[test]
fn on_720x720_the_3x_frame_stands_above_the_middle_between_two_bands() {
    for platform in PLATFORMS {
        let s = screen_on(Canvas::SQUARE, platform, VideoMode::Actual);
        assert_eq!(s.view.rect, [0.0, 90.0, 720.0, 480.0]);
        assert_eq!(s.view.filter, Filter::Lcd3x);
        assert_eq!(
            s.bands,
            vec![[0.0, 0.0, 720.0, 90.0], [0.0, 570.0, 720.0, 150.0]]
        );
    }
}

#[test]
fn the_picture_and_its_bands_cover_every_canvas_exactly_once() {
    for c in Canvas::ALL {
        for platform in PLATFORMS {
            for mode in MODES {
                let s = screen_on(c, platform, mode);
                let covered =
                    area(s.view.rect, c) + s.bands.iter().map(|b| area(*b, c)).sum::<f32>();
                assert!(
                    (covered - (c.w * c.h) as f32).abs() < 0.5,
                    "{c:?} {platform:?} {mode:?}: {covered} of {} px covered",
                    c.w * c.h
                );
            }
        }
    }
}

#[test]
fn the_picture_is_the_game_s_own_window_inside_its_view() {
    use omega_zero::video_mode::picture_rect;
    let at = |c, p, m| picture_rect(p, screen_on(c, p, m).view);
    assert_eq!(
        at(Canvas::FOUR_THREE, Platform::Gba, VideoMode::Actual),
        [0.0, 0.0, 640.0, 427.0]
    );
    assert_eq!(
        at(Canvas::FOUR_THREE, Platform::Gb, VideoMode::Actual),
        [80.0, 24.0, 480.0, 432.0]
    );
    assert_eq!(
        at(Canvas::FOUR_THREE, Platform::Gbc, VideoMode::Stretch),
        [0.0, 0.0, 640.0, 480.0]
    );
    assert_eq!(
        at(Canvas::WIDE, Platform::Gb, VideoMode::Actual),
        [120.0, 24.0, 480.0, 432.0]
    );
    assert_eq!(
        at(Canvas::WIDE, Platform::Gba, VideoMode::Actual),
        [0.0, 0.0, 720.0, 480.0]
    );
}

use omega_zero::video_mode::{picture_rect, screen_on_frame};

const CONSOLES: [Platform; 2] = [Platform::Snes, Platform::Ps1];

/// Every picture size the SNES and the PlayStation draw.
const FRAMES: [(u32, u32); 10] = [
    (256, 224),
    (256, 239),
    (512, 224),
    (512, 448),
    (256, 240),
    (320, 240),
    (368, 240),
    (512, 240),
    (640, 480),
    (320, 256),
];

#[test]
fn a_snes_game_is_twice_its_size_in_the_middle_of_every_canvas() {
    for (c, x, y) in [
        (Canvas::FOUR_THREE, 64.0, 16.0),
        (Canvas::WIDE, 104.0, 16.0),
        (Canvas::SQUARE, 104.0, 136.0),
    ] {
        let s = screen_on_frame(c, Platform::Snes, VideoMode::Actual, (256, 224));
        assert_eq!(s.view.rect, [x, y, 512.0, 448.0], "{c:?}");
        assert_eq!(s.view.uv, WHOLE_TEXTURE);
        assert_eq!(s.view.filter, Filter::Sharp);
        assert_eq!(s.still, s.view);
        assert_eq!(
            s.bands.len(),
            4,
            "{c:?}: the console's body is not all round the picture"
        );
    }
}

#[test]
fn a_hi_res_or_interlaced_snes_frame_fills_the_same_rect() {
    for c in Canvas::ALL {
        let low = screen_on_frame(c, Platform::Snes, VideoMode::Actual, (256, 224)).view;
        for frame in [(512, 224), (512, 448), (256, 448)] {
            let s = screen_on_frame(c, Platform::Snes, VideoMode::Actual, frame);
            assert_eq!(s.view, low, "{c:?} {frame:?}");
        }
    }
}

#[test]
fn a_snes_game_showing_its_overscan_is_still_twice_its_size() {
    let s = screen_on_frame(
        Canvas::FOUR_THREE,
        Platform::Snes,
        VideoMode::Actual,
        (256, 239),
    );
    assert_eq!(s.view.rect, [64.0, 1.0, 512.0, 478.0]);
}

#[test]
fn a_stretched_snes_game_takes_a_television_s_shape() {
    for (c, rect) in [
        (Canvas::FOUR_THREE, [0.0, 0.0, 640.0, 480.0]),
        (Canvas::WIDE, [40.0, 0.0, 640.0, 480.0]),
        (Canvas::SQUARE, [0.0, 90.0, 720.0, 540.0]),
    ] {
        let s = screen_on_frame(c, Platform::Snes, VideoMode::Stretch, (256, 224));
        assert_eq!(s.view.rect, rect, "{c:?}");
        assert_eq!(s.view.filter, Filter::Sharp);
    }
}

#[test]
fn a_playstation_game_is_4_3_and_any_frame_size_fills_it() {
    for (c, rect) in [
        (Canvas::FOUR_THREE, [0.0, 0.0, 640.0, 480.0]),
        (Canvas::WIDE, [40.0, 0.0, 640.0, 480.0]),
        (Canvas::SQUARE, [0.0, 90.0, 720.0, 540.0]),
    ] {
        for frame in FRAMES {
            let s = screen_on_frame(c, Platform::Ps1, VideoMode::Actual, frame);
            assert_eq!(s.view.rect, rect, "{c:?} {frame:?}");
            assert_eq!(s.view.uv, WHOLE_TEXTURE);
            assert_eq!(s.view.filter, Filter::Sharp);
        }
    }
    assert!(
        screen_on_frame(
            Canvas::FOUR_THREE,
            Platform::Ps1,
            VideoMode::Actual,
            (320, 240)
        )
        .bands
        .is_empty(),
        "320x240 at 2x fills 640x480"
    );
    assert_eq!(
        screen_on_frame(Canvas::WIDE, Platform::Ps1, VideoMode::Actual, (320, 240)).bands,
        vec![[0.0, 0.0, 40.0, 480.0], [680.0, 0.0, 40.0, 480.0]]
    );
}

#[test]
fn a_stretched_playstation_game_fills_the_canvas() {
    for c in Canvas::ALL {
        let s = screen_on_frame(c, Platform::Ps1, VideoMode::Stretch, (320, 240));
        assert_eq!(s.view.rect, [0.0, 0.0, c.w as f32, c.h as f32]);
        assert!(s.bands.is_empty());
    }
}

#[test]
fn a_console_s_picture_and_its_bands_cover_every_canvas_exactly_once() {
    for c in Canvas::ALL {
        for platform in CONSOLES {
            for mode in MODES {
                for frame in FRAMES {
                    let s = screen_on_frame(c, platform, mode, frame);
                    let covered =
                        area(s.view.rect, c) + s.bands.iter().map(|b| area(*b, c)).sum::<f32>();
                    assert!(
                        (covered - (c.w * c.h) as f32).abs() < 0.5,
                        "{c:?} {platform:?} {mode:?} {frame:?}: {covered} px covered"
                    );
                    let [x, y, w, h] = s.view.rect;
                    assert!(
                        x >= 0.0 && y >= 0.0 && x + w <= c.w as f32 && y + h <= c.h as f32,
                        "{c:?} {platform:?} {mode:?} {frame:?}: the picture leaves the canvas"
                    );
                    assert_eq!(
                        picture_rect(platform, s.view),
                        s.view.rect,
                        "the whole frame is the game's picture"
                    );
                }
            }
        }
    }
}

#[test]
fn a_handheld_s_screen_ignores_the_frame_size() {
    for c in Canvas::ALL {
        for platform in PLATFORMS {
            for mode in MODES {
                assert_eq!(
                    screen_on_frame(c, platform, mode, (640, 480)),
                    screen_on(c, platform, mode),
                    "{c:?} {platform:?} {mode:?}"
                );
            }
        }
    }
}

#[test]
fn before_its_first_frame_a_console_game_is_laid_out_for_its_usual_picture() {
    for c in Canvas::ALL {
        for platform in CONSOLES {
            assert_eq!(
                screen_on(c, platform, VideoMode::Actual),
                screen_on_frame(c, platform, VideoMode::Actual, platform.picture())
            );
        }
    }
}

/// A PSP game: Fit as wide as the canvas holds its 480x272 in its own shape, Actual at that
/// 480x272 in the middle, Stretch the whole canvas; whatever size PPSSPP draws at.
#[test]
fn a_psp_game_fits_its_own_shape_shows_its_actual_size_or_stretches() {
    for (c, fit, actual) in [
        (
            Canvas::FOUR_THREE,
            [0.0, 59.0, 640.0, 363.0],
            [80.0, 104.0, 480.0, 272.0],
        ),
        (
            Canvas::WIDE,
            [0.0, 36.0, 720.0, 408.0],
            [120.0, 104.0, 480.0, 272.0],
        ),
        (
            Canvas::SQUARE,
            [0.0, 156.0, 720.0, 408.0],
            [120.0, 224.0, 480.0, 272.0],
        ),
    ] {
        for frame in [(480, 272), (960, 544)] {
            let at = |mode| screen_on_frame(c, Platform::Psp, mode, frame).view;
            assert_eq!(at(VideoMode::Fit).rect, fit, "{c:?} {frame:?} fit");
            assert_eq!(at(VideoMode::Actual).rect, actual, "{c:?} {frame:?} actual");
            assert_eq!(
                at(VideoMode::Stretch).rect,
                [0.0, 0.0, c.w as f32, c.h as f32],
                "{c:?} stretch"
            );
            assert_eq!(at(VideoMode::Fit).filter, Filter::Sharp);
            assert_eq!(at(VideoMode::Fit).uv, WHOLE_TEXTURE);
        }
    }
    assert_eq!(VideoMode::parse("fit"), Some(VideoMode::Fit));
    assert_eq!(
        VideoMode::Fit.step_in(&VideoMode::PSP, true),
        VideoMode::Actual
    );
    assert_eq!(
        VideoMode::Fit.step_in(&VideoMode::PSP, false),
        VideoMode::Stretch
    );
}

/// A DS game: one screen's view, as the core draws it or One Screen at twice that, at a whole
/// 2x in the middle of each canvas, every DS pixel two by two; two screens stacked or side by
/// side as big as the canvas holds them.
#[test]
fn a_ds_screen_shows_at_a_whole_scale_and_two_screens_fit() {
    for (c, one) in [
        (Canvas::FOUR_THREE, [64.0, 48.0, 512.0, 384.0]),
        (Canvas::WIDE, [104.0, 48.0, 512.0, 384.0]),
        (Canvas::SQUARE, [104.0, 168.0, 512.0, 384.0]),
    ] {
        for frame in [(256, 192), (512, 384), (0, 0)] {
            let view = screen_on_frame(c, Platform::Nds, VideoMode::Fit, frame).view;
            assert_eq!(view.rect, one, "{c:?} {frame:?}");
        }
    }
    let stacked = screen_on_frame(Canvas::WIDE, Platform::Nds, VideoMode::Fit, (256, 384)).view;
    assert_eq!(stacked.rect, [200.0, 0.0, 320.0, 480.0]);
    let beside = screen_on_frame(Canvas::WIDE, Platform::Nds, VideoMode::Fit, (512, 192)).view;
    assert_eq!(beside.rect, [0.0, 105.0, 720.0, 270.0]);
}
