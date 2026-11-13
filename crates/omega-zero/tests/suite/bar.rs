//! The bar along the bottom: which actions it shows where, how they swap between the shelf and
//! a menu, how it leaves for a game and comes back, the hints between its actions, the shelf's
//! system in its pill between them on the shelf and nothing else there, and the shelf's chrome in
//! the palette the player picked.

use crate::common;

use std::sync::{Arc, Mutex};

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use omega_input::{Action, Btn, Millis};
use omega_store::{write_app_state, AppState, Theme};
use omega_ui::kit::action::{action_pair, Font, Side, BAR, RISE};
use omega_ui::kit::bar::{bar_layout, BarLayout, Hint, SINK};
use omega_ui::kit::palette::palette;
use omega_ui::kit::pixel::quad;
use omega_ui::Keyboard;
use omega_ui::{out_h, out_w, ChromeColours, Draw, SettingsRow, TexId, SPRITE_PX, TOP_BAR_H};
use omega_zero::app::{App, Phase, SEATED_AT};
use omega_zero::core_picker::OPEN_MS;
use omega_zero::label_screen::LabelsScreen;
use omega_zero::label_sync::Account;
use omega_zero::nav::SLIDE_MS;
use omega_zero::screens::bar::{Actions, BarSet, SWAP};
use omega_zero::screens::bar::{INFO, SCAN};
use omega_zero::wifi::{Saved, Seen, WifiJob, WifiJobs, WifiNews};
use omega_zero::wifi_screen::{Sel, Typing, WifiScreen};
use tempfile::TempDir;

const SHELF_LEFT: usize = 800;
const SHELF_RIGHT: usize = 801;
const MENU_LEFT: usize = 810;
const MENU_RIGHT: usize = 811;
const CONFIRM_LEFT: usize = 820;
const CONFIRM_RIGHT: usize = 821;
const SHELF_HINTS: usize = 850;
const MENU_HINTS: usize = 851;
const CONFIRM_HINTS: usize = 852;
/// The shelf's system baked in its pill, and how wide it is in art pixels: "GBA"'s.
const PILL: usize = 860;
const PILL_W: u32 = 21;

/// How long the pill takes to rise into place with a new system or letter (app.rs's
/// PILL_IN_MS).
const PILL_IN_MS: Millis = 220;

/// How far into its way in a cart is when the bar has sunk out of the screen (screens/bar.rs's
/// SUNK_AT).
const SUNK_AT: f32 = 0.62;

/// A radio that comes up with Home saved, and whose scans hear Home and Café, one it has not.
#[derive(Clone, Default)]
struct Radio(Arc<Mutex<Vec<WifiNews>>>);

impl WifiJobs for Radio {
    fn present(&self) -> bool {
        true
    }

    fn ask(&mut self, job: WifiJob) {
        let mut news = self.0.lock().unwrap();
        match job {
            WifiJob::Up => {
                news.push(WifiNews::Started(true));
                news.push(WifiNews::Saved(vec![Saved {
                    id: 1,
                    ssid: b"Home".to_vec(),
                    current: false,
                    refused: false,
                }]));
            }
            WifiJob::Scan => news.push(WifiNews::Seen(
                [("Home", -48), ("Café", -60)]
                    .map(|(ssid, rssi)| Seen {
                        ssid: ssid.as_bytes().to_vec(),
                        rssi,
                        secured: true,
                    })
                    .to_vec(),
            )),
            _ => {}
        }
    }

    fn news(&mut self) -> Vec<WifiNews> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

fn booted(d: &TempDir) -> App {
    let mut a = booted_without_bar(d);
    fake_bar(&mut a);
    a
}

/// The same app before the bar's faces are made, which draws no bar.
fn booted_without_bar(d: &TempDir) -> App {
    write_app_state(
        d.path(),
        &AppState {
            clock_set: true,
            wifi: true,
            ..AppState::default()
        },
    )
    .unwrap();
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    a.set_wifi_jobs(Box::new(Radio::default()));
    a.tick_ms(16);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    a
}

fn on_shelf() -> (TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let a = booted(&d);
    (d, a)
}

/// Both sets of actions as the kit lays them out, each pair made together, uploaded as made-up
/// textures, and the hints between each.
fn fake_bar(a: &mut App) {
    let width = out_w() / SPRITE_PX;
    let pal = a.palette();
    let sets = Actions::ALL.map(|actions| {
        let [l, r] = action_pair(
            actions.action(Side::Left),
            actions.action(Side::Right),
            Font::Bold,
            pal,
        );
        let base = SHELF_LEFT + 10 * actions.index();
        BarSet {
            layout: bar_layout(width, &l, &r),
            left: (TexId::from_raw(base), l.face.w, l.face.h),
            right: (TexId::from_raw(base + 1), r.face.w, r.face.h),
        }
    });
    a.set_bar_sets(sets);
    a.set_bar_hints_face(Actions::Shelf, TexId::from_raw(SHELF_HINTS));
    a.set_bar_hints_face(Actions::Menu, TexId::from_raw(MENU_HINTS));
    a.set_bar_hints_face(Actions::Confirm, TexId::from_raw(CONFIRM_HINTS));
}

fn hint(key: &'static str, word: &'static str) -> Hint<'static> {
    Hint { key, word }
}

