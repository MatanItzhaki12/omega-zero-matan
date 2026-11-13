use crate::common;

use common::{app_playing_in, app_playing_with, tmp_root_with_carts, StubSnapshot};
use omega_input::{Action, Btn};
use omega_store::{Cart, Core, Platform, StateRing};
use omega_ui::{out_h, out_w, Draw, TOP_BAR_H};
use omega_zero::app::{App, Phase};

fn app_playing(stem: &str) -> App {
    let mut a = App::new(vec![Cart {
        platform: Platform::Gba,
        stem: stem.to_string(),
        rom: format!("Games/GBA/{stem}.gba").into(),
        label: None,
        code: String::new(),
        shell: None,
        title: stem.to_uppercase(),
    }]);
    a.apply(Action::Insert);
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    a
}

#[test]
fn double_tap_menu_with_no_states_does_not_open_the_switcher() {
    let mut a = app_playing("Emerald");
    a.apply(Action::Polaroids);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn a_cart_that_has_never_been_saved_does_not_open_the_switcher() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::Polaroids);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn saving_pushes_a_polaroid_with_its_picture() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::SaveState);
    let entries = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
        .list()
        .expect("list");
    assert_eq!(entries.len(), 1);
    assert_eq!(
        std::fs::read(&entries[0].state).expect("state"),
        [9u8; 1024]
    );
    assert!(
        !std::fs::read(&entries[0].thumb).expect("thumb").is_empty(),
        "the polaroid has no picture"
    );
}

#[test]
fn two_saves_in_the_same_second_are_two_entries() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::SaveState);
    a.apply(Action::SaveState);
    assert_eq!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .list()
            .expect("list")
            .len(),
        2
    );
}

#[test]
fn a_saved_state_opens_the_switcher_and_a_loads_it_back() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (snapshot, loaded) = StubSnapshot::pair();
    let mut a = app_playing_with(d.path(), "Emerald", snapshot);
    a.apply(Action::SaveState);
    a.apply(Action::Polaroids);
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(
        loaded.lock().expect("loaded").as_deref(),
        Some(&[9u8; 1024][..])
    );
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "loading must hand the game back"
    );
}

#[test]
fn b_dismisses_the_switcher_without_loading() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (snapshot, loaded) = StubSnapshot::pair();
    let mut a = app_playing_with(d.path(), "Emerald", snapshot);
    a.apply(Action::SaveState);
    a.apply(Action::Polaroids);
    a.apply(Action::GbaDown(Btn::B));
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert!(loaded.lock().expect("loaded").is_none());
}

#[test]
fn menu_dismisses_the_switcher_it_opened() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::SaveState);
    a.apply(Action::Polaroids);
    a.apply(Action::Polaroids);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn flicking_selects_which_state_a_loads() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    ring.push(&[1u8; 16], b"png", "2026-08-09_00-00-01")
        .expect("push");
    ring.push(&[2u8; 16], b"png", "2026-08-09_00-00-02")
        .expect("push");
    let (snapshot, loaded) = StubSnapshot::pair();
    let mut a = app_playing_with(d.path(), "Emerald", snapshot);
    a.apply(Action::Polaroids);
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(
        loaded.lock().expect("loaded").as_deref(),
        Some(&[1u8; 16][..])
    );
}

#[test]
fn load_state_takes_the_newest_entry_without_the_switcher() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    ring.push(&[1u8; 16], b"png", "2026-08-09_00-00-01")
        .expect("push");
    ring.push(&[2u8; 16], b"png", "2026-08-09_00-00-02")
        .expect("push");
    let (snapshot, loaded) = StubSnapshot::pair();
    let mut a = app_playing_with(d.path(), "Emerald", snapshot);
    a.apply(Action::LoadState);
    assert_eq!(
        loaded.lock().expect("loaded").as_deref(),
        Some(&[2u8; 16][..])
    );
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

/// The save states show the shelf's top bar over their screenshot, in its colours, and nothing
/// see through, once the toast the save put up has gone.
#[test]
fn the_save_states_stand_under_the_shelfs_top_bar() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply_at(Action::SaveState, 1_000);
    a.apply_at(Action::Polaroids, 1_100);
    a.tick_ms(10_000);
    let mut out = Vec::new();
    a.draw(&mut out);
    let housing = a.chrome_colours().housing;
    let photo = out
        .iter()
        .position(|d| matches!(*d, Draw::Rect { y: 0.0, h, .. } if h == out_h() as f32))
        .expect("no screenshot");
    let bar = out
        .iter()
        .position(|d| {
            matches!(*d, Draw::Rect { x: 0.0, y: 0.0, w, h, colour }
                if w == out_w() as f32 && h == TOP_BAR_H - 2.0 && colour == housing)
        })
        .expect("no top bar");
    assert!(photo < bar, "the top bar is under the screenshot");
    assert!(
        out.iter().all(|d| match *d {
            Draw::Rect { colour, .. } => colour[3] == 1.0,
            Draw::Tex { alpha, .. } => alpha == 1.0,
            _ => true,
        }),
        "something over the save states is see through"
    );
}

