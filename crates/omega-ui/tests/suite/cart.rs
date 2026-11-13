use std::path::Path;
use std::time::{Duration, SystemTime};

use omega_store::{scan, Platform};
use omega_ui::{
    cart_face, clean_label, cut_label, cut_path, foot_y, gb_label_panel, gb_silhouette,
    generated_face, label_colour, label_panel, label_text, label_uncut, model_box, model_of,
    model_scale, model_shadow, model_shelf_box, out_w, rest_y, seated_box, shelf_box, shell_for,
    silhouette, snes_silhouette, stage_y, sway_reach, wears_label, CartModel, Finish, GbShell,
    SnesShell, BAR_H, CART_H, CART_W, GB_CART_H, GB_CART_W, GB_LABEL_H, GB_LABEL_W, GB_LABEL_X,
    GB_LABEL_Y, GB_SHELF_SCALE, LABEL_H, LABEL_W, LABEL_X, LABEL_Y, MOCKUP_H, SFC_SHELL,
    SNES_SHELL, TOP_BAR_H,
};
use tempfile::TempDir;

fn gx(v: u32) -> u32 {
    let w = seated_box(Platform::Gb).0;
    (v * GB_CART_W + w / 2) / w
}

fn gy(v: u32) -> u32 {
    let h = seated_box(Platform::Gb).1;
    (v * GB_CART_H + h / 2) / h
}

fn tmp_root() -> TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    for sub in [
        "Games",
        "Games/GBA",
        "Labels",
        "Saves",
        "States",
        "System",
        "Config",
    ] {
        std::fs::create_dir(d.path().join(sub)).expect("create content dir");
    }
    d
}

fn write_gb_rom(d: &TempDir, dir: &str, name: &str, cgb: u8) {
    let mut rom = vec![0u8; 0x150];
    rom[0x143] = cgb;
    let games = d.path().join("Games").join(dir);
    std::fs::create_dir_all(&games).expect("create games dir");
    std::fs::write(games.join(name), rom).expect("write rom");
}

fn write_rom(d: &TempDir, name: &str, title: &str) {
    let mut rom = vec![0u8; 0x100];
    rom[0xa0..0xa0 + title.len()].copy_from_slice(title.as_bytes());
    std::fs::write(d.path().join("Games/GBA").join(name), rom).expect("write rom");
}

fn write_label(d: &TempDir, name: &str, w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 3]) {
    std::fs::create_dir_all(d.path().join("Labels/GBA")).expect("create labels dir");
    write_png(&d.path().join("Labels/GBA").join(name), w, h, px);
}

fn write_png(path: &Path, w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 3]) {
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            rgb.extend_from_slice(&px(x, y));
        }
    }
    let f = std::fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .expect("png header")
        .write_image_data(&rgb)
        .expect("png data");
}

fn pixel(face: &omega_ui::CartFace, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * face.w + x) * 4) as usize;
    [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]]
}

fn pixel_alpha(face: &omega_ui::CartFace, x: u32, y: u32) -> u8 {
    face.rgba[((y * face.w + x) * 4 + 3) as usize]
}

fn label_pixel(face: &omega_ui::CartFace, x: u32, y: u32) -> [u8; 3] {
    pixel(face, LABEL_X + x, LABEL_Y + y)
}

#[test]
fn a_malformed_label_falls_back_to_a_generated_one() {
    let d = tmp_root();
    write_rom(&d, "Broken.gba", "BROKEN");
    std::fs::create_dir_all(d.path().join("Labels/GBA")).unwrap();
    std::fs::write(d.path().join("Labels/GBA/Broken.png"), b"not a png").unwrap();
    let cart = &scan(d.path()).unwrap()[0];
    let face = cart_face(cart);
    assert_eq!((face.w, face.h), (CART_W, CART_H));
    assert!(face.rgba.iter().any(|b| *b != 0), "face is blank");
}

fn modified(path: &Path) -> SystemTime {
    std::fs::metadata(path).unwrap().modified().unwrap()
}

fn set_modified(path: &Path, at: SystemTime) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(at)
        .unwrap();
}

fn png_size(path: &Path) -> (u32, u32) {
    let file = std::fs::File::open(path).expect("open png");
    let reader = png::Decoder::new(file).read_info().expect("png header");
    (reader.info().width, reader.info().height)
}

fn gradient(x: u32, y: u32) -> [u8; 3] {
    [(x % 256) as u8, (y % 256) as u8, ((x * 3 + y) % 256) as u8]
}

#[test]
fn a_cut_label_draws_the_face_its_scan_draws() {
    let d = tmp_root();
    write_rom(&d, "Cut.gba", "CUT");
    write_label(&d, "Cut.png", 600, 310, gradient);
    let cart = &scan(d.path()).unwrap()[0];
    assert!(label_uncut(cart), "a scan no cut was made of");
    let from_scan = cart_face(cart);

    assert!(cut_label(cart), "the scan is cut");
    let cut = d.path().join("Labels/GBA/.cut/Cut.png");
    assert_eq!(cut_path(cart.label.as_deref().unwrap()), Some(cut.clone()));
    assert_eq!(png_size(&cut), (LABEL_W, LABEL_H), "cut to the label panel");
    assert!(!label_uncut(cart), "the cut is kept for the next boot");
    assert!(
        cart_face(cart).rgba == from_scan.rgba,
        "a face drawn from the cut is the face drawn from the scan"
    );
}

#[test]
fn a_face_draws_its_label_from_the_cut() {
    let d = tmp_root();
    write_rom(&d, "Kept.gba", "KEPT");
    write_label(&d, "Kept.png", 600, 310, |_, _| [0xd0, 0x20, 0xa0]);
    let cart = &scan(d.path()).unwrap()[0];
    assert!(cut_label(cart));
    let cut = d.path().join("Labels/GBA/.cut/Kept.png");
    write_png(&cut, LABEL_W, LABEL_H, |_, _| [0x10, 0xc0, 0x40]);
    set_modified(&cut, modified(cart.label.as_deref().unwrap()));

    let face = cart_face(cart);
    assert_eq!(
        label_pixel(&face, LABEL_W / 2, LABEL_H / 2),
        [0x10, 0xc0, 0x40]
    );
}

#[test]
fn a_scan_put_in_place_of_the_one_cut_is_cut_again() {
    let d = tmp_root();
    write_rom(&d, "Swapped.gba", "SWAPPED");
    write_label(&d, "Swapped.png", 600, 310, |_, _| [0xd0, 0x20, 0xa0]);
    let cart = &scan(d.path()).unwrap()[0];
    assert!(cut_label(cart));
    let scan_path = cart.label.clone().unwrap();
    let before = modified(&scan_path);
    write_label(&d, "Swapped.png", 600, 310, |_, _| [0x20, 0x40, 0xe0]);
    set_modified(&scan_path, before - Duration::from_secs(3600));

    assert!(
        label_uncut(cart),
        "an older scan in its place is cut again too"
    );
    let face = cart_face(cart);
    assert_eq!(
        label_pixel(&face, LABEL_W / 2, LABEL_H / 2),
        [0x20, 0x40, 0xe0],
        "until it is, the face shows the new scan, not the old cut"
    );
}

#[test]
fn a_cut_of_another_size_is_cut_again() {
    let d = tmp_root();
    write_rom(&d, "Small.gba", "SMALL");
    write_label(&d, "Small.png", 600, 310, |_, _| [0xd0, 0x20, 0xa0]);
    let cart = &scan(d.path()).unwrap()[0];
    let cut = d.path().join("Labels/GBA/.cut/Small.png");
    std::fs::create_dir_all(cut.parent().unwrap()).unwrap();
    write_png(&cut, 10, 10, |_, _| [0x10, 0xc0, 0x40]);
    set_modified(&cut, modified(cart.label.as_deref().unwrap()));

    assert!(label_uncut(cart), "a cut made for another panel");
    let face = cart_face(cart);
    assert_eq!(
        label_pixel(&face, LABEL_W / 2, LABEL_H / 2),
        [0xd0, 0x20, 0xa0]
    );
    assert!(cut_label(cart));
    assert_eq!(png_size(&cut), (LABEL_W, LABEL_H));
}

