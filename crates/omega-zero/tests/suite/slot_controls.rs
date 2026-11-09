//! The buttons as Slot has them: on the shelf MENU opens Settings, START picks the core and Y
//! stars the cart onto FAVORITES; in a game a tap of MENU opens the MENU panel over it, and
//! SELECT+A and SELECT+B step the screen filter.

use crate::common;

use common::{app_playing_in, app_playing_with, boot, tmp_root_with_carts, CoreSnapshot};
use omega_input::{Action, Btn};
use omega_store::{read_app_state, ScreenFilter};
use omega_zero::app::{App, Phase};
use omega_zero::pause_menu::{PausePage, PauseRow, MENU_REVEAL_MS, UNDO_GRACE_MS};

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn favorites_file(d: &tempfile::TempDir) -> String {
    std::fs::read_to_string(d.path().join("Config/favorites.txt")).unwrap_or_default()
}

#[test]
fn menu_opens_settings_on_the_shelf_and_start_picks_the_core() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    a.apply(Action::QuickMenu);
    assert!(
        matches!(a.phase(), Phase::Settings { .. }),
        "{:?}",
        a.phase()
    );
    a.apply(Action::QuickMenu);
    a.finish_slide();
    assert!(matches!(a.phase(), Phase::Shelf), "MENU did not close it");

    press(&mut a, Btn::Start);
    assert!(a.core_picker().is_some(), "START opened no core picker");
    assert!(matches!(a.phase(), Phase::Shelf), "START left the shelf");
}

#[test]
fn y_stars_the_cart_onto_favorites_first_and_y_again_takes_it_off() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    assert_eq!(a.pill_text().as_deref(), Some("GBA"), "no favorites yet");
    press(&mut a, Btn::Right);
    assert_eq!(a.selected_stem(), Some("Fusion"));
    press(&mut a, Btn::Y);
    assert_eq!(favorites_file(&d), "gba/Fusion\n");

    // FAVORITES is the shelf before the GBA's
    press(&mut a, Btn::L1);
    assert!(a.on_favorites());
    assert_eq!(a.pill_text().as_deref(), Some("FAVORITES"));
    assert_eq!(a.selected_stem(), Some("Fusion"));

    // taking the last star off leaves FAVORITES empty, and the shelf moves on
    press(&mut a, Btn::Y);
    assert_eq!(favorites_file(&d), "");
    assert!(!a.on_favorites(), "still on an empty FAVORITES");
    assert_eq!(a.pill_text().as_deref(), Some("GBA"));
}

#[test]
fn a_card_from_slot_boots_on_its_favorites() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion", "Zero"]);
    std::fs::write(
        d.path().join("Config/favorites.txt"),
        "gba/Zero\r\ngba/Emerald\ngbc/Not On This Card\n",
    )
    .unwrap();
    let mut a = boot(d.path());
    assert!(a.on_favorites());
    assert_eq!(a.selected_stem(), Some("Emerald"), "not in name order");
    press(&mut a, Btn::Right);
    assert_eq!(a.selected_stem(), Some("Zero"));
    press(&mut a, Btn::Right);
    assert_eq!(
        a.selected_stem(),
        Some("Emerald"),
        "a missing cart was shelved"
    );
}

/// A tap of MENU, and the moment the menu takes to come up after it.
fn open_menu(a: &mut App) {
    let now = a.now();
    a.apply(Action::QuickMenu);
    a.tick_ms(now + MENU_REVEAL_MS);
}

fn lit(a: &App) -> Option<PauseRow> {
    a.pause_view().map(|v| v.lines[v.lit].row)
}

fn rows(a: &App) -> Vec<PauseRow> {
    a.pause_view()
        .map(|v| v.lines.iter().map(|l| l.row).collect())
        .unwrap_or_default()
}

/// Moves the cursor down to `row` on the page shown.
fn down_to(a: &mut App, row: PauseRow) {
    for _ in 0..12 {
        if lit(a) == Some(row) {
            return;
        }
        press(a, Btn::Down);
    }
    panic!("Down never reached {row:?}: {:?}", rows(a));
}

