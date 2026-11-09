#![cfg(any(target_os = "macos", target_os = "linux"))]

use crate::common;

use std::collections::VecDeque;
use std::path::Path;

use common::{clocked, repo_root, tmp_root_with_carts};
use omega_gfx::{out_h, out_w, Compositor, HeadlessSurface};
use omega_input::{Action, Btn, InputSource, Millis, RawEvent};
use omega_power::SimPlatform;
use omega_store::{Cart, Platform as CartPlatform};
use omega_ui::kit::action::{pill_face, BAR};
use omega_ui::kit::palette::Palette;
use omega_ui::{
    cart_box, cart_face, clean_label, label_colour, label_text, rest_y, shelf_box, shelf_scale,
    stage_x, stage_y, sway_reach, CartDive, Draw, Pose, Shelf, CART_W, GB_LABEL_H, GB_LABEL_Y,
    LABEL_H, LABEL_Y, SPRITE_PX, TOP_BAR_H,
};
use omega_zero::app::{App, SEATED_AT};
use omega_zero::frontend::Frontend;

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

fn tap(f: &mut Frontend, input: &mut Script, btn: Btn) {
    input.0.push_back(vec![RawEvent::Down(btn)]);
    f.advance(input);
    input.0.push_back(vec![RawEvent::Up(btn)]);
    f.advance(input);
}

fn at(px: &[u8], x: usize, y: usize) -> [u8; 3] {
    let o = (y * out_w() as usize + x) * 4;
    [px[o], px[o + 1], px[o + 2]]
}

fn patch(px: &[u8], x: usize, y: usize) -> [u32; 3] {
    let mut sum = [0u32; 3];
    let mut n = 0;
    for py in y - 6..y + 6 {
        for qx in x - 20..x + 20 {
            let c = at(px, qx, py);
            for (k, v) in c.iter().enumerate() {
                sum[k] += *v as u32;
            }
            n += 1;
        }
    }
    [sum[0] / n, sum[1] / n, sum[2] / n]
}

fn apart(a: [u32; 3], b: [u32; 3]) -> u32 {
    (0..3).map(|k| a[k].abs_diff(b[k])).sum()
}

/// Lit pixels across the top of the shelf, between the bar and the tallest cart, where a
/// name banner would go.
/// Lit pixels over a lone pak, where a banner would go: clear of the top bar, and of the
/// highest the pak reaches as it sways.
fn banner_ink(px: &[u8]) -> usize {
    let top = (TOP_BAR_H + stage_y()) as usize + 1;
    let (w, h) = shelf_box(CartPlatform::Gb);
    let reach = sway_reach(w, h).ceil();
    (top..(rest_y(h) - reach) as usize)
        .flat_map(|y| (200..520).map(move |x| (x, y)))
        .filter(|(x, y)| at(px, *x, *y).iter().all(|c| *c > 0x80))
        .count()
}

/// Lit pixels on the bar across the top, where the battery and the clock are printed.
fn hud_ink(px: &[u8]) -> usize {
    (0..TOP_BAR_H as usize)
        .flat_map(|y| (0..out_w() as usize).map(move |x| (x, y)))
        .filter(|(x, y)| at(px, *x, *y).iter().all(|c| *c > 0x80))
        .count()
}

/// Where the shelf's pill showing `text` stands, its top left in screen pixels, and its face: in
/// the middle of the bar across the bottom, on the bar's top, between its actions.
fn pill_at(text: &str, pal: &Palette) -> (usize, usize, omega_ui::UndoFace) {
    let face = pill_face(text, pal);
    let x = (out_w() / SPRITE_PX / 2 - face.w / 2) * SPRITE_PX;
    let y = out_h() - BAR * SPRITE_PX;
    (x as usize, y as usize, face)
}

/// How many of the pill's own pixels, showing `text` as the kit bakes it, are not on the screen
/// where the bar puts it.
fn pill_misses(px: &[u8], text: &str, pal: &Palette) -> usize {
    let (x0, y0, face) = pill_at(text, pal);
    let art = SPRITE_PX as usize;
    (0..face.h as usize)
        .flat_map(|j| (0..face.w as usize).map(move |i| (i, j)))
        .filter(|(i, j)| {
            let o = (j * face.w as usize + i) * 4;
            face.rgba[o + 3] == 255 && {
                let want = [face.rgba[o], face.rgba[o + 1], face.rgba[o + 2]];
                let got = at(px, x0 + i * art + art / 2, y0 + j * art + art / 2);
                (0..3).any(|k| got[k].abs_diff(want[k]) > 8)
            }
        })
        .count()
}