#[test]
fn a_cart_without_a_label_scan_has_nothing_to_cut() {
    let d = tmp_root();
    write_rom(&d, "Bare.gba", "BARE");
    let cart = &scan(d.path()).unwrap()[0];
    assert!(!label_uncut(cart));
    assert!(!cut_label(cart));
    assert!(!d.path().join("Labels/GBA/.cut").exists());
}

/// A boot asks the label workers to draw only the carts that wear a scan: the face of one
/// without is its generated face, final as it is.
#[test]
fn only_a_cart_with_a_label_scan_wears_one() {
    let d = tmp_root();
    write_rom(&d, "Bare.gba", "BARE");
    write_rom(&d, "Labelled.gba", "LABELLED");
    write_label(&d, "Labelled.png", 64, 64, |_, _| [0xd0, 0x20, 0xa0]);
    let carts = scan(d.path()).unwrap();
    let cart = |stem: &str| carts.iter().find(|c| c.stem == stem).unwrap();
    assert!(!wears_label(cart("Bare")));
    assert!(
        cart_face(cart("Bare")).rgba == generated_face(cart("Bare")).rgba,
        "a cart that wears no scan is not drawn in its generated face"
    );
    assert!(wears_label(cart("Labelled")));
    assert!(cart_face(cart("Labelled")).rgba != generated_face(cart("Labelled")).rgba);
}

#[test]
fn label_colour_is_stable_across_calls() {
    assert_eq!(label_colour("POKEMON EMER"), label_colour("POKEMON EMER"));
    assert_ne!(label_colour("POKEMON EMER"), label_colour("ADVANCEWARS"));
}

#[test]
fn a_label_that_decodes_is_what_the_face_shows() {
    let d = tmp_root();
    write_rom(&d, "Labelled.gba", "LABELLED");
    write_label(&d, "Labelled.png", 64, 64, |_, _| [0xd0, 0x20, 0xa0]);
    let face = cart_face(&scan(d.path()).unwrap()[0]);
    assert_eq!((face.w, face.h), (CART_W, CART_H));
    for y in [0, LABEL_H / 2, LABEL_H - 1] {
        for x in [0, LABEL_W / 2, LABEL_W - 1] {
            assert_eq!(label_pixel(&face, x, y), [0xd0, 0x20, 0xa0], "at {x},{y}");
        }
    }
}

#[test]
fn a_label_of_the_wrong_aspect_is_cropped_not_squashed() {
    let d = tmp_root();
    write_rom(&d, "Tall.gba", "TALL");
    write_label(&d, "Tall.png", 160, 480, |_, y| match y / 160 {
        0 => [0xff, 0x00, 0x00],
        1 => [0xff, 0xff, 0xff],
        _ => [0x00, 0x00, 0xff],
    });
    let face = cart_face(&scan(d.path()).unwrap()[0]);
    for y in 0..LABEL_H {
        for x in 0..LABEL_W {
            assert_eq!(label_pixel(&face, x, y), [0xff, 0xff, 0xff], "at {x},{y}");
        }
    }
}

#[test]
fn a_long_title_stays_inside_the_label() {
    for stem in [
        "Supercalifragilisticexpialidocious Anniversary Edition",
        "Super Mario Kart",
    ] {
        let d = tmp_root();
        std::fs::write(d.path().join(format!("Games/GBA/{stem}.gba")), [0u8; 8]).unwrap();
        let face = cart_face(&scan(d.path()).unwrap()[0]);
        let bg = label_colour(stem);
        let margin = 5;
        for y in 0..LABEL_H {
            for x in 0..LABEL_W {
                let edge =
                    x < margin || y < margin || x >= LABEL_W - margin || y >= LABEL_H - margin;
                if edge {
                    assert_eq!(
                        label_pixel(&face, x, y),
                        bg,
                        "{stem}: text spills into the border at {x},{y}"
                    );
                }
            }
        }
        assert!(
            (0..LABEL_H * LABEL_W).any(|i| label_pixel(&face, i % LABEL_W, i / LABEL_W) != bg),
            "{stem}: no text was drawn"
        );
    }
}

#[test]
fn the_cart_box_matches_the_traced_outline() {
    let ratio = CART_W as f32 / CART_H as f32;
    assert!(
        (ratio - 1.703).abs() < 0.02,
        "aspect {ratio:.3}, the svg is being stretched"
    );
}

#[test]
fn the_label_is_wide_and_sits_low() {
    let (x0, y0, x1, y1) = label_panel(CART_W, CART_H);
    let (w, h) = (CART_W as f32, CART_H as f32);
    let side = x0 as f32 / w;
    let top = y0 as f32 / h;
    let bottom = 1.0 - y1 as f32 / h;
    let width = (x1 - x0) as f32 / w;

    assert!(
        width > 0.70,
        "the label is only {:.0}% of the cart wide, that reads as a panel not a label",
        width * 100.0
    );
    assert!(
        top > 0.18,
        "only {:.0}% of grip band above the label",
        top * 100.0
    );
    assert!(
        top > side * 1.5,
        "top band {top:.2} against side margin {side:.2}: that is a uniform border"
    );
    assert!(
        top > bottom * 1.6,
        "the label is not sitting low, top {top:.2} bottom {bottom:.2}"
    );
}

#[test]
fn the_body_is_narrower_than_its_grip_ears() {
    let m = silhouette(CART_W, CART_H);
    let solid = |x: u32, y: u32| m[(y * CART_W + x) as usize] > 128;
    let width_at = |y: u32| (0..CART_W).filter(|&x| solid(x, y)).count();

    let ears = width_at(CART_H / 12);
    let body = width_at(CART_H / 2);
    assert!(
        ears > body,
        "top spans {ears}px and the body {body}px: the grip ears are missing"
    );
    assert!(
        ears - body >= 3,
        "the ears stick out by only {}px, which will not read at all",
        ears - body
    );
    assert!(solid(CART_W / 2, CART_H / 2), "the middle is not solid");
    assert!(
        solid(CART_W / 2, CART_H - 2),
        "the bottom edge is not straight"
    );
}

#[test]
fn cart_faces_are_clipped_to_the_silhouette() {
    let d = tmp_root();
    write_rom(&d, "Emerald.gba", "EMERALD");
    let cart = &scan(d.path()).unwrap()[0];
    let f = cart_face(cart);
    let px = |x: u32, y: u32| f.rgba[((y * f.w + x) * 4 + 3) as usize];
    assert_eq!(
        px(2, 2),
        0,
        "the rounded corner is opaque, the mask was not applied"
    );
    assert!(px(f.w / 2, f.h / 2) > 250);
}

#[test]
fn a_supplied_label_is_clipped_to_the_silhouette_too() {
    let d = tmp_root();
    write_rom(&d, "Labelled.gba", "LABELLED");
    write_label(&d, "Labelled.png", LABEL_W, LABEL_H, |_, _| {
        [0x40, 0x80, 0xc0]
    });
    let f = cart_face(&scan(d.path()).unwrap()[0]);
    let px = |x: u32, y: u32| f.rgba[((y * f.w + x) * 4 + 3) as usize];
    assert_eq!(px(2, 2), 0, "the label escaped the rounded corner");
    assert!(px(f.w / 2, f.h / 2) > 250);
}