fn frame(a: &App) -> Vec<Draw> {
    let mut out = Vec::new();
    a.draw(&mut out);
    out
}

/// Where `tex` is drawn, its top left.
fn at(out: &[Draw], tex: usize) -> Option<(f32, f32)> {
    out.iter().find_map(|d| match *d {
        Draw::Tex { x, y, tex: t, .. } if t == TexId::from_raw(tex) => Some((x, y)),
        _ => None,
    })
}

/// How far the actions' faces stand above the screen's bottom edge at rest: the bar, and the
/// notch rising over it.
fn rest_y() -> f32 {
    BarLayout::top(out_h()) - (RISE * SPRITE_PX) as f32
}

/// Where the hints' top is on the screen: the bar's own, level with the actions' rails.
fn hints_y() -> f32 {
    BarLayout::top(out_h())
}

/// The actions shown, each with how far below its place it is in screen pixels.
fn shown(out: &[Draw]) -> Vec<(usize, f32)> {
    [
        SHELF_LEFT,
        SHELF_RIGHT,
        MENU_LEFT,
        MENU_RIGHT,
        CONFIRM_LEFT,
        CONFIRM_RIGHT,
    ]
    .into_iter()
    .filter_map(|tex| at(out, tex).map(|(_, y)| (tex, y - rest_y())))
    .collect()
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

/// The shelf's pill baked and handed over as sync_clock does, and risen into place.
fn pill_up(a: &mut App) {
    a.set_shelf_platform_face(TexId::from_raw(PILL), PILL_W);
    for _ in 0..30 {
        a.update(1.0 / 60.0);
    }
}

/// Where the pill is drawn, its top left, and its size.
fn pill(out: &[Draw]) -> Option<[f32; 4]> {
    out.iter().find_map(|d| match *d {
        Draw::Tex {
            x, y, w, h, tex, ..
        } if tex == TexId::from_raw(PILL) => Some([x, y, w, h]),
        _ => None,
    })
}

#[test]
fn the_shelf_shows_controls_and_settings_and_a_menu_return_and_confirm() {
    let (_d, mut a) = on_shelf();
    let out = frame(&a);
    assert_eq!(shown(&out), [(SHELF_LEFT, 0.0), (SHELF_RIGHT, 0.0)]);
    let set = a.bar_faces().unwrap().sets[Actions::Shelf.index()];
    let edge = (out_w() - set.right.1 * SPRITE_PX) as f32;
    assert_eq!(
        at(&out, SHELF_LEFT).unwrap().0,
        0.0,
        "CONTROLS is off the left edge"
    );
    assert_eq!(
        at(&out, SHELF_RIGHT).unwrap().0,
        edge,
        "SETTINGS is off the right edge"
    );
    assert_eq!(
        at(&out, SHELF_HINTS),
        Some(((set.left.1 * SPRITE_PX) as f32, hints_y())),
        "the hints do not start at CONTROLS's end"
    );

    a.apply(Action::QuickMenu);
    a.finish_slide();
    assert!(matches!(a.phase(), Phase::Settings { .. }));
    let out = frame(&a);
    assert_eq!(shown(&out), [(MENU_LEFT, 0.0), (MENU_RIGHT, 0.0)]);
    assert!(at(&out, MENU_HINTS).is_some(), "Settings has no hints");
    assert_eq!(at(&out, SHELF_HINTS), None);
}

/// The bar draws its hints, its two actions and on the shelf the system's pill, and nothing else,
/// so nothing but the hints and the pill is between the actions: a frame with the bar is the same
/// frame without it and those quads, the hints' across the room from the left action's end to the
/// right one's and the pill in the middle of the screen, on the shelf; a menu has no pill.
#[test]
fn the_bar_draws_nothing_between_its_actions_but_the_hints_and_the_shelfs_pill() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = booted(&d);
    let mut bare = booted_without_bar(&d);
    for app in [&mut a, &mut bare] {
        pill_up(app);
    }
    for (actions, hints) in [(Actions::Shelf, SHELF_HINTS), (Actions::Menu, MENU_HINTS)] {
        let set = a.bar_faces().unwrap().sets[actions.index()];
        let base = SHELF_LEFT + 10 * actions.index();
        let ours = [base, base + 1, hints, PILL].map(TexId::from_raw);
        let (with, without) = (frame(&a), frame(&bare));
        let (bar, rest): (Vec<Draw>, Vec<Draw>) = with
            .into_iter()
            .partition(|d| matches!(*d, Draw::Tex { tex, .. } if ours.contains(&tex)));
        assert_eq!(rest, without, "{actions:?}: the bar draws more");
        let px = |art: u32| (art * SPRITE_PX) as f32;
        let (width, lw, rw) = (out_w() / SPRITE_PX, set.left.1, set.right.1);
        let top = BarLayout::top(out_h());
        let mut want = vec![quad(ours[2], width - lw - rw, BAR, px(lw), top)];
        if actions == Actions::Shelf {
            want.push(quad(ours[3], PILL_W, BAR, px(width / 2 - PILL_W / 2), top));
        }
        want.extend([
            quad(ours[0], lw, set.left.2, 0.0, rest_y()),
            quad(ours[1], rw, set.right.2, px(width - rw), rest_y()),
        ]);
        assert_eq!(
            bar, want,
            "{actions:?}: the bar is not its hints between its two actions, and on the shelf the \
             pill"
        );
        for app in [&mut a, &mut bare] {
            app.apply(Action::QuickMenu);
            app.finish_slide();
        }
    }
}

