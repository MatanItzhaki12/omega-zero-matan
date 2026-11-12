//! A game's saves as Settings › Saves manages them: what each game has on the card, its backups
//! kept and put back, and its save and states deleted, none of it losing a save.

use std::fs;
use std::path::Path;

use omega_store::{Core, Platform, StateRing};
use omega_zero::save_manager::{self, Game, StateKind};

const NOW: &str = "2026-11-12_16-00-00";
const LATER: &str = "2026-11-12_16-05-00";

fn gba(stem: &str) -> Game {
    Game {
        platform: Platform::Gba,
        stem: stem.into(),
        code: String::new(),
    }
}

fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let at = root.join(rel);
    fs::create_dir_all(at.parent().unwrap()).unwrap();
    fs::write(at, bytes).unwrap();
}

#[test]
fn a_game_s_save_is_its_files_in_saves_and_not_its_disc() {
    let d = tempfile::tempdir().unwrap();
    write(d.path(), "Saves/GBA/Emerald.sav", b"one");
    write(d.path(), "Saves/GBA/Emerald.disc", b"0\n");
    write(d.path(), "Saves/GBA/Emerald Two.sav", b"other");
    assert_eq!(
        save_manager::save_files(d.path(), &gba("Emerald")),
        vec!["GBA/Emerald.sav"]
    );
    let saves = save_manager::look(d.path(), &gba("Emerald"));
    assert_eq!(saves.save_bytes, 3);
    assert!(saves.save_written.is_some());
}

#[test]
fn a_psp_game_s_save_is_every_folder_its_id_starts() {
    let d = tempfile::tempdir().unwrap();
    write(
        d.path(),
        "Saves/PSP/SAVEDATA/ULJM05681MAIN00/DATA.BIN",
        b"main",
    );
    write(
        d.path(),
        "Saves/PSP/SAVEDATA/ULJM05681MAIN00/PARAM.SFO",
        b"sfo",
    );
    write(d.path(), "Saves/PSP/SAVEDATA/ULJM05681SYS/SYS.BIN", b"sys");
    write(
        d.path(),
        "Saves/PSP/SAVEDATA/NPJH50465MAIN00/DATA.BIN",
        b"another game",
    );
    let game = Game {
        platform: Platform::Psp,
        stem: "Miku".into(),
        code: "ULJM05681".into(),
    };
    assert_eq!(
        save_manager::save_files(d.path(), &game),
        vec![
            "PSP/SAVEDATA/ULJM05681MAIN00/DATA.BIN",
            "PSP/SAVEDATA/ULJM05681MAIN00/PARAM.SFO",
            "PSP/SAVEDATA/ULJM05681SYS/SYS.BIN",
        ]
    );
    // deleting it takes its folders, and leaves the other game's
    save_manager::delete_save(d.path(), &game, NOW).unwrap();
    assert!(!d.path().join("Saves/PSP/SAVEDATA/ULJM05681MAIN00").exists());
    assert!(d
        .path()
        .join("Saves/PSP/SAVEDATA/NPJH50465MAIN00/DATA.BIN")
        .exists());
    // and the backup kept puts them all back
    save_manager::restore(d.path(), &game, NOW, LATER).unwrap();
    assert_eq!(
        fs::read(d.path().join("Saves/PSP/SAVEDATA/ULJM05681SYS/SYS.BIN")).unwrap(),
        b"sys"
    );
}

#[test]
fn a_backup_kept_is_put_back_and_the_save_it_replaced_kept_too() {
    let d = tempfile::tempdir().unwrap();
    let game = gba("Emerald");
    write(d.path(), "Saves/GBA/Emerald.sav", b"before the gym");
    assert!(save_manager::back_up(d.path(), &game, NOW).unwrap());
    write(d.path(), "Saves/GBA/Emerald.sav", b"after a bad trade");

    save_manager::restore(d.path(), &game, NOW, LATER).unwrap();
    assert_eq!(
        fs::read(d.path().join("Saves/GBA/Emerald.sav")).unwrap(),
        b"before the gym"
    );
    // the save it replaced is a backup of its own, the newest first
    assert_eq!(save_manager::backups(d.path(), &game), vec![LATER, NOW]);
    let kept = d
        .path()
        .join(format!("Backups/GBA/Emerald/{LATER}/GBA/Emerald.sav"));
    assert_eq!(fs::read(kept).unwrap(), b"after a bad trade");
}