#[test]
fn a_tap_of_menu_in_a_game_opens_slots_menu_and_holds_the_game() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    let now = a.now();
    a.apply(Action::QuickMenu);
    assert!(a.game_menu_open(), "the game is not held under the menu");
    assert!(
        a.pause_view().is_none(),
        "the menu came up before a double tap could land"
    );
    a.tick_ms(now + MENU_REVEAL_MS);
    let view = a.pause_view().expect("the menu never came up");
    assert_eq!(view.title, "MENU");
    // no state saved, nothing to undo: Resume, Settings, Restart and Exit Game
    assert_eq!(
        rows(&a),
        [
            PauseRow::Resume,
            PauseRow::Settings,
            PauseRow::Restart,
            PauseRow::ExitGame
        ]
    );
    // B, or MENU again, goes back to the game
    press(&mut a, Btn::B);
    assert!(!a.pause_menu_open());
    open_menu(&mut a);
    a.apply(Action::QuickMenu);
    assert!(!a.pause_menu_open());
}

#[test]
fn settings_opens_screen_gameplay_and_controls_and_b_steps_back() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    open_menu(&mut a);
    down_to(&mut a, PauseRow::Settings);
    press(&mut a, Btn::A);
    assert_eq!(a.pause_view().unwrap().title, "SETTINGS");
    assert_eq!(
        rows(&a),
        [
            PauseRow::Screen,
            PauseRow::Gameplay,
            PauseRow::Achievements,
            PauseRow::Controls,
            PauseRow::Wifi,
            PauseRow::Bluetooth
        ]
    );
    press(&mut a, Btn::A);
    assert_eq!(a.pause_view().unwrap().title, "SCREEN");
    assert_eq!(
        rows(&a),
        [
            PauseRow::ScreenFilter,
            PauseRow::ColourCorrection,
            PauseRow::Blending,
            PauseRow::Brightness,
            PauseRow::BlueLight
        ],
        "a GBA game has no Game Boy palette"
    );
    press(&mut a, Btn::B);
    assert_eq!(lit(&a), Some(PauseRow::Screen), "B came back to its row");
    down_to(&mut a, PauseRow::Controls);
    press(&mut a, Btn::A);
    assert_eq!(a.pause_page(), Some(PausePage::Controls));
    assert!(a.controls_view().is_some(), "no CONTROLS to show");
    press(&mut a, Btn::B);
    press(&mut a, Btn::B);
    assert_eq!(a.pause_view().unwrap().title, "MENU");
    press(&mut a, Btn::B);
    assert!(!a.pause_menu_open());
}

#[test]
fn the_screen_page_changes_the_filter_in_place_and_keeps_it() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    open_menu(&mut a);
    down_to(&mut a, PauseRow::Settings);
    press(&mut a, Btn::A);
    press(&mut a, Btn::A);
    assert_eq!(lit(&a), Some(PauseRow::ScreenFilter));
    assert_eq!(a.screen_filter(), ScreenFilter::Lcd);
    press(&mut a, Btn::Right);
    assert_eq!(a.screen_filter(), ScreenFilter::Grid);
    press(&mut a, Btn::Right);
    assert_eq!(a.screen_filter(), ScreenFilter::Off);
    assert_eq!(read_app_state(d.path()).screen_filter, ScreenFilter::Off);
    let view = a.pause_view().unwrap();
    assert_eq!(view.lines[view.lit].value.as_deref(), Some("OFF"));
    assert!(a.pause_menu_open(), "changing a value closed the menu");
}

#[test]
fn exit_game_ejects_the_cart() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    open_menu(&mut a);
    press(&mut a, Btn::Up);
    assert_eq!(lit(&a), Some(PauseRow::ExitGame), "Up wraps to the end");
    press(&mut a, Btn::A);
    assert!(!a.pause_menu_open());
    assert!(
        matches!(a.phase(), Phase::Ejecting { .. }),
        "{:?}",
        a.phase()
    );
}

#[test]
fn a_second_tap_of_menu_opens_the_save_states_as_before() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    // the switcher opens only with a state to show
    a.apply(Action::SaveState);
    a.apply(Action::QuickMenu);
    a.apply(Action::Polaroids);
    assert!(!a.pause_menu_open());
    assert!(
        matches!(a.phase(), Phase::Polaroids { .. }),
        "{:?}",
        a.phase()
    );
}

/// A save deleted from the switcher is gone for Undo Save too: there is nothing to take back.
#[test]
fn deleting_the_save_just_made_takes_its_undo_with_it() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::SaveState);
    a.apply(Action::Polaroids);
    a.apply(Action::GbaDown(Btn::X));
    a.apply(Action::GbaDown(Btn::A));
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    open_menu(&mut a);
    assert!(
        !rows(&a).contains(&PauseRow::UndoSave),
        "Undo Save still offers to take back a save that is gone"
    );
}

