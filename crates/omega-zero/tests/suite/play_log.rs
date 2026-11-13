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
