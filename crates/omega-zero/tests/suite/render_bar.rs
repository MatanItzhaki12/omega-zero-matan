#![cfg(any(target_os = "macos", target_os = "linux"))]
//! The bar along the bottom as the panel shows it: every pixel of its actions, notches and
//! all, and of the hints between them is the kit's face drawn at twice its size on whole
//! pixels, on the shelf, in Settings and under the core picker with ◀▶ SWAP between RETURN and
//! CONFIRM, in Night and in Amber, and in the font Settings > Theme picks, from the frame it is
//! picked in. Between the actions the background shows through everywhere but the hints.

use crate::common;

use std::collections::VecDeque;

use common::{clocked, tmp_root_with_carts};
use omega_gfx::{out_h, out_w, Compositor, HeadlessSurface};
use omega_input::{Btn, InputSource, Millis, RawEvent};
use omega_power::SimPlatform;
use omega_store::{read_app_state, write_app_state, BackdropStyle};
use omega_ui::backdrop_rgb;
use omega_ui::kit::action::{action_pair, Font, Side, RISE, TALL};
use omega_ui::kit::bar::{bar_layout, hints_face, Hint};
use omega_ui::kit::palette::Palette;
use omega_ui::{UndoFace, SPRITE_PX};
use omega_zero::app::Phase;
use omega_zero::frontend::Frontend;
use omega_zero::label_sync::Account;
use omega_zero::screens::bar::{Actions, SWAP};

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

fn tap(f: &mut Frontend, input: &mut Script, btn: Btn) {
    input.0.push_back(vec![RawEvent::Down(btn)]);
    f.advance(input);
    input.0.push_back(vec![RawEvent::Up(btn)]);
    f.advance(input);
}

/// The bar the screen's width with `actions` at its ends, made together as a pair with their
/// words in `font`, and `hints` between them, at one texel to an art pixel, on a face `TALL`
/// high whose row `RISE` is the bar's top and which is clear wherever the bar draws nothing.
fn kit_bar(actions: Actions, hints: &[Hint], font: Font, pal: &Palette) -> UndoFace {
    let width = out_w() / SPRITE_PX;
    let [left, right] = action_pair(
        actions.action(Side::Left),
        actions.action(Side::Right),
        font,
        pal,
    );
    let layout = bar_layout(width, &left, &right);
    let mut bar = UndoFace {
        rgba: vec![0; (width * TALL * 4) as usize],
        w: width,
        h: TALL,
    };
    let [hx, hy, hw, _] = layout.hints;
    let hints = hints_face(hw, hints, pal);
    over(&mut bar, &hints, hx as i32, (hy + RISE) as i32);
    for (face, [x, y]) in [(&left.face, layout.left), (&right.face, layout.right)] {
        over(&mut bar, face, x, y + RISE as i32);
    }
    bar
}

fn over(onto: &mut UndoFace, face: &UndoFace, x: i32, y: i32) {
    for fy in 0..face.h as i32 {
        for fx in 0..face.w as i32 {
            let i = ((fy * face.w as i32 + fx) * 4) as usize;
            let (tx, ty) = (x + fx, y + fy);
            if face.rgba[i + 3] == 0
                || tx < 0
                || ty < 0
                || tx >= onto.w as i32
                || ty >= onto.h as i32
            {
                continue;
            }
            let o = ((ty * onto.w as i32 + tx) * 4) as usize;
            onto.rgba[o..o + 4].copy_from_slice(&face.rgba[i..i + 4]);
        }
    }
}

/// Where the frame `px` is not `bar`'s opaque pixels drawn at twice their size along the
/// bottom of the screen, as (x, y, got, want).
fn misses(px: &[u8], bar: &UndoFace) -> Vec<(usize, usize, [u8; 3], [u8; 3])> {
    let (w, top) = (out_w() as usize, (out_h() - TALL * SPRITE_PX) as usize);
    let mut off = Vec::new();
    for ty in 0..bar.h as usize {
        for tx in 0..bar.w as usize {
            let t = (ty * bar.w as usize + tx) * 4;
            if bar.rgba[t + 3] != 0xff {
                continue;
            }
            let want = [bar.rgba[t], bar.rgba[t + 1], bar.rgba[t + 2]];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (x, y) = (tx * 2 + dx, top + ty * 2 + dy);
                let o = (y * w + x) * 4;
                let got = [px[o], px[o + 1], px[o + 2]];
                if got != want {
                    off.push((x, y, got, want));
                }
            }
        }
    }
    off
}

