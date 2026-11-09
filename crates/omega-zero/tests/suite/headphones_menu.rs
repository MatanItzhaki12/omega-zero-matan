//! The HEADPHONES menu, in the game menu and in Settings: what each make of headphones has,
//! and nothing while none are connected.

use crate::common;

use omega_zero::headphones::{galaxy, Battery, Kind, Noise, Phones};
use omega_zero::headphones_menu::HpRow;
use omega_zero::options::{OptionRow, OptionsPage};
use omega_zero::pause_menu::{PausePage, PauseRow};

fn airpods() -> Phones {
    let mut p = Phones::new(Kind::AirPods);
    p.left = Some(Battery {
        level: 80,
        charging: false,
    });
    p.right = Some(Battery {
        level: 75,
        charging: false,
    });
    p.noise = Some(Noise::Cancellation);
    p.conversation = Some(true);
    p
}

#[test]
fn headphones_connected_have_a_page_of_their_own_in_the_game_menu() {
    let d = common::tmp_root_with_carts(&[]);
    let mut app = common::boot(d.path());
    assert!(!app
        .pause_rows(PausePage::Main)
        .contains(&PauseRow::Headphones));
    app.stand_in_headphones("74:77:86:47:EE:5D", airpods());
    assert!(app
        .pause_rows(PausePage::Main)
        .contains(&PauseRow::Headphones));
    assert_eq!(
        app.pause_rows(PausePage::Headphones),
        [
            PauseRow::Hp(HpRow::Noise),
            PauseRow::Hp(HpRow::Conversation),
            PauseRow::Hp(HpRow::Adaptive),
            PauseRow::Hp(HpRow::Battery),
        ]
    );
    assert_eq!(app.headphones_value(HpRow::Noise), "NOISE CANCEL");
    assert_eq!(app.headphones_value(HpRow::Battery), "L 80% R 75%");
    assert_eq!(app.headphones_value(HpRow::Adaptive), "…", "not said yet");
}

#[test]
fn settings_shows_the_same_rows_galaxy_buds_without_apple_s_own() {
    let d = common::tmp_root_with_carts(&[]);
    let mut app = common::boot(d.path());
    app.stand_in_headphones(
        "1A:2B:3C:4D:5E:6F",
        Phones::new(Kind::Buds(galaxy::Model::Buds2Pro)),
    );
    assert_eq!(
        app.page_rows(OptionsPage::Headphones),
        [OptionRow::Hp(HpRow::Noise), OptionRow::Hp(HpRow::Battery)]
    );
}