/// The systems a shelf's pill can show.
const SYSTEMS: [&str; 3] = ["GBA", "GB", "GBC"];

/// Whether the pill on the screen shows `text`, every pixel of it, and none of the other
/// systems.
fn pill_shows(px: &[u8], text: &str, pal: &Palette) -> bool {
    pill_misses(px, text, pal) == 0
        && SYSTEMS
            .iter()
            .filter(|other| **other != text)
            .all(|other| pill_misses(px, other, pal) > 0)
}

fn let_the_name_in(f: &mut Frontend, c: &mut Compositor, input: &mut Script) {
    f.compose(c);
    f.advance(input);
    std::thread::sleep(std::time::Duration::from_millis(300));
    f.advance(input);
}

fn composed(f: &mut Frontend, c: &mut Compositor, name: &str) -> Vec<u8> {
    f.compose(c);
    let px = c.read_frame();
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let path = format!("{dir}/shelves-{name}.png");
        let file = std::fs::File::create(&path).expect("create png");
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), out_w(), out_h());
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .expect("png header")
            .write_image_data(&px)
            .expect("png data");
        println!("wrote {path}");
    }
    px
}

fn put_game_boy_carts(root: &Path) {
    for (dir, stem, ext, cgb) in [
        ("GB", "Tetris Rosy Retrospection", "gb", 0x00u8),
        ("GBC", "Tetris Chromatic", "gbc", 0xc0),
    ] {
        let from = repo_root().join(format!("sdcard/Games/{dir}/{stem}.{ext}"));
        let to = root.join(format!("Games/{dir}/{stem}.{ext}"));
        match std::fs::read(&from) {
            Ok(rom) => std::fs::write(&to, rom).expect("copy the card's cart"),
            Err(_) => std::fs::write(&to, gb_rom(cgb)).expect("write a stand-in cart"),
        }
    }
}

fn gb_rom(cgb: u8) -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x143] = cgb;
    rom
}

/// A point `down` face pixels from the top of the pak standing alone on its shelf, at the
/// size the shelf holds it.
fn on_the_lone_pak(down: u32) -> (usize, usize) {
    let scale = shelf_scale(CartPlatform::Gb);
    (
        (out_w() / 2) as usize,
        (rest_y(shelf_box(CartPlatform::Gb).1) + down as f32 * scale) as usize,
    )
}

fn alone() -> (usize, usize) {
    on_the_lone_pak(GB_LABEL_Y / 2)
}

fn alone_label() -> (usize, usize) {
    on_the_lone_pak(GB_LABEL_Y + GB_LABEL_H / 2)
}

/// Where the left and right neighbours and the selection stand, on the 720x480 mockup and
/// so wherever the stage puts them.
fn on_stage(x: f32, y: f32) -> (usize, usize) {
    ((x + stage_x()) as usize, (y + stage_y()) as usize)
}

fn side_left() -> (usize, usize) {
    on_stage(90.0, 250.0)
}

fn side_right() -> (usize, usize) {
    on_stage(630.0, 250.0)
}

fn middle() -> (usize, usize) {
    on_stage(360.0, 250.0)
}
const GROUND: [u32; 3] = [0x05, 0x05, 0x08];