#[test]
fn two_backups_in_one_second_are_both_kept() {
    let d = tempfile::tempdir().unwrap();
    let game = gba("Emerald");
    write(d.path(), "Saves/GBA/Emerald.sav", b"one");
    save_manager::back_up(d.path(), &game, NOW).unwrap();
    write(d.path(), "Saves/GBA/Emerald.sav", b"two");
    save_manager::back_up(d.path(), &game, NOW).unwrap();
    assert_eq!(save_manager::backups(d.path(), &game).len(), 2);
}

#[test]
fn a_game_with_no_save_backs_up_nothing() {
    let d = tempfile::tempdir().unwrap();
    assert!(!save_manager::back_up(d.path(), &gba("Emerald"), NOW).unwrap());
    assert!(save_manager::backups(d.path(), &gba("Emerald")).is_empty());
}

#[test]
fn deleting_a_save_keeps_it_as_a_backup_and_sets_the_resume_state_aside() {
    let d = tempfile::tempdir().unwrap();
    let game = gba("Emerald");
    write(d.path(), "Saves/GBA/Emerald.sav", b"twenty hours");
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    ring.write_resume(b"where it stopped").unwrap();

    save_manager::delete_save(d.path(), &game, NOW).unwrap();
    assert!(!d.path().join("Saves/GBA/Emerald.sav").exists());
    assert_eq!(save_manager::backups(d.path(), &game), vec![NOW]);
    // the resume state held the old save, and would have brought it back
    assert!(!ring.has_resume(), "the resume state stayed");
    let saves = save_manager::look(d.path(), &game);
    assert!(saves.save.is_empty());
    assert!(
        saves
            .states
            .iter()
            .any(|s| matches!(s.kind, StateKind::SetAside(_))),
        "{:?}",
        saves.states
    );
}

#[test]
fn every_state_of_a_game_is_listed_and_each_deleted_on_its_own() {
    let d = tempfile::tempdir().unwrap();
    let game = gba("Emerald");
    let mgba = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    let gpsp = StateRing::new(d.path(), Platform::Gba, Core::Gpsp, "Emerald");
    mgba.write_resume(b"resume").unwrap();
    mgba.push(b"saved", b"png", NOW).unwrap();
    gpsp.push(b"saved on gpsp", b"png", LATER).unwrap();

    let states = save_manager::look(d.path(), &game).states;
    assert_eq!(states.len(), 3, "{states:?}");
    assert_eq!(states[0].kind, StateKind::Resume);
    assert_eq!(states[0].bytes, 6);

    for s in &states {
        save_manager::delete_state(d.path(), &game, s).unwrap();
    }
    assert!(save_manager::look(d.path(), &game).states.is_empty());
    assert!(!save_manager::has_any(d.path(), &game));
}

#[test]
fn a_backup_deleted_is_gone_and_the_save_stays() {
    let d = tempfile::tempdir().unwrap();
    let game = gba("Emerald");
    write(d.path(), "Saves/GBA/Emerald.sav", b"save");
    save_manager::back_up(d.path(), &game, NOW).unwrap();
    save_manager::delete_backup(d.path(), &game, NOW).unwrap();
    assert!(save_manager::backups(d.path(), &game).is_empty());
    assert!(d.path().join("Saves/GBA/Emerald.sav").exists());
    // a name that is no backup's is refused, not taken for a path
    assert!(save_manager::delete_backup(d.path(), &game, "../../Saves").is_err());
}
