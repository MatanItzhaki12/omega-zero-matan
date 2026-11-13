//! The DISC screen SELECT + MENU opens over a PlayStation game on several discs: Left and Right
//! pick a disc, A swaps it into the drive and says so, B, SELECT or SELECT + MENU again close it
//! with the disc left in. A game on one disc says there is nothing to swap; a SNES game has no
//! link and no discs.
//!
//! With SCRATCH_PNG_DIR set, the screen is written there as disc-<name>.png.

use crate::common;
use crate::keys::root_with_games;

use std::time::{Duration, Instant};

use omega_input::{Action, Btn, Millis, RawEvent};
use omega_store::{write_app_state, AppState, Cart, Platform};
use omega_ui::kit::action::{action_pair, Font, Side};
use omega_ui::kit::bar::{bar_layout, hints_face};
use omega_ui::kit::palette::palette;
use omega_ui::{cart_face, out_h, out_w, Draw, TexId, Toast, UndoFace, SPRITE_PX};
use omega_zero::app::{App, Phase};
use omega_zero::disc_screen::{
    disc_action, disc_panel_face, disc_places, panel_at, DiscBar, DiscView, FADED, PICK, RAISE,
};
use omega_zero::screens::bar::BarSet;
use omega_zero::screens::lone::Lone;
use omega_zero::session::Session;
use tempfile::TempDir;

const GAME: &str = "Final Fantasy VII (USA)";

/// A card with a PlayStation game on three discs, a playlist naming them, and one on a disc of
/// its own.
fn card() -> TempDir {
    let d = root_with_games(Platform::Ps1, &["Ridge Racer (USA)"]);
    let dir = d.path().join("Games/PS1");
    std::fs::create_dir_all(dir.join("discs")).unwrap();
    let mut playlist = String::new();
    for n in 1..=3 {
        let disc = format!("discs/{GAME} (Disc {n}).cue");
        std::fs::write(dir.join(&disc), "FILE \"x.bin\" BINARY\n").unwrap();
        playlist.push_str(&disc);
        playlist.push('\n');
    }
    std::fs::write(dir.join(format!("{GAME}.m3u")), playlist).unwrap();
    d
}

fn playing(d: &TempDir, stem: &str) -> App {
    let mut a = common::app_playing_in(d.path(), stem);
    a.set_platform(Platform::Ps1);
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "{stem} is not playing: {:?}",
        a.phase()
    );
    a
}

fn view(at: usize, in_drive: usize) -> DiscView {
    DiscView {
        at,
        count: 3,
        in_drive,
        title: "Final Fantasy VII".into(),
    }
}

#[test]
fn select_and_menu_open_the_disc_screen_on_the_disc_in_the_drive() {
    let d = card();
    let mut a = playing(&d, GAME);
    a.apply(Action::GameMenu);
    assert_eq!(a.disc_view(), Some(view(0, 0)));
    assert!(a.disc_screen_open());
    assert!(a.game_menu_open(), "the game is not held under the screen");
    assert_eq!(
        a.game_menu(),
        None,
        "the link screen opened over a PlayStation game"
    );
}

#[test]
fn left_and_right_pick_a_disc_and_stop_at_either_end() {
    let d = card();
    let mut a = playing(&d, GAME);
    a.apply(Action::GameMenu);
    a.apply(Action::GbaDown(Btn::Left));
    assert_eq!(a.disc_view().unwrap().at, 0, "left wrapped round");
    for want in [1, 2, 2] {
        a.apply(Action::GbaDown(Btn::Right));
        assert_eq!(a.disc_view(), Some(view(want, 0)));
    }
    a.apply(Action::GbaDown(Btn::Left));
    assert_eq!(a.disc_view().unwrap().at, 1);
    assert_eq!(a.take_disc_swap(), None, "picking swapped a disc in");
}