#[test]
fn undo_save_takes_the_state_just_saved_back_for_thirty_seconds() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    let ring = omega_store::StateRing::new(
        d.path(),
        omega_store::Platform::Gba,
        omega_store::Core::Mgba,
        "Emerald",
    );
    a.apply(Action::SaveState);
    assert_eq!(ring.list().unwrap().len(), 1);
    open_menu(&mut a);
    assert_eq!(
        rows(&a),
        [
            PauseRow::Resume,
            PauseRow::UndoSave,
            PauseRow::States,
            PauseRow::Settings,
            PauseRow::Restart,
            PauseRow::ExitGame
        ]
    );
    down_to(&mut a, PauseRow::UndoSave);
    press(&mut a, Btn::A);
    assert!(!a.pause_menu_open());
    assert!(
        ring.list().unwrap().is_empty(),
        "the save was not taken back"
    );

    a.apply(Action::SaveState);
    let later = a.now() + UNDO_GRACE_MS + 1;
    a.tick_ms(later);
    open_menu(&mut a);
    assert!(
        !rows(&a).contains(&PauseRow::UndoSave),
        "Undo Save outlived its 30 seconds"
    );
}

#[test]
fn undo_load_offers_itself_after_a_load() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::SaveState);
    a.apply(Action::LoadState);
    open_menu(&mut a);
    assert!(rows(&a).contains(&PauseRow::UndoLoad), "{:?}", rows(&a));
    assert!(
        !rows(&a).contains(&PauseRow::UndoSave),
        "the load replaced the save's undo"
    );
    down_to(&mut a, PauseRow::UndoLoad);
    press(&mut a, Btn::A);
    assert!(!a.pause_menu_open());
    open_menu(&mut a);
    assert!(
        !rows(&a).contains(&PauseRow::UndoLoad),
        "undone once, it is gone"
    );
}

#[test]
fn select_a_and_b_step_the_screen_filter_in_a_game() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::FilterNext);
    assert_eq!(a.screen_filter(), ScreenFilter::Grid);
    a.apply(Action::FilterPrev);
    a.apply(Action::FilterPrev);
    assert_eq!(a.screen_filter(), ScreenFilter::Off);
    assert_eq!(read_app_state(d.path()).screen_filter, ScreenFilter::Off);
}

fn booted_with(d: &tempfile::TempDir, change: impl FnOnce(&mut omega_store::AppState)) -> App {
    let mut s = omega_store::AppState {
        clock_set: true,
        ..Default::default()
    };
    change(&mut s);
    omega_store::write_app_state(d.path(), &s).unwrap();
    App::boot(d.path())
}

#[test]
fn holding_power_powers_off_with_no_menu_between() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    a.apply(Action::PowerPress);
    a.apply(Action::PowerHold);
    assert_eq!(a.power_menu(), None, "the POWER menu opened");
    assert!(a.powering_off(), "holding POWER did not power off");
}

#[test]
fn with_load_state_on_insert_off_a_starts_the_game_afresh() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = booted_with(&d, |s| s.auto_load = false);
    press(&mut a, Btn::A);
    assert!(
        matches!(a.phase(), Phase::Inserting { .. }),
        "{:?}",
        a.phase()
    );
    assert!(a.starting_clean(), "the state was loaded anyway");

    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    press(&mut a, Btn::A);
    assert!(
        !a.starting_clean(),
        "on by default, A picks up where it left off"
    );
}

#[test]
fn fast_forward_off_leaves_r2_doing_nothing() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    assert!(a.may_fast_forward());
    let mut s = read_app_state(d.path());
    s.ff_speed = 0;
    omega_store::write_app_state(d.path(), &s).unwrap();
    let a2 = App::boot(d.path());
    assert!(!a2.may_fast_forward());
    a.apply(Action::FfStart);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn recents_stands_first_with_the_last_game_put_in() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = booted_with(&d, |s| s.show_recents = true);
    assert_ne!(
        a.pill_text().as_deref(),
        Some("RECENTS"),
        "nothing played yet"
    );
    press(&mut a, Btn::Right);
    press(&mut a, Btn::A);
    let file = std::fs::read_to_string(d.path().join("Config/recents.txt")).unwrap();
    assert_eq!(file, "gba/Fusion\n");
    let a = booted_with(&d, |s| s.show_recents = true);
    assert_eq!(a.pill_text().as_deref(), Some("RECENTS"));
    assert_eq!(a.selected_stem(), Some("Fusion"));
    let hidden = booted_with(&d, |_| {});
    assert_ne!(
        hidden.pill_text().as_deref(),
        Some("RECENTS"),
        "shown while hidden"
    );
}

