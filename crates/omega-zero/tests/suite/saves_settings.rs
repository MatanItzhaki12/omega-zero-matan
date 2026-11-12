//! Settings › Saves as it is used: SAVES lists the games with anything on the card, GAME a
//! game's save, backups and states, and what can lose something asks first.

use crate::common;

use common::{boot, tmp_root_with_carts};
use omega_input::{Action, Btn};
use omega_store::{Core, Platform, StateRing};
use omega_ui::SettingsRow;
use omega_zero::app::App;
use omega_zero::options::OptionRow;
use omega_zero::save_manager::{self, Game};

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
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
    a.finish_slide();
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
fn saves_lists_the_nas_and_the_games_with_anything_on_the_card() {
    let d = card();
    let mut a = boot(d.path());
    assert_eq!(SettingsRow::Backup.label(), "Saves");
    open_saves(&mut a);
    let v = a.options_view().unwrap();
    assert_eq!(v.title, "SAVES");
    assert_eq!(
        rows(&a),
        [
            (OptionRow::SavesNas, "NAS Backup".into(), "OFF".into()),
            (OptionRow::SavesGame(0), "Emerald".into(), "GBA".into()),
        ],
        "Fusion has nothing, and is not listed"
    );
    // NAS Backup opens SAVE BACKUP, and B comes back to it
    press(&mut a, Btn::A);
    a.finish_slide();
    assert_eq!(a.options_view().unwrap().title, "SAVE BACKUP");
    press(&mut a, Btn::B);
    a.finish_slide();
    assert_eq!(a.options_view().unwrap().title, "SAVES");
    assert_eq!(lit(&a), OptionRow::SavesNas);
}

#[test]
fn a_game_s_save_is_backed_up_and_put_back_and_its_state_deleted() {
    let d = card();
    let mut a = boot(d.path());
    open_saves(&mut a);
    down_to(&mut a, OptionRow::SavesGame(0));
    press(&mut a, Btn::A);
    a.finish_slide();
    assert_eq!(a.options_view().unwrap().title, "GAME");
    let shown: Vec<OptionRow> = rows(&a).iter().map(|r| r.0).collect();
    assert_eq!(
        shown,
        [
            OptionRow::GameSave,
            OptionRow::GameBackUp,
            OptionRow::GameState(0),
            OptionRow::GameDelete,
        ]
    );
    assert_eq!(rows(&a)[0].1, "Emerald");
    assert_eq!(rows(&a)[2].1, "Resume State");

    // Back Up Save keeps a copy, and it is listed
    down_to(&mut a, OptionRow::GameBackUp);
    press(&mut a, Btn::A);
    assert_eq!(save_manager::backups(d.path(), &emerald()).len(), 1);
    assert!(rows(&a).iter().any(|r| r.0 == OptionRow::GameBackup(0)));

    // the game plays on; the backup is put back, asked first
    let sav = d.path().join("Saves/GBA/Emerald.sav");
    std::fs::write(&sav, b"lost the rival fight").unwrap();
    down_to(&mut a, OptionRow::GameBackup(0));
    press(&mut a, Btn::A);
    assert!(note(&a).contains("A again"), "{}", note(&a));
    assert_eq!(std::fs::read(&sav).unwrap(), b"lost the rival fight");
    press(&mut a, Btn::A);
    assert_eq!(std::fs::read(&sav).unwrap(), b"badge eight");
    // the save it replaced was kept too, and the resume state set aside
    assert_eq!(save_manager::backups(d.path(), &emerald()).len(), 2);
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    assert!(!ring.has_resume());

    // the state set aside is deleted with Y, asked first
    let state = rows(&a)
        .iter()
        .find(|r| matches!(r.0, OptionRow::GameState(_)))
        .map(|r| r.0)
        .expect("the state set aside is listed");
    down_to(&mut a, state);
    press(&mut a, Btn::Y);
    assert!(note(&a).contains("Y again"), "{}", note(&a));
    press(&mut a, Btn::Y);
    assert!(ring.set_aside().is_empty(), "the state stayed");
}

#[test]
fn moving_off_a_row_asked_once_lets_the_question_go() {
    let d = card();
    let mut a = boot(d.path());
    open_saves(&mut a);
    down_to(&mut a, OptionRow::SavesGame(0));
    press(&mut a, Btn::A);
    a.finish_slide();
    down_to(&mut a, OptionRow::GameDelete);
    press(&mut a, Btn::A);
    press(&mut a, Btn::Up);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    assert!(
        d.path().join("Saves/GBA/Emerald.sav").exists(),
        "a press after moving away deleted the save"
    );
    // asked and answered on the row, it goes, and is kept as a backup
    press(&mut a, Btn::A);
    assert!(!d.path().join("Saves/GBA/Emerald.sav").exists());
    assert_eq!(save_manager::backups(d.path(), &emerald()).len(), 1);
    assert_eq!(rows(&a)[0].2, "NO SAVE");
}
