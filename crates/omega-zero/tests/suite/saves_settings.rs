//! Settings › Saves as it is used: SAVES lists the systems with games that have anything on the
//! card, and a search finds a game by name; a game has its Save and its Save States, each a
//! panel of its own; and whatever can lose something asks first, B NO and A YES.

use crate::common;

use common::{boot, tmp_root_with_carts};
use omega_input::{Action, Btn};
use omega_store::{Core, Platform, StateRing};
use omega_ui::SettingsRow;
use omega_zero::app::App;
use omega_zero::options::OptionRow;
use omega_zero::save_manager::{self, Game};
use omega_zero::screens::bar::Actions;

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
    a.finish_slide();
}

fn title(a: &App) -> &'static str {
    a.options_view().expect("a panel").title
}

fn lit(a: &App) -> OptionRow {
    let v = a.options_view().expect("a panel");
    v.lines[v.lit].0
}

fn rows(a: &App) -> Vec<(OptionRow, String, String)> {
    a.options_view().expect("a panel").lines
}

fn note(a: &App) -> String {
    a.options_view().expect("a panel").note
}

fn down_to(a: &mut App, row: OptionRow) {
    for _ in 0..24 {
        if lit(a) == row {
            return;
        }
        press(a, Btn::Down);
    }
    panic!("Down never reached {row:?} in {:?}", rows(a));
}

/// Settings > Saves.
fn open_saves(a: &mut App) {
    a.apply(Action::QuickMenu);
    for _ in 0..12 {
        if a.settings_row() == Some(SettingsRow::Backup) {
            break;
        }
        press(a, Btn::Down);
    }
    assert_eq!(a.settings_row(), Some(SettingsRow::Backup));
    press(a, Btn::A);
}

/// Settings > Saves > Game Boy Advance > Emerald.
fn open_emerald(a: &mut App) {
    open_saves(a);
    down_to(a, OptionRow::SavesSystem(Platform::Gba));
    press(a, Btn::A);
    assert_eq!(title(a), "GAMES");
    down_to(a, OptionRow::SavesGame(0));
    press(a, Btn::A);
    assert_eq!(title(a), "GAME");
}

fn emerald() -> Game {
    Game {
        platform: Platform::Gba,
        stem: "Emerald".into(),
        code: String::new(),
    }
}

/// A card with Emerald saved and a resume state for it, and Fusion with nothing.
fn card() -> tempfile::TempDir {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let sav = d.path().join("Saves/GBA/Emerald.sav");
    std::fs::create_dir_all(sav.parent().unwrap()).unwrap();
    std::fs::write(&sav, b"badge eight").unwrap();
    StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
        .write_resume(b"at the league")
        .unwrap();
    d
}

#[test]
fn saves_lists_the_nas_a_search_and_the_systems_with_anything_saved() {
    let d = card();
    let mut a = boot(d.path());
    assert_eq!(SettingsRow::Backup.label(), "Saves");
    open_saves(&mut a);
    assert_eq!(title(&a), "SAVES");
    assert_eq!(
        rows(&a),
        [
            (OptionRow::SavesNas, "NAS Backup".into(), "OFF".into()),
            (OptionRow::SavesSearch, "Search Games".into(), String::new()),
            (
                OptionRow::SavesSystem(Platform::Gba),
                "Game Boy Advance".into(),
                "1 GAME".into()
            ),
        ],
        "Fusion has nothing, and is not counted"
    );
    // a system lists its games, each with its save's size
    down_to(&mut a, OptionRow::SavesSystem(Platform::Gba));
    press(&mut a, Btn::A);
    assert_eq!(
        rows(&a),
        [(OptionRow::SavesGame(0), "Emerald".into(), "11 B".into())]
    );
    // NAS Backup opens SAVE BACKUP, and B comes back to it
    press(&mut a, Btn::B);
    down_to(&mut a, OptionRow::SavesNas);
    press(&mut a, Btn::A);
    assert_eq!(title(&a), "SAVE BACKUP");
    press(&mut a, Btn::B);
    assert_eq!(title(&a), "SAVES");
    assert_eq!(lit(&a), OptionRow::SavesNas);
}

#[test]
fn a_search_finds_a_game_by_part_of_its_name_in_any_case() {
    let d = card();
    let mut a = boot(d.path());
    open_saves(&mut a);
    a.search_typed("MER");
    a.finish_slide();
    assert_eq!(title(&a), "GAMES");
    assert_eq!(rows(&a)[0].1, "Emerald");
    // and says so when none matches
    press(&mut a, Btn::B);
    a.search_typed("zelda");
    a.finish_slide();
    assert_eq!(lit(&a), OptionRow::SavesNoMatch);
    assert!(note(&a).contains("zelda"), "{}", note(&a));
}

#[test]
fn a_game_has_its_save_and_its_states_each_in_a_panel_of_its_own() {
    let d = card();
    let mut a = boot(d.path());
    open_emerald(&mut a);
    assert_eq!(
        rows(&a),
        [
            (OptionRow::GameSaveMenu, "Save".into(), "11 B".into()),
            (OptionRow::GameStatesMenu, "Save States".into(), "1".into()),
        ]
    );
    assert_eq!(note(&a), "Emerald");

    press(&mut a, Btn::A);
    assert_eq!(title(&a), "SAVE");
    assert_eq!(
        rows(&a).iter().map(|r| r.0).collect::<Vec<_>>(),
        [OptionRow::GameSave, OptionRow::GameBackUp]
    );
    assert_eq!(rows(&a)[0].1, "Current Save");
    assert!(note(&a).contains("X deletes it"), "{}", note(&a));

    press(&mut a, Btn::B);
    down_to(&mut a, OptionRow::GameStatesMenu);
    press(&mut a, Btn::A);
    assert_eq!(title(&a), "STATES");
    assert_eq!(rows(&a)[0].1, "Resume Point");
}