#[test]
fn a_swaps_the_picked_disc_in_and_says_so() {
    let d = card();
    let mut a = playing(&d, GAME);
    a.apply(Action::GameMenu);
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaDown(Btn::A));
    assert!(!a.disc_screen_open(), "A left the screen open");
    assert_eq!(
        a.take_disc_swap(),
        Some(1),
        "the core was not asked for disc 2"
    );
    assert_eq!(a.take_disc_swap(), None, "the swap was handed over twice");
    assert_eq!(a.disc_in(), 1);
    assert_eq!(a.toast(), Some(Toast::Disc(2)));

    // it opens again on the disc in the drive, and A there swaps nothing
    a.apply(Action::GameMenu);
    assert_eq!(a.disc_view(), Some(view(1, 1)));
    a.update(3.0);
    a.apply(Action::GbaDown(Btn::A));
    assert!(!a.disc_screen_open());
    assert_eq!(
        a.take_disc_swap(),
        None,
        "the disc in the drive was swapped for itself"
    );
    assert_eq!(a.toast(), None, "a swap of nothing was toasted");
}

#[test]
fn b_select_and_select_menu_close_it_with_the_disc_left_in() {
    let d = card();
    let mut a = playing(&d, GAME);
    for close in [
        Action::GbaDown(Btn::B),
        Action::GbaDown(Btn::Select),
        Action::GameMenu,
    ] {
        a.apply(Action::GameMenu);
        a.apply(Action::GbaDown(Btn::Right));
        a.apply(close);
        assert!(!a.disc_screen_open(), "{close:?} left the screen open");
        assert_eq!(a.take_disc_swap(), None, "{close:?} swapped a disc");
        assert_eq!(a.disc_in(), 0);
    }
}

#[test]
fn a_game_on_one_disc_says_there_is_nothing_to_swap() {
    let d = card();
    let mut a = playing(&d, "Ridge Racer (USA)");
    a.apply(Action::GameMenu);
    assert!(
        !a.disc_screen_open(),
        "a game on one disc opened the screen"
    );
    assert!(!a.game_menu_open());
    assert_eq!(a.toast(), Some(Toast::OneDisc));
}

#[test]
fn a_snes_game_never_starts_a_link() {
    let d = root_with_games(Platform::Snes, &["Super Metroid", "Zelda"]);
    let mut a = common::app_playing_in(d.path(), "Super Metroid");
    a.set_platform(Platform::Snes);
    a.set_core(omega_store::Core::Snes9x);
    a.apply(Action::GameMenu);
    assert!(
        !a.game_menu_open(),
        "SELECT+MENU opened a screen over a SNES game"
    );
    assert_eq!(a.toast(), Some(Toast::NoLink));
}

#[test]
fn an_ejected_game_goes_back_in_on_its_first_disc() {
    let d = card();
    let mut a = playing(&d, GAME);
    a.apply(Action::GameMenu);
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(a.disc_in(), 1);

    // the screen holds the game as the link screen does: a MENU hold under it ejects nothing
    a.apply(Action::GameMenu);
    a.apply(Action::Eject);
    assert!(a.disc_screen_open(), "the eject went through the screen");
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    a.apply(Action::GbaDown(Btn::B));

    a.apply(Action::Eject);
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    a.apply(Action::Insert);
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
    assert_eq!(
        a.disc_in(),
        0,
        "the game went back in on the disc it left on"
    );
    assert_eq!(a.take_disc_swap(), None);
}

#[test]
fn the_session_hands_the_swap_to_the_core_and_holds_the_game_meanwhile() {
    let d = card();
    write_app_state(
        d.path(),
        &AppState {
            cart: Some(GAME.into()),
            cart_platform: Some(Platform::Ps1),
            clock_set: true,
            ..Default::default()
        },
    )
    .unwrap();
    let mut s = Session::boot(d.path().to_path_buf());
    let mut now: Millis = 0;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the game never came up");
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Select)]);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Menu)]);
    assert!(
        s.app().disc_screen_open(),
        "SELECT+MENU did not open the DISC screen"
    );
    step(
        &mut s,
        &mut now,
        &[RawEvent::Up(Btn::Menu), RawEvent::Up(Btn::Select)],
    );
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Right)]);
    assert_eq!(
        s.emu().unwrap().input().0,
        0,
        "the game was handed a press under the screen"
    );
    step(&mut s, &mut now, &[RawEvent::Up(Btn::Right)]);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::A)]);
    assert!(!s.app().disc_screen_open());
    assert_eq!(s.app().disc_in(), 1);
    assert_eq!(
        s.app_mut().take_disc_swap(),
        None,
        "the session never took the swap to the core"
    );
}

