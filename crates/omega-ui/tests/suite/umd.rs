//! The PSP's UMD on the shelf, as it is made: a round-topped shell with a round clear window
//! across its front, the disc's printed side filling it, and the shell's foot square.

use omega_store::{Cart, Platform, ShellChoice};
use omega_ui::{
    cart_face, holo_frames, model_box, model_of, shell_for, CartModel, Finish, HOLO_FRAMES, UMD_H,
    UMD_SHELL, UMD_W,
};

fn game(platform: Platform, shell: &str) -> Cart {
    Cart {
        platform,
        stem: "Monster Hunter Freedom Unite (USA)".into(),
        rom: "none.iso".into(),
        label: None,
        title: "Monster Hunter Freedom Unite".into(),
        code: String::new(),
        shell: ShellChoice::parse(shell),
    }
}

fn alpha(face: &omega_ui::CartFace, x: u32, y: u32) -> u8 {
    face.rgba[((y * face.w + x) * 4 + 3) as usize]
}

/// On the disc's print, in the window.
const ON_PRINT: (u32, u32) = (138, 200);
/// On the shell's foot, clear of the window.
const ON_FOOT: (u32, u32) = (24, 262);
/// Beside the top's half circle, off the UMD.
const OFF: (u32, u32) = (4, 4);

#[test]
fn a_psp_game_stands_as_a_umd_and_a_playstation_game_as_a_disc() {
    assert_eq!(model_of(&game(Platform::Psp, "")), CartModel::Umd);
    assert_eq!(model_of(&game(Platform::Ps1, "")), CartModel::Disc);
    assert_eq!(model_box(CartModel::Umd), (UMD_W, UMD_H));
}

#[test]
fn a_umd_comes_in_smoky_clear_plastic_unless_set_otherwise() {
    let shell = shell_for(&game(Platform::Psp, ""));
    assert_eq!(shell, UMD_SHELL);
    assert_eq!(shell.finish, Finish::Translucent);
    let red = shell_for(&game(Platform::Psp, "auto c2332e clear"));
    assert_eq!(red.colour, [0xc2, 0x33, 0x2e]);
}

#[test]
fn the_window_shows_the_disc_s_print_and_the_top_is_round() {
    let face = cart_face(&game(Platform::Psp, ""));
    assert_eq!((face.w, face.h), (UMD_W, UMD_H));
    assert_eq!(
        alpha(&face, ON_PRINT.0, ON_PRINT.1),
        255,
        "the shelf shows through the disc's print"
    );
    let foot = alpha(&face, ON_FOOT.0, ON_FOOT.1);
    assert!(foot > 0 && foot < 255, "the clear foot is {foot} opaque");
    assert_eq!(alpha(&face, OFF.0, OFF.1), 0, "the top is not round");
    // the foot is square: its corner is shell
    assert!(
        alpha(&face, 8, UMD_H - 12) > 0,
        "the foot's corner is cut away"
    );
}

#[test]
fn the_disc_s_print_catches_the_light_as_the_umd_turns_and_the_shell_does_not() {
    let frames = holo_frames(CartModel::Umd);
    assert_eq!(frames.len(), HOLO_FRAMES);
    let half = |(x, y): (u32, u32)| (x / 2, y / 2);
    let (px, py) = half(ON_PRINT);
    let lit: Vec<u8> = frames.iter().map(|f| alpha(f, px, py)).collect();
    assert!(lit.iter().all(|&a| a > 0), "a frame leaves the print bare");
    assert!(
        lit.iter().max() > lit.iter().min(),
        "the light does not move over the print: {lit:?}"
    );
    let (fx, fy) = half(ON_FOOT);
    assert!(
        frames.iter().all(|f| alpha(f, fx, fy) == 0),
        "the shell shines"
    );
}

/// A UMD's disc comes on its own, round and clear round its edge, for it to spin in the window
/// while the game loads; a cart that is no UMD has none.
#[test]
fn a_umd_s_disc_comes_on_its_own_and_round() {
    let disc = omega_ui::umd_disc(&game(Platform::Psp, "")).expect("a UMD's disc");
    let alpha = |x: u32, y: u32| disc.rgba[((y * disc.w + x) * 4 + 3) as usize];
    assert_eq!(alpha(0, 0), 0, "the disc's corner is not clear");
    assert_eq!(alpha(disc.w / 2, disc.h / 2), 255, "the hub is not there");
    assert_eq!(
        alpha(disc.w / 2, 4),
        255,
        "the print near its edge is not there"
    );
    assert!(omega_ui::umd_disc(&game(Platform::Gba, "")).is_none());
    let (x, y, w, h) = omega_ui::umd_window();
    assert!(
        x > 0.0 && y > 0.0 && x + w < 1.0 && y + h < 1.0,
        "the window is not in the face"
    );
}