#[test]
fn game_titles_name_the_picked_cart_only_while_shown() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    assert_eq!(boot(d.path()).shelf_title(), None);
    let a = booted_with(&d, |s| s.show_titles = true);
    assert_eq!(a.shelf_title().as_deref(), Some("Emerald"));
}

#[test]
fn a_boot_on_favorites_draws_its_labels_first_and_no_cart_twice() {
    let d = tmp_root_with_carts(&["Advance Wars", "Emerald", "Fusion", "Zero"]);
    std::fs::write(d.path().join("Config/favorites.txt"), "gba/Zero\n").unwrap();
    let a = boot(d.path());
    assert!(a.on_favorites());
    let order: Vec<&str> = a
        .carts_nearest_first()
        .into_iter()
        .map(|c| c.stem.as_str())
        .collect();
    assert_eq!(
        order[0], "Zero",
        "FAVORITES, in view, is not drawn first: {order:?}"
    );
    assert_eq!(order.len(), 4, "a favorite was drawn twice: {order:?}");
    assert_eq!(
        a.carts_in_view()
            .into_iter()
            .map(|(_, s)| s)
            .collect::<Vec<_>>(),
        ["Zero"],
        "the carts on screen are not FAVORITES' own"
    );
}

/// Settings > RetroAchievements > Account, on the keyboard: the user name, then the password,
/// kept on the card for the sign-in, which turns RetroAchievements on; Y signs out and forgets
/// them.
#[test]
fn account_signs_in_to_retroachievements_on_the_keyboard_and_y_signs_out() {
    // no request may leave the test: the sign-in is answered as a network error
    omega_cheevos::go_offline();
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    a.apply(Action::QuickMenu);
    for _ in 0..5 {
        press(&mut a, Btn::Down);
    }
    assert_eq!(a.settings_row(), Some(omega_ui::SettingsRow::Achievements));
    press(&mut a, Btn::A);
    a.finish_slide();
    let lit = |a: &App| a.options_view().map(|v| v.lines[v.lit].0);
    let down_to = |a: &mut App, row| {
        for _ in 0..12 {
            if lit(a) == Some(row) {
                return;
            }
            press(a, Btn::Down);
        }
        panic!("Down never reached {row:?}");
    };
    assert_eq!(a.options_view().unwrap().title, "ACHIEVEMENTS");
    down_to(&mut a, omega_zero::options::OptionRow::Account);
    let view = a.options_view().expect("the ACHIEVEMENTS panel");
    assert_eq!(view.lines[view.lit].2, "SIGN IN");

    press(&mut a, Btn::A);
    let prompt = |a: &App| a.typing().map(|t| t.prompt);
    assert_eq!(prompt(&a).as_deref(), Some("RetroAchievements user name"));
    press(&mut a, Btn::A);
    press(&mut a, Btn::A);
    let user = a.typing().unwrap().keys.text.clone();
    assert_eq!(user.chars().count(), 2, "{user:?}");
    press(&mut a, Btn::Start);
    assert_eq!(
        prompt(&a),
        Some(format!("RetroAchievements password for {user}"))
    );
    press(&mut a, Btn::A);
    let password = a.typing().unwrap().keys.text.clone();
    press(&mut a, Btn::Start);
    assert!(a.typing().is_none(), "the keyboard stayed up");

    let file = d.path().join("Config/retroachievements.ini");
    let text = std::fs::read_to_string(&file).expect("the account was not kept");
    assert!(text.contains(&format!("username = {user}")), "{text}");
    assert!(text.contains(&format!("password = {password}")), "{text}");
    assert!(
        read_app_state(d.path()).cheevos,
        "signing in left achievements off"
    );
    assert!(a.achievements_account());

    // no network: the sign-in is tried again, after a wait, rather than given up
    let now = a.now();
    a.tick_ms(now + 100);
    a.tick_ms(now + 200);
    assert_eq!(a.achievements_note(), "No network yet: trying again");
    a.tick_ms(now + 200 + omega_zero::achievements::RETRY_FIRST_MS);
    assert_eq!(
        a.achievements_note(),
        "Signing in…",
        "the sign-in was not tried again"
    );

    press(&mut a, Btn::Y);
    assert!(!file.exists(), "signing out kept the account");
    assert!(!a.achievements_account());
}