/// The shelf's system stands in its pill in the middle of the bar, between CONTROLS and
/// SETTINGS, the bar's full height on the bar's top: on a card with one shelf as on one with
/// more.
#[test]
fn the_shelfs_system_is_a_pill_in_the_middle_between_the_actions() {
    for shelves in [1, 2] {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        if shelves == 2 {
            common::write_gb_cart(&d, "Tetris", "TETRIS");
        }
        let mut a = booted(&d);
        assert_eq!(a.pill_text().as_deref(), Some("GBA"));
        assert_eq!(pill(&frame(&a)), None, "a pill is drawn with no face baked");
        pill_up(&mut a);
        let out = frame(&a);
        let [x, y, w, h] = pill(&out).expect("no pill on the shelf");
        let art = SPRITE_PX as f32;
        assert_eq!(
            (w, h),
            (PILL_W as f32 * art, BAR as f32 * art),
            "the pill is not its face at the art grid's size"
        );
        assert_eq!(y, BarLayout::top(out_h()), "the pill is not on the bar");
        assert_eq!(x % art, 0.0, "the pill is off the art grid");
        let (left, _) = at(&out, SHELF_LEFT).unwrap();
        let (right, _) = at(&out, SHELF_RIGHT).unwrap();
        let set = a.bar_faces().unwrap().sets[Actions::Shelf.index()];
        let left_end = left + (set.left.1 * SPRITE_PX) as f32;
        assert!(
            x >= left_end && x + w <= right,
            "the pill {x}..{} is not between CONTROLS's end {left_end} and SETTINGS {right}",
            x + w
        );
        let (before, after) = (x, out_w() as f32 - (x + w));
        assert!(
            (before - after).abs() <= art,
            "the pill is not in the middle: {before} px before it and {after} after"
        );
    }
}

/// The pill rises into place as the system it shows comes, over PILL_IN_MS, on whole art pixels,
/// and rises again with the next system when a shoulder switches the shelf.
#[test]
fn the_pill_rises_into_place_with_each_system() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    common::write_gb_cart(&d, "Tetris", "TETRIS");
    let mut a = booted(&d);
    assert_eq!(a.pill_text().as_deref(), Some("GBA"));
    let top = BarLayout::top(out_h());
    for (round, tex) in [(0, PILL), (1, PILL + 1)] {
        a.set_shelf_platform_face(TexId::from_raw(tex), PILL_W);
        a.update(0.0);
        let from = a.now();
        let mut seen = Vec::new();
        while a.now() <= from + PILL_IN_MS + 50 {
            seen.push(frame(&a).iter().find_map(|d| match *d {
                Draw::Tex { y, tex: t, .. } if t == TexId::from_raw(tex) => Some(y),
                _ => None,
            }));
            a.update(1.0 / 120.0);
        }
        // sunk all the way it is out of the screen and not drawn; once drawn, it stays
        assert_eq!(
            seen[0], None,
            "round {round}: the pill did not start out of sight"
        );
        let drawn = seen.iter().position(Option::is_some);
        let drawn = drawn.expect("the pill is not drawn as it rises");
        assert!(
            seen[drawn..].iter().all(Option::is_some),
            "round {round}: the pill went out of sight as it rose: {seen:?}"
        );
        let ys: Vec<f32> = seen.into_iter().flatten().collect();
        assert!(
            ys[0] > top && ys[0] < top + (SINK * SPRITE_PX) as f32,
            "round {round}: the pill came in sight at {} rather than under its place",
            ys[0]
        );
        assert_eq!(
            *ys.last().unwrap(),
            top,
            "round {round}: the pill did not rise into its place"
        );
        assert!(
            ys.windows(2).all(|w| w[1] <= w[0]),
            "round {round}: the pill went back down as it rose: {ys:?}"
        );
        assert!(
            ys.iter().all(|y| (y - top) % SPRITE_PX as f32 == 0.0),
            "round {round}: the pill is off the art grid: {ys:?}"
        );
        let a_third = ys[ys.len() / 3];
        assert!(
            a_third > top,
            "round {round}: the pill was in its place a third of the way through its rise"
        );
        if round == 0 {
            a.apply(Action::GbaDown(Btn::R1));
            a.apply(Action::GbaUp(Btn::R1));
            assert_eq!(a.pill_text().as_deref(), Some("GB"));
            assert_eq!(
                pill(&frame(&a)),
                None,
                "the old system's pill stayed up after the switch"
            );
        }
    }
}