#[test]
fn the_switcher_swallows_the_shelf_and_eject_bindings() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::SaveState);
    a.apply(Action::Polaroids);
    a.apply(Action::Eject);
    a.apply(Action::Insert);
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
}

/// Two saves, a second apart, in the switcher over Emerald: the newer one shown.
fn two_saves_shown() -> (tempfile::TempDir, App, StateRing) {
    let d = tmp_root_with_carts(&["Emerald"]);
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    ring.push(&[1u8; 16], b"png", "2026-08-09_00-00-01")
        .expect("push");
    ring.push(&[2u8; 16], b"png", "2026-08-09_00-00-02")
        .expect("push");
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::Polaroids);
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
    (d, a, ring)
}

#[test]
fn x_asks_before_deleting_and_b_or_x_keeps_the_save() {
    use omega_zero::screens::bar::Actions;
    let (_d, mut a, ring) = two_saves_shown();
    assert_eq!(a.bar_place().map(|p| p.actions), Some(Actions::Switcher));

    a.apply(Action::GbaDown(Btn::X));
    assert!(a.delete_asked(), "X asked nothing");
    assert_eq!(
        a.bar_place().map(|p| p.actions),
        Some(Actions::Delete),
        "the bar does not say B CANCEL and A DELETE"
    );
    assert!(a.bar_hints(a.phase()).is_empty());
    assert_eq!(
        Actions::Delete.action(omega_ui::kit::action::Side::Left),
        ("CANCEL", "B")
    );
    assert_eq!(
        Actions::Delete.action(omega_ui::kit::action::Side::Right),
        ("DELETE", "A")
    );

    a.apply(Action::GbaDown(Btn::B));
    assert!(!a.delete_asked());
    assert!(
        matches!(a.phase(), Phase::Polaroids { .. }),
        "B left the switcher"
    );
    a.apply(Action::GbaDown(Btn::X));
    a.apply(Action::GbaDown(Btn::X));
    assert!(!a.delete_asked(), "X again kept the question up");
    assert_eq!(ring.list().expect("list").len(), 2, "a save went without A");
}

#[test]
fn a_deletes_the_save_shown_and_the_switcher_shows_the_next() {
    let (_d, mut a, ring) = two_saves_shown();
    a.apply(Action::GbaDown(Btn::X));
    a.apply(Action::GbaDown(Btn::A));
    assert!(!a.delete_asked());
    let left = ring.list().expect("list");
    assert_eq!(
        left.iter().map(|e| e.stamp.as_str()).collect::<Vec<_>>(),
        ["2026-08-09_00-00-01"],
        "the newer save, the one shown, was not the one deleted"
    );
    assert!(!left[0]
        .thumb
        .with_file_name("2026-08-09_00-00-02.png")
        .exists());
    assert_eq!(a.toast(), Some(omega_ui::Toast::StateDeleted));
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
    assert_eq!(a.polaroids().map(omega_ui::Polaroids::dots), Some((1, 0)));

    // the last one gone, the game is back
    a.apply(Action::GbaDown(Btn::X));
    a.apply(Action::GbaDown(Btn::A));
    assert!(ring.list().expect("list").is_empty());
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "the switcher stayed open with nothing in it"
    );
}

/// X DELETE stands on the kit's hint line, as tall as the bar and on its row, in the middle
/// between RETURN and LOAD; it goes while the question is up.
#[test]
fn x_delete_stands_level_with_return_and_load() {
    use omega_gfx::{out_h, out_w};
    use omega_ui::kit::action::{Font, BAR as HINT_LINE_H};
    use omega_ui::kit::palette::palette;
    use omega_zero::screens::delete_state::delete_hint_face;
    use omega_zero::screens::lone::Lone;
    let face = delete_hint_face(Font::Bold, palette(0));
    assert_eq!(face.h, HINT_LINE_H, "X DELETE is not the bar's height");
    let (_d, mut a, _ring) = two_saves_shown();
    a.set_delete_hint(Lone {
        tex: omega_ui::TexId::from_raw(4_242),
        w: face.w,
        h: face.h,
    });
    let px = omega_ui::SPRITE_PX;
    let [x, y] = a.delete_hint_at().expect("a place for the line");
    assert_eq!(
        y + HINT_LINE_H,
        out_h() / px,
        "it does not stand on the bar's row"
    );
    assert_eq!(
        x * 2 + face.w,
        out_w() / px - (out_w() / px - face.w) % 2,
        "not centred"
    );
    let drawn = |a: &App| {
        let mut out = Vec::new();
        a.draw(&mut out);
        out.iter()
            .any(|d| matches!(d, Draw::Tex { tex, .. } if *tex == omega_ui::TexId::from_raw(4_242)))
    };
    assert!(drawn(&a), "X DELETE is not drawn");
    a.apply(Action::GbaDown(Btn::X));
    assert!(!drawn(&a), "X DELETE stayed under the question");
}
