//! Settings' Update takes releases or test builds: Left or Right on its row switches, says so,
//! and keeps it on the card.

use crate::common;

use common::{boot, tmp_root_with_carts};
use omega_input::{Action, Btn};
use omega_store::read_app_state;
use omega_ui::SettingsRow;
use omega_zero::update::Channel;

fn press(a: &mut omega_zero::app::App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

#[test]
fn left_or_right_on_update_switches_between_releases_and_test_builds() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    assert_eq!(a.update_channel(), Channel::Stable);
    a.apply(Action::QuickMenu);
    for _ in 0..16 {
        if a.settings_row() == Some(SettingsRow::Update) {
            break;
        }
        press(&mut a, Btn::Down);
    }
    assert_eq!(a.settings_row(), Some(SettingsRow::Update));

    press(&mut a, Btn::Right);
    assert_eq!(a.update_channel(), Channel::Test);
    assert!(
        read_app_state(d.path()).test_updates,
        "the channel was not kept"
    );
    press(&mut a, Btn::Left);
    assert_eq!(a.update_channel(), Channel::Stable);
    assert!(!read_app_state(d.path()).test_updates);
    assert_eq!(a.settings_row(), Some(SettingsRow::Update), "the row moved");
}
