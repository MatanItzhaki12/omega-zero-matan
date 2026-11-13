//! A label scan, once cut, is shrunk to the smallest of its own shape that still covers every
//! face its system draws it on: no face changes, its cut stays current, and it takes a fraction
//! of the card.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use omega_store::{scan, Cart, Platform};
use omega_ui::{
    cart_face, cut_label, face_labels, keep_size, label_uncut, shrink_label, LABEL_H, LABEL_W,
};
use tempfile::TempDir;

fn card() -> TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    for sub in ["Games/GBA", "Games/PS1", "Labels/GBA", "Labels/PS1"] {
        std::fs::create_dir_all(d.path().join(sub)).expect("create content dir");
    }
    d
}

fn write_rom(d: &TempDir, name: &str) {
    std::fs::write(d.path().join("Games/GBA").join(name), vec![0u8; 0x100]).expect("write rom");
}

fn write_png(path: &Path, w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 4]) {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            rgba.extend_from_slice(&px(x, y));
        }
    }
    let f = std::fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .expect("png header")
        .write_image_data(&rgba)
        .expect("png data");
}

/// A scan that shades smoothly across, as a printed label mostly does, but no two rows alike.
fn shaded(w: u32, h: u32) -> impl Fn(u32, u32) -> [u8; 4] {
    move |x, y| {
        [
            (x * 255 / w) as u8,
            (y * 255 / h) as u8,
            ((x + y) * 127 / (w + h)) as u8 + 64,
            255,
        ]
    }
}

fn png_info(path: &Path) -> (u32, u32, png::ColorType) {
    let file = std::fs::File::open(path).expect("open png");
    let reader = png::Decoder::new(file).read_info().expect("png header");
    let info = reader.info();
    (info.width, info.height, info.color_type)
}

fn modified(path: &Path) -> SystemTime {
    std::fs::metadata(path).unwrap().modified().unwrap()
}

fn len(path: &Path) -> u64 {
    std::fs::metadata(path).unwrap().len()
}

#[test]
fn a_kept_size_never_grows_keeps_the_scans_shape_and_covers_every_face_of_its_system() {
    assert_eq!(face_labels(Platform::Gb).len(), 2, "both Game Boy outlines");
    assert_eq!(face_labels(Platform::Snes).len(), 2, "both SNES shells");
    let scans = [
        (600, 310),
        (600, 600),
        (600, 200),
        (300, 600),
        (432, 432),
        (1200, 90),
    ];
    for platform in [
        Platform::Gba,
        Platform::Gb,
        Platform::Gbc,
        Platform::Snes,
        Platform::Ps1,
    ] {
        for (sw, sh) in scans {
            let Some((kw, kh)) = keep_size(platform, sw, sh) else {
                continue;
            };
            let at = format!("{platform:?} {sw}x{sh} kept at {kw}x{kh}");
            assert!(kw <= sw && kh <= sh && (kw < sw || kh < sh), "{at}: grew");
            // each side rounded up by under a pixel
            let skew = (kw as i64 * sh as i64 - kh as i64 * sw as i64).abs();
            assert!(skew < sw.max(sh) as i64, "{at}: not the scan's shape");
            for (w, h) in face_labels(platform) {
                let scale = (w as f64 / kw as f64).max(h as f64 / kh as f64);
                assert!(scale <= 1.0, "{at}: a {w}x{h} face drawn from it stretched");
            }
            assert_eq!(keep_size(platform, kw, kh), None, "{at}: shrunk again");
        }
    }
    assert_eq!(keep_size(Platform::Gba, LABEL_W, LABEL_H), None);
    assert_eq!(
        keep_size(Platform::Gba, 100, 50),
        None,
        "a small scan is not grown"
    );
}