#[test]
fn the_shoulders_ring_over_a_shelf_for_each_system() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    put_game_boy_carts(d.path());
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    f.advance(&mut input);
    let_the_name_in(&mut f, &mut c, &mut input);

    let gba = composed(&mut f, &mut c, "gba");
    let pal = f.app().palette();
    assert_eq!(
        banner_ink(&gba),
        0,
        "the carousel named a system nobody had switched to"
    );
    assert!(
        pill_shows(&gba, "GBA", pal),
        "the bar came up without the GBA pill between its actions"
    );
    let left = patch(&gba, side_left().0, side_left().1);
    let right = patch(&gba, side_right().0, side_right().1);
    let middle = patch(&gba, middle().0, middle().1);
    for (name, slot) in [("left", left), ("middle", middle), ("right", right)] {
        assert!(
            apart(slot, GROUND) > 60,
            "the {name} slot of a two-cart shelf is bare ground: {slot:?}"
        );
    }
    assert!(
        apart(left, right) < 30,
        "the two carts did not repeat around the ring: the side slots hold {left:?} and \
         {right:?}, which are different carts"
    );
    assert!(
        apart(left, middle) > 60,
        "the repeat put the selected cart beside itself: {left:?} either side of {middle:?}"
    );

    let mut seen = Vec::new();
    for (name, banner, system) in [
        ("game-boy", "Game Boy", "GB"),
        ("game-boy-color", "Game Boy Color", "GBC"),
    ] {
        tap(&mut f, &mut input, Btn::R1);
        let_the_name_in(&mut f, &mut c, &mut input);
        let px = composed(&mut f, &mut c, name);
        let (ax, ay) = alone();
        let cart = patch(&px, ax, ay);
        let (lx, ly) = alone_label();
        let label = patch(&px, lx, ly);
        assert!(
            apart(cart, GROUND) > 60,
            "no cart in the middle of the {banner} shelf: {cart:?}"
        );
        for (side, (x, y)) in [("left", side_left()), ("right", side_right())] {
            let beside = patch(&px, x, y);
            assert!(
                apart(beside, GROUND) < 30,
                "the {banner} shelf holds one cart but drew something on its {side}: {beside:?}"
            );
        }
        assert_eq!(
            banner_ink(&px),
            0,
            "the {banner} shelf banner'd its name over the carts"
        );
        assert!(
            pill_shows(&px, system, pal),
            "the {banner} shelf did not put {system}, and only it, in the pill"
        );
        assert!(
            seen.iter()
                .all(|(c, l)| apart(cart, *c) + apart(label, *l) > 60),
            "{banner} is showing a cart another shelf already showed: {cart:?} in {label:?}"
        );
        seen.push((cart, label));
    }

    tap(&mut f, &mut input, Btn::R1);
    let_the_name_in(&mut f, &mut c, &mut input);
    let back = composed(&mut f, &mut c, "gba-again");
    assert!(
        apart(patch(&back, side_left().0, side_left().1), left) < 30,
        "the ring did not come back to the cart the first shelf was left on"
    );
    assert_eq!(banner_ink(&back), 0, "the way back put a banner up");
    assert!(
        pill_shows(&back, "GBA", pal),
        "the ring came back to the Game Boy Advance shelf under another system's pill"
    );

    // the pill does not fade: the system stays in it
    std::thread::sleep(std::time::Duration::from_millis(2400));
    f.advance(&mut input);
    let later = composed(&mut f, &mut c, "gba-later");
    assert!(
        pill_shows(&later, "GBA", pal),
        "the GBA pill went from the bar"
    );
}