fn step(s: &mut Session, now: &mut Millis, events: &[RawEvent]) {
    *now += 16;
    s.feed(events.iter().copied(), *now);
    s.update(1.0 / 60.0);
}

#[test]
fn the_panel_says_which_disc_is_picked_of_how_many_and_which_is_in() {
    let pal = palette(0);
    let a = disc_panel_face(&view(1, 0), pal);
    let b = disc_panel_face(&view(2, 0), pal);
    let c = disc_panel_face(&view(1, 1), pal);
    assert_eq!(
        (a.w, a.h),
        (b.w, b.h),
        "the panel changed size with the disc"
    );
    assert_eq!(a.w, omega_zero::disc_screen::PANEL_W);
    assert_ne!(a.rgba, b.rgba, "the picked disc is not shown");
    assert_ne!(a.rgba, c.rgba, "the disc in the drive is not shown");
    assert_eq!(a.w % 2, 0);
}

#[test]
fn the_discs_stand_side_by_side_on_whole_pixels_the_picked_one_raised() {
    for count in 2..=5 {
        for at in 0..count {
            let places = disc_places(count, at, 60, 220);
            assert_eq!(places.len(), count);
            let page = (out_w() / SPRITE_PX) as i32;
            let y = places[(at + 1) % count][1];
            for (i, [x, py, w, h]) in places.iter().copied().enumerate() {
                assert!(
                    x >= 0 && x + w <= page,
                    "disc {i} of {count} is off the screen"
                );
                assert!(
                    py >= 60 && py + h <= 220,
                    "disc {i} of {count} leaves its room"
                );
                let want = if i == at { y - RAISE } else { y };
                assert_eq!(py, want, "disc {i} of {count} with {at} picked");
                if i > 0 {
                    assert!(places[i - 1][0] + places[i - 1][2] < x, "discs overlap");
                }
            }
            let left = places[0][0];
            let right = page - (places[count - 1][0] + places[count - 1][2]);
            assert!((left - right).abs() <= 1, "the row is off the middle");
        }
    }
}

/// The screen as the frontend has it: the panel for what it shows, the bar and the discs' faces.
struct Shown {
    faces: Vec<(TexId, UndoFace)>,
}

impl Shown {
    fn new(a: &mut App) -> Shown {
        let pal = palette(0);
        let mut faces: Vec<(TexId, UndoFace)> = Vec::new();
        let mut n = 600;
        let mut put = |face: UndoFace, faces: &mut Vec<(TexId, UndoFace)>| {
            n += 1;
            let tex = TexId::from_raw(n);
            let (w, h) = (face.w, face.h);
            faces.push((tex, face));
            (tex, w, h)
        };
        // every cart's face, as the frontend uploads them, a disc's included
        let carts: Vec<Cart> = a.carts().cloned().collect();
        let ids: Vec<TexId> = carts
            .iter()
            .map(|c| {
                let f = cart_face(c);
                put(
                    UndoFace {
                        rgba: f.rgba,
                        w: f.w,
                        h: f.h,
                    },
                    &mut faces,
                )
                .0
            })
            .collect();
        a.set_faces(ids);
        let [left, right] = action_pair(
            disc_action(Side::Left),
            disc_action(Side::Right),
            Font::Bold,
            pal,
        );
        let layout = bar_layout(out_w() / SPRITE_PX, &left, &right);
        let set = BarSet {
            layout,
            left: put(left.face, &mut faces),
            right: put(right.face, &mut faces),
        };
        let hints = put(hints_face(layout.hints[2], &[PICK], pal), &mut faces).0;
        a.set_disc_bar(DiscBar {
            set,
            hints: Some(hints),
        });
        Shown { faces }
    }

    fn panel(&mut self, a: &mut App) -> Lone {
        let face = disc_panel_face(&a.disc_view().unwrap(), palette(0));
        let tex = TexId::from_raw(700 + self.faces.len());
        let lone = Lone {
            tex,
            w: face.w,
            h: face.h,
        };
        self.faces.push((tex, face));
        a.set_disc_panel(lone);
        lone
    }