#[test]
fn the_label_is_the_filename_without_its_tags() {
    let cases = [
        (
            "Pokemon - Emerald Version (USA, Europe)",
            "Pokemon Emerald Version",
        ),
        (
            "Pokemon - LeafGreen Version (USA, Europe) (Rev 1)",
            "Pokemon LeafGreen Version",
        ),
        ("Shrek (USA) (Rev 6)", "Shrek"),
        ("Metroid Fusion", "Metroid Fusion"),
        ("Button Test", "Button Test"),
        ("Pokemon - Corrupt", "Pokemon Corrupt"),
        ("Some Game [!]", "Some Game"),
        ("Spaced   Out  (USA)", "Spaced Out"),
    ];
    for (stem, want) in cases {
        assert_eq!(clean_label(stem), want, "for {stem}");
    }
}

#[test]
fn a_numbered_set_s_number_is_not_on_the_label() {
    assert_eq!(
        clean_label("0017 - Need for Speed - Underground Rivals (USA)"),
        "Need for Speed Underground Rivals"
    );
    assert_eq!(
        clean_label("2040 - Tekken 6 (USA) (En,Fr,De,Es,It,Ru)"),
        "Tekken 6"
    );
    // a number that is the name, or not set off by a dash, stays
    assert_eq!(clean_label("1942 (USA, Europe)"), "1942");
    assert_eq!(clean_label("Tetris 2 - 1993"), "Tetris 2 1993");
}

#[test]
fn an_unspaced_hyphen_survives() {
    assert_eq!(clean_label("Spider-Man 2 (USA)"), "Spider-Man 2");
    assert_eq!(
        clean_label("Wario Land 4 - Time Attack"),
        "Wario Land 4 Time Attack"
    );
}

#[test]
fn a_name_that_is_all_tags_falls_back_rather_than_going_blank() {
    assert_eq!(clean_label("(USA) (Rev 1)"), "(USA) (Rev 1)");
    assert_eq!(clean_label(""), "");
}

#[test]
fn the_header_title_no_longer_reaches_the_label() {
    let d = tmp_root();
    write_rom(
        &d,
        "Pokemon - Emerald Version (USA, Europe).gba",
        "POKEMON EMER",
    );
    let cart = &scan(d.path()).unwrap()[0];
    assert_eq!(label_text(cart), "Pokemon Emerald Version");
}

#[test]
fn two_regions_of_one_game_get_the_same_generated_colour() {
    let a = label_colour(&clean_label("Pokemon - Ruby Version (USA, Europe) (Rev 2)"));
    let b = label_colour(&clean_label("Pokemon - Ruby Version (Japan)"));
    assert_eq!(a, b, "the same game came out two colours");
}

#[test]
fn a_rom_with_no_header_title_is_labelled_from_its_stem() {
    let d = tmp_root();
    std::fs::write(d.path().join("Games/GBA/Homebrew Demo.gba"), [0u8; 8]).unwrap();
    let face = cart_face(&scan(d.path()).unwrap()[0]);
    let bg = label_colour("Homebrew Demo");
    assert!(
        (0..LABEL_H * LABEL_W).any(|i| label_pixel(&face, i % LABEL_W, i / LABEL_W) != bg),
        "an untitled rom got a blank label"
    );
}

#[test]
fn the_game_boy_pak_body_has_the_scanned_aspect() {
    let (sw, _) = seated_box(Platform::Gba);
    let body = |w: u32, h: u32| (w as f64 * 227.0 / 240.0) / h as f64;
    assert!(
        (body(GB_CART_W, GB_CART_H) - 0.877).abs() < 0.01,
        "the pak body is {:.3}:1, not a real pak's 0.877",
        body(GB_CART_W, GB_CART_H)
    );
    let (gw, gh) = seated_box(Platform::Gb);
    assert_eq!(
        gw, sw,
        "both paks are 57 mm wide, so they seat at one width"
    );
    assert!((body(gw, gh) - 0.877).abs() < 0.01);
}

#[test]
fn the_game_boy_outline_has_parallel_sides_and_no_grip_ears() {
    for shell in [GbShell::Notched, GbShell::Rounded] {
        let gb = gb_silhouette(shell, GB_CART_W, GB_CART_H);
        let width_at = |y: u32| {
            (0..GB_CART_W)
                .filter(|&x| gb[(y * GB_CART_W + x) as usize] > 128)
                .count()
        };
        let (top, middle, bottom) = (
            width_at(GB_CART_H / 8),
            width_at(GB_CART_H / 2),
            width_at(GB_CART_H * 7 / 8),
        );
        assert_eq!(top, middle, "{shell:?}: wider at the top than the middle");
        assert_eq!(middle, bottom, "{shell:?}: tapers toward the bottom");

        let gba = silhouette(CART_W, CART_H);
        let gba_body = (0..CART_W)
            .filter(|&x| gba[((CART_H / 2) * CART_W + x) as usize] > 128)
            .count();
        let gba_body = (gba_body * GB_CART_W as usize + CART_W as usize / 2) / CART_W as usize;
        assert!(
            middle.abs_diff(gba_body) <= 2,
            "{shell:?}: the pak's body is {middle}px against the GBA body's {gba_body}px, \
             and both are 57 mm"
        );
    }
}

#[test]
fn the_colour_only_shell_loses_the_notch_and_rounds_the_corners() {
    let notched = gb_silhouette(GbShell::Notched, GB_CART_W, GB_CART_H);
    let rounded = gb_silhouette(GbShell::Rounded, GB_CART_W, GB_CART_H);
    fn covered(m: &[u8], y: u32) -> Vec<u32> {
        (0..GB_CART_W)
            .filter(|&x| m[(y * GB_CART_W + x) as usize] > 128)
            .collect()
    }
    let width_at = |m: &[u8], y: u32| covered(m, y).len();
    let left_at = |m: &[u8], y: u32| covered(m, y)[0];
    let right_at = |m: &[u8], y: u32| *covered(m, y).last().expect("an empty row");

    let y = 6;
    assert!(
        right_at(&rounded, y) > right_at(&notched, y) + 10,
        "the clear pak ends at {} against the notched {} at row {y}: the notch is not cut",
        right_at(&rounded, y),
        right_at(&notched, y)
    );
    assert!(
        left_at(&rounded, 0) > left_at(&notched, 0) + 10,
        "the top row starts at {} against the notched {}: the corners are not rounded",
        left_at(&rounded, 0),
        left_at(&notched, 0)
    );
    for y in 30..GB_CART_H {
        assert_eq!(
            width_at(&notched, y),
            width_at(&rounded, y),
            "the shells disagree at row {y}, which is below the shoulder"
        );
    }
}

#[test]
fn the_game_boy_label_well_is_near_square_and_sits_under_the_lettering_plate() {
    let (x0, y0, x1, y1) = gb_label_panel(GB_CART_W, GB_CART_H);
    let aspect = (x1 - x0) as f32 / (y1 - y0) as f32;
    assert!(
        (aspect - 1.135).abs() < 0.02,
        "the well is {aspect:.2}:1, which is not the 42x37 label's shape"
    );
    assert_eq!(x0, GB_CART_W - x1, "the well is not centred across the pak");
    let (above, below) = (y0, GB_CART_H - y1);
    assert!(
        above > below * 2,
        "{above}px above the label and {below}px below: the shoulder has lost its plate"
    );
}

#[test]
fn the_colour_only_shell_has_a_rolled_top_edge_and_the_older_mould_does_not() {
    let d = tmp_root();
    write_gb_rom(&d, "GBC", "Rolled.gbc", 0xc0);
    write_gb_rom(&d, "GB", "Flat.gb", 0x00);
    let carts = scan(d.path()).expect("scan");
    let face = |stem: &str| {
        cart_face(
            carts
                .iter()
                .find(|c| c.stem == stem)
                .unwrap_or_else(|| panic!("no cart {stem}")),
        )
    };
    let down = |f: &omega_ui::CartFace, y: u32| pixel(f, GB_CART_W / 2, y)[1] as i32;

    let rolled = face("Rolled");
    let band = (gy(2)..=gy(9))
        .map(|y| down(&rolled, y))
        .min()
        .expect("a band");
    let brk = (gy(10)..=gy(13))
        .map(|y| down(&rolled, y))
        .min()
        .expect("a break");
    let shoulder = down(&rolled, gy(14));
    assert!(
        band > shoulder + 10,
        "the class C top edge is {band} against a {shoulder} shoulder: the roll carries no light"
    );
    assert!(
        brk < shoulder - 10,
        "the break under the roll is {brk} against a {shoulder} shoulder: the roll does not turn, \
         it just fades"
    );

    let flat = face("Flat");
    let shoulder = down(&flat, gy(14));
    for y in 0..=gy(13) {
        assert_eq!(
            down(&flat, y),
            shoulder,
            "the class A/B top edge moved at row {y}: the roll is not class C only"
        );
    }
}

