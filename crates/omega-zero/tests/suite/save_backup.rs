//! Save Backup against a real Samba server, when one is named: OMEGA_SMB_TEST holds its
//! address, user, password and a share that user may write to, apart by `|`, and
//! OMEGA_SMB_DIR the folder that share serves, to look at what arrived. Without them each test
//! passes at once, as CI runs them.

use std::sync::Mutex;

use crate::common::{boot, tmp_root_with_carts};
use omega_input::{Action, Btn};
use omega_store::read_app_state;
use omega_ui::SettingsRow;
use omega_zero::app::App;
use omega_zero::nas::{Login, Nas};
use omega_zero::options::OptionRow;
use omega_zero::save_backup::{pass, List, Status, Target};

fn server() -> Option<(Login, String, std::path::PathBuf)> {
    let named = std::env::var("OMEGA_SMB_TEST").ok()?;
    let dir = std::env::var("OMEGA_SMB_DIR").ok()?;
    let mut parts = named.split('|');
    let login = Login {
        host: parts.next()?.to_string(),
        user: parts.next()?.to_string(),
        password: parts.next()?.to_string(),
    };
    Some((login, parts.next()?.to_string(), dir.into()))
}

#[test]
fn a_share_is_listed_a_folder_made_and_a_file_put_in_it() {
    let Some((login, share, served)) = server() else {
        return;
    };
    let mut nas = Nas::open(&login).expect("signed in");
    let shares = nas.shares().expect("the shares");
    assert!(shares.contains(&share), "{shares:?}");
    let top = format!("put-{}", std::process::id());
    nas.make_folder(&share, &format!("{top}/a/b"))
        .expect("made");
    assert!(nas
        .folders(&share, &top)
        .expect("listed")
        .contains(&"a".to_string()));
    nas.put(&share, &format!("{top}/a/b/one.sav"), b"first")
        .expect("put");
    nas.put(&share, &format!("{top}/a/b/one.sav"), b"second")
        .expect("put over");
    assert_eq!(
        std::fs::read(served.join(&top).join("a/b/one.sav")).unwrap(),
        b"second"
    );
    assert!(
        !served.join(&top).join("a/b/one.sav.part").exists(),
        "the part file stayed"
    );
}

#[test]
fn a_wrong_password_is_told_apart() {
    let Some((login, ..)) = server() else {
        return;
    };
    let wrong = Login {
        password: "not it".into(),
        ..login
    };
    let e = Nas::open(&wrong)
        .err()
        .expect("signed in with a wrong password");
    assert_eq!(e, omega_zero::nas::NasFail::Login, "{e}");
}