/// The pill goes with the shelf's actions while the core picker is open, and comes back with
/// them as it shuts.
#[test]
fn the_pill_is_not_shown_while_the_core_picker_is_open() {
    let (_d, mut a) = on_shelf();
    pill_up(&mut a);
    assert!(pill(&frame(&a)).is_some(), "no pill on the shelf");
    picker_faces(&mut a);
    a.apply(Action::GbaDown(Btn::X));
    a.apply(Action::GbaUp(Btn::X));
    assert!(a.core_picker().is_some(), "X opened no picker");
    for _ in 0..40 {
        a.update(1.0 / 60.0);
        assert_eq!(
            pill(&frame(&a)),
            None,
            "the pill shows over the open picker"
        );
    }
    press(&mut a, Btn::B);
    a.update(1.0);
    assert_eq!(a.core_picker(), None, "B did not close the picker");
    assert_eq!(
        pill(&frame(&a)).map(|p| p[1]),
        Some(BarLayout::top(out_h())),
        "the pill did not come back as the picker shut"
    );
}

/// Sliding from the shelf to a menu, the pill sinks with the shelf's actions over the slide's
/// first half; coming back, it rises with them over the second. It is never drawn without them.
#[test]
fn the_pill_sinks_and_rises_with_the_shelfs_actions_as_the_shelf_slides_to_a_menu_and_back() {
    let (_d, mut a) = on_shelf();
    pill_up(&mut a);
    let top = BarLayout::top(out_h());
    // MENU opens Settings from the shelf, as on Slot; B comes back
    for (open, way) in [(true, "leaving"), (false, "coming back")] {
        match open {
            true => a.apply(Action::QuickMenu),
            false => press(&mut a, Btn::B),
        }
        let mut moved = false;
        for _ in 0..24 {
            let out = frame(&a);
            let shelf = shown(&out).into_iter().find(|&(tex, _)| tex == SHELF_LEFT);
            match (pill(&out), shelf) {
                (Some([_, y, ..]), Some((_, sink))) => {
                    assert_eq!(
                        y - top,
                        sink,
                        "{way}, the pill is {} below its place and the actions {sink}",
                        y - top
                    );
                    moved |= sink > 0.0;
                }
                (None, None) => {}
                (pill, shelf) => {
                    panic!("{way}, the pill ({pill:?}) and the actions ({shelf:?}) went apart")
                }
            }
            a.update(SLIDE_MS as f32 / 1000.0 / 20.0);
        }
        a.finish_slide();
        assert!(moved, "{way}, the pill never moved with the actions");
    }
    assert!(pill(&frame(&a)).is_some(), "the pill did not come back");
}

/// As a cart goes in, the pill sinks with the actions, as far as they have, and is gone with
/// the bar; it rises back with them as the cart comes out.
#[test]
fn the_pill_sinks_with_the_actions_as_a_cart_goes_in_and_rises_back_on_eject() {
    let (_d, mut a) = on_shelf();
    pill_up(&mut a);
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Inserting { .. }));
    let top = BarLayout::top(out_h());
    let mut sank = false;
    for _ in 0..60 {
        let out = frame(&a);
        let actions = shown(&out);
        match (pill(&out), actions.first()) {
            (Some([_, y, ..]), Some(&(_, sink))) => {
                assert_eq!(
                    y - top,
                    sink,
                    "the pill is {} below its place and the actions {sink}, {} of the way in",
                    y - top,
                    a.seat()
                );
                sank |= sink > 0.0;
            }
            (None, None) => {}
            (pill, actions) => panic!(
                "the pill ({pill:?}) and the actions ({actions:?}) did not go together, {} of \
                 the way in",
                a.seat()
            ),
        }
        a.update(SEATED_AT / 40.0);
    }
    assert!(sank, "the pill never sank");
    assert_eq!(pill(&frame(&a)), None, "the pill stayed with the cart in");

    a.on_core_ready();
    a.update(1.0);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(pill(&frame(&a)), None, "the pill shows over the game");

    a.apply(Action::Eject);
    let mut rose = false;
    for _ in 0..200 {
        a.update(1.0 / 60.0);
        if matches!(a.phase(), Phase::Shelf) {
            break;
        }
        let out = frame(&a);
        if let (Some([_, y, ..]), Some(&(_, sink))) = (pill(&out), shown(&out).first()) {
            assert_eq!(y - top, sink, "the pill and the actions rose apart");
            rose = true;
        }
    }
    assert!(
        rose,
        "the pill did not rise with the actions as the cart came out"
    );
    assert_eq!(
        pill(&frame(&a)).map(|p| p[1]),
        Some(top),
        "the pill is not back in its place on the shelf"
    );
}

/// flow/player.js's sink: even((p / 0.5) · 38) over the first half, the new actions
/// even(((1 − p) / 0.5) · 38) over the second.
fn players_sink(p: f32) -> f32 {
    let even = |x: f32| 2.0 * (x / 2.0).round();
    let sink = 2.0 * SINK as f32;
    if p < 0.5 {
        even(p / 0.5 * sink)
    } else {
        even((1.0 - p) / 0.5 * sink)
    }
}