#[test]
fn a_card_nobody_has_organised_comes_up_an_empty_shelf() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };

    let d = tmp_root_with_carts(&[]);
    std::fs::write(d.path().join("Games/Emerald.gba"), vec![0u8; 0x100]).expect("loose rom");
    std::fs::write(d.path().join("Saves/Emerald.sav"), vec![7u8; 0x10000]).expect("loose save");
    std::fs::write(d.path().join("Labels/Emerald.png"), b"png").expect("loose label");
    clocked(d.path());

    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    f.advance(&mut input);
    let empty = composed(&mut f, &mut c, "loose-card");

    let plate = patch(&empty, 150, 470 + stage_y() as usize);
    assert!(
        apart(plate, GROUND) > 20,
        "the bottom plate is not there, so this is a blank screen rather than an empty shelf: \
         {plate:?}"
    );
    assert!(
        hud_ink(&empty) > 100,
        "the plate came up with no battery and no clock printed on it: {} lit pixels",
        hud_ink(&empty)
    );
    for (name, (x, y)) in [
        ("left", side_left()),
        ("middle", middle()),
        ("right", side_right()),
    ] {
        let slot = patch(&empty, x, y);
        assert!(
            apart(slot, GROUND) < 30,
            "the {name} slot of an unorganised card is holding something: {slot:?}"
        );
    }

    for _ in 0..60 {
        f.advance(&mut input);
    }
    let later = composed(&mut f, &mut c, "loose-card-later");
    for (name, (x, y)) in [
        ("left", side_left()),
        ("middle", middle()),
        ("right", side_right()),
    ] {
        let slot = patch(&later, x, y);
        assert!(
            apart(slot, GROUND) < 30,
            "a cart appeared in the {name} slot a second after an unorganised card booted: \
             {slot:?}"
        );
    }

    let organised = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(organised.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(organised.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.advance(&mut input);
    let full = composed(&mut f, &mut c, "organised-card");
    let middle = patch(&full, middle().0, middle().1);
    assert!(
        apart(middle, GROUND) > 60,
        "the organised card's own shelf is bare too, so this test cannot see a cart at all: \
         {middle:?}"
    );
}

/// A card with one shelf has nowhere to switch to, but its pill still names its system, and a
/// shoulder press leaves it be. A letter jumped to shows in the pill in the system's place.
#[test]
fn a_card_on_one_shelf_still_names_its_system_in_the_pill() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };

    let one = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(one.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(one.path().to_path_buf())));
    f.upload_faces(&mut c);
    let pal = f.app().palette();
    let mut input = Script(VecDeque::new());
    f.advance(&mut input);
    let_the_name_in(&mut f, &mut c, &mut input);
    let bare = composed(&mut f, &mut c, "one-shelf");
    assert!(
        pill_shows(&bare, "GBA", pal),
        "a card with one shelf did not name its system in the pill"
    );
    tap(&mut f, &mut input, Btn::R1);
    let_the_name_in(&mut f, &mut c, &mut input);
    let pressed = composed(&mut f, &mut c, "one-shelf-after-r1");
    assert!(
        pill_shows(&pressed, "GBA", pal),
        "R1 changed the pill on a card that has one shelf"
    );

    tap(&mut f, &mut input, Btn::Down);
    assert_eq!(f.app().selected_stem(), Some("Fusion"));
    let_the_name_in(&mut f, &mut c, &mut input);
    let jumped = composed(&mut f, &mut c, "one-shelf-letter");
    assert_eq!(
        pill_misses(&jumped, "F", pal),
        0,
        "the letter jumped to is not in the pill"
    );
    assert!(
        pill_misses(&jumped, "GBA", pal) > 0,
        "the system stayed in the pill over the letter jumped to"
    );

    let two = tmp_root_with_carts(&["Emerald", "Fusion"]);
    put_game_boy_carts(two.path());
    clocked(two.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(two.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.advance(&mut input);
    let_the_name_in(&mut f, &mut c, &mut input);
    let marked = composed(&mut f, &mut c, "two-shelves");
    assert!(
        pill_shows(&marked, "GBA", pal),
        "a card with two shelves did not say which one it was on"
    );
}

#[test]
fn a_cart_going_in_from_a_repeated_row_takes_both_copies_of_its_neighbour_with_it() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let mut app = App::boot(d.path());
    let ink = label_colour(&clean_label("Emerald"));

    let standing = shot(&app, &mut c, Some("insert-0-standing"));
    let (from, _) = span(&standing, ink);
    let west = patch(&standing, side_left().0, side_left().1);
    let east = patch(&standing, side_right().0, side_right().1);
    app.apply(Action::Insert);
    app.update(SEATED_AT / 2.0);
    let halfway = shot(&app, &mut c, Some("insert-1-halfway"));
    let (mid, y) = span(&halfway, ink);
    app.update(SEATED_AT * 0.3);
    let diving = shot(&app, &mut c, Some("insert-2-diving"));
    let (low, low_y) = span(&diving, ink);
    for _ in 0..120 {
        app.update(1.0 / 60.0);
    }
    let seated = shot(&app, &mut c, Some("insert-3-in"));

    for (side, slot) in [("left", west), ("right", east)] {
        assert!(
            apart(slot, GROUND) > 60,
            "the {side} of the selection is bare ground: {slot:?}"
        );
    }
    assert!(
        apart(west, east) < 30,
        "the other cart was not repeated on both sides of the selection: {west:?} and {east:?}"
    );
    assert!(
        (from - out_w() as f32 / 2.0).abs() < 8.0,
        "the row did not stand its selection in the middle: {from}"
    );
    assert!(
        (mid - from).abs() < 8.0 && (low - from).abs() < 8.0,
        "the cart slid sideways on its way in: {from} then {mid} then {low}"
    );
    assert!(
        low_y > y,
        "the cart was not pulled down into the handheld: its top at {y} then {low_y}"
    );
    assert_eq!(
        paper_rows(&seated, ink),
        None,
        "the cart is still on the screen all the way in"
    );
    for (side, (x, y)) in [("left", side_left()), ("right", side_right())] {
        let slot = patch(&seated, x, y);
        assert!(
            apart(slot, GROUND) < 30,
            "the {side} copy of the other cart is still on the row with the chosen one in \
             the handheld: {slot:?}"
        );
    }
}

