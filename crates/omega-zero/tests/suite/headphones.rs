use crate::common;

use std::sync::atomic::Ordering;

use common::{app_playing_with_jack, tmp_root_with_carts};
use omega_input::Action;
use omega_store::read_app_state;
use omega_ui::GaugeIcon;

/// Gives the jack time to be read twice over, as a plug or unplug takes to be believed.
fn settle(a: &mut omega_zero::app::App) {
    for _ in 0..2 {
        let at = a.now() + 600;
        a.tick_ms(at);
    }
}

#[test]
fn one_odd_read_of_the_jack_never_moves_the_volume() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, jack) = app_playing_with_jack(d.path(), "Emerald");
    jack.store(true, Ordering::Relaxed);
    settle(&mut a);
    let headphones = a.volume();
    a.apply(Action::VolumeDown);
    let headphones = headphones.min(a.volume());
    // the jack reads out for one poll, as a read of it that failed would
    jack.store(false, Ordering::Relaxed);
    let at = a.now() + 600;
    a.tick_ms(at);
    jack.store(true, Ordering::Relaxed);
    settle(&mut a);
    assert_eq!(a.volume(), headphones, "the volume moved on one odd read");
    assert_eq!(a.hud_icon(), GaugeIcon::Headphones);
}

#[test]
fn speaker_and_headphones_keep_their_own_volume() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, jack) = app_playing_with_jack(d.path(), "Emerald");
    let speaker = a.volume();

    jack.store(true, Ordering::Relaxed);
    settle(&mut a);
    assert_eq!(
        a.hud_icon(),
        GaugeIcon::Headphones,
        "plugging in did not show headphones"
    );
    for _ in 0..3 {
        a.apply(Action::VolumeDown);
    }
    let headphones = a.volume();
    assert!(
        headphones < speaker,
        "the buttons did not move the headphone level"
    );

    jack.store(false, Ordering::Relaxed);
    settle(&mut a);
    assert_eq!(
        a.volume(),
        speaker,
        "the speaker came back at the headphone level"
    );
    // out of the jack, the game never plays aloud by surprise
    assert_eq!(
        a.hud_icon(),
        GaugeIcon::Muted,
        "the speaker was not muted as the headphones came out"
    );

    jack.store(true, Ordering::Relaxed);
    settle(&mut a);
    assert_eq!(
        a.volume(),
        headphones,
        "the headphone level was not remembered"
    );

    let saved = read_app_state(d.path());
    assert_eq!((saved.volume, saved.volume_hp), (speaker, headphones));
}

#[test]
fn the_headphones_keep_a_mute_of_their_own() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, jack) = app_playing_with_jack(d.path(), "Emerald");
    let speaker = a.volume();

    jack.store(true, Ordering::Relaxed);
    settle(&mut a);
    a.apply(Action::MuteToggle);
    assert_eq!(a.output_volume(), 0);
    assert_eq!(a.hud_icon(), GaugeIcon::Muted);

    // out of the jack the speaker is muted too, and unmuted it plays at its own level
    jack.store(false, Ordering::Relaxed);
    settle(&mut a);
    assert_eq!(
        a.output_volume(),
        0,
        "the speaker played as the headphones came out"
    );
    a.apply(Action::MuteToggle);
    assert_eq!(a.output_volume(), speaker, "the speaker lost its level");

    jack.store(true, Ordering::Relaxed);
    settle(&mut a);
    assert_eq!(
        a.output_volume(),
        0,
        "the headphones forgot they were muted"
    );

    let saved = read_app_state(d.path());
    assert_eq!((saved.muted, saved.muted_hp), (false, true));
}

#[test]
fn plugging_headphones_in_leaves_the_speaker_s_mute_as_it_was() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, jack) = app_playing_with_jack(d.path(), "Emerald");
    assert!(!a.muted());
    jack.store(true, Ordering::Relaxed);
    settle(&mut a);
    assert!(!a.muted(), "the headphones came in muted");
    assert!(
        !read_app_state(d.path()).muted,
        "plugging in muted the speaker"
    );
    assert_eq!(a.output(), omega_zero::app::Output::Wired);
}