#[test]
fn the_shelfs_actions_sink_and_a_menus_rise_as_flow_does() {
    let (_d, mut a) = on_shelf();
    let start: Millis = 10_000;
    a.apply_at(Action::QuickMenu, start);
    let mut seen = Vec::new();
    for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
        a.tick_ms(start + (t * SLIDE_MS as f32) as Millis);
        let p = a.slide().map_or(1.0, |s| s.progress(a.now()));
        let out = frame(&a);
        let sink = players_sink(p);
        let want: Vec<(usize, f32)> = match (p < 0.5, sink < 2.0 * SINK as f32) {
            (_, false) => vec![],
            (true, _) => vec![(SHELF_LEFT, sink), (SHELF_RIGHT, sink)],
            (false, _) => vec![(MENU_LEFT, sink), (MENU_RIGHT, sink)],
        };
        assert_eq!(shown(&out), want, "at t {t} (p {p})");
        let hints = if p < 0.5 { SHELF_HINTS } else { MENU_HINTS };
        assert_eq!(
            at(&out, hints).map(|(_, y)| y),
            Some(hints_y()),
            "the hints moved at t {t}"
        );
        seen.push(want);
    }
    assert_eq!(
        seen,
        [
            vec![(SHELF_LEFT, 0.0), (SHELF_RIGHT, 0.0)],
            vec![(SHELF_LEFT, 8.0), (SHELF_RIGHT, 8.0)],
            vec![],
            vec![(MENU_LEFT, 8.0), (MENU_RIGHT, 8.0)],
            vec![(MENU_LEFT, 0.0), (MENU_RIGHT, 0.0)],
        ],
        "the swap is not where flow.html has it at a quarter, a half and three quarters"
    );
}

#[test]
fn going_back_to_the_shelf_swaps_them_the_other_way() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::QuickMenu);
    a.finish_slide();
    let start: Millis = 20_000;
    a.apply_at(Action::GbaDown(Btn::B), start);
    assert!(matches!(a.phase(), Phase::Shelf));
    a.tick_ms(start + SLIDE_MS / 4);
    assert_eq!(shown(&frame(&a)), [(MENU_LEFT, 8.0), (MENU_RIGHT, 8.0)]);
    a.tick_ms(start + 3 * SLIDE_MS / 4);
    assert_eq!(shown(&frame(&a)), [(SHELF_LEFT, 8.0), (SHELF_RIGHT, 8.0)]);
}

#[test]
fn between_two_menus_the_actions_stay_put() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::QuickMenu);
    a.finish_slide();
    while a.settings_row() != Some(SettingsRow::Network) {
        press(&mut a, Btn::Down);
    }
    let start: Millis = 20_000;
    a.apply_at(Action::GbaDown(Btn::A), start);
    assert!(matches!(a.phase(), Phase::Network(_)));
    for t in [0.25, 0.5, 0.75] {
        a.tick_ms(start + (t * SLIDE_MS as f32) as Millis);
        assert!(a.slide().is_some());
        assert_eq!(
            shown(&frame(&a)),
            [(MENU_LEFT, 0.0), (MENU_RIGHT, 0.0)],
            "the actions moved at t {t}"
        );
    }
}

#[test]
fn the_bar_sinks_as_a_cart_goes_in_stays_away_in_the_game_and_comes_back_on_eject() {
    let (_d, mut a) = on_shelf();
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Inserting { .. }));
    let dropped = |out: &[Draw]| at(out, SHELF_HINTS).map(|(_, y)| y - hints_y());
    let mut last = 0.0;
    let mut gone_at = None;
    for _ in 0..60 {
        let out = frame(&a);
        match dropped(&out) {
            Some(drop) => {
                assert!(drop >= last, "the bar came back up as the cart went in");
                assert_eq!(drop % SPRITE_PX as f32, 0.0, "the bar is off the art grid");
                for (tex, sink) in shown(&out) {
                    assert!(sink >= drop, "action {tex} stayed up as the bar sank");
                }
                last = drop;
            }
            None => {
                assert!(
                    shown(&out).is_empty(),
                    "the actions stayed without the hints"
                );
                gone_at.get_or_insert_with(|| a.seat());
            }
        }
        a.update(SEATED_AT / 40.0);
    }
    // gone, eased, by SUNK_AT, before the cart pulled down reaches it, and not in a blink
    let gone_at = gone_at.expect("the bar never left as the cart went in");
    assert!(
        gone_at > SUNK_AT / 2.0 && gone_at <= SUNK_AT,
        "the bar was gone at seat {gone_at}"
    );
    assert!(a.seat() >= 1.0);

    a.on_core_ready();
    a.update(1.0);
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
    let out = frame(&a);
    assert_eq!(dropped(&out), None, "the bar shows over the game");
    assert!(shown(&out).is_empty(), "the actions show over the game");

    a.apply(Action::Eject);
    assert!(matches!(a.phase(), Phase::Ejecting { .. }));
    let mut back = false;
    for _ in 0..200 {
        a.update(1.0 / 60.0);
        if matches!(a.phase(), Phase::Shelf) {
            break;
        }
        back |= dropped(&frame(&a)).is_some();
    }
    assert!(back, "the bar did not rise as the cart came out");
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    assert_eq!(
        shown(&frame(&a)),
        [(SHELF_LEFT, 0.0), (SHELF_RIGHT, 0.0)],
        "the bar is not back in its place on the shelf"
    );
}