#[test]
fn a_scrolled_row_slides_by_a_pitch_rather_than_swapping_its_carts() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    for (carts, stems, least) in [
        (2, &["Emerald", "Fusion", ""][..2], 2),
        (3, &["Emerald", "Fusion", "Sapphire"][..], 2),
    ] {
        let d = tmp_root_with_carts(stems);
        clocked(d.path());
        let mut app = App::boot(d.path());
        let mut frames = vec![shot(&app, &mut c, Some(&format!("scroll-{carts}-00")))];
        app.apply(Action::ShelfRight);
        app.apply(Action::GbaUp(Btn::Right));
        for f in 1..=30 {
            app.update(1.0 / 60.0);
            let name = (f % 4 == 0).then(|| format!("scroll-{carts}-{f:02}"));
            frames.push(shot(&app, &mut c, name.as_deref()));
        }

        let mut stood = 1.0f32;
        let mut frames_since = 1.0f32;
        for (f, px) in frames.iter().enumerate() {
            let runs = row_runs(px);
            assert!(
                (least..=4).contains(&runs.len()),
                "{carts} carts, frame {f}: {} carts on screen, not the {least} to four a \
                 {} px row of them holds",
                runs.len(),
                out_w()
            );
            let row: Vec<f32> = runs
                .iter()
                .filter(|(a, b)| *a > 0 && *b < out_w() as usize - 1)
                .map(|(a, b)| ((a + b) as f32 / 2.0 - out_w() as f32 / 2.0) / CART_W as f32)
                .collect();
            if row.is_empty() && out_w() < 720 {
                // Mid-scroll, a row narrower than the mockup can hold no cart whole.
                frames_since += 1.0;
                continue;
            }
            assert!(
                !row.is_empty(),
                "{carts} carts, frame {f}: only {} carts are wholly on screen, so there is \
                 nothing to compare the row against itself with",
                row.len()
            );
            let phase = row[0] - row[0].round();
            for q in &row {
                assert!(
                    (q - q.round() - phase).abs() < 0.03,
                    "{carts} carts, frame {f}: a cart stands at {q} pitches while another \
                     stands at {}, so this is not one row moving",
                    row[0]
                );
            }
            let now = [phase - 1.0, phase, phase + 1.0]
                .into_iter()
                .fold(f32::MAX, |a, b| {
                    if (b - stood).abs() < (a - stood).abs() {
                        b
                    } else {
                        a
                    }
                });
            assert!(
                now <= stood + 0.01,
                "{carts} carts, frame {f}: the row turned round, from {stood} to {now}"
            );
            assert!(
                stood - now < 0.12 * frames_since,
                "{carts} carts, frame {f}: the row jumped {} of a pitch, which is a cart \
                 teleporting rather than sliding",
                stood - now
            );
            stood = now;
            frames_since = 1.0;
        }
        assert!(
            stood.abs() < 0.01,
            "{carts} carts: the row came to rest {stood} of a pitch off the middle"
        );
    }
}

fn row_runs(px: &[u8]) -> Vec<(usize, usize)> {
    let lit = |x: usize| {
        (210..300).any(|y| {
            let c = at(px, x, y + stage_y() as usize);
            c.iter().map(|v| *v as u32).sum::<u32>() > 60
        })
    };
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for x in 0..out_w() as usize {
        match runs.last_mut() {
            Some(run) if run.1 + 1 == x && lit(x) => run.1 = x,
            _ if lit(x) => runs.push((x, x)),
            _ => {}
        }
    }
    runs
}

/// Where the cart in `ink` is across, the middle of its columns, and the top row of it.
fn span(px: &[u8], ink: [u8; 3]) -> (f32, usize) {
    let close = |c: [u8; 3]| (0..3).all(|k| c[k].abs_diff(ink[k]) <= 24);
    let mut cols: Vec<usize> = Vec::new();
    let mut top = None;
    for y in 0..out_h() as usize {
        for x in 0..out_w() as usize {
            if close(at(px, x, y)) {
                cols.push(x);
                top.get_or_insert(y);
            }
        }
    }
    let first = *cols
        .iter()
        .min()
        .expect("nothing on screen in the cart's colour");
    let last = *cols.iter().max().expect("nothing in the cart's colour");
    ((first + last) as f32 / 2.0, top.unwrap())
}