/// RetroAchievements stands in CONFIGS under Labels, a row with only its ›, and opens its own
/// panel; B comes back to it. GAMEPLAY holds none of it.
#[test]
fn retroachievements_opens_from_configs_under_labels() {
    use omega_ui::SettingsRow;
    use omega_zero::options::OptionRow;
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    a.apply(Action::QuickMenu);
    for _ in 0..4 {
        press(&mut a, Btn::Down);
    }
    assert_eq!(a.settings_row(), Some(SettingsRow::Labels));
    press(&mut a, Btn::Down);
    assert_eq!(a.settings_row(), Some(SettingsRow::Achievements));
    assert_eq!(
        a.settings_text(SettingsRow::Achievements),
        None,
        "no ON or OFF"
    );
    assert!(SettingsRow::Achievements.opens());
    press(&mut a, Btn::A);
    a.finish_slide();
    let view = a.options_view().expect("the ACHIEVEMENTS panel");
    assert_eq!(view.title, "ACHIEVEMENTS");
    assert_eq!(
        view.lines.iter().map(|(r, _, _)| *r).collect::<Vec<_>>(),
        [OptionRow::Cheevos, OptionRow::Account, OptionRow::Hardcore]
    );
    press(&mut a, Btn::B);
    a.finish_slide();
    assert_eq!(a.settings_row(), Some(SettingsRow::Achievements));
    assert!(
        !omega_zero::options::OptionsPage::Gameplay
            .rows()
            .iter()
            .any(|r| matches!(
                r,
                OptionRow::Cheevos | OptionRow::Account | OptionRow::Hardcore
            )),
        "achievements stayed on GAMEPLAY"
    );
}

/// GAMEPLAY's last row, Emulators, opens EMULATORS: each system's default core, which the
/// games not switched by hand follow, and how many were switched, which A puts back.
#[test]
fn emulators_set_each_systems_default_and_put_switched_games_back() {
    use omega_store::{core_for_platform, write_selected_core, Core, Platform};
    use omega_zero::options::OptionRow;
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_selected_core(d.path(), "Emerald", Core::Gpsp).unwrap();
    let mut a = boot(d.path());
    a.apply(Action::QuickMenu);
    for _ in 0..2 {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    a.finish_slide();
    let view = a.options_view().expect("the GAMEPLAY panel");
    let (last, _, value) = view.lines.last().unwrap().clone();
    assert_eq!(
        (last, value.as_str()),
        (OptionRow::Emulators, ""),
        "only a ›"
    );
    press(&mut a, Btn::Up);
    press(&mut a, Btn::A);
    a.finish_slide();
    let view = a.options_view().expect("the EMULATORS panel");
    assert_eq!(view.title, "EMULATORS");
    let said: Vec<(OptionRow, String)> =
        view.lines.iter().map(|(r, _, v)| (*r, v.clone())).collect();
    assert_eq!(
        said,
        [
            (OptionRow::DefaultCore(Platform::Gba), "MGBA".to_string()),
            (OptionRow::Switched(Platform::Gba), "1".to_string()),
            (OptionRow::DefaultCore(Platform::Snes), "SNES9X".to_string()),
            (OptionRow::Switched(Platform::Snes), "NONE".to_string()),
        ]
    );

    // the default moves the games not switched, and leaves the switched one where it is
    press(&mut a, Btn::Right);
    assert_eq!(a.option_text(OptionRow::DefaultCore(Platform::Gba)), "GPSP");
    assert_eq!(
        core_for_platform(d.path(), "Fusion", Platform::Gba),
        Core::Gpsp
    );
    assert_eq!(a.option_text(OptionRow::Switched(Platform::Gba)), "NONE");
    press(&mut a, Btn::Left);
    assert_eq!(
        core_for_platform(d.path(), "Fusion", Platform::Gba),
        Core::Mgba
    );
    assert_eq!(
        core_for_platform(d.path(), "Emerald", Platform::Gba),
        Core::Gpsp
    );

    // A on Switched puts Emerald back on the default
    press(&mut a, Btn::Down);
    assert_eq!(a.option_text(OptionRow::Switched(Platform::Gba)), "1");
    press(&mut a, Btn::A);
    assert_eq!(a.option_text(OptionRow::Switched(Platform::Gba)), "NONE");
    assert_eq!(
        core_for_platform(d.path(), "Emerald", Platform::Gba),
        Core::Mgba
    );
    assert!(omega_store::read_selected_cores(d.path()).is_empty());

    press(&mut a, Btn::B);
    a.finish_slide();
    let view = a.options_view().expect("B left EMULATORS");
    assert_eq!(view.title, "GAMEPLAY");
    assert_eq!(view.lines[view.lit].0, OptionRow::Emulators);
}

/// Settings > Achievements in a game: whether the game was found, and the switches; with
/// RetroAchievements off there is nothing to count or list.
#[test]
fn the_in_game_achievements_page_says_whether_the_game_was_found() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    open_menu(&mut a);
    down_to(&mut a, PauseRow::Settings);
    press(&mut a, Btn::A);
    down_to(&mut a, PauseRow::Achievements);
    press(&mut a, Btn::A);
    let view = a.pause_view().unwrap();
    assert_eq!(view.title, "ACHIEVEMENTS");
    assert_eq!(
        rows(&a),
        [PauseRow::Game, PauseRow::Cheevos, PauseRow::Hardcore],
        "off, there is no progress or list to show"
    );
    assert_eq!(view.lines[0].value.as_deref(), Some("OFF"));
    assert!(!view.lines[0].carets, "the Game row says, it does not step");
    assert_eq!(view.note, "Switch RetroAchievements on below");
    press(&mut a, Btn::B);
    assert_eq!(
        lit(&a),
        Some(PauseRow::Achievements),
        "B came back to its row"
    );
}