#[test]
fn a_resumed_game_never_shows_the_bar() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_app_state(
        d.path(),
        &AppState {
            clock_set: true,
            cart: Some("Emerald".into()),
            ..AppState::default()
        },
    )
    .unwrap();
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    fake_bar(&mut a);
    assert!(matches!(a.phase(), Phase::Inserting { resumed: true, .. }));
    assert_eq!(a.bar_place(), None);
    assert!(shown(&frame(&a)).is_empty());
}

#[test]
fn the_core_pickers_cancel_and_choose_are_the_bars_return_and_confirm() {
    let (_d, mut a) = on_shelf();
    let id = TexId::from_raw;
    a.set_core_part_faces(
        vec![id(902), id(903)],
        vec![id(904), id(905)],
        id(906),
        [id(907), id(908)],
    );
    a.set_core_board_faces(id(900), id(901));
    a.apply(Action::GbaDown(Btn::X));
    a.update(0.0);
    assert!(a.core_picker().is_some(), "X opened no picker");
    a.update(0.07);
    let opening = shown(&frame(&a));
    assert!(
        opening
            .iter()
            .all(|&(tex, sink)| [SHELF_LEFT, SHELF_RIGHT].contains(&tex) && sink > 0.0),
        "the shelf's actions are not sinking as the picker opens: {opening:?}"
    );
    a.update(0.5);
    let out = frame(&a);
    assert_eq!(
        shown(&out),
        [(MENU_LEFT, 0.0), (MENU_RIGHT, 0.0)],
        "the open picker's B and A are not RETURN and CONFIRM"
    );
    assert!(
        at(&out, MENU_HINTS).is_some(),
        "the open picker has no hints"
    );
    assert_eq!(a.bar_hints_shown(), [SWAP], "the swap is not on the bar");

    press(&mut a, Btn::B);
    a.update(1.0);
    assert_eq!(a.core_picker(), None, "B did not close the picker");
    assert_eq!(
        shown(&frame(&a)),
        [(SHELF_LEFT, 0.0), (SHELF_RIGHT, 0.0)],
        "the shelf's actions did not come back as the picker shut"
    );
}

fn network_at(ssid: &str) -> Phase {
    Phase::Network(WifiScreen {
        sel: Sel::Network(ssid.as_bytes().to_vec()),
        ..WifiScreen::default()
    })
}

#[test]
fn network_puts_x_scan_on_the_bar_and_y_info_or_y_forget_beside_it() {
    let (_d, mut a) = on_shelf();
    a.tick_ms(1_000);
    assert!(
        a.wifi().entries().iter().any(|e| e.saved.is_some()),
        "the radio saved nothing"
    );
    assert_eq!(SCAN, hint("X", "SCAN"));
    assert_eq!(
        a.bar_hints(&network_at("Home")),
        [hint("X", "SCAN"), hint("Y", "FORGET")]
    );
    assert_eq!(a.bar_hints(&network_at("Café")), [hint("X", "SCAN")]);
    assert_eq!(INFO, hint("Y", "INFO"));
    for sel in [Sel::Wifi, Sel::Clock, Sel::Ssh] {
        let row = Phase::Network(WifiScreen {
            sel,
            ..WifiScreen::default()
        });
        assert_eq!(a.bar_hints(&row), [hint("X", "SCAN"), hint("Y", "INFO")]);
    }
    let typing = Phase::Network(WifiScreen {
        typing: Some(Typing {
            ssid: "Café".as_bytes().to_vec(),
            replace: None,
            keys: Keyboard::default(),
            refused: false,
        }),
        ..WifiScreen::default()
    });
    assert_eq!(
        a.bar_hints(&typing),
        [],
        "hints over the passphrase's keyboard"
    );
    assert_eq!(a.bar_hints(&Phase::Shelf), []);
}

#[test]
fn a_signed_in_account_puts_y_sign_out_on_the_bar() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let a = booted(&d);
    assert_eq!(a.bar_hints(&Phase::Labels(LabelsScreen::default())), []);
    Account {
        user: "zero".into(),
        password: "x".into(),
    }
    .save(d.path())
    .unwrap();
    let a = booted(&d);
    assert_eq!(
        a.bar_hints(&Phase::Labels(LabelsScreen::default())),
        [hint("Y", "SIGN OUT")]
    );
}