#[test]
fn a_game_boy_cart_face_is_drawn_at_the_game_boy_size() {
    let d = tmp_root();
    write_gb_rom(&d, "GB", "Tetris.gb", 0x00);
    let face = cart_face(&scan(d.path()).unwrap()[0]);
    assert_eq!((face.w, face.h), (GB_CART_W, GB_CART_H));
    assert!(face.rgba.iter().any(|b| *b != 0), "face is blank");
}

/// The room the row stands in: from under the bar across the top to the top of the bar across
/// the bottom.
fn between_the_bars() -> (f32, f32) {
    (stage_y() + TOP_BAR_H, stage_y() + MOCKUP_H - BAR_H)
}

#[test]
fn every_cartridge_is_centred_on_the_row_and_clears_both_bars() {
    let (ceiling, floor) = between_the_bars();
    for (name, model) in [
        ("the GBA cart", CartModel::Gba),
        ("the Game Boy pak", CartModel::Gb(GbShell::Notched)),
        ("the American SNES cart", CartModel::Snes(SnesShell::Ntsc)),
        (
            "the Super Famicom cart",
            CartModel::Snes(SnesShell::Famicom),
        ),
        ("the PlayStation disc", CartModel::Disc),
        ("the UMD", CartModel::Umd),
    ] {
        let h = model_shelf_box(model).1;
        let (top, foot) = (rest_y(h), foot_y(h));
        assert!(
            (top + foot - (ceiling + floor)).abs() < 0.01,
            "{name} stands at {top}..{foot}, which is not centred between the top bar at \
             {ceiling} and the bottom bar at {floor}"
        );
        assert!(
            top >= ceiling,
            "{name}'s top edge at {top} is under the top bar at {ceiling}"
        );
        assert!(
            foot <= floor,
            "{name}'s foot at {foot} is over the bottom bar at {floor}"
        );
    }
}

#[test]
fn a_pak_stands_at_four_fifths_and_sways_clear_of_both_bars() {
    assert_eq!(
        GB_SHELF_SCALE, 0.8,
        "the shelf is not the layout chosen for it: paks at 80%"
    );
    let (w, h) = shelf_box(Platform::Gb);
    let reach = sway_reach(w, h);
    let (top, foot) = (rest_y(h), foot_y(h));
    let (ceiling, floor) = between_the_bars();
    assert!(
        top - reach >= ceiling,
        "the swaying pak reaches {} under the top bar at {ceiling}",
        top - reach
    );
    assert!(
        foot + reach <= floor,
        "the swaying pak reaches {} over the bottom bar at {floor}",
        foot + reach
    );
}

#[test]
fn each_snes_cart_sways_clear_of_both_bars_at_its_own_size() {
    let (ceiling, floor) = between_the_bars();
    for shell in [SnesShell::Ntsc, SnesShell::Famicom] {
        let model = CartModel::Snes(shell);
        assert_eq!(
            model_scale(model),
            1.0,
            "the {shell:?} face is not made at the size it stands at"
        );
        let (w, h) = model_shelf_box(model);
        let reach = sway_reach(w, h);
        let (top, foot) = (rest_y(h), foot_y(h));
        assert!(
            top - reach >= ceiling && foot + reach <= floor,
            "the swaying {shell:?} cart reaches {}..{}, past the bars at {ceiling} and {floor}",
            top - reach,
            foot + reach
        );
        // standing still in the middle of the row, it lands on whole art pixels
        let left = (out_w() as f32 - w) / 2.0;
        assert!(
            left % 2.0 == 0.0 && top % 2.0 == 0.0,
            "the {shell:?} cart stands at {left},{top}, off the art grid"
        );
    }
}

/// A LoROM image in Games/SNES with a header naming `title`, sold in `destination`.
fn write_snes_rom(d: &TempDir, name: &str, title: &str, destination: u8) {
    let mut rom = vec![0xffu8; 0x8_0000];
    let h = &mut rom[0x7fc0..0x8000];
    h[..21].fill(b' ');
    h[..title.len()].copy_from_slice(title.as_bytes());
    h[0x15] = 0x20;
    h[0x17] = 0x0a;
    h[0x19] = destination;
    h[0x1c..0x1e].copy_from_slice(&0xa5c3u16.to_le_bytes());
    h[0x1e..0x20].copy_from_slice(&0x5a3cu16.to_le_bytes());
    h[0x3c..0x3e].copy_from_slice(&0x8000u16.to_le_bytes());
    let games = d.path().join("Games/SNES");
    std::fs::create_dir_all(&games).expect("create games dir");
    std::fs::write(games.join(name), rom).expect("write rom");
}

fn snes_cart<'a>(carts: &'a [omega_store::Cart], stem: &str) -> &'a omega_store::Cart {
    carts
        .iter()
        .find(|c| c.stem == stem)
        .unwrap_or_else(|| panic!("{stem} was not scanned"))
}

#[test]
fn a_snes_game_from_the_usa_stands_as_the_american_cart_and_any_other_as_the_super_famicom_s() {
    let d = tmp_root();
    write_snes_rom(&d, "Super Metroid (USA).sfc", "SUPER METROID", 0x01);
    write_snes_rom(&d, "Rockman X (Japan).sfc", "ROCKMAN X", 0x00);
    write_snes_rom(&d, "Terranigma (Europe).sfc", "TERRANIGMA", 0x02);
    write_snes_rom(&d, "Chrono Trigger.sfc", "CHRONO TRIGGER", 0x01);
    write_snes_rom(&d, "Star Ocean.sfc", "STAR OCEAN", 0x00);
    write_snes_rom(&d, "Mystery (Korea).smc", "MYSTERY", 0x01);
    std::fs::write(d.path().join("Games/SNES/Blank.sfc"), vec![0u8; 0x8000]).unwrap();
    let carts = scan(d.path()).unwrap();
    for (stem, want) in [
        ("Super Metroid (USA)", SnesShell::Ntsc),
        ("Rockman X (Japan)", SnesShell::Famicom),
        ("Terranigma (Europe)", SnesShell::Famicom),
        ("Chrono Trigger", SnesShell::Ntsc),
        ("Star Ocean", SnesShell::Famicom),
        ("Mystery (Korea)", SnesShell::Famicom),
        ("Blank", SnesShell::Famicom),
    ] {
        assert_eq!(
            model_of(snes_cart(&carts, stem)),
            CartModel::Snes(want),
            "{stem}"
        );
    }
}

#[test]
fn cart_shell_ini_forces_either_snes_shape_whatever_the_region() {
    let d = tmp_root();
    write_snes_rom(&d, "Super Metroid (USA).sfc", "SUPER METROID", 0x01);
    write_snes_rom(&d, "Rockman X (Japan).sfc", "ROCKMAN X", 0x00);
    write_snes_rom(&d, "Terranigma (Europe).sfc", "TERRANIGMA", 0x02);
    std::fs::write(
        d.path().join("Config/cart_shell.ini"),
        "Super Metroid (USA) = sfc b7b6bb solid\n\
         Rockman X (Japan) = ntsc 8b8a91 solid\n\
         Terranigma (Europe) = notched 9a978f solid\n",
    )
    .unwrap();
    let carts = scan(d.path()).unwrap();
    for (stem, want) in [
        ("Super Metroid (USA)", SnesShell::Famicom),
        ("Rockman X (Japan)", SnesShell::Ntsc),
        ("Terranigma (Europe)", SnesShell::Famicom),
    ] {
        assert_eq!(
            model_of(snes_cart(&carts, stem)),
            CartModel::Snes(want),
            "{stem}"
        );
    }
    let forced = cart_face(snes_cart(&carts, "Rockman X (Japan)"));
    assert_eq!(
        (forced.w, forced.h),
        model_box(CartModel::Snes(SnesShell::Ntsc)),
        "a forced shape is not drawn at its own size"
    );
}