/// Restart starts the game over from power on, and says so.
#[test]
fn restart_starts_the_game_over() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let core = CoreSnapshot::new();
    let mut a = app_playing_with(d.path(), "Emerald", core.boxed());
    for _ in 0..5 {
        core.run_frame();
    }
    assert_ne!(core.bytes(), 0u64.to_le_bytes(), "the game never ran");
    open_menu(&mut a);
    down_to(&mut a, PauseRow::Restart);
    press(&mut a, Btn::A);
    assert!(!a.pause_menu_open(), "the menu stayed up");
    assert_eq!(
        core.bytes(),
        0u64.to_le_bytes(),
        "the game did not start over"
    );
    assert_eq!(a.toast(), Some(omega_ui::Toast::Restarted));
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
}

/// GAMEPLAY in a game on gpSP has Run-Ahead, as Settings > Gameplay does: switched there, it is
/// on for the game at once, and kept.
#[test]
fn run_ahead_is_switched_from_the_game_menu() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    // as the session says once it has opened the game's core
    a.set_core(omega_store::Core::Gpsp);
    open_menu(&mut a);
    down_to(&mut a, PauseRow::Settings);
    press(&mut a, Btn::A);
    down_to(&mut a, PauseRow::Gameplay);
    press(&mut a, Btn::A);
    assert_eq!(a.pause_view().unwrap().title, "GAMEPLAY");
    down_to(&mut a, PauseRow::RunAhead);
    let shown = |a: &App| {
        let view = a.pause_view().unwrap();
        view.lines[view.lit].value.clone()
    };
    assert_eq!(shown(&a).as_deref(), Some("OFF"));
    press(&mut a, Btn::Right);
    assert_eq!(shown(&a).as_deref(), Some("ON"));
    assert!(a.run_ahead_on(), "the game was not switched to run ahead");
    assert!(read_app_state(d.path()).run_ahead, "it was not kept");
    assert!(a.pause_menu_open(), "changing a value closed the menu");
}

/// Run-Ahead is gpSP's alone: a game on mGBA has no Run-Ahead row under GAMEPLAY.
#[test]
fn a_game_on_mgba_has_no_run_ahead_row() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    assert_eq!(a.core(), omega_store::Core::Mgba);
    open_menu(&mut a);
    down_to(&mut a, PauseRow::Settings);
    press(&mut a, Btn::A);
    down_to(&mut a, PauseRow::Gameplay);
    press(&mut a, Btn::A);
    assert_eq!(a.pause_view().unwrap().title, "GAMEPLAY");
    assert!(
        !rows(&a).contains(&PauseRow::RunAhead),
        "mGBA has a Run-Ahead row: {:?}",
        rows(&a)
    );
}