/// Where the frame `px` is not the plain background `ground` between `actions`, on the bar's
/// rows, where `bar` (their kit bar in `font`) draws nothing: the background should show
/// through all of it. As (x, y, got).
fn covered(
    px: &[u8],
    bar: &UndoFace,
    actions: Actions,
    font: Font,
    pal: &Palette,
    ground: [u8; 3],
) -> Vec<(usize, usize, [u8; 3])> {
    let [left, right] = action_pair(
        actions.action(Side::Left),
        actions.action(Side::Right),
        font,
        pal,
    );
    let [x0, _, room, _] = bar_layout(bar.w, &left, &right).hints;
    let (w, top) = (out_w() as usize, (out_h() - TALL * SPRITE_PX) as usize);
    let mut off = Vec::new();
    for ty in RISE as usize..bar.h as usize {
        for tx in x0 as usize..(x0 + room) as usize {
            if bar.rgba[(ty * bar.w as usize + tx) * 4 + 3] != 0 {
                continue;
            }
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (x, y) = (tx * 2 + dx, top + ty * 2 + dy);
                let o = (y * w + x) * 4;
                let got = [px[o], px[o + 1], px[o + 2]];
                if got != ground {
                    off.push((x, y, got));
                }
            }
        }
    }
    off
}

fn composed(f: &mut Frontend, c: &mut Compositor, name: &str) -> Vec<u8> {
    f.app_mut().finish_slide();
    f.compose(c);
    let px = c.read_frame();
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let path = format!("{dir}/bar-{name}.png");
        let file = std::fs::File::create(&path).expect("create png");
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), out_w(), out_h());
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .expect("png header")
            .write_image_data(&px)
            .expect("png data");
    }
    px
}

#[test]
fn the_bar_is_the_kits_pixel_for_pixel_on_the_shelf_and_in_settings() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    for colour in ["night", "amber"] {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        clocked(d.path());
        let mut state = read_app_state(d.path());
        state.backdrop_colour = omega_store::colour_index(colour).expect("a backdrop colour");
        write_app_state(d.path(), &state).unwrap();
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        f.upload_faces(&mut c);
        let pal = f.app().palette();

        let shelf = composed(&mut f, &mut c, &format!("shelf-{colour}"));
        let off = misses(&shelf, &kit_bar(Actions::Shelf, &[], Font::Bold, pal));
        assert!(
            off.is_empty(),
            "{colour}: the shelf's bar is off the kit's at {} pixels, first {:?}",
            off.len(),
            &off[..off.len().min(4)]
        );

        let mut input = Script(VecDeque::new());
        tap(&mut f, &mut input, Btn::Menu);
        let settings = composed(&mut f, &mut c, &format!("settings-{colour}"));
        let off = misses(&settings, &kit_bar(Actions::Menu, &[], Font::Bold, pal));
        assert!(
            off.is_empty(),
            "{colour}: Settings' bar is off the kit's at {} pixels, first {:?}",
            off.len(),
            &off[..off.len().min(4)]
        );
    }
}

#[test]
fn the_core_pickers_bar_has_the_swap_between_its_actions_pixel_for_pixel() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    for colour in ["night", "amber"] {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        clocked(d.path());
        let mut state = read_app_state(d.path());
        state.backdrop_colour = omega_store::colour_index(colour).expect("a backdrop colour");
        write_app_state(d.path(), &state).unwrap();
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        f.upload_faces(&mut c);
        f.compose(&mut c);
        let pal = f.app().palette();

        let mut input = Script(VecDeque::new());
        tap(&mut f, &mut input, Btn::X);
        assert!(
            f.app().core_picker().is_some(),
            "{colour}: X opened no picker"
        );
        // past the wait for the board's faces, then all the way open
        f.app_mut().update(2.0);
        f.app_mut().update(1.0);
        assert_eq!(f.app().bar_hints_shown(), [SWAP], "{colour}");
        let picker = composed(&mut f, &mut c, &format!("core-picker-{colour}"));
        let off = misses(&picker, &kit_bar(Actions::Menu, &[SWAP], Font::Bold, pal));
        assert!(
            off.is_empty(),
            "{colour}: the picker's bar is off the kit's at {} pixels, first {:?}",
            off.len(),
            &off[..off.len().min(4)]
        );
        // and the swap really is there: it draws pixels the bar without it leaves clear, and
        // every one of them is in the frame
        let drawn = |bar: UndoFace| bar.rgba.chunks_exact(4).filter(|p| p[3] != 0).count();
        assert!(
            drawn(kit_bar(Actions::Menu, &[SWAP], Font::Bold, pal))
                > drawn(kit_bar(Actions::Menu, &[], Font::Bold, pal))
        );
    }
}