#[test]
fn the_bars_hints_change_halfway_through_a_slide() {
    let (_d, mut a) = on_shelf();
    a.tick_ms(1_000);
    a.apply(Action::QuickMenu);
    a.finish_slide();
    while a.settings_row() != Some(SettingsRow::Network) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    a.finish_slide();
    for _ in 0..10 {
        if a.network_forgets() {
            break;
        }
        press(&mut a, Btn::Down);
    }
    assert!(
        a.network_forgets(),
        "the cursor never reached a saved network"
    );
    let network = [hint("X", "SCAN"), hint("Y", "FORGET")];
    assert_eq!(a.bar_hints_shown(), network);
    let start: Millis = 30_000;
    a.apply_at(Action::GbaDown(Btn::B), start);
    a.tick_ms(start + SLIDE_MS / 4);
    assert_eq!(a.bar_hints_shown(), network, "Network's hints left early");
    a.tick_ms(start + 3 * SLIDE_MS / 4);
    assert_eq!(a.bar_hints_shown(), [], "Settings kept Network's hint");
}

fn top_bar_fill(a: &App) -> Option<[f32; 4]> {
    frame(a).iter().find_map(|d| match *d {
        Draw::Rect {
            x: 0.0,
            y: 0.0,
            w,
            h,
            colour,
        } if w == out_w() as f32 && h == TOP_BAR_H - 2.0 => Some(colour),
        _ => None,
    })
}

#[test]
fn a_new_colour_in_background_retints_the_top_bar() {
    let (_d, mut a) = on_shelf();
    let night = ChromeColours::of(palette(0));
    assert_eq!(
        a.chrome_colours(),
        night,
        "the shelf is not in Night's chrome"
    );
    assert_eq!(top_bar_fill(&a), Some(night.housing));

    a.apply(Action::QuickMenu);
    a.finish_slide();
    while a.settings_row() != Some(SettingsRow::Background) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    a.finish_slide();
    for _ in 0..3 {
        if a.palette_colour() != 0 {
            break;
        }
        press(&mut a, Btn::Down);
        press(&mut a, Btn::Right);
    }
    let colour = a.palette_colour();
    assert_ne!(colour, 0, "the Background screen changed no colour");
    let picked = ChromeColours::of(palette(colour));
    assert_ne!(picked, night);
    assert_eq!(a.chrome_colours(), picked);

    press(&mut a, Btn::B);
    a.finish_slide();
    press(&mut a, Btn::B);
    a.finish_slide();
    assert!(matches!(a.phase(), Phase::Shelf));
    assert_eq!(
        top_bar_fill(&a),
        Some(picked.housing),
        "the top bar kept Night"
    );
    let out = frame(&a);
    let night_left = out
        .iter()
        .filter(|d| matches!(**d, Draw::Rect { colour, .. } if colour == night.housing))
        .count();
    assert_eq!(night_left, 0, "something on the shelf kept Night's chrome");
}

#[test]
fn a_cards_theme_is_the_chrome_whatever_the_colour() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    std::fs::write(
        d.path().join("Config").join(omega_store::THEME_FILE),
        "housing #102030\n",
    )
    .unwrap();
    let a = booted(&d);
    let theme = Theme::parse("housing #102030\n");
    assert_eq!(a.chrome_colours(), ChromeColours::themed(&theme));
    assert_eq!(
        top_bar_fill(&a),
        Some(ChromeColours::themed(&theme).housing)
    );
}

#[test]
fn the_first_boot_clock_offers_only_confirm() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    fake_bar(&mut a);
    assert!(
        matches!(
            a.phase(),
            Phase::SetClock {
                from_menu: false,
                ..
            }
        ),
        "{:?}",
        a.phase()
    );
    let out = frame(&a);
    assert_eq!(
        shown(&out),
        [(CONFIRM_LEFT, 0.0), (CONFIRM_RIGHT, 0.0)],
        "the first clock offers a way back it has not got"
    );
    assert!(at(&out, CONFIRM_HINTS).is_some());
    assert_eq!(Actions::Confirm.action(Side::Left), ("", ""));
    assert_eq!(Actions::Confirm.action(Side::Right), ("CONFIRM", "A"));
}

fn picker_faces(a: &mut App) {
    let id = TexId::from_raw;
    a.set_core_part_faces(
        vec![id(902), id(903)],
        vec![id(904), id(905)],
        id(906),
        [id(907), id(908)],
    );
    a.set_core_board_faces(id(900), id(901));
}

