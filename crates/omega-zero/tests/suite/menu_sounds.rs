//! The menus' sounds, the shelf's tune, and the shelf's small conveniences around them: X on
//! RECENTS, L2 and R2.

use crate::common;

use common::{app_playing_in, boot, tmp_root_with_carts};
use omega_input::{Action, Btn};
use omega_zero::app::{App, Phase};
use omega_zero::audio::{ring_capacity, Ring, Sfx};

fn booted_with(d: &tempfile::TempDir, change: impl FnOnce(&mut omega_store::AppState)) -> App {
    let mut s = omega_store::AppState {
        clock_set: true,
        ..Default::default()
    };
    change(&mut s);
    omega_store::write_app_state(d.path(), &s).unwrap();
    App::boot(d.path())
}

/// The sound `action` makes, after a frame for the shelf to move.
fn heard(a: &mut App, action: Action) -> Option<Sfx> {
    a.take_sfx();
    a.apply(action);
    let pressed = a.take_sfx();
    a.update(1.0 / 60.0);
    a.update(1.0 / 60.0);
    pressed.or_else(|| a.take_sfx())
}

#[test]
fn a_menu_chimes_up_as_it_opens_ticks_as_it_moves_and_chimes_down_as_it_closes() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    assert_eq!(heard(&mut a, Action::QuickMenu), Some(Sfx::Confirm));
    assert!(matches!(a.phase(), Phase::Settings { .. }));
    assert_eq!(heard(&mut a, Action::GbaDown(Btn::Down)), Some(Sfx::Move));
    assert_eq!(heard(&mut a, Action::GbaDown(Btn::B)), Some(Sfx::Back));
    a.finish_slide();
    assert!(matches!(a.phase(), Phase::Shelf));
}

#[test]
fn the_shelf_ticks_as_its_pick_moves() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion", "Zero"]);
    let mut a = boot(d.path());
    a.update(1.0 / 60.0);
    a.take_sfx();
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaUp(Btn::Right));
    let mut sounds = Vec::new();
    for _ in 0..10 {
        a.update(1.0 / 60.0);
        sounds.extend(a.take_sfx());
    }
    assert_eq!(
        sounds,
        vec![Sfx::Move],
        "moving one cart sounded {sounds:?}"
    );
}

#[test]
fn menu_sounds_off_keeps_the_menus_quiet_and_the_carts_heard() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = booted_with(&d, |s| s.menu_sounds = false);
    assert_eq!(heard(&mut a, Action::QuickMenu), None);
    assert_eq!(heard(&mut a, Action::GbaDown(Btn::Down)), None);
    let mut a = booted_with(&d, |s| s.menu_sounds = false);
    a.apply(Action::Insert);
    let mut sounds = Vec::new();
    for _ in 0..90 {
        a.update(1.0 / 60.0);
        sounds.extend(a.take_sfx());
    }
    assert_eq!(sounds, vec![Sfx::Insert], "the cart went in unheard");
}

#[test]
fn a_game_s_buttons_make_no_menu_sound() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    for btn in [Btn::A, Btn::B, Btn::Up, Btn::Left, Btn::Start] {
        assert_eq!(
            heard(&mut a, Action::GbaDown(btn)),
            None,
            "{btn:?} in a game"
        );
    }
}

#[test]
fn the_menu_sounds_are_short_quiet_and_end_softly() {
    for s in [Sfx::Move, Sfx::Confirm, Sfx::Back, Sfx::Shelf] {
        assert!(s.is_menu());
        let v = s.render(48_000);
        let secs = v.len() as f32 / 2.0 / 48_000.0;
        assert!(secs > 0.02 && secs < 0.25, "{s:?} runs {secs:.2} s");
        let peak = v.iter().map(|x| x.unsigned_abs()).max().unwrap();
        assert!(peak > 500, "{s:?} is silent");
        assert!(
            peak < 20_000,
            "{s:?} peaks at {peak}, louder than a menu's sound should"
        );
        assert!(
            v[v.len() - 40..].iter().all(|x| x.abs() < 200),
            "{s:?} ends on a step"
        );
    }
    assert!(!Sfx::Insert.is_menu() && !Sfx::Blip.is_menu());
}

#[test]
fn a_quick_run_of_sounds_overlaps_rather_than_cutting_off() {
    let r = Ring::new(ring_capacity(48_000));
    r.reopen(48_000);
    let mut drained = vec![0i16; r.queued_frames() * 2];
    r.fill(&mut drained);
    let first = vec![1_000i16; 4_000];
    let second = vec![10i16; 1_000];
    r.mix(&first);
    r.mix(&second);
    let mut out = vec![0i16; 4_000];
    r.fill(&mut out);
    assert_eq!(
        out[0], 1_010,
        "the second sound was not laid over the first"
    );
    assert_eq!(out[3_999], 1_000, "the second sound cut the first off");
}