#[test]
fn each_snes_shell_is_the_real_cart_s_shape_the_american_one_the_bigger() {
    let (nw, nh) = model_box(CartModel::Snes(SnesShell::Ntsc));
    let (sw, sh) = model_box(CartModel::Snes(SnesShell::Famicom));
    assert!(
        nw > sw && nh > sh,
        "the American cart, {nw}x{nh}, is not bigger than the Super Famicom's, {sw}x{sh}"
    );
    for (name, (w, h), real) in [
        ("American", (nw, nh), 136.0 / 88.0),
        ("Super Famicom", (sw, sh), 130.0 / 86.0),
    ] {
        let aspect = w as f32 / h as f32;
        assert!(
            (aspect / real - 1.0).abs() < 0.01,
            "the {name} face is {aspect}:1, the real cart {real}:1"
        );
        assert!(
            w % 4 == 0 && h % 2 == 0,
            "the {name} face, {w}x{h}, does not halve onto the art grid"
        );
    }
}

#[test]
fn a_snes_face_is_clipped_to_its_own_silhouette_and_backed_by_its_own_shadow() {
    let d = tmp_root();
    write_snes_rom(&d, "Super Metroid (USA).sfc", "SUPER METROID", 0x01);
    write_snes_rom(&d, "Rockman X (Japan).sfc", "ROCKMAN X", 0x00);
    let carts = scan(d.path()).unwrap();
    for (stem, shell) in [
        ("Super Metroid (USA)", SnesShell::Ntsc),
        ("Rockman X (Japan)", SnesShell::Famicom),
    ] {
        let face = cart_face(snes_cart(&carts, stem));
        let model = CartModel::Snes(shell);
        assert_eq!((face.w, face.h), model_box(model));
        let mask = snes_silhouette(shell, face.w, face.h);
        for (i, (px, cover)) in face.rgba.chunks_exact(4).zip(&mask).enumerate() {
            assert!(
                px[3] <= *cover,
                "{stem}: pixel {i} is {} opaque outside its outline's {cover}",
                px[3]
            );
        }
        assert_eq!(pixel_alpha(&face, 1, 1), 0, "{stem}: the corner is opaque");
        assert!(pixel_alpha(&face, face.w / 2, face.h / 2) > 250);
        let shadow = model_shadow(model);
        assert_eq!((shadow.w, shadow.h), (face.w, face.h));
        for (px, cover) in shadow.rgba.chunks_exact(4).zip(&mask) {
            assert_eq!(&px[..3], &[0, 0, 0], "{stem}: the shadow is not black");
            assert_eq!(px[3], *cover, "{stem}: the shadow is not the shell's shape");
        }
    }
    let ntsc = snes_silhouette(SnesShell::Ntsc, 376, 242);
    let sfc = snes_silhouette(SnesShell::Famicom, 376, 242);
    let apart: u32 = ntsc
        .iter()
        .zip(&sfc)
        .map(|(a, b)| u32::from(a.abs_diff(*b)))
        .sum();
    assert!(apart / 255 > 100, "the two shells are all but one shape");
}

/// The box of the pixels of `face` that are `colour`.
fn colour_box(face: &omega_ui::CartFace, colour: [u8; 3]) -> Option<(u32, u32, u32, u32)> {
    let mut b: Option<(u32, u32, u32, u32)> = None;
    for y in 0..face.h {
        for x in 0..face.w {
            if pixel(face, x, y) == colour && pixel_alpha(face, x, y) == 255 {
                b = Some(match b {
                    None => (x, y, x, y),
                    Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                });
            }
        }
    }
    b
}

#[test]
fn a_snes_label_fills_its_well_on_the_plastic_where_the_real_cart_has_it() {
    let d = tmp_root();
    write_snes_rom(&d, "Super Metroid (USA).sfc", "SUPER METROID", 0x01);
    write_snes_rom(&d, "Rockman X (Japan).sfc", "ROCKMAN X", 0x00);
    let art = [0x20, 0xc0, 0x40];
    for stem in ["Super Metroid (USA)", "Rockman X (Japan)"] {
        let dir = d.path().join("Labels/SNES");
        std::fs::create_dir_all(&dir).unwrap();
        let f = std::fs::File::create(dir.join(format!("{stem}.png"))).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(f), 64, 24);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let rgb: Vec<u8> = (0..64 * 24).flat_map(|_| art).collect();
        enc.write_header().unwrap().write_image_data(&rgb).unwrap();
    }
    let carts = scan(d.path()).unwrap();
    // the share of the face's width and height the label takes, and how far down it starts,
    // measured off straight-on photos of real carts: the American one's well starts just under
    // the top of its column, a little lower here to leave room for its bevel; the Super
    // Famicom's a tenth smaller each way than the photos had it, which reads truer
    for (stem, (wide, tall), top) in [
        ("Super Metroid (USA)", (0.6, 0.38), (0.0, 0.05)),
        ("Rockman X (Japan)", (0.75, 0.38), (0.05, 0.07)),
    ] {
        let face = cart_face(snes_cart(&carts, stem));
        let (x0, y0, x1, y1) = colour_box(&face, art).expect("the label art was not pasted");
        let (lw, lh) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
        let (fw, fh) = (face.w as f32, face.h as f32);
        assert!(
            lw / fw > wide && lh / fh > tall,
            "{stem}: the label is {lw}x{lh} on a {fw}x{fh} face"
        );
        assert!(
            ((x0 + x1 + 1) as f32 / 2.0 - fw / 2.0).abs() <= 1.0,
            "{stem}: the label is not across the middle"
        );
        assert!(
            (top.0..=top.1).contains(&(y0 as f32 / fh)),
            "{stem}: the label starts {y0} px down"
        );
        for y in y0..=y1 {
            for x in x0..=x1 {
                assert_eq!(
                    pixel_alpha(&face, x, y),
                    255,
                    "{stem}: label off the plastic"
                );
            }
        }
    }
}

/// The colour a solid shell's label well is shaded in on its upper and left sides.
fn well_shade(colour: [u8; 3]) -> [u8; 3] {
    colour.map(|c| (c as f32 * 0.55) as u8)
}