/// Nothing but the hints is drawn between the bar's actions: over a plain background, every
/// pixel there on the bar's rows that the hints leave clear is the background's own colour, in
/// Settings, which has none, and on Labels signed in, with Y SIGN OUT, in Night and in Amber.
/// (On the shelf the system's pill stands between them as well, bar.rs.)
#[test]
fn the_background_shows_between_the_bars_actions_but_for_the_hints() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    for colour in ["night", "amber"] {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        clocked(d.path());
        let mut state = read_app_state(d.path());
        state.backdrop = Some(BackdropStyle::Solid);
        state.backdrop_colour = omega_store::colour_index(colour).expect("a backdrop colour");
        write_app_state(d.path(), &state).unwrap();
        Account {
            user: "zero".into(),
            password: "x".into(),
        }
        .save(d.path())
        .unwrap();
        let ground = backdrop_rgb(state.backdrop_colour);
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        f.upload_faces(&mut c);
        let pal = f.app().palette();
        let check = |px: &[u8], hints: &[Hint], name: &str| {
            let bar = kit_bar(Actions::Menu, hints, Font::Bold, pal);
            assert!(misses(px, &bar).is_empty(), "{colour}: {name}'s bar is off");
            let off = covered(px, &bar, Actions::Menu, Font::Bold, pal, ground);
            assert!(
                off.is_empty(),
                "{colour}: {name} draws over the background between the actions at {} \
                 pixels, first {:?}",
                off.len(),
                &off[..off.len().min(4)]
            );
        };

        let mut input = Script(VecDeque::new());
        tap(&mut f, &mut input, Btn::Menu);
        assert!(
            f.app().settings_row().is_some(),
            "{colour}: START opened no Settings"
        );
        let settings = composed(&mut f, &mut c, &format!("solid-settings-{colour}"));
        check(&settings, &[], "Settings");

        common::to_shelf_row(f.app_mut(), omega_zero::options::OptionRow::Labels);
        tap(&mut f, &mut input, Btn::A);
        f.app_mut().finish_slide();
        assert!(matches!(f.app().phase(), Phase::Labels(_)), "{colour}");
        let hints = f.app().bar_hints_shown();
        let sign_out = Hint {
            key: "Y",
            word: "SIGN OUT",
        };
        assert!(
            hints.contains(&sign_out),
            "{colour}: Labels shows {hints:?}"
        );
        let labels = composed(&mut f, &mut c, &format!("solid-labels-{colour}"));
        check(&labels, &hints, "Labels");
    }
}

/// FONT on the THEME screen redraws the bar's actions under it in the frame it changes in:
/// thin, every action narrower and the room for the hints between them wider, the shelf's
/// CONTROLS and SETTINGS 107 art pixels each where bold has 115; and the shelf's bar is thin too
/// once THEME is left.
#[test]
fn a_new_font_redraws_the_bars_actions_in_the_next_frame() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    let pal = f.app().palette();
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::Menu);
    common::to_shelf_row(f.app_mut(), omega_zero::options::OptionRow::Theme);
    tap(&mut f, &mut input, Btn::A);
    assert!(matches!(f.app().phase(), Phase::Background(_)));
    let bold = composed(&mut f, &mut c, "theme-bold");
    let off = misses(&bold, &kit_bar(Actions::Menu, &[], Font::Bold, pal));
    assert!(
        off.is_empty(),
        "THEME's bar is not bold at {} pixels",
        off.len()
    );
    let was = f.app().bar_faces().expect("the bar is made").sets;

    tap(&mut f, &mut input, Btn::Up);
    tap(&mut f, &mut input, Btn::Right);
    assert_eq!(f.app().font(), Font::Thin, "FONT did not change");
    let thin = composed(&mut f, &mut c, "theme-thin");
    let off = misses(&thin, &kit_bar(Actions::Menu, &[], Font::Thin, pal));
    assert!(
        off.is_empty(),
        "THEME's bar is off the thin kit's at {} pixels, first {:?}",
        off.len(),
        &off[..off.len().min(4)]
    );
    assert!(!misses(&thin, &kit_bar(Actions::Menu, &[], Font::Bold, pal)).is_empty());
    let now = f.app().bar_faces().expect("the bar is made").sets;
    for actions in Actions::ALL {
        let (was, now) = (was[actions.index()], now[actions.index()]);
        if actions != Actions::Confirm {
            assert!(
                now.left.1 < was.left.1,
                "{actions:?}: the left action is no narrower"
            );
        }
        assert!(
            now.right.1 < was.right.1,
            "{actions:?}: the right action is no narrower"
        );
        assert!(
            now.layout.hints[2] > was.layout.hints[2],
            "{actions:?}: the room between the actions did not grow"
        );
    }
    let shelf = Actions::Shelf.index();
    assert_eq!(
        [
            was[shelf].left.1,
            was[shelf].right.1,
            now[shelf].left.1,
            now[shelf].right.1
        ],
        [115, 115, 107, 107]
    );

    // THEME, SHELF, then Settings
    for _ in 0..3 {
        tap(&mut f, &mut input, Btn::B);
        f.app_mut().finish_slide();
    }
    assert!(matches!(f.app().phase(), Phase::Shelf));
    let px = composed(&mut f, &mut c, "shelf-thin");
    let off = misses(&px, &kit_bar(Actions::Shelf, &[], Font::Thin, pal));
    assert!(
        off.is_empty(),
        "the shelf's bar is off the thin kit's at {} pixels, first {:?}",
        off.len(),
        &off[..off.len().min(4)]
    );
}
