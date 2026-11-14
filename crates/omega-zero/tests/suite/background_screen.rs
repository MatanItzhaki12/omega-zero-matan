use crate::common;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use omega_input::{Action, Btn, Millis};
use omega_store::{read_app_state, write_app_state, AppState, BackdropStyle, Font};
use omega_ui::kit::action::Font as BarFont;
use omega_ui::{backdrop_period, backdrop_rgb, backdrop_shift, out_h, out_w, Draw, TexId};
use omega_zero::app::{App, Phase};
use omega_zero::background_screen::{Backdrop, Sel};
use omega_zero::list_view::{RowLabel, RowValue};
use tempfile::TempDir;

struct Rig {
    d: TempDir,
    a: App,
    now: Millis,
}

impl Rig {
    fn with(state: AppState) -> Rig {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        write_app_state(
            d.path(),
            &AppState {
                clock_set: true,
                ..state
            },
        )
        .unwrap();
        let (a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
        let mut r = Rig { d, a, now: 1_000 };
        r.tick(16);
        r
    }

    fn new() -> Rig {
        Rig::with(AppState::default())
    }

    fn press(&mut self, btn: Btn) {
        self.a.apply(Action::GbaDown(btn));
        self.a.apply(Action::GbaUp(btn));
    }

    fn tick(&mut self, ms: Millis) {
        self.now += ms;
        self.a.tick_ms(self.now);
    }

    fn open(&mut self) {
        self.a.apply(Action::QuickMenu);
        assert!(self.a.settings_row().is_some(), "START opened no Settings");
        common::to_shelf_row(&mut self.a, omega_zero::options::OptionRow::Theme);
        self.press(Btn::A);
        assert!(
            matches!(self.a.phase(), Phase::Background(_)),
            "A on Background did not open it"
        );
    }

    fn sel(&self) -> Sel {
        match self.a.phase() {
            Phase::Background(s) => s.sel,
            other => panic!("not on the THEME screen: {other:?}"),
        }
    }

    fn rows(&self) -> Vec<(RowLabel, Option<RowValue>)> {
        self.a
            .background_view()
            .expect("the list is showing")
            .rows
            .into_iter()
            .filter(|r| !r.dim)
            .map(|r| (r.label, r.value))
            .collect()
    }

    fn frame(&self) -> Vec<Draw> {
        let mut out = Vec::new();
        self.a.draw(&mut out);
        out
    }
}

fn choice(label: &'static str, value: &'static str) -> (RowLabel, Option<RowValue>) {
    (RowLabel::Setting(label), Some(RowValue::Choice(value)))
}

#[test]
fn a_card_that_never_picked_drifts_a_night_grid_or_shows_its_wallpaper() {
    let r = Rig::new();
    assert_eq!(
        r.a.backdrop(),
        Backdrop {
            style: BackdropStyle::Grid,
            moving: true,
            colour: 0
        }
    );
    assert_eq!(
        Backdrop::resolve(None, true, 0, true).style,
        BackdropStyle::Wallpaper,
        "a card with a wallpaper lost it"
    );
    assert_eq!(
        Backdrop::resolve(Some(BackdropStyle::Wallpaper), true, 0, false).style,
        BackdropStyle::Grid,
        "a wallpaper picked and since deleted left nothing to draw"
    );
    assert_eq!(
        Backdrop::resolve(Some(BackdropStyle::Bits), false, 99, true),
        Backdrop {
            style: BackdropStyle::Bits,
            moving: false,
            colour: 7
        },
        "a colour off the end of the palette was kept"
    );
}

#[test]
fn theme_opens_from_settings_and_b_comes_back_to_its_row() {
    let mut r = Rig::new();
    r.open();
    assert_eq!(r.sel(), Sel::Style);
    assert_eq!(
        r.rows(),
        [
            choice("Background", "Grid"),
            choice("Motion", "Moving"),
            choice("Colour", "Night"),
            choice("Font", "Bold")
        ]
    );
    r.press(Btn::B);
    assert_eq!(
        common::shelf_row(&r.a),
        Some(omega_zero::options::OptionRow::Theme),
        "B came back to SHELF's Theme"
    );
}

#[test]
fn left_and_right_go_round_the_styles_and_each_shows_only_the_rows_it_uses() {
    let mut r = Rig::new();
    r.open();
    for style in [
        BackdropStyle::Plates,
        BackdropStyle::Dotted,
        BackdropStyle::Dither,
        BackdropStyle::Rivets,
        BackdropStyle::Bits,
    ] {
        r.press(Btn::Right);
        assert_eq!(r.a.backdrop().style, style);
        assert_eq!(
            r.rows(),
            [
                choice("Background", style.label()),
                choice("Motion", "Moving"),
                choice("Colour", "Night"),
                choice("Font", "Bold")
            ],
            "{style:?} lost a row"
        );
    }
    r.press(Btn::Right);
    assert_eq!(r.a.backdrop().style, BackdropStyle::Solid);
    assert_eq!(
        r.rows(),
        [
            choice("Background", "Solid"),
            choice("Colour", "Night"),
            choice("Font", "Bold")
        ],
        "a plain colour offered to move"
    );
    r.press(Btn::Right);
    assert_eq!(
        r.a.backdrop().style,
        BackdropStyle::Grid,
        "a card with no wallpaper offered one"
    );
    r.press(Btn::Left);
    assert_eq!(r.a.backdrop().style, BackdropStyle::Solid);
    r.a.set_wallpaper(TexId::from_raw(900));
    r.press(Btn::Right);
    assert_eq!(r.a.backdrop().style, BackdropStyle::Wallpaper);
    assert_eq!(
        r.rows(),
        [choice("Background", "Wallpaper"), choice("Font", "Bold")],
        "a picture offered a colour"
    );
}

#[test]
fn motion_and_colour_change_and_every_change_is_kept_in_app_state() {
    let mut r = Rig::new();
    r.open();
    r.press(Btn::Down);
    assert_eq!(r.sel(), Sel::Motion);
    r.press(Btn::Right);
    assert!(!r.a.backdrop().moving);
    r.press(Btn::Down);
    assert_eq!(r.sel(), Sel::Colour);
    r.press(Btn::Left);
    assert_eq!(r.a.backdrop().colour, 7, "the colours did not wrap");
    r.press(Btn::A);
    r.press(Btn::A);
    assert_eq!(r.a.backdrop().colour, 1);
    let kept = read_app_state(r.d.path());
    assert_eq!(
        (kept.backdrop, kept.backdrop_moving, kept.backdrop_colour),
        (Some(BackdropStyle::Grid), false, 1)
    );
    r.press(Btn::Down);
    assert_eq!(r.sel(), Sel::Font);
    r.press(Btn::Down);
    assert_eq!(r.sel(), Sel::Style, "the rows did not wrap");
}

#[test]
fn the_cursor_leaves_a_row_its_style_no_longer_has() {
    let mut r = Rig::new();
    r.open();
    r.press(Btn::Up);
    r.press(Btn::Up);
    assert_eq!(r.sel(), Sel::Colour);
    r.press(Btn::Up);
    r.press(Btn::Up);
    assert_eq!(r.sel(), Sel::Style);
    r.press(Btn::Left);
    assert_eq!(r.a.backdrop().style, BackdropStyle::Solid);
    r.press(Btn::Down);
    assert_eq!(r.sel(), Sel::Colour, "Solid has no Motion row to land on");
}

fn pattern_x(out: &[Draw], tex: TexId) -> Option<f32> {
    out.iter().find_map(|d| match *d {
        Draw::Tex { x, tex: t, .. } if t == tex => Some(x),
        _ => None,
    })
}

#[test]
fn the_shelf_drifts_its_pattern_by_the_clock_and_holds_it_still_when_asked() {
    let mut r = Rig::new();
    let face = TexId::from_raw(700);
    let w = out_w() + backdrop_period(BackdropStyle::Grid);
    r.a.set_backdrop_face(face, w, BackdropStyle::Grid, 0);
    assert!(matches!(r.a.phase(), Phase::Shelf));
    let shift = |r: &Rig| backdrop_shift(BackdropStyle::Grid, true, r.a.now()) as f32;
    let x0 = pattern_x(&r.frame(), face).expect("the shelf has no pattern under it");
    assert_eq!(x0, -shift(&r));
    r.tick(1_000);
    let x1 = pattern_x(&r.frame(), face).unwrap();
    assert_eq!(x1, -shift(&r));
    assert_ne!(x0, x1, "a second went by and the grid did not move");

    let mut still = Rig::with(AppState {
        backdrop: Some(BackdropStyle::Grid),
        backdrop_moving: false,
        ..AppState::default()
    });
    still.a.set_backdrop_face(face, w, BackdropStyle::Grid, 0);
    still.tick(5_000);
    assert_eq!(pattern_x(&still.frame(), face), Some(0.0));
}

#[test]
fn a_pattern_made_for_another_colour_is_not_shown_while_the_new_one_is_made() {
    let mut r = Rig::with(AppState {
        backdrop: Some(BackdropStyle::Grid),
        backdrop_colour: 2,
        ..AppState::default()
    });
    let face = TexId::from_raw(700);
    r.a.set_backdrop_face(face, out_w() + 40, BackdropStyle::Grid, 0);
    let out = r.frame();
    assert_eq!(pattern_x(&out, face), None, "a stale pattern was drawn");
    let [cr, cg, cb] = backdrop_rgb(2);
    let solid = [cr as f32 / 255.0, cg as f32 / 255.0, cb as f32 / 255.0, 1.0];
    assert!(
        out.iter()
            .any(|d| matches!(*d, Draw::Rect { w, h, colour, .. }
            if w == out_w() as f32 && h == out_h() as f32 && colour == solid)),
        "nothing stood in for it"
    );
    assert_eq!(r.a.backdrop_pattern(), Some((BackdropStyle::Grid, 2)));
}

#[test]
fn the_background_screen_shows_the_background_it_is_choosing_behind_its_list() {
    let mut r = Rig::new();
    let face = TexId::from_raw(700);
    r.a.set_backdrop_face(face, out_w() + 40, BackdropStyle::Grid, 0);
    r.open();
    r.a.finish_slide();
    let out = r.frame();
    let under = out
        .iter()
        .position(|d| matches!(*d, Draw::Tex { tex, .. } if tex == face))
        .expect("the list hid what it is choosing");
    let veiled = out[under + 1..].iter().any(
        |d| matches!(*d, Draw::Rect { w, h, .. } if w == out_w() as f32 && h == out_h() as f32),
    );
    assert!(
        !veiled,
        "something was laid over the background it is choosing"
    );
}

#[test]
fn a_row_a_style_has_no_use_for_stays_faint_so_the_panel_keeps_its_size() {
    let mut r = Rig::new();
    r.open();
    while r.a.backdrop().style != BackdropStyle::Solid {
        r.press(Btn::Right);
    }
    let view = r.a.background_view().expect("the list is showing");
    let dim: Vec<bool> = view.rows.iter().map(|row| row.dim).collect();
    assert_eq!(
        dim,
        [false, true, false, false],
        "Solid's Motion is faint, not gone"
    );
}

#[test]
fn left_and_right_switch_the_font_and_the_bar_takes_it() {
    let mut r = Rig::new();
    r.open();
    assert_eq!(
        r.a.font(),
        BarFont::Bold,
        "a card that never picked is bold"
    );
    r.press(Btn::Up);
    assert_eq!(r.sel(), Sel::Font, "Up from the top did not wrap to FONT");
    r.press(Btn::Right);
    assert_eq!(r.a.font(), BarFont::Thin);
    assert_eq!(r.rows()[3], choice("Font", "Thin"));
    r.press(Btn::Right);
    assert_eq!(r.a.font(), BarFont::Bold, "Right did not go round the two");
    r.press(Btn::Left);
    assert_eq!(r.a.font(), BarFont::Thin);
    r.press(Btn::Left);
    assert_eq!(r.a.font(), BarFont::Bold);
    r.press(Btn::A);
    assert_eq!(r.a.font(), BarFont::Thin, "A did not change it");
    assert_eq!(
        r.a.backdrop(),
        Backdrop {
            style: BackdropStyle::Grid,
            moving: true,
            colour: 0
        },
        "the font changed the background"
    );
}

#[test]
fn the_font_is_kept_in_app_state_and_comes_back_after_a_reboot() {
    let mut r = Rig::new();
    r.open();
    r.press(Btn::Up);
    r.press(Btn::Right);
    assert_eq!(read_app_state(r.d.path()).font, Font::Thin);
    let (mut again, _) = app_booting_at(r.d.path(), CLOCK_IS_SET);
    again.tick_ms(16);
    assert_eq!(again.font(), BarFont::Thin, "the reboot lost the font");
    let mut r = Rig { a: again, ..r };
    r.open();
    assert_eq!(r.rows()[3], choice("Font", "Thin"));
    r.press(Btn::Up);
    r.press(Btn::Left);
    assert_eq!(read_app_state(r.d.path()).font, Font::Bold);
}

#[test]
fn the_font_is_never_faint_and_the_cursor_always_reaches_it() {
    let mut r = Rig::new();
    r.a.set_wallpaper(TexId::from_raw(900));
    r.open();
    let mut seen = Vec::new();
    for _ in BackdropStyle::ALL {
        let style = r.a.backdrop().style;
        seen.push(style);
        let view = r.a.background_view().expect("the list is showing");
        assert_eq!(view.rows.len(), 4, "{style:?}");
        assert!(!view.rows[3].dim, "{style:?} made the font faint");
        r.press(Btn::Up);
        assert_eq!(r.sel(), Sel::Font, "{style:?} kept the cursor off FONT");
        r.press(Btn::Down);
        assert_eq!(r.sel(), Sel::Style);
        r.press(Btn::Right);
    }
    seen.sort_by_key(|style| BackdropStyle::ALL.iter().position(|s| s == style));
    assert_eq!(seen, BackdropStyle::ALL, "a style was left out");
}

#[test]
fn up_and_down_go_round_all_four_rows() {
    let mut r = Rig::new();
    r.open();
    let mut down = vec![r.sel()];
    for _ in 0..4 {
        r.press(Btn::Down);
        down.push(r.sel());
    }
    assert_eq!(
        down,
        [Sel::Style, Sel::Motion, Sel::Colour, Sel::Font, Sel::Style]
    );
    let mut up = vec![r.sel()];
    for _ in 0..4 {
        r.press(Btn::Up);
        up.push(r.sel());
    }
    assert_eq!(
        up,
        [Sel::Style, Sel::Font, Sel::Colour, Sel::Motion, Sel::Style]
    );
    assert_eq!(
        r.a.background_view().expect("the list is showing").lit,
        Some(0)
    );
}
