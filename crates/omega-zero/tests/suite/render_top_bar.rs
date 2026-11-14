#![cfg(any(target_os = "macos", target_os = "linux"))]
//! The top bar as the panel shows it: the clock, the battery and what the handheld is connected
//! to printed in the palette's cream, what is off in its plate colour, as flow.html's shelf;
//! a card's theme.txt keeping the white and grey it had; and a new colour in Settings >
//! Background printing it all again.

use crate::common;

use common::{clocked, tmp_root_with_carts, CLOCK_IS_SET};
use omega_gfx::{out_w, Compositor, HeadlessSurface};
use omega_input::{Action, Btn};
use omega_power::SimPlatform;
use omega_store::{colour_index, read_app_state, write_app_state, THEME_FILE};
use omega_ui::kit::palette::palette;
use omega_ui::{LINK_OFF, LINK_ON, TOP_BAR_H};
use omega_zero::app::Phase;
use omega_zero::frontend::Frontend;
use tempfile::TempDir;

/// A shelf in backdrop colour `colour`, with `theme` as the card's Config/theme.txt.
fn booted(colour: &str, theme: Option<&str>, c: &mut Compositor) -> (TempDir, Frontend) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let mut state = read_app_state(d.path());
    state.backdrop_colour = colour_index(colour).expect("a backdrop colour");
    write_app_state(d.path(), &state).unwrap();
    if let Some(theme) = theme {
        std::fs::create_dir_all(d.path().join("Config")).unwrap();
        std::fs::write(d.path().join("Config").join(THEME_FILE), theme).unwrap();
    }
    // a stopped clock, so every shelf prints the same time
    let platform = SimPlatform::at(d.path().to_path_buf()).stopped_at(CLOCK_IS_SET);
    let mut f = Frontend::boot(Box::new(platform));
    f.upload_faces(c);
    (d, f)
}

/// How many of the top bar's pixels, above its rule, are each of `inks`.
fn printed(f: &mut Frontend, c: &mut Compositor, inks: &[[u8; 3]]) -> Vec<usize> {
    f.app_mut().finish_slide();
    f.compose(c);
    let px = c.read_frame();
    let band = (out_w() as usize) * (TOP_BAR_H as usize - 2);
    inks.iter()
        .map(|ink| {
            px[..band * 4]
                .chunks(4)
                .filter(|p| p[..3] == ink[..])
                .count()
        })
        .collect()
}

fn press(f: &mut Frontend, btn: Btn) {
    f.app_mut().apply(Action::GbaDown(btn));
    f.app_mut().apply(Action::GbaUp(btn));
    f.app_mut().finish_slide();
}

#[test]
fn the_top_bar_prints_in_the_palettes_cream_and_plate() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let night = palette(colour_index("night").unwrap());
    let amber = palette(colour_index("amber").unwrap());
    let inks = [
        night.cream,
        night.plate,
        amber.cream,
        amber.plate,
        LINK_ON,
        LINK_OFF,
    ];

    let (_d, mut f) = booted("night", None, &mut c);
    let night_bar = printed(&mut f, &mut c, &inks);
    // the clock, the battery and its figure lit; Wi-Fi, Bluetooth and SSH off
    assert!(
        night_bar[0] > 200,
        "Night's cream is missing: {night_bar:?}"
    );
    assert!(night_bar[1] > 40, "Night's plate is missing: {night_bar:?}");
    assert_eq!(night_bar[2..], [0, 0, 0, 0], "Night: {night_bar:?}");

    let (_d, mut f) = booted("amber", None, &mut c);
    let amber_bar = printed(&mut f, &mut c, &inks);
    assert_eq!(
        amber_bar[2..4],
        night_bar[..2],
        "Amber prints other pixels than Night"
    );
    assert_eq!(amber_bar[..2], [0, 0], "Amber: {amber_bar:?}");
    assert_eq!(amber_bar[4..], [0, 0], "Amber: {amber_bar:?}");
}

#[test]
fn a_cards_theme_keeps_the_top_bar_white_whatever_the_colour() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let amber = palette(colour_index("amber").unwrap());
    let inks = [LINK_ON, LINK_OFF, amber.cream, amber.plate];
    let (_d, mut f) = booted("night", None, &mut c);
    let lit = printed(&mut f, &mut c, &[palette(0).cream, palette(0).plate]);

    let (_d, mut f) = booted("amber", Some("housing #102030\n"), &mut c);
    let themed = printed(&mut f, &mut c, &inks);
    assert_eq!(
        themed[..2],
        lit[..],
        "the theme's bar is not the white and grey: {themed:?}"
    );
    assert_eq!(
        themed[2..],
        [0, 0],
        "the theme's bar took Amber: {themed:?}"
    );
}

#[test]
fn a_new_colour_in_background_prints_the_top_bar_again() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let (_d, mut f) = booted("night", None, &mut c);
    let night = palette(0);
    let before = printed(&mut f, &mut c, &[night.cream]);
    assert!(before[0] > 0);

    f.app_mut().apply(Action::QuickMenu);
    f.app_mut().finish_slide();
    common::to_shelf_row(f.app_mut(), omega_zero::options::OptionRow::Theme);
    press(&mut f, Btn::A);
    for _ in 0..3 {
        if f.app().palette_colour() != 0 {
            break;
        }
        press(&mut f, Btn::Down);
        press(&mut f, Btn::Right);
    }
    let colour = f.app().palette_colour();
    assert_ne!(colour, 0, "the Background screen changed no colour");
    // THEME, SHELF, then Settings
    press(&mut f, Btn::B);
    press(&mut f, Btn::B);
    press(&mut f, Btn::B);
    assert!(matches!(f.app().phase(), Phase::Shelf));

    let picked = palette(colour);
    let after = printed(&mut f, &mut c, &[picked.cream, night.cream]);
    assert_eq!(
        after,
        [before[0], 0],
        "the top bar kept Night's cream after the colour changed"
    );
}