#[test]
fn backups_stand_under_their_heading_named_for_when_they_were_made() {
    let d = card();
    let mut a = boot(d.path());
    open_emerald(&mut a);
    press(&mut a, Btn::A);
    down_to(&mut a, OptionRow::GameBackUp);
    press(&mut a, Btn::A);
    assert_eq!(save_manager::backups(d.path(), &emerald()).len(), 1);
    assert_eq!(
        rows(&a).iter().map(|r| r.0).collect::<Vec<_>>(),
        [
            OptionRow::GameSave,
            OptionRow::GameBackUp,
            OptionRow::GameBackupsHeading,
            OptionRow::GameBackup(0),
        ]
    );
    assert_eq!(rows(&a)[2].1, "Backups");
    assert!(rows(&a)[3].1.contains(", "), "{:?}", rows(&a)[3]);
    // the cursor passes over the heading
    press(&mut a, Btn::Down);
    assert_eq!(
        lit(&a),
        OptionRow::GameBackup(0),
        "the cursor stood on the heading"
    );
    assert_eq!(note(&a), "Y restores it, X deletes it");
}

#[test]
fn y_restores_a_backup_after_asking_and_keeps_the_save_it_replaced() {
    let d = card();
    let mut a = boot(d.path());
    open_emerald(&mut a);
    press(&mut a, Btn::A);
    down_to(&mut a, OptionRow::GameBackUp);
    press(&mut a, Btn::A);
    let sav = d.path().join("Saves/GBA/Emerald.sav");
    std::fs::write(&sav, b"lost the rival fight").unwrap();
    down_to(&mut a, OptionRow::GameBackup(0));

    press(&mut a, Btn::Y);
    // asked first, over the rows, the bar's actions B NO and A YES
    assert_eq!(a.bar_place().map(|b| b.actions), Some(Actions::YesNo));
    assert!(a.saves_asking().is_some());
    assert_eq!(std::fs::read(&sav).unwrap(), b"lost the rival fight");
    press(&mut a, Btn::B);
    assert!(a.saves_asking().is_none(), "B left the question up");
    assert_eq!(std::fs::read(&sav).unwrap(), b"lost the rival fight");

    press(&mut a, Btn::Y);
    press(&mut a, Btn::A);
    assert_eq!(std::fs::read(&sav).unwrap(), b"badge eight");
    assert_eq!(save_manager::backups(d.path(), &emerald()).len(), 2);
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    assert!(!ring.has_resume(), "the resume state was not set aside");
}

#[test]
fn x_deletes_a_backup_after_asking() {
    let d = card();
    let mut a = boot(d.path());
    open_emerald(&mut a);
    press(&mut a, Btn::A);
    down_to(&mut a, OptionRow::GameBackUp);
    press(&mut a, Btn::A);
    down_to(&mut a, OptionRow::GameBackup(0));
    press(&mut a, Btn::X);
    assert!(a.saves_asking().is_some(), "X deleted it without asking");
    press(&mut a, Btn::A);
    assert!(save_manager::backups(d.path(), &emerald()).is_empty());
    assert!(
        d.path().join("Saves/GBA/Emerald.sav").exists(),
        "the save went too"
    );
}

#[test]
fn x_deletes_a_state_and_y_makes_one_the_game_resumes_from() {
    let d = card();
    StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
        .push(b"before the elite four", b"png", "2026-11-12_10-00-00")
        .unwrap();
    let mut a = boot(d.path());
    open_emerald(&mut a);
    down_to(&mut a, OptionRow::GameStatesMenu);
    press(&mut a, Btn::A);
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");

    // the saved state, made the resume point; the one before is set aside
    down_to(&mut a, OptionRow::GameState(1));
    press(&mut a, Btn::Y);
    assert!(a.saves_asking().is_some(), "Y did it without asking");
    press(&mut a, Btn::A);
    assert_eq!(
        ring.read_resume().unwrap().as_deref(),
        Some(&b"before the elite four"[..])
    );
    assert_eq!(ring.set_aside().len(), 1);

    // the resume point is deleted with X, asked first
    down_to(&mut a, OptionRow::GameState(0));
    press(&mut a, Btn::X);
    press(&mut a, Btn::A);
    assert!(!ring.has_resume(), "the state stayed");
}

#[test]
fn x_on_the_current_save_deletes_it_after_asking_and_keeps_it_as_a_backup() {
    let d = card();
    let mut a = boot(d.path());
    open_emerald(&mut a);
    press(&mut a, Btn::A);
    press(&mut a, Btn::X);
    assert!(d.path().join("Saves/GBA/Emerald.sav").exists());
    press(&mut a, Btn::A);
    assert!(!d.path().join("Saves/GBA/Emerald.sav").exists());
    assert_eq!(save_manager::backups(d.path(), &emerald()).len(), 1);
    assert_eq!(rows(&a)[0].2, "NO SAVE");
}

#[test]
fn a_backup_is_named_for_when_it_was_made_on_either_clock() {
    use omega_zero::saves_settings::made_at;
    assert_eq!(made_at("2026-11-12_16-40-05", true), "12 Nov 2026, 16:40");
    assert_eq!(
        made_at("2026-11-12_16-40-05", false),
        "12 Nov 2026, 4:40 PM"
    );
    assert_eq!(
        made_at("2026-01-03_00-05-00", false),
        "3 Jan 2026, 12:05 AM"
    );
}
