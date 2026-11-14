//! The cart editor SELECT+START opens on the shelf: its rows for the cart's system, each change
//! shown on the cart as it is made, Save keeping it in Labels/cart_shell.ini, B putting the cart
//! back as it was, and Original the cart's own look.

use crate::common::{boot, tmp_root_with_carts};
use omega_input::{Action, Btn};
use omega_store::{ShellChoice, ShellFinish};
use omega_zero::app::App;
use omega_zero::cart_editor::EditRow;

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

/// The look the cart picked has on the shelf now.
fn shown(a: &App) -> Option<ShellChoice> {
    let stem = a.selected_stem().expect("a cart picked").to_string();
    a.carts().find(|c| c.stem == stem).and_then(|c| c.shell)
}

/// Half a second of frames: the editor's slide out over.
fn settle(a: &mut App) {
    for _ in 0..30 {
        a.update(1.0 / 60.0);
    }
}

fn kept(d: &tempfile::TempDir) -> String {
    std::fs::read_to_string(d.path().join("Labels/cart_shell.ini")).unwrap_or_default()
}

#[test]
fn select_and_start_open_the_editor_with_the_carts_own_rows() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    assert!(a.keys().shelf, "the shelf takes SELECT+START");
    a.apply(Action::CartEditor);
    let ed = a.cart_editor().expect("the editor did not open");
    assert_eq!(ed.stem, "Emerald");
    assert_eq!(ed.row, EditRow::Colour, "a GBA cart has no shape to pick");
    let view = a.cart_editor_view().unwrap();
    assert_eq!(view.title, "GBA CART");
    let labels: Vec<&str> = view.lines.iter().map(|l| l.1.as_str()).collect();
    assert_eq!(
        labels,
        ["Colour", "Hue", "Shade", "Finish", "Original", "Save"]
    );
    assert!(!a.keys().shelf, "SELECT+START again while it is up");
}

#[test]
fn a_change_shows_on_the_cart_and_save_keeps_it_for_the_cards_next_look() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    assert_eq!(shown(&a), None);
    a.apply(Action::CartEditor);
    press(&mut a, Btn::Right);
    let picked = shown(&a).expect("the colour was not shown on the cart");
    // Finish, three rows down, on to the next
    for _ in 0..3 {
        press(&mut a, Btn::Down);
    }
    assert_eq!(a.cart_editor().unwrap().row, EditRow::Finish);
    press(&mut a, Btn::Right);
    let finished = shown(&a).unwrap();
    assert_ne!(finished.finish, picked.finish);
    // to Save, and A
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    assert!(
        kept(&d).contains(&format!("Emerald = {}", finished.to_value())),
        "{}",
        kept(&d)
    );
    // it closes, and the cart keeps the look
    settle(&mut a);
    assert!(a.cart_editor().is_none(), "the editor stayed");
    assert_eq!(shown(&a), Some(finished));
}

#[test]
fn b_puts_the_cart_back_and_original_keeps_its_own_look() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    a.apply(Action::CartEditor);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::Right);
    assert!(shown(&a).is_some());
    press(&mut a, Btn::B);
    assert_eq!(shown(&a), None, "B left the change on the cart");
    assert!(kept(&d).is_empty(), "B wrote to the card");

    // Original then Save: the cart's own look, whatever the other configs say
    settle(&mut a);
    a.apply(Action::CartEditor);
    press(&mut a, Btn::Right);
    for _ in 0..4 {
        press(&mut a, Btn::Down);
    }
    assert_eq!(a.cart_editor().unwrap().row, EditRow::Original);
    press(&mut a, Btn::A);
    assert_eq!(shown(&a), None);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    assert!(kept(&d).contains("Emerald = auto"), "{}", kept(&d));
}

#[test]
fn hue_turns_a_colour_and_shade_darkens_it() {
    use omega_zero::cart_editor::{hsv, HUE_STEP};
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    a.apply(Action::CartEditor);
    // a plain red, past the GBA's own colours
    while a.cart_editor().unwrap().value(EditRow::Colour) != "RED" {
        press(&mut a, Btn::Right);
    }
    let red = shown(&a).unwrap().colour;
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Right);
    let turned = shown(&a).unwrap().colour;
    let dh = (hsv(turned).0 - hsv(red).0).rem_euclid(360.0);
    assert!((dh - HUE_STEP).abs() < 2.0, "turned {dh}°");
    assert_eq!(a.cart_editor().unwrap().value(EditRow::Colour), "CUSTOM");
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Left);
    assert!(hsv(shown(&a).unwrap().colour).2 < hsv(turned).2);
    assert_eq!(shown(&a).unwrap().finish, ShellFinish::Solid);
}