/// A first pass copies the whole Saves folder; after a save, the next copies that one alone,
/// the newest of it over the old.
#[test]
fn saves_are_copied_and_a_new_save_replaces_the_old() {
    let Some((login, share, served)) = server() else {
        return;
    };
    let d = tempfile::tempdir().unwrap();
    let saves = d.path().join("Saves");
    std::fs::create_dir_all(saves.join("GBA")).unwrap();
    std::fs::create_dir_all(saves.join("PS1")).unwrap();
    std::fs::write(saves.join("GBA/Emerald.sav"), vec![7u8; 131072]).unwrap();
    std::fs::write(saves.join("PS1/Crash.mcd"), b"card").unwrap();
    // a PSP save is a folder of files, and PPSSPP keeps files of its own beside them
    let psp = saves.join("PSP/SAVEDATA/ULUS10391DATA00");
    std::fs::create_dir_all(&psp).unwrap();
    std::fs::write(psp.join("DATA.BIN"), b"hunt").unwrap();
    let sfo = omega_store::psp::param_sfo(&[("TITLE", "Monster Hunter Freedom Unite")]);
    std::fs::write(psp.join("PARAM.SFO"), &sfo).unwrap();
    std::fs::create_dir_all(saves.join("PSP/SYSTEM")).unwrap();
    std::fs::write(saves.join("PSP/SYSTEM/settings.bin"), b"ppsspp").unwrap();
    let dir = format!("pass-{}/RG SP", std::process::id());
    let to = Target {
        login,
        share,
        dir: dir.clone(),
    };
    let status = Mutex::new(Status::default());
    assert_eq!(pass(d.path(), &to, &status), 0);
    let s = status.lock().unwrap().clone();
    assert!(s.fail.is_none() && s.done_at.is_some(), "{s:?}");
    let there = served.join(&dir);
    assert_eq!(
        std::fs::read(there.join("GBA/Emerald.sav")).unwrap(),
        vec![7u8; 131072]
    );
    assert_eq!(std::fs::read(there.join("PS1/Crash.mcd")).unwrap(), b"card");
    assert_eq!(
        std::fs::read(there.join("PSP/SAVEDATA/ULUS10391DATA00/DATA.BIN")).unwrap(),
        b"hunt"
    );
    assert_eq!(List::read(d.path()).files.len(), 5);

    std::fs::write(saves.join("GBA/Emerald.sav"), vec![9u8; 131072]).unwrap();
    std::fs::remove_file(there.join("PS1/Crash.mcd")).unwrap();
    assert_eq!(pass(d.path(), &to, &status), 0);
    assert_eq!(
        std::fs::read(there.join("GBA/Emerald.sav")).unwrap(),
        vec![9u8; 131072]
    );
    assert!(
        !there.join("PS1/Crash.mcd").exists(),
        "a save that had not changed was copied again"
    );

    // the NAS's saves changed by another: Sync From Backup finds them, a PSP save's folder as
    // one, named for its game, and PPSSPP's own files not at all
    std::fs::write(there.join("GBA/Emerald.sav"), b"from the pc").unwrap();
    std::fs::write(
        there.join("PSP/SAVEDATA/ULUS10391DATA00/DATA.BIN"),
        b"hunted on the pc",
    )
    .unwrap();
    std::fs::write(there.join("PSP/SYSTEM/settings.bin"), b"other").unwrap();
    let differs = omega_zero::save_backup::differing(d.path(), &to).expect("compared");
    assert_eq!(
        differs.iter().map(|d| d.rel.as_str()).collect::<Vec<_>>(),
        ["GBA/Emerald.sav", "PSP/SAVEDATA/ULUS10391DATA00"],
        "the changed saves alone: one gone from the NAS is not offered"
    );
    assert_eq!(differs[1].name(), "Monster Hunter Freedom Unite");
    assert_eq!(differs[1].files.len(), 2, "the PSP save came in part");
    omega_zero::save_backup::restore(d.path(), &differs[1], "2026-11-03_12-00-00").unwrap();
    assert_eq!(
        std::fs::read(psp.join("DATA.BIN")).unwrap(),
        b"hunted on the pc"
    );
    omega_zero::save_backup::restore(d.path(), &differs[0], "2026-11-03_12-00-00").unwrap();
    assert_eq!(
        std::fs::read(saves.join("GBA/Emerald.sav")).unwrap(),
        b"from the pc"
    );
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

/// Settings > Saves > NAS Backup, the SAVE BACKUP panel open.
fn open_backup(a: &mut App) {
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
    assert_eq!(a.options_view().unwrap().title, "SAVES");
    assert_eq!(lit(a), OptionRow::SavesNas);
    press(a, Btn::A);
    a.finish_slide();
}

fn lit(a: &App) -> OptionRow {
    let v = a.options_view().expect("a panel");
    v.lines[v.lit].0
}

fn value(a: &App, row: OptionRow) -> String {
    let v = a.options_view().expect("a panel");
    v.lines
        .iter()
        .find(|(r, ..)| *r == row)
        .map(|(_, _, v)| v.clone())
        .unwrap_or_else(|| panic!("no {row:?} in {:?}", v.lines))
}

fn down_to(a: &mut App, row: OptionRow) {
    for _ in 0..16 {
        if lit(a) == row {
            return;
        }
        press(a, Btn::Down);
    }
    panic!("Down never reached {row:?}");
}

/// The SAVE BACKUP panel, as Settings opens it: switched off, with no NAS, a guest, no
/// password and no folder, and Back Up Now saying it waits on them; the user and password
/// typed on the keyboard, kept on the card.
#[test]
fn the_save_backup_panel_is_set_up_on_the_keyboard() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    open_backup(&mut a);
    let v = a.options_view().unwrap();
    assert_eq!(v.title, "SAVE BACKUP");
    let shown: Vec<(OptionRow, String, String)> = v.lines.clone();
    assert_eq!(
        shown,
        [
            (OptionRow::BackupOn, "Save Backup".into(), "OFF".into()),
            (OptionRow::BackupNas, "NAS".into(), "FIND".into()),
            (OptionRow::BackupUser, "User".into(), "GUEST".into()),
            (OptionRow::BackupPassword, "Password".into(), "NONE".into()),
            (OptionRow::BackupFolder, "Folder".into(), "PICK".into()),
            (OptionRow::BackupNow, "Back Up Now".into(), "OFF".into()),
            (
                OptionRow::BackupRestore,
                "Sync From Backup".into(),
                String::new()
            ),
        ]
    );

    press(&mut a, Btn::Right);
    assert_eq!(value(&a, OptionRow::BackupOn), "ON");
    assert!(read_app_state(d.path()).backup, "ON was not kept");
    assert_eq!(value(&a, OptionRow::BackupNow), "SET UP");

    // a user typed: the keyboard's first key, then START
    down_to(&mut a, OptionRow::BackupUser);
    press(&mut a, Btn::A);
    assert!(a.typing().is_some(), "A on User raised no keyboard");
    press(&mut a, Btn::A);
    press(&mut a, Btn::Start);
    a.finish_slide();
    assert!(a.typing().is_none());
    let user = read_app_state(d.path()).backup_user;
    assert_eq!(user.chars().count(), 1, "{user:?}");
    assert_eq!(value(&a, OptionRow::BackupUser), user);

    down_to(&mut a, OptionRow::BackupPassword);
    press(&mut a, Btn::A);
    press(&mut a, Btn::A);
    press(&mut a, Btn::Start);
    a.finish_slide();
    assert_eq!(value(&a, OptionRow::BackupPassword), "SET");

    // no NAS yet: there is no folder to pick, nor anything to copy
    down_to(&mut a, OptionRow::BackupFolder);
    press(&mut a, Btn::A);
    assert!(a.refusal_active(a.now()), "Folder opened with no NAS");
    assert_eq!(a.options_view().unwrap().title, "SAVE BACKUP");
    down_to(&mut a, OptionRow::BackupNow);
    press(&mut a, Btn::A);
    assert!(
        a.refusal_active(a.now()),
        "Back Up Now with nowhere to copy"
    );
    down_to(&mut a, OptionRow::BackupRestore);
    press(&mut a, Btn::A);
    assert!(a.refusal_active(a.now()), "Sync From Backup with no NAS");
    assert_eq!(a.options_view().unwrap().title, "SAVE BACKUP");

    // FIND NAS lists what answers, and a row to type an address; B goes back
    down_to(&mut a, OptionRow::BackupNas);
    press(&mut a, Btn::A);
    a.finish_slide();
    let v = a.options_view().unwrap();
    assert_eq!(v.title, "FIND NAS");
    assert!(v.lines.iter().any(|(r, ..)| *r == OptionRow::NasAddress));
    down_to(&mut a, OptionRow::NasAddress);
    press(&mut a, Btn::A);
    press(&mut a, Btn::A);
    press(&mut a, Btn::Start);
    a.finish_slide();
    assert_eq!(a.options_view().unwrap().title, "SAVE BACKUP");
    assert_eq!(
        lit(&a),
        OptionRow::BackupUser,
        "picking a NAS did not go on to its user"
    );
    assert_eq!(read_app_state(d.path()).backup_host.chars().count(), 1);
    assert_ne!(value(&a, OptionRow::BackupNas), "FIND");
    // B goes back to SAVES, on NAS Backup, and again to Settings, on Saves
    press(&mut a, Btn::B);
    a.finish_slide();
    assert_eq!(a.options_view().unwrap().title, "SAVES");
    assert_eq!(lit(&a), OptionRow::SavesNas);
    press(&mut a, Btn::B);
    a.finish_slide();
    assert_eq!(a.settings_row(), Some(SettingsRow::Backup));
}