/// ◀▶ SWAP is the picker's one key beyond the bar's actions: it comes in between RETURN and
/// CONFIRM with them, halfway through their swap as the picker opens, and goes with them
/// halfway through as it closes. The shelf's own bar never has it.
#[test]
fn the_swap_is_on_the_bar_only_while_the_picker_has_it() {
    let (_d, mut a) = on_shelf();
    picker_faces(&mut a);
    assert_eq!(a.bar_hints_shown(), [], "the shelf has a hint");
    assert_eq!(a.bar_hints(&Phase::Shelf), []);

    let start: Millis = 10_000;
    a.apply_at(Action::GbaDown(Btn::X), start);
    a.apply_at(Action::GbaUp(Btn::X), start);
    assert!(a.core_picker().is_some(), "X opened no picker");
    let mut came = None;
    for t in (0..=OPEN_MS).step_by(10) {
        a.tick_ms(start + t);
        let menu = a.bar_place().map(|p| p.actions) == Some(Actions::Menu);
        let hints = a.bar_hints_shown();
        assert_eq!(
            hints,
            if menu { vec![SWAP] } else { vec![] },
            "at {t} ms the swap is not with the picker's actions"
        );
        if menu {
            came.get_or_insert(t);
        }
    }
    let came = came.expect("the picker never took the bar");
    assert!(
        came > 0 && came < OPEN_MS,
        "the swap came at {came} ms, not partway through the opening"
    );
    assert_eq!(a.bar_hints(&Phase::Shelf), [SWAP]);

    let shut: Millis = start + 2 * OPEN_MS;
    a.tick_ms(shut);
    a.apply_at(Action::GbaDown(Btn::B), shut);
    a.apply_at(Action::GbaUp(Btn::B), shut);
    let mut left = None;
    for t in (0..=OPEN_MS).step_by(10) {
        a.tick_ms(shut + t);
        let menu = a.bar_place().map(|p| p.actions) == Some(Actions::Menu);
        assert_eq!(
            a.bar_hints_shown(),
            if menu { vec![SWAP] } else { vec![] },
            "at {t} ms into closing the swap is not with the picker's actions"
        );
        if !menu {
            left.get_or_insert(t);
        }
    }
    assert!(
        left.is_some_and(|t| t > 0),
        "the swap left with the picker still open"
    );
    a.update(1.0);
    assert_eq!(a.core_picker(), None, "B did not close the picker");
    assert_eq!(a.bar_hints_shown(), [], "the shelf kept the picker's swap");
}

/// The swap fits between RETURN and CONFIRM with room to spare, at 640 as at 720.
#[test]
fn the_swap_fits_between_return_and_confirm() {
    let (_d, a) = on_shelf();
    let room = a.bar_faces().unwrap().sets[Actions::Menu.index()]
        .layout
        .hints[2];
    let wide = omega_ui::kit::bar::hints_width(&[SWAP]);
    assert_eq!(
        room,
        if out_w() == 640 { 126 } else { 166 },
        "the room between the actions"
    );
    assert!(
        wide + 8 <= room,
        "the swap is {wide} wide in a room {room} wide"
    );
}

/// The save states over a game have the bar, B RETURN and A LOAD at its ends, from the moment
/// they open to the moment they close; the game under them has none.
#[test]
fn the_save_states_have_return_and_load_and_the_game_none() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = common::app_playing_in(d.path(), "Emerald");
    fake_bar(&mut a);
    a.apply_at(Action::SaveState, 1_000);
    assert_eq!(a.bar_place(), None, "the game has a bar");
    assert_eq!(Actions::Switcher.action(Side::Left), ("RETURN", "B"));
    assert_eq!(Actions::Switcher.action(Side::Right), ("LOAD", "A"));

    a.apply_at(Action::Polaroids, 1_100);
    let place = a.bar_place().expect("the save states have no bar");
    assert_eq!(
        (place.actions, place.sink, place.drop),
        (Actions::Switcher, 0, 0)
    );
    let set = a.bar_faces().unwrap().sets[Actions::Switcher.index()];
    let px = |art: u32| (art * SPRITE_PX) as f32;
    let right = px(out_w() / SPRITE_PX - set.right.1);
    let out = frame(&a);
    let (l, r) = (set.left, set.right);
    assert!(
        out.contains(&quad(l.0, l.1, l.2, 0.0, rest_y())),
        "no RETURN"
    );
    assert!(
        out.contains(&quad(r.0, r.1, r.2, right, rest_y())),
        "no LOAD"
    );

    a.apply_at(Action::GbaDown(Btn::B), 1_200);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(a.bar_place(), None, "the bar stayed over the game");
}

/// The save states' bar has no hints of its own between RETURN and LOAD: X DELETE stands on a
/// line of its own there (polaroids.rs), since bare hints would sit on the game's screenshot.
#[test]
fn nothing_of_the_bars_own_is_between_return_and_load() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = common::app_playing_in(d.path(), "Emerald");
    a.apply_at(Action::SaveState, 1_000);
    a.apply_at(Action::Polaroids, 1_100);
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
    assert!(a.bar_hints(a.phase()).is_empty(), "the bar has hints");
    assert!(a.bar_hints_shown().is_empty(), "the bar shows hints");
}

/// Every pair of hints the Bluetooth screen shows fits between RETURN and CONFIRM at 640, the
/// narrowest screen: on headphones connected, START MENU takes X SCAN's place.
#[test]
fn bluetooth_s_hints_fit_between_return_and_confirm() {
    use omega_zero::screens::bar::{FORGET, HEADPHONES};
    let (_d, a) = on_shelf();
    let room = a.bar_faces().unwrap().sets[Actions::Menu.index()]
        .layout
        .hints[2];
    let narrowest = if out_w() == 640 { room } else { room - 40 };
    for hints in [[SCAN, FORGET], [FORGET, HEADPHONES]] {
        let wide = omega_ui::kit::bar::hints_width(&hints);
        assert!(
            wide + 8 <= narrowest,
            "{hints:?} are {wide} wide in a room {narrowest} wide"
        );
    }
}