#[test]
fn a_shrunk_scan_keeps_its_time_so_its_cut_stays_and_the_face_is_the_one_it_drew() {
    let d = card();
    write_rom(&d, "Shrunk.gba");
    let scan_path = d.path().join("Labels/GBA/Shrunk.png");
    write_png(&scan_path, 600, 310, shaded(600, 310));
    let cart = &scan(d.path()).unwrap()[0];
    assert!(cut_label(cart));
    let face = cart_face(cart);
    let (was, scanned) = (len(&scan_path), modified(&scan_path));
    let cut = d.path().join("Labels/GBA/.cut/Shrunk.png");
    let cut_bytes = std::fs::read(&cut).unwrap();

    shrink_label(cart);
    let (w, h, colour) = png_info(&scan_path);
    assert_eq!((w, h), (268, 138), "shrunk to cover the GBA's label");
    assert_eq!(colour, png::ColorType::Rgb, "an opaque scan keeps no alpha");
    assert!(
        len(&scan_path) * 4 < was,
        "{} of {was} bytes",
        len(&scan_path)
    );
    assert_eq!(modified(&scan_path), scanned, "the scan keeps its time");
    assert!(!label_uncut(cart), "its cut is still current");
    assert_eq!(
        std::fs::read(&cut).unwrap(),
        cut_bytes,
        "the cut is left as it was"
    );
    assert!(cart_face(cart).rgba == face.rgba);

    // cut again from the shrunk scan, the face is the one the whole scan drew, near enough
    std::fs::remove_file(&cut).unwrap();
    assert!(cut_label(cart));
    let again = cart_face(cart);
    let off = again
        .rgba
        .iter()
        .zip(&face.rgba)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    assert!(off <= 3, "a face cut from the shrunk scan is {off} off");
}

#[test]
fn a_scan_no_bigger_than_its_faces_or_shrunk_already_is_left_as_it_is() {
    let d = card();
    write_rom(&d, "Small.gba");
    let scan_path = d.path().join("Labels/GBA/Small.png");
    write_png(&scan_path, LABEL_W, LABEL_H, shaded(LABEL_W, LABEL_H));
    let cart = &scan(d.path()).unwrap()[0];
    let before = std::fs::read(&scan_path).unwrap();
    assert!(cut_label(cart));
    shrink_label(cart);
    assert_eq!(std::fs::read(&scan_path).unwrap(), before, "a small scan");

    write_png(&scan_path, 600, 310, shaded(600, 310));
    assert!(cut_label(cart));
    shrink_label(cart);
    let shrunk = std::fs::read(&scan_path).unwrap();
    shrink_label(cart);
    assert_eq!(std::fs::read(&scan_path).unwrap(), shrunk, "shrunk twice");
}

/// A PlayStation game on two discs, each with a scan of its own, and the game's own label, a
/// copy of disc 1's.
fn two_discs(d: &TempDir) -> Cart {
    let games = d.path().join("Games/PS1");
    std::fs::create_dir_all(games.join("Saga")).unwrap();
    let mut list = String::new();
    for n in 1..=2 {
        let disc = format!("Saga (USA) (Disc {n})");
        std::fs::write(games.join("Saga").join(format!("{disc}.cue")), "").unwrap();
        let _ = writeln!(list, "Saga/{disc}.cue");
    }
    std::fs::write(games.join("Saga (USA).m3u"), list).unwrap();
    // a disc's printed face, round, its corners clear
    let disc = |x: u32, y: u32| {
        let (dx, dy) = (x as i64 - 300, y as i64 - 300);
        let inside = dx * dx + dy * dy <= 300 * 300;
        [
            (x * 255 / 600) as u8,
            90,
            (y * 255 / 600) as u8,
            if inside { 255 } else { 0 },
        ]
    };
    for name in ["Saga (USA)", "Saga (USA) (Disc 1)", "Saga (USA) (Disc 2)"] {
        write_png(&labels(d, name), 600, 600, disc);
    }
    scan(d.path()).unwrap().remove(0)
}

fn labels(d: &TempDir, name: &str) -> PathBuf {
    d.path().join("Labels/PS1").join(format!("{name}.png"))
}

#[test]
fn a_discs_scan_keeps_its_clear_corners_and_its_games_own_copy_is_shrunk_too() {
    let d = card();
    let cart = two_discs(&d);
    assert!(cut_label(&cart));
    let face = cart_face(&cart);
    shrink_label(&cart);
    for name in ["Saga (USA)", "Saga (USA) (Disc 1)", "Saga (USA) (Disc 2)"] {
        let (w, h, colour) = png_info(&labels(&d, name));
        assert_eq!((w, h), (288, 288), "{name}: shrunk to the disc's face");
        assert_eq!(
            colour,
            png::ColorType::Rgba,
            "{name}: lost its clear corners"
        );
    }
    assert!(!label_uncut(&cart), "the discs' cuts are still current");
    assert!(cart_face(&cart).rgba == face.rgba);
}