/// With a Samba server named, FOLDER lists its shares, opens one, makes a folder in it and
/// picks it; saves on the card are copied there.
#[test]
fn a_folder_is_picked_on_the_nas_and_the_saves_go_there() {
    let Some((login, share, served)) = server() else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let saves = d.path().join("Saves/GBA");
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(saves.join("Emerald.sav"), b"emerald").unwrap();
    let mut s = read_app_state(d.path());
    s.backup = true;
    s.backup_host = login.host.clone();
    s.backup_user = login.user.clone();
    s.backup_password = login.password.clone();
    omega_store::write_app_state(d.path(), &s).unwrap();
    let mut a = boot(d.path());
    open_backup(&mut a);
    down_to(&mut a, OptionRow::BackupFolder);
    press(&mut a, Btn::A);
    a.finish_slide();
    assert_eq!(a.options_view().unwrap().title, "FOLDER");
    let settle = |a: &mut App| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while a.options_view().unwrap().note == "Opening the NAS" {
            assert!(
                std::time::Instant::now() < deadline,
                "the NAS never answered"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
            a.update(0.02);
        }
    };
    settle(&mut a);
    let v = a.options_view().unwrap();
    let at = v
        .lines
        .iter()
        .position(|(_, label, _)| *label == share)
        .unwrap_or_else(|| panic!("{share} not among {:?}", v.lines));
    down_to(&mut a, v.lines[at].0);
    press(&mut a, Btn::A);
    settle(&mut a);
    assert_eq!(lit(&a), OptionRow::FolderUse);
    assert_eq!(value(&a, OptionRow::FolderUse), share);

    // a new folder: the keyboard's first key for its name
    down_to(&mut a, OptionRow::FolderNew);
    press(&mut a, Btn::A);
    press(&mut a, Btn::A);
    press(&mut a, Btn::Start);
    a.finish_slide();
    settle(&mut a);
    let made = value(&a, OptionRow::FolderUse);
    let name = made.rsplit('/').next().unwrap().to_string();
    assert!(made.starts_with(&format!("{share}/")), "{made}");
    assert!(
        served.join(&name).is_dir(),
        "{name} was not made on the NAS"
    );

    down_to(&mut a, OptionRow::FolderUse);
    press(&mut a, Btn::A);
    a.finish_slide();
    assert_eq!(a.options_view().unwrap().title, "SAVE BACKUP");
    let kept = read_app_state(d.path());
    assert_eq!(
        (kept.backup_share.as_str(), kept.backup_dir.as_str()),
        (share.as_str(), name.as_str())
    );

    press(&mut a, Btn::A);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while std::fs::read(served.join(&name).join("GBA/Emerald.sav"))
        .ok()
        .as_deref()
        != Some(b"emerald".as_slice())
    {
        assert!(
            std::time::Instant::now() < deadline,
            "the save never arrived"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
        a.update(0.05);
    }
    assert_eq!(
        std::fs::read(served.join(&name).join("GBA/Emerald.sav")).unwrap(),
        b"emerald"
    );
    let told = Some(omega_ui::Toast::Backup("Saves backed up to the NAS"));
    while a.toast() != told {
        assert!(
            std::time::Instant::now() < deadline,
            "no toast for the copy"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
        a.update(0.0);
    }

    // Sync From Backup: the NAS's save, now unlike the handheld's, is listed and copied back,
    // once every pass asked for has ended
    let mut quiet = 0;
    while quiet < 10 {
        assert!(
            std::time::Instant::now() < deadline,
            "the backup never went quiet"
        );
        let s = a.backup_status();
        quiet = if !s.busy && s.waiting == 0 {
            quiet + 1
        } else {
            0
        };
        std::thread::sleep(std::time::Duration::from_millis(100));
        a.update(0.1);
    }
    std::fs::write(served.join(&name).join("GBA/Emerald.sav"), b"from the pc").unwrap();
    down_to(&mut a, OptionRow::BackupRestore);
    press(&mut a, Btn::A);
    a.finish_slide();
    assert_eq!(a.options_view().unwrap().title, "FROM BACKUP");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while a.options_view().unwrap().note == "Comparing with the NAS" {
        assert!(
            std::time::Instant::now() < deadline,
            "the NAS never answered"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
        a.update(0.02);
    }
    let v = a.options_view().unwrap();
    assert_eq!(
        v.lines,
        [(
            OptionRow::RestoreItem(0),
            "Emerald".to_string(),
            "GBA".to_string()
        )],
        "{}",
        v.note
    );
    press(&mut a, Btn::A);
    assert_eq!(
        std::fs::read(saves.join("Emerald.sav")).unwrap(),
        b"emerald",
        "one press replaced the handheld's save"
    );
    assert_eq!(
        a.options_view().unwrap().note,
        "A again replaces the handheld's"
    );
    press(&mut a, Btn::A);
    assert_eq!(
        std::fs::read(saves.join("Emerald.sav")).unwrap(),
        b"from the pc"
    );
    assert!(a.options_view().unwrap().lines.is_empty());
    assert_eq!(a.options_view().unwrap().note, "Every save matches the NAS");
}

/// The cart picked when the handheld goes off is the one picked when it comes on, the game not
/// opened.
#[test]
fn the_shelf_comes_up_on_the_cart_picked_when_it_went_off() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion", "Zelda"]);
    let mut a = boot(d.path());
    let first = a.selected_stem().map(str::to_string);
    press(&mut a, Btn::Right);
    a.update(1.0);
    let picked = a.selected_stem().map(str::to_string);
    assert_ne!(picked, first, "Right picked no other cart");
    a.poweroff();
    let a = boot(d.path());
    assert_eq!(a.selected_stem().map(str::to_string), picked);
    assert!(
        matches!(a.phase(), omega_zero::app::Phase::Shelf),
        "the cart was opened: {:?}",
        a.phase()
    );
}

/// A NAS out of reach is told of in a toast once, not at every try after; a copy that goes
/// through is told of every time.
#[test]
fn a_failed_backup_is_told_of_once_until_saves_copy_again() {
    use omega_ui::Toast;
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let saves = d.path().join("Saves/GBA");
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(saves.join("Emerald.sav"), b"emerald").unwrap();
    let mut s = read_app_state(d.path());
    s.backup = true;
    // nothing listens on port 9 here: refused at once
    s.backup_host = "127.0.0.1:9".into();
    s.backup_share = "Backups".into();
    omega_store::write_app_state(d.path(), &s).unwrap();
    let mut a = boot(d.path());
    let failed_twice = |a: &mut App, n: u64| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while a.backup_status().failed < n {
            assert!(std::time::Instant::now() < deadline, "no pass failed");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        a.update(0.0);
    };
    failed_twice(&mut a, 1);
    assert_eq!(
        a.toast(),
        Some(Toast::Backup("Save backup failed: NAS away, will retry"))
    );
    // the toast goes, and the next try fails too: no toast again
    a.update(10.0);
    assert_eq!(a.toast(), None);
    open_backup(&mut a);
    failed_twice(&mut a, 2);
    assert_eq!(a.toast(), None, "a NAS still away was told of again");
}

/// A pass asked for as a game's save is being written looks once the write is done: the save
/// written just after the ask is the one the pass finds, not the Saves folder before it.
#[test]
fn a_pass_asked_for_as_a_save_is_written_finds_that_save() {
    use omega_zero::save_backup::{SaveBackup, SETTLE};
    let d = tempfile::tempdir().unwrap();
    let saves = d.path().join("Saves/GBA");
    std::fs::create_dir_all(&saves).unwrap();
    // nothing listens on port 9 here: the save waits, and the count says it was seen
    let to = Target {
        login: Login {
            host: "127.0.0.1:9".into(),
            ..Login::default()
        },
        share: "Backups".into(),
        dir: String::new(),
    };
    let backup = SaveBackup::start(d.path().to_path_buf(), None);
    backup.set_target(Some(to));
    backup.pass();
    std::thread::sleep(SETTLE / 4);
    std::fs::write(saves.join("Emerald.sav"), b"saved in the game").unwrap();
    let deadline = std::time::Instant::now() + SETTLE * 3;
    while backup.status().failed == 0 {
        assert!(std::time::Instant::now() < deadline, "no pass ran");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        backup.status().waiting,
        1,
        "the pass looked before the game's save was written"
    );
}
