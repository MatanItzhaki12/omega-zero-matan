//! How long a game is played: counted while it is on screen, not while its menu is up over it,
//! and written to the card as the cart comes out.

use crate::common;

use common::{app_playing_with, tmp_root_with_carts, StubSnapshot};
use omega_input::Action;
use omega_store::Platform;

#[test]
fn time_played_counts_while_the_game_is_on_screen_and_not_under_its_menu() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_with(d.path(), "Emerald", StubSnapshot::boxed());
    let before = a.played(Platform::Gba, "Emerald").map_or(0, |p| p.secs);
    for _ in 0..5 * 60 {
        a.update(1.0 / 60.0);
    }
    let played = a.played(Platform::Gba, "Emerald").expect("no time counted");
    assert!(
        (4..=6).contains(&(played.secs - before)),
        "{} s counted for 5 s played",
        played.secs - before
    );
    assert!(
        played.last.is_some(),
        "when it was last played was not kept"
    );

    // the game's menu up over it: the time stands still
    a.apply(Action::QuickMenu);
    let at_menu = a.played(Platform::Gba, "Emerald").map_or(0, |p| p.secs);
    for _ in 0..3 * 60 {
        a.update(1.0 / 60.0);
    }
    if a.pause_view().is_some() {
        assert_eq!(
            a.played(Platform::Gba, "Emerald").map_or(0, |p| p.secs),
            at_menu,
            "time was counted under the menu"
        );
    }
}

#[test]
fn the_time_is_written_to_the_card_as_the_cart_comes_out() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_with(d.path(), "Emerald", StubSnapshot::boxed());
    for _ in 0..3 * 60 {
        a.update(1.0 / 60.0);
    }
    assert!(
        a.played(Platform::Gba, "Emerald").is_some(),
        "no time counted before the eject"
    );
    a.apply(Action::Eject);
    for _ in 0..600 {
        a.update(1.0 / 60.0);
    }
    assert!(
        matches!(a.phase(), omega_zero::app::Phase::Shelf),
        "{:?}",
        a.phase()
    );
    omega_zero::card_writer::settle();
    let text = std::fs::read_to_string(d.path().join(omega_zero::play_log::FILE))
        .expect("the play log was not written");
    assert!(text.starts_with("gba/Emerald\t"), "{text}");
}

#[test]
fn the_shelf_stands_its_games_by_name_or_the_most_played_first() {
    use omega_input::Btn;
    let d = tmp_root_with_carts(&["Alpha", "Beta", "Gamma"]);
    std::fs::write(
        d.path().join(omega_zero::play_log::FILE),
        "gba/Beta\t100\ngba/Gamma\t500\n",
    )
    .unwrap();
    let mut a = common::boot(d.path());
    assert_eq!(a.shelf_stems(), ["Alpha", "Beta", "Gamma"]);
    assert_eq!(a.selected_stem(), Some("Alpha"));
    a.apply(Action::QuickMenu);
    a.finish_slide();
    common::to_shelf_row(&mut a, omega_zero::options::OptionRow::ShelfSort);
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaUp(Btn::Right));
    assert!(omega_store::read_app_state(d.path()).shelf_by_play);
    assert_eq!(a.shelf_stems(), ["Gamma", "Beta", "Alpha"]);
    assert_eq!(a.selected_stem(), Some("Alpha"), "the cart picked moved");
    a.apply(Action::GbaDown(Btn::Left));
    a.apply(Action::GbaUp(Btn::Left));
    assert_eq!(a.shelf_stems(), ["Alpha", "Beta", "Gamma"]);
    // kept: the next boot stands them the same
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaUp(Btn::Right));
    let again = common::boot(d.path());
    assert_eq!(again.shelf_stems(), ["Gamma", "Beta", "Alpha"]);
}

#[test]
fn the_shelf_stands_again_on_the_card_as_it_changed_and_keeps_the_cart_picked() {
    use omega_input::Btn;
    use omega_ui::TexId;
    let d = tmp_root_with_carts(&["Alpha", "Beta"]);
    let mut a = common::boot(d.path());
    a.set_faces(vec![TexId::from_raw(501), TexId::from_raw(502)]);
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaUp(Btn::Right));
    assert_eq!(a.selected_stem(), Some("Beta"));
    assert!(a.may_restock(), "the shelf may be stood again");

    // a game copied in: on the shelf, on the face drawn for it, the cart picked as it was
    std::fs::write(d.path().join("Games/GBA/Gamma.gba"), [0u8; 512]).unwrap();
    let carts = omega_store::scan(d.path()).unwrap();
    let added = [((Platform::Gba, "Gamma".to_string()), TexId::from_raw(503))].into();
    assert!(a.restock(carts, &added).is_empty(), "nothing went");
    assert_eq!(a.shelf_stems(), ["Alpha", "Beta", "Gamma"]);
    assert_eq!(a.selected_stem(), Some("Beta"));
    assert_eq!(
        a.face_of(Platform::Gba, "Gamma"),
        Some(TexId::from_raw(503))
    );
    assert_eq!(
        a.face_of(Platform::Gba, "Alpha"),
        Some(TexId::from_raw(501))
    );

    // a game taken away: its face handed back for the next game added
    std::fs::remove_file(d.path().join("Games/GBA/Alpha.gba")).unwrap();
    let carts = omega_store::scan(d.path()).unwrap();
    let freed = a.restock(carts, &std::collections::HashMap::new());
    assert_eq!(freed, [TexId::from_raw(501)]);
    assert_eq!(a.shelf_stems(), ["Beta", "Gamma"]);
    assert_eq!(a.selected_stem(), Some("Beta"));

    // a game whose label changed is drawn again
    a.redraw_faces(vec![(Platform::Gba, "Gamma".to_string())]);
    let stale: Vec<String> = a
        .take_stale_faces()
        .into_iter()
        .map(|(c, _)| c.stem)
        .collect();
    assert_eq!(stale, ["Gamma"]);
}