    /// What `a` draws, as its textures land on the screen.
    fn pixels(&self, a: &App, name: &str) -> (Vec<Draw>, Vec<u8>) {
        let mut out = Vec::new();
        a.draw(&mut out);
        let (w, h) = (out_w() as usize, out_h() as usize);
        let mut px = vec![0u8; w * h * 4];
        for d in &out {
            match *d {
                Draw::Rect { colour, .. } => {
                    for p in px.chunks_exact_mut(4) {
                        for k in 0..3 {
                            p[k] = (colour[k] * 255.0).round() as u8;
                        }
                        p[3] = 255;
                    }
                }
                Draw::Tex {
                    x,
                    y,
                    w: dw,
                    h: dh,
                    tex,
                    alpha,
                } => {
                    let Some((_, face)) = self.faces.iter().find(|(t, _)| *t == tex) else {
                        continue;
                    };
                    for qy in (y as i32).max(0)..((y + dh) as i32).min(h as i32) {
                        for qx in (x as i32).max(0)..((x + dw) as i32).min(w as i32) {
                            let fx = ((qx as f32 - x) * face.w as f32 / dw) as usize;
                            let fy = ((qy as f32 - y) * face.h as f32 / dh) as usize;
                            let s = (fy * face.w as usize + fx) * 4;
                            let a = face.rgba[s + 3] as f32 / 255.0 * alpha;
                            let o = (qy as usize * w + qx as usize) * 4;
                            for k in 0..3 {
                                px[o + k] = (face.rgba[s + k] as f32 * a
                                    + px[o + k] as f32 * (1.0 - a))
                                    .round() as u8;
                            }
                            px[o + 3] = 255;
                        }
                    }
                }
                _ => {}
            }
        }
        if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
            let path = format!("{dir}/disc-{name}.png");
            let file = std::fs::File::create(&path).unwrap();
            let mut e = png::Encoder::new(std::io::BufWriter::new(file), out_w(), out_h());
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&px).unwrap();
        }
        (out, px)
    }
}

#[test]
fn the_screen_draws_the_panel_the_discs_and_the_bar_over_the_backdrop() {
    let d = card();
    let mut a = playing(&d, GAME);
    let mut shown = Shown::new(&mut a);
    a.apply(Action::GameMenu);
    a.apply(Action::GbaDown(Btn::Right));
    let panel = shown.panel(&mut a);
    let (out, _) = shown.pixels(&a, "picking");

    assert!(
        matches!(out.first(), Some(Draw::Rect { .. } | Draw::Tex { .. })),
        "the backdrop is not under the screen"
    );
    assert!(
        !out.contains(&Draw::Game),
        "the game shows through the screen"
    );
    let [px, py] = panel_at(&panel);
    let s = SPRITE_PX as f32;
    assert!(
        out.iter().any(|d| matches!(*d, Draw::Tex { tex, x, y, .. }
            if tex == panel.tex && x == px as f32 * s && y == py as f32 * s)),
        "the panel is not where the LINK panel goes"
    );

    let face = a
        .carts()
        .position(|c| c.stem == GAME)
        .map(|i| TexId::from_raw(601 + i))
        .unwrap();
    let discs: Vec<(f32, f32, f32)> = out
        .iter()
        .filter_map(|d| match *d {
            Draw::Tex {
                tex, x, y, alpha, ..
            } if tex == face => Some((x, y, alpha)),
            _ => None,
        })
        .collect();
    assert_eq!(discs.len(), 3, "not a disc each: {discs:?}");
    for (i, (x, y, alpha)) in discs.iter().copied().enumerate() {
        assert_eq!((x % 2.0, y % 2.0), (0.0, 0.0), "disc {i} is off the grid");
        let want = if i == 1 { 1.0 } else { FADED };
        assert_eq!(alpha, want, "disc {i}'s alpha");
    }
    assert_eq!(
        discs[0].1 - discs[1].1,
        (RAISE * SPRITE_PX as i32) as f32,
        "the picked disc is not raised"
    );
    assert!(
        discs[0].1 >= (py as f32 + panel.h as f32) * s,
        "the discs run under the panel"
    );
}