fn shot(app: &App, c: &mut Compositor, name: Option<&str>) -> Vec<u8> {
    let mut out = Vec::new();
    app.draw(&mut out);
    frame_named(c, &out, name)
}

fn frame(c: &mut Compositor, out: &[Draw], name: &str) -> Vec<u8> {
    frame_named(c, out, Some(name))
}

fn frame_named(c: &mut Compositor, out: &[Draw], name: Option<&str>) -> Vec<u8> {
    c.set_screen_power(1.0);
    c.begin_frame();
    c.draw_list(out);
    let px = c.read_frame();
    if let (Some(name), Ok(dir)) = (name, std::env::var("SCRATCH_PNG_DIR")) {
        let path = format!("{dir}/shelves-{name}.png");
        let file = std::fs::File::create(&path).expect("create png");
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), out_w(), out_h());
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .expect("png header")
            .write_image_data(&px)
            .expect("png data");
        println!("wrote {path}");
    }
    px
}

fn cartridges() -> [(&'static str, Cart); 2] {
    [
        (
            "gba",
            Cart {
                platform: CartPlatform::Gba,
                stem: "Emerald".into(),
                rom: "Games/GBA/Emerald.gba".into(),
                label: None,
                code: String::new(),
                shell: None,
                title: "POKEMON EMER".into(),
            },
        ),
        (
            "pak",
            Cart {
                platform: CartPlatform::Gb,
                stem: "Tetris".into(),
                rom: "Games/GB/Tetris.gb".into(),
                label: None,
                code: String::new(),
                shell: None,
                title: "TETRIS".into(),
            },
        ),
    ]
}

fn label_top(p: CartPlatform) -> usize {
    match p {
        CartPlatform::Gba => LABEL_Y as usize,
        CartPlatform::Gb | CartPlatform::Gbc => GB_LABEL_Y as usize,
        other => unreachable!("no {other:?} cart on this card"),
    }
}

fn paper_rows(px: &[u8], ink: [u8; 3]) -> Option<(usize, usize)> {
    let close = |c: [u8; 3]| (0..3).all(|k| c[k].abs_diff(ink[k]) <= 24);
    let mut rows =
        (0..out_h() as usize).filter(|y| (0..out_w() as usize).any(|x| close(at(px, x, *y))));
    let first = rows.next()?;
    Some((first, rows.next_back().unwrap_or(first)))
}

/// The rows a cartridge drawn alone covers: whatever is not the black the frame was cleared to.
fn shell_rows(px: &[u8]) -> Option<(usize, usize)> {
    let cart = |c: [u8; 3]| c.iter().any(|v| *v > 8);
    let mut rows =
        (0..out_h() as usize).filter(|y| (0..out_w() as usize).any(|x| cart(at(px, x, *y))));
    let first = rows.next()?;
    Some((first, rows.next_back().unwrap_or(first)))
}

/// Both cartridges go into the handheld at the size they stood on the shelf: standing where the
/// carousel centres them, their labels as the shelf holds them; rising the same few pixels; then
/// pulled down out of the bottom of the screen at their shelf size, a pak no taller than it
/// stood; and gone, all of them, once in.
#[test]
fn both_cartridges_dive_into_the_handheld_at_their_shelf_size() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let beats = [
        ("0-standing", 0.0),
        ("1-rising", 0.1),
        ("2-risen", 0.2),
        ("3-pulled", 0.5),
        ("4-diving", 0.8),
        ("5-in", 1.0),
    ];
    for (name, cart) in cartridges() {
        let face = cart_face(&cart);
        let (w, h) = cart_box(cart.platform);
        assert_eq!(
            (face.w, face.h),
            (w, h),
            "{name}: the face is not the size the layout thinks it is"
        );
        let tex = c.create_texture(face.w, face.h, &face.rgba);
        let ink = label_colour(&clean_label(&cart.stem));
        let (rest, scale) = Shelf::new(vec![cart.clone()]).selected_at();
        let h = shelf_box(cart.platform).1;
        let shelf = shelf_scale(cart.platform);
        let mut tops = Vec::new();

        for (beat, seat) in beats {
            let mut out = Vec::new();
            CartDive {
                cart: &cart,
                face: Some(tex),
                mesh: None,
                stack: None,
                rest,
                scale,
                pose: Pose::REST,
                seat,
                alert: None,
                dim: 0.0,
                screen: 0.0,
                game: false,
                disc: None,
            }
            .draw(&mut out);
            let px = frame(&mut c, &out, &format!("insert-{name}-{beat}"));

            if seat >= 1.0 {
                assert_eq!(
                    shell_rows(&px),
                    None,
                    "{name} is still on the screen all the way in"
                );
                continue;
            }
            let Some((top, bottom)) = shell_rows(&px) else {
                panic!("{name} at {beat}: no cartridge on the screen at all");
            };
            tops.push(top as f32);
            let tall = (bottom - top + 1) as f32;
            if bottom + 1 < out_h() as usize {
                assert!(
                    (tall - h).abs() < 1.5,
                    "{name} at {beat} is {tall} px tall, not the {h} it stands on the shelf"
                );
            } else {
                assert!(
                    tall <= h + 1.5,
                    "{name} at {beat} reaches {tall} px up from the bottom, taller than the {h} \
                     it stands on the shelf"
                );
            }
            if seat == 0.0 {
                assert!(
                    (top as f32 - rest_y(h)).abs() < 1.5,
                    "{name} stands with its top edge at {top}, not at {} where the carousel \
                     centres a {h} px cartridge",
                    rest_y(h)
                );
                let middle = (top + bottom) as f32 / 2.0;
                let want = rest_y(h) + h / 2.0;
                assert!(
                    (middle - want).abs() < 1.5,
                    "{name} stands {top}..{bottom}, centred on {middle} rather than on {want}"
                );
                let (paper_top, paper_bottom) =
                    paper_rows(&px, ink).expect("a standing cartridge shows its label");
                let inset = (paper_top - top) as f32;
                let want = label_top(cart.platform) as f32 * shelf;
                assert!(
                    (inset - want).abs() <= 2.0,
                    "{name}'s paper starts {inset} px down its face, not the {want} its \
                     platform puts it at on the shelf"
                );
                let paper = (paper_bottom - paper_top + 1) as f32;
                let want = match cart.platform {
                    CartPlatform::Gba => LABEL_H,
                    _ => GB_LABEL_H,
                } as f32
                    * shelf;
                assert!(
                    (paper - want).abs() <= 2.0,
                    "{name}'s label came out {paper} px tall, not the {want} the shelf holds \
                     it at: it is being scaled"
                );
            }
        }
        let stood = tops[0];
        assert!(
            (stood - tops[2] - 8.0).abs() < 1.5,
            "{name} rose from {stood} to {}, not the 8 px a hand letting go lifts it",
            tops[2]
        );
        assert!(
            tops[1] < stood && tops[1] > tops[2],
            "{name} did not rise on its way to the top of its lift: {tops:?}"
        );
        assert!(
            tops[3] > stood && tops[4] > tops[3],
            "{name} was not pulled down past where it stood: {tops:?}"
        );
    }
}

