//! With SCRATCH_DIR set, writes the SNES carts' faces there as PNGs to look at: the American
//! cart and the Super Famicom's, each with a generated label and with label art, standing where
//! the shelf stands them between its bars, and each face on its own at four times its size.

use omega_store::scan;
use omega_ui::{cart_face, out_h, rest_y, CartFace, BAR_H, TOP_BAR_H};
use tempfile::TempDir;

const GROUND: [u8; 3] = [0x14, 0x15, 0x1a];
const PLATE: [u8; 3] = [0x25, 0x27, 0x2e];
const FLOOR: [u8; 3] = [0x3a, 0x3d, 0x46];
const GAP: u32 = 24;

/// A LoROM image with a header naming `title`, sold in `destination`.
fn write_rom(d: &TempDir, name: &str, title: &str, destination: u8) {
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

/// A label `w` by `h`: bands of colour under a dark title bar, like a printed label.
fn write_label(d: &TempDir, stem: &str, w: u32, h: u32) {
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let c = if y < h / 4 {
                [0x18, 0x18, 0x20]
            } else {
                let t = (x * 255 / w) as u8;
                [0x30 + t / 2, 0x60, 0xd0 - t / 2]
            };
            rgb.extend_from_slice(&c);
        }
    }
    let dir = d.path().join("Labels/SNES");
    std::fs::create_dir_all(&dir).expect("create labels dir");
    let f = std::fs::File::create(dir.join(format!("{stem}.png"))).expect("create label");
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .expect("png header")
        .write_image_data(&rgb)
        .expect("png data");
}

fn write_png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) {
    let f = std::fs::File::create(path).expect("create png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .expect("png header")
        .write_image_data(rgba)
        .expect("png data");
}

fn zoomed(face: &CartFace, zoom: u32) -> Vec<u8> {
    let (w, h) = (face.w * zoom, face.h * zoom);
    let mut px = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let s = (((y / zoom) * face.w + x / zoom) * 4) as usize;
            let d = ((y * w + x) * 4) as usize;
            px[d..d + 4].copy_from_slice(&face.rgba[s..s + 4]);
        }
    }
    px
}

#[test]
fn render_snes_cart() {
    let Ok(dir) = std::env::var("SCRATCH_DIR") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("create the scratch dir");
    let d = tempfile::tempdir().expect("tempdir");
    write_rom(&d, "Super Metroid (USA).sfc", "SUPER METROID", 0x01);
    write_rom(&d, "Chrono Trigger.sfc", "CHRONO TRIGGER", 0x01);
    write_rom(
        &d,
        "Seiken Densetsu 3 (Japan).sfc",
        "SEIKEN DENSETSU 3",
        0x00,
    );
    write_rom(&d, "Terranigma.sfc", "TERRANIGMA", 0x02);
    write_label(&d, "Chrono Trigger", 640, 280);
    write_label(&d, "Terranigma", 900, 300);
    let carts = scan(d.path()).expect("scan");
    let faces: Vec<CartFace> = carts.iter().map(cart_face).collect();

    let sheet_w: u32 = faces.iter().map(|f| f.w + GAP).sum::<u32>() + GAP;
    let sheet_h = out_h();
    let mut frame = Vec::with_capacity((sheet_w * sheet_h * 4) as usize);
    for y in 0..sheet_h {
        let floor = y == (sheet_h as f32 - BAR_H) as u32;
        let c = match () {
            () if floor => FLOOR,
            () if (y as f32) < TOP_BAR_H => PLATE,
            () => GROUND,
        };
        for _ in 0..sheet_w {
            frame.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    let mut left = GAP;
    for (cart, face) in carts.iter().zip(&faces) {
        let top = rest_y(face.h as f32) as u32;
        for y in 0..face.h {
            for x in 0..face.w {
                let s = ((y * face.w + x) * 4) as usize;
                let a = face.rgba[s + 3] as u32;
                let d = (((y + top) * sheet_w + x + left) * 4) as usize;
                for c in 0..3 {
                    frame[d + c] =
                        ((face.rgba[s + c] as u32 * a + frame[d + c] as u32 * (255 - a) + 127)
                            / 255) as u8;
                }
            }
        }
        let name = cart.stem.to_lowercase().replace([' ', '(', ')'], "");
        write_png(
            &dir.join(format!("snes-face-{name}-4x.png")),
            face.w * 4,
            face.h * 4,
            &zoomed(face, 4),
        );
        println!(
            "{} is {}x{}, standing {}..{}",
            cart.stem,
            face.w,
            face.h,
            top,
            top + face.h
        );
        left += face.w + GAP;
    }
    write_png(&dir.join("snes-faces.png"), sheet_w, sheet_h, &frame);
    println!("wrote {}", dir.display());
}