#[test]
fn only_the_super_famicom_label_has_round_corners_on_the_art_grid() {
    let d = tmp_root();
    write_rom(&d, "Advance.gba", "ADVANCE");
    write_gb_rom(&d, "GB", "Grey.gb", 0x00);
    write_snes_rom(&d, "Super Metroid (USA).sfc", "SUPER METROID", 0x01);
    write_snes_rom(&d, "Rockman X (Japan).sfc", "ROCKMAN X", 0x00);
    let art = [0x20, 0xc0, 0x40];
    for (dir, stem) in [
        ("GBA", "Advance"),
        ("GB", "Grey"),
        ("SNES", "Super Metroid (USA)"),
        ("SNES", "Rockman X (Japan)"),
    ] {
        let dir = d.path().join("Labels").join(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = std::fs::File::create(dir.join(format!("{stem}.png"))).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(f), 64, 24);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let rgb: Vec<u8> = (0..64 * 24).flat_map(|_| art).collect();
        enc.write_header().unwrap().write_image_data(&rgb).unwrap();
    }
    let carts = scan(d.path()).unwrap();
    // the GBA cart, the Game Boy pak and the American SNES cart keep their square labels in
    // square wells, as before: every corner of the label is the art, and just outside each of
    // its upper corners the well's shade
    for stem in ["Advance", "Grey", "Super Metroid (USA)"] {
        let cart = snes_cart(&carts, stem);
        let face = cart_face(cart);
        let (x0, y0, x1, y1) = colour_box(&face, art).expect("the label art was not pasted");
        for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
            assert_eq!(
                pixel(&face, x, y),
                art,
                "{stem}: the label's corner {x},{y}"
            );
        }
        let dark = well_shade(shell_for(cart).colour);
        assert_eq!(
            pixel(&face, x0 - 1, y0 - 1),
            dark,
            "{stem}: the well's corner"
        );
        assert_eq!(
            pixel(&face, x1 + 1, y0 - 1),
            dark,
            "{stem}: the well's corner"
        );
    }
    // the Super Famicom label's corners are round, its well's with them, stepped two pixels at
    // a time: the label's square corner is the well's shade, a step in is the art
    let cart = snes_cart(&carts, "Rockman X (Japan)");
    let face = cart_face(cart);
    let (x0, y0, x1, y1) = colour_box(&face, art).expect("the label art was not pasted");
    // a tenth smaller each way than it was first drawn, 312 by 102, on whole art pixels
    assert_eq!(
        (x1 + 1 - x0, y1 + 1 - y0),
        (280, 92),
        "the Super Famicom label is not a tenth smaller"
    );
    assert!(x0 % 2 == 0 && y0 % 2 == 0, "the label is off the art grid");
    let dark = well_shade(shell_for(cart).colour);
    for (x, y, dx, dy) in [(x0, y0, 1i32, 1i32), (x1, y0, -1, 1)] {
        let at = |i: i32, j: i32| {
            pixel(
                &face,
                (x as i32 + i * dx) as u32,
                (y as i32 + j * dy) as u32,
            )
        };
        assert_eq!(at(0, 0), dark, "the label's corner {x},{y} is not cut off");
        assert_eq!(
            at(1, 1),
            at(0, 0),
            "the corner does not step on the art grid"
        );
        assert_eq!(at(6, 6), art, "the corner is cut too deep");
    }
    for (x, y) in [(x0, y1), (x1, y1)] {
        assert_ne!(
            pixel(&face, x, y),
            art,
            "the label's lower corner {x},{y} is square"
        );
    }
    // and the well's own square corner, beyond the round one, is the plastic
    let (wx, wy) = (x0 - 5, y0 - 5);
    assert_ne!(pixel(&face, wx, wy), dark, "the well's corner is square");
    assert_eq!(
        pixel(&face, wx + 20, wy),
        dark,
        "the well's top is not where it was"
    );
}

#[test]
fn a_snes_cart_with_no_label_art_is_labelled_with_its_name() {
    let d = tmp_root();
    write_snes_rom(&d, "Super Metroid (USA).sfc", "SUPER METROID", 0x01);
    write_snes_rom(&d, "Rockman X (Japan).sfc", "ROCKMAN X", 0x00);
    let carts = scan(d.path()).unwrap();
    for stem in ["Super Metroid (USA)", "Rockman X (Japan)"] {
        let cart = snes_cart(&carts, stem);
        let face = cart_face(cart);
        let bg = label_colour(&label_text(cart));
        let (x0, y0, x1, y1) = colour_box(&face, bg).expect("no generated label");
        let inked = (y0..=y1)
            .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
            .filter(|&(x, y)| pixel(&face, x, y) != bg)
            .count();
        assert!(inked > 100, "{stem}: no title on its label");
    }
}

#[test]
fn each_snes_shell_comes_in_its_own_grey() {
    let d = tmp_root();
    write_snes_rom(&d, "Super Metroid (USA).sfc", "SUPER METROID", 0x01);
    write_snes_rom(&d, "Rockman X (Japan).sfc", "ROCKMAN X", 0x00);
    write_snes_rom(&d, "Chosen (Japan).sfc", "CHOSEN", 0x00);
    std::fs::write(
        d.path().join("Config/cart_shell.ini"),
        "Chosen (Japan) = auto c23f3a solid\n",
    )
    .unwrap();
    let carts = scan(d.path()).unwrap();
    assert_eq!(
        shell_for(snes_cart(&carts, "Super Metroid (USA)")),
        SNES_SHELL
    );
    assert_eq!(shell_for(snes_cart(&carts, "Rockman X (Japan)")), SFC_SHELL);
    let chosen = shell_for(snes_cart(&carts, "Chosen (Japan)"));
    assert_eq!(
        chosen.colour,
        [0xc2, 0x3f, 0x3a],
        "cart_shell.ini's colour is not used"
    );
    let lum = |c: [u8; 3]| c.iter().map(|v| u32::from(*v)).sum::<u32>();
    assert!(
        lum(SFC_SHELL.colour) > lum(SNES_SHELL.colour) + 60,
        "the Super Famicom's grey is not the lighter"
    );
    for s in [SNES_SHELL, SFC_SHELL] {
        assert_eq!(s.finish, Finish::Solid);
        let [r, g, b] = s.colour;
        assert!(
            r.abs_diff(g) < 8 && g.abs_diff(b) < 12,
            "{s:?} is not a grey"
        );
    }
}

#[test]
fn each_of_the_three_cgb_flags_gets_its_own_plastic() {
    let d = tmp_root();
    write_gb_rom(&d, "GB", "Tetris.gb", 0x00);
    write_gb_rom(&d, "GBC", "Colour Enhanced.gbc", 0x80);
    write_gb_rom(&d, "GBC", "Colour Only.gbc", 0xc0);
    let carts = scan(d.path()).unwrap();
    let by_stem = |stem: &str| {
        carts
            .iter()
            .find(|c| c.stem == stem)
            .unwrap_or_else(|| panic!("{stem} was not scanned"))
    };

    let grey = shell_for(by_stem("Tetris"));
    let black = shell_for(by_stem("Colour Enhanced"));
    let clear = shell_for(by_stem("Colour Only"));

    assert_eq!(grey.finish, Finish::Solid);
    assert_eq!(
        black.finish,
        Finish::Solid,
        "the 0x80 pak is black plastic, not clear"
    );
    assert_eq!(clear.finish, Finish::Translucent);

    let luma = |s: omega_ui::Shell| s.colour.iter().map(|c| *c as u32).sum::<u32>();
    assert!(
        luma(black) + 120 < luma(grey),
        "the black pak at {} is not darker than the grey one at {}",
        luma(black),
        luma(grey)
    );
    for (a, b) in [(grey, black), (black, clear), (grey, clear)] {
        assert_ne!(
            a.colour, b.colour,
            "two of the three plastics are one colour"
        );
    }
}

#[test]
fn the_notch_follows_the_cgb_flag_and_not_the_folder() {
    let d = tmp_root();
    write_gb_rom(&d, "GB", "Filed As Mono.gb", 0xc0);
    write_gb_rom(&d, "GBC", "Filed As Colour.gbc", 0x80);
    let carts = scan(d.path()).unwrap();
    let notched_away = |stem: &str| {
        let cart = carts.iter().find(|c| c.stem == stem).expect("scanned");
        pixel_alpha(&cart_face(cart), GB_CART_W - 20, 8) < 8
    };
    assert!(
        !notched_away("Filed As Mono"),
        "a 0xc0 rom in the GB folder was drawn with a notch it never had"
    );
    assert!(
        notched_away("Filed As Colour"),
        "a 0x80 rom in the GBC folder lost the notch its shell was moulded with"
    );
}