#[test]
fn a_sound_laid_over_a_fed_stream_leaves_the_stream_unbroken() {
    let r = Ring::new(ring_capacity(48_000));
    r.reopen(48_000);
    let mut drained = vec![0i16; r.queued_frames() * 2];
    r.fill(&mut drained);
    r.push(&[100i16; 200]);
    r.mix(&[5i16; 2_000]);
    // the sound reaches no further than the stream fed so far
    assert_eq!(r.queued_frames(), 100);
    r.push(&[100i16; 2_000]);
    let mut out = vec![0i16; 2_200];
    r.fill(&mut out);
    assert!(
        out[..2_000].iter().all(|&s| s == 105),
        "the sound and stream did not add up"
    );
    assert!(
        out[2_000..].iter().all(|&s| s == 100),
        "the stream was shifted"
    );
}

#[test]
fn x_on_recents_takes_the_game_off_it_and_keeps_it_on_its_shelf() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion", "Zero"]);
    std::fs::write(
        d.path().join("Config/recents.txt"),
        "gba/Zero\ngba/Fusion\n",
    )
    .unwrap();
    let mut a = booted_with(&d, |s| s.show_recents = true);
    assert_eq!(a.pill_text().as_deref(), Some("RECENTS"));
    assert_eq!(a.selected_stem(), Some("Zero"));
    a.apply(Action::GbaDown(Btn::X));
    assert_eq!(a.toast(), Some(omega_ui::Toast::RecentRemoved));
    assert!(
        a.core_picker().is_none(),
        "X opened the core picker on RECENTS"
    );
    assert_eq!(a.selected_stem(), Some("Fusion"));
    let file = std::fs::read_to_string(d.path().join("Config/recents.txt")).unwrap();
    assert_eq!(file, "gba/Fusion\n");
    a.apply(Action::GbaDown(Btn::X));
    assert_ne!(
        a.pill_text().as_deref(),
        Some("RECENTS"),
        "an empty RECENTS stayed on screen"
    );
    assert!(
        a.carts().any(|c| c.stem == "Zero"),
        "the game left the card's shelf"
    );
}

#[test]
fn x_still_picks_the_core_on_a_system_s_shelf() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    a.apply(Action::GbaDown(Btn::X));
    assert!(a.core_picker().is_some());
}

#[test]
fn l2_goes_back_to_the_first_cart_and_r2_picks_another_at_random() {
    let d = tmp_root_with_carts(&["Advance Wars", "Emerald", "Fusion", "Zero"]);
    let mut a = boot(d.path());
    assert_eq!(a.selected_stem(), Some("Advance Wars"));
    let mut seen = std::collections::HashSet::new();
    for _ in 0..12 {
        let was = a.selected_stem().map(str::to_string);
        a.apply(Action::FfStart);
        a.apply(Action::FfStop);
        let now = a.selected_stem().map(str::to_string);
        assert_ne!(now, was, "R2 stayed on the same cart");
        seen.extend(now);
        for _ in 0..7 {
            a.update(1.0 / 60.0);
        }
    }
    assert!(seen.len() >= 2, "R2 only ever picked {seen:?}");
    a.apply(Action::RewindStart);
    a.apply(Action::RewindStop);
    assert_eq!(a.selected_stem(), Some("Advance Wars"));
}

#[test]
fn the_tune_plays_on_the_shelf_and_stops_for_a_game() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut s = omega_zero::session::Session::boot(d.path().to_path_buf());
    let ring = s.audio_ring();
    let mut out = vec![0i16; ring.queued_frames() * 2];
    ring.fill(&mut out);
    for _ in 0..90 {
        s.update(1.0 / 60.0);
        let mut out = vec![0i16; 1_092];
        ring.fill(&mut out);
    }
    let mut out = vec![0i16; ring.queued_frames() * 2];
    ring.fill(&mut out);
    assert!(out.iter().any(|v| v.abs() > 20), "the shelf is silent");
}

#[test]
fn a_umd_spins_up_like_a_drive_before_its_core_opens() {
    let d = tmp_root_with_carts(&[]);
    let iso = d.path().join("Games/PSP/Patapon.iso");
    std::fs::create_dir_all(iso.parent().unwrap()).unwrap();
    std::fs::write(&iso, vec![0u8; 64 * 1024]).unwrap();
    // a second game, so the card is not one that starts its only game at boot
    std::fs::write(iso.with_file_name("Patapon 2.iso"), vec![0u8; 64 * 1024]).unwrap();
    let mut a = boot(d.path());
    assert_eq!(a.selected_stem(), Some("Patapon"));
    assert!(a.core_may_open(), "the shelf holds a core back");
    a.apply(Action::Insert);
    assert!(!a.core_may_open(), "the core opened before the disc spun");
    let mut speeds = Vec::new();
    for _ in 0..70 {
        a.update(1.0 / 60.0);
        speeds.push(a.disc_turns());
    }
    assert!(
        a.core_may_open(),
        "the core still waits after a second's spin"
    );
    assert!(
        speeds[5] < 1.0,
        "the disc started at {} turns a second",
        speeds[5]
    );
    assert!(
        speeds.windows(2).all(|w| w[1] >= w[0]),
        "the disc slowed as it spun up"
    );
    assert!(
        speeds[69] > 18.0,
        "the disc reached only {} turns a second",
        speeds[69]
    );
}