#[test]
fn no_frame_of_the_travel_jumps_further_than_the_cartridge_is_tall() {
    for (name, cart) in cartridges() {
        let h = cart_box(cart.platform).1;
        let (rest, scale) = Shelf::new(vec![cart.clone()]).selected_at();
        let ink = label_colour(&label_text(&cart));
        let ink = ink.map(|v| v as f32 / 255.0);
        // a frame at 60 Hz all the way in, short of the last, on which it is gone
        let frames = (SEATED_AT * 60.0).round() as u32;
        let ys: Vec<f32> = (0..frames)
            .map(|f| {
                let mut out = Vec::new();
                CartDive {
                    cart: &cart,
                    face: None,
                    mesh: None,
                    stack: None,
                    rest,
                    scale,
                    pose: Pose::REST,
                    seat: f as f32 / frames as f32,
                    alert: None,
                    dim: 0.0,
                    screen: 0.0,
                    game: false,
                    disc: None,
                }
                .draw(&mut out);
                out.iter()
                    .find_map(|d| match d {
                        Draw::Rect { y, colour, .. }
                            if (0..3).all(|k| (colour[k] - ink[k]).abs() < 0.002) =>
                        {
                            Some(*y)
                        }
                        _ => None,
                    })
                    .expect("no cartridge in the list")
            })
            .collect();
        let steps: Vec<f32> = ys.windows(2).map(|w| (w[1] - w[0]).round()).collect();
        println!("{name}: {steps:?}");
        let worst = steps.iter().cloned().fold(0.0f32, f32::max);
        assert!(
            worst < h as f32 / 2.0,
            "{name} moves {worst} px in one frame, over half of its own {h} px: that is a \
             cut, not a movement"
        );
    }
}