#[test]
fn only_the_notched_shell_has_lines_across_its_shoulder() {
    let d = tmp_root();
    write_gb_rom(&d, "GB", "Grey.gb", 0x00);
    write_gb_rom(&d, "GB", "Black.gb", 0x80);
    write_gb_rom(&d, "GBC", "Clear.gbc", 0xc0);
    let carts = scan(d.path()).unwrap();
    let steps = |stem: &str| {
        let cart = carts.iter().find(|c| c.stem == stem).expect("scanned");
        let face = cart_face(cart);
        let lum = |x: u32, y: u32| {
            let p = pixel(&face, x, y);
            p.iter().map(|c| u32::from(*c)).sum::<u32>() / 3
        };
        (gx(16)..gx(32))
            .flat_map(|x| (gy(21)..gy(58)).map(move |y| (x, y)))
            .filter(|(x, y)| lum(*x, *y).abs_diff(lum(*x, y - 1)) > 10)
            .count()
    };
    for stem in ["Grey", "Black"] {
        assert!(
            steps(stem) > 200,
            "the {stem} pak's shoulder came up smooth: {} stepped pixels, and five ribs a \
             side should leave hundreds",
            steps(stem)
        );
    }
    assert!(
        steps("Clear") < 10,
        "the Colour pak has lines across its header, which that shell does not have: {}",
        steps("Clear")
    );
}

#[test]
fn the_clear_shell_lightens_at_its_rim_and_the_plain_one_does_not() {
    let d = tmp_root();
    write_gb_rom(&d, "GB", "Tetris.gb", 0x00);
    write_gb_rom(&d, "GBC", "Colour Only.gbc", 0xc0);
    let carts = scan(d.path()).unwrap();
    let luma = |face: &omega_ui::CartFace, x: u32, y: u32| {
        let p = pixel(face, x, y);
        p[0] as u32 + p[1] as u32 + p[2] as u32
    };
    let y = GB_CART_H / 2;
    for cart in &carts {
        let face = cart_face(cart);
        let (rim, body) = (luma(&face, gx(8), y), luma(&face, gx(20), y));
        match cart.stem.as_str() {
            "Tetris" => assert_eq!(rim, body, "the plain pak has a lit rim"),
            _ => assert!(
                rim > body + 30,
                "the clear pak's rim is {rim} against {body} inside: it reads as solid"
            ),
        }
    }
}

/// A generated label's title is set at one art pixel (two of the label's) to one of Pixel Code's,
/// short or long, as every word in the UI is: never at one and a half (27 px) or two.
#[test]
fn a_generated_title_is_set_one_art_pixel_to_a_face_pixel() {
    let gba = |stem: &str| {
        let d = tmp_root();
        std::fs::write(d.path().join(format!("Games/GBA/{stem}.gba")), [0u8; 8]).unwrap();
        (
            cart_face(&scan(d.path()).unwrap()[0]),
            [LABEL_X, LABEL_Y, LABEL_W, LABEL_H],
        )
    };
    let gb = |stem: &str| {
        let d = tmp_root();
        write_gb_rom(&d, "GB", &format!("{stem}.gb"), 0x00);
        let label = [GB_LABEL_X, GB_LABEL_Y, GB_LABEL_W, GB_LABEL_H];
        (cart_face(&scan(d.path()).unwrap()[0]), label)
    };
    let long = "Supercalifragilisticexpialidocious Anniversary Edition";
    for (stem, (face, [lx, ly, lw, lh]), face_px) in [
        ("Zelda", gba("Zelda"), 2),
        ("Minish Cap", gba("Minish Cap"), 2),
        ("Pokemon Emerald Version", gba("Pokemon Emerald Version"), 2),
        (long, gba(long), 2),
        ("Tetris", gb("Tetris"), 2),
        ("Links Awakening", gb("Links Awakening"), 2),
    ] {
        let bg = label_colour(stem);
        let ink = |x: u32, y: u32| pixel(&face, lx + x, ly + y) != bg;
        let rows: Vec<u32> = (0..lh).filter(|&y| (0..lw).any(|x| ink(x, y))).collect();
        let caps = 1 + rows.windows(2).take_while(|r| r[1] == r[0] + 1).count() as u32;
        assert_eq!(caps, 7 * face_px, "{stem}: capitals {caps} rows tall");
        for y in 0..lh {
            let mut run = 0;
            for x in 0..=lw {
                if x < lw && ink(x, y) {
                    run += 1;
                } else if run > 0 {
                    assert_eq!(run % face_px, 0, "{stem}: a run of {run} at row {y}");
                    run = 0;
                }
            }
        }
    }
}

#[test]
fn a_game_boy_title_stays_inside_its_near_square_label() {
    for stem in [
        "Supercalifragilisticexpialidocious Anniversary Edition",
        "Wario Land 3",
        "Tetris",
    ] {
        let d = tmp_root();
        write_gb_rom(&d, "GB", &format!("{stem}.gb"), 0x00);
        let face = cart_face(&scan(d.path()).unwrap()[0]);
        let bg = label_colour(stem);
        let at = |x: u32, y: u32| pixel(&face, GB_LABEL_X + x, GB_LABEL_Y + y);
        let margin = 5;
        for y in 0..GB_LABEL_H {
            for x in 0..GB_LABEL_W {
                let edge = x < margin
                    || y < margin
                    || x >= GB_LABEL_W - margin
                    || y >= GB_LABEL_H - margin;
                if edge {
                    assert_eq!(
                        at(x, y),
                        bg,
                        "{stem}: text spills into the border at {x},{y}"
                    );
                }
            }
        }
        assert!(
            (0..GB_LABEL_H * GB_LABEL_W).any(|i| at(i % GB_LABEL_W, i / GB_LABEL_W) != bg),
            "{stem}: no text was drawn"
        );
    }
}

#[test]
fn the_cart_shadow_is_the_cart_in_black() {
    let s = omega_ui::cart_shadow();
    assert_eq!((s.w, s.h), (CART_W, CART_H));
    let mask = silhouette(CART_W, CART_H);
    for (px, cover) in s.rgba.chunks_exact(4).zip(&mask) {
        assert_eq!(&px[..3], &[0, 0, 0], "the shadow is not black");
        assert_eq!(px[3], *cover, "the shadow is not the cart's shape");
    }
    assert!(
        s.rgba.chunks_exact(4).any(|p| p[3] > 250),
        "the shadow is transparent everywhere, so it backs nothing"
    );
}

#[test]
fn each_game_boy_shell_gets_a_black_shadow_of_its_own_exact_outline() {
    for shell in [GbShell::Notched, GbShell::Rounded] {
        let s = omega_ui::gb_cart_shadow(shell);
        assert_eq!((s.w, s.h), (GB_CART_W, GB_CART_H));
        let own = gb_silhouette(shell, GB_CART_W, GB_CART_H);
        for (i, px) in s.rgba.chunks_exact(4).enumerate() {
            assert_eq!(&px[..3], &[0, 0, 0], "{shell:?}: the shadow is not black");
            assert_eq!(
                px[3], own[i],
                "{shell:?}: the shadow is not the shell's own shape at pixel {i}"
            );
        }
    }
    let notched = gb_silhouette(GbShell::Notched, GB_CART_W, GB_CART_H);
    let rounded = gb_silhouette(GbShell::Rounded, GB_CART_W, GB_CART_H);
    let bare: u32 = notched
        .iter()
        .zip(&rounded)
        .map(|(n, r)| u32::from(n.abs_diff(*r)))
        .sum();
    assert!(
        bare / 255 > 100,
        "the two shells now differ by {} px, so the shared backing was harmless after all",
        bare / 255
    );
}

#[test]
fn a_clear_pak_shows_its_board_and_a_solid_one_does_not() {
    let d = tmp_root();
    write_gb_rom(&d, "GBC", "Clear.gbc", 0xc0);
    write_gb_rom(&d, "GB", "Grey.gb", 0x00);
    let carts = scan(d.path()).unwrap();
    let face = |stem: &str| cart_face(carts.iter().find(|c| c.stem == stem).expect("scanned"));
    let (clear, grey) = (face("Clear"), face("Grey"));
    let (board, contact) = ((gx(24), GB_CART_H / 2), (gx(26), GB_CART_H - gy(8)));

    let [r, g, b] = pixel(&clear, board.0, board.1);
    assert!(
        g > r + 8 && g > b,
        "no board beside the label: {:?}",
        [r, g, b]
    );
    let [r, _, b] = pixel(&clear, contact.0, contact.1);
    assert!(r > b + 20, "no gold contact at the bottom: {:?}", [r, b]);

    for (x, y) in [board, contact] {
        let [r, g, b] = pixel(&grey, x, y);
        assert!(
            r.abs_diff(g) < 12 && g.abs_diff(b) < 12,
            "the grey pak shows something through it at {x},{y}: {:?}",
            [r, g, b]
        );
    }
}

#[test]
fn every_mould_is_lettered_with_its_platform() {
    let d = tmp_root();
    write_rom(&d, "Advance.gba", "ADVANCE");
    write_gb_rom(&d, "GB", "Grey.gb", 0x00);
    write_gb_rom(&d, "GBC", "Clear.gbc", 0xc0);
    let carts = scan(d.path()).unwrap();
    for (stem, rows, plain) in [
        ("Advance", 19..28, (120, 3)),
        ("Grey", 30..48, (197, 40)),
        ("Clear", 26..44, (197, 40)),
    ] {
        let face = cart_face(carts.iter().find(|c| c.stem == stem).expect("scanned"));
        let lum = |(x, y): (u32, u32)| {
            pixel(&face, x, y)
                .iter()
                .map(|c| u32::from(*c))
                .sum::<u32>()
        };
        let base = lum(plain);
        let band: Vec<u32> = rows
            .flat_map(|y| (60..180).map(move |x| (x, y)))
            .map(lum)
            .collect();
        assert!(
            band.iter().any(|l| *l > base + 30) && band.iter().any(|l| *l + 30 < base),
            "the {stem} face has no raised lettering where its platform name goes"
        );
    }
}

fn clear_gba(d: &TempDir) -> omega_ui::CartFace {
    let mut rom = vec![0u8; 0x100];
    rom[0xac..0xb0].copy_from_slice(b"U3IE");
    std::fs::write(d.path().join("Games/GBA/Boktai.gba"), rom).expect("write rom");
    let carts = scan(d.path()).unwrap();
    let cart = carts.iter().find(|c| c.stem == "Boktai").expect("scanned");
    assert_eq!(shell_for(cart).finish, Finish::Translucent);
    cart_face(cart)
}

fn bevel() -> u32 {
    (3 * CART_W + seated_box(Platform::Gba).0 / 2) / seated_box(Platform::Gba).0
}

/// A clear GBA cart shows its board round the label, darker than the hollow above it, as a
/// clear pak does; and the board's edge stands clear of the label's well, so the two never
/// read as one crooked frame.
#[test]
fn a_clear_gba_cart_shows_its_board_round_the_label_its_edge_clear_of_the_well() {
    let d = tmp_root();
    let face = clear_gba(&d);
    let luma = |[r, g, b]: [u8; 3]| r as u32 * 54 + g as u32 * 183 + b as u32 * 19;
    let y = LABEL_Y + LABEL_H / 2;
    let well = LABEL_X - bevel();
    let beside = pixel(&face, well - 2, y);
    let above = pixel(&face, well - 2, CART_H / 20);
    assert!(
        luma(beside) < luma(above),
        "no board beside the label: {beside:?} against the hollow {above:?}"
    );
    let run: Vec<[u8; 3]> = (well - 8..well).map(|x| pixel(&face, x, y)).collect();
    assert!(
        run.iter().all(|p| *p == run[0]),
        "the board's edge meets the label's well: {run:?}"
    );
}

#[test]
fn a_clear_cart_lights_its_label_recess_no_brighter_than_its_plastic() {
    let d = tmp_root();
    let face = clear_gba(&d);
    let luma = |[r, g, b]: [u8; 3]| (r as u32 * 54 + g as u32 * 183 + b as u32 * 19) / 256;
    let y = LABEL_Y + LABEL_H / 2;
    let lit = pixel(&face, LABEL_X + LABEL_W + 1, y);
    let plastic = pixel(&face, LABEL_X + LABEL_W + bevel() + 2, y);
    assert!(
        luma(lit) <= luma(plastic) * 13 / 10 + 4,
        "the recess edge {lit:?} glares against the plastic {plastic:?}"
    );
}

/// The holo sheen lies over the label alone, faint all over it and brighter in a band that moves
/// along it frame by frame, as the light does over a holographic card; a disc has none.
#[test]
fn the_holo_sheen_is_over_the_label_and_its_light_moves() {
    use omega_ui::CartModel;
    use omega_ui::{holo_frames, HOLO_FRAMES};
    assert!(holo_frames(CartModel::Disc).is_empty());
    let frames = holo_frames(CartModel::Gba);
    assert_eq!(frames.len(), HOLO_FRAMES);
    let alpha = |f: &omega_ui::CartFace, x: u32, y: u32| f.rgba[((y * f.w + x) * 4 + 3) as usize];
    let first = &frames[0];
    assert_eq!((first.w, first.h), (CART_W.div_ceil(2), CART_H.div_ceil(2)));
    // clear off the label, never clear on it
    assert_eq!(alpha(first, 1, 1), 0, "sheen over the plastic");
    let (cx, cy) = (
        u32::midpoint(LABEL_X, LABEL_W / 2),
        u32::midpoint(LABEL_Y, LABEL_H / 2),
    );
    assert!(
        frames.iter().all(|f| alpha(f, cx, cy) > 0),
        "a frame leaves the label bare"
    );
    // where the light stands across the label's middle row, weighed by how much brighter than
    // the faintest it is there, moves along it from frame to frame
    let light = |f: &omega_ui::CartFace| {
        let row: Vec<(f32, f32)> = (LABEL_X / 2..u32::midpoint(LABEL_X, LABEL_W))
            .map(|x| (x as f32, alpha(f, x, cy) as f32))
            .collect();
        let floor = row.iter().map(|r| r.1).fold(f32::MAX, f32::min);
        let weight: f32 = row.iter().map(|r| r.1 - floor).sum();
        row.iter().map(|r| r.0 * (r.1 - floor)).sum::<f32>() / weight.max(1.0)
    };
    let path: Vec<f32> = frames.iter().map(light).collect();
    assert!(
        path.windows(2).all(|p| p[0] <= p[1] + 0.5) && path[0] + 20.0 < path[HOLO_FRAMES - 1],
        "the light does not travel along the label: {path:?}"
    );
}

#[test]
fn a_spinning_disc_s_blur_is_the_same_all_the_way_round_and_keeps_its_edge() {
    // a disc with a label on one side only: the blur spreads it round each ring
    let (w, h) = (64u32, 64u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let d = (x as f32 + 0.5 - 32.0).hypot(y as f32 + 0.5 - 32.0);
            let i = ((y * w + x) * 4) as usize;
            if d < 31.0 {
                let red = if x < 32 { 255 } else { 0 };
                rgba[i..i + 4].copy_from_slice(&[red, 0, 255 - red, 255]);
            }
        }
    }
    let disc = omega_ui::CartFace { rgba, w, h };
    let blur = omega_ui::umd_disc_blur(&disc);
    let at = |x: u32, y: u32| &blur.rgba[((y * w + x) * 4) as usize..][..4];
    // two points on the same ring, either side, are alike, and half red, half blue
    let (left, right) = (at(12, 32), at(51, 32));
    for c in 0..3 {
        assert!(
            left[c].abs_diff(right[c]) <= 4,
            "{left:?} and {right:?} differ"
        );
    }
    assert!(
        left[0].abs_diff(128) < 20 && left[2].abs_diff(128) < 20,
        "{left:?}"
    );
    // outside the disc stays clear, inside stays solid
    assert_eq!(at(1, 1)[3], 0);
    assert_eq!(at(32, 32)[3], 255);
}
