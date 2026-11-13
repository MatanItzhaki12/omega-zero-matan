//! The boot images as an old PC's BIOS screen, after Award's, told about omega-zero: the
//! still screen the bootloader shows while Linux starts, in the opening's letters and where its
//! terminal prints, so it runs on into the opening. The OmegaZero logo stands where Award put
//! its Energy Star.
//!
//! `bootscreens OUT_DIR` writes OUT_DIR/bootlogo-<w>x<h>.bmp for each panel omega-zero runs on,
//! with the logo `assets/logo/bioslogo.sh` made.

use std::path::{Path, PathBuf};

use omega_ui::{opening_line, UndoFace, OPENING_ADVANCE, OPENING_PITCH, OPENING_RISE};
use omega_zero::opening::terminal_origin;

const PANELS: [(u32, u32); 3] = [(640, 480), (720, 480), (720, 720)];

/// How far the logo's right edge stands past the terminal's last column, in screen pixels.
const LOGO_OVERHANG: u32 = 8;

const HEADER: usize = 54;

/// The screen's lines, by the terminal's row.
fn lines() -> Vec<(u32, String)> {
    let version = env!("CARGO_PKG_VERSION");
    [
        (0, format!("OZ-Matan BIOS v{version},")),
        (1, "An Anbernic Ally".into()),
        (2, "Copyright (C) 2026,".into()),
        (3, "Matan Software Inc.".into()),
        (5, "ANBERNIC H700 Rev Beta".into()),
        (7, "H700 x4 CPU at 1.5GHz".into()),
        (8, "Memory Test : 1048576K OK".into()),
        (10, "BaseOS Plug and Play Extension".into()),
        (11, "Copyright (C) 2026, BaseOS".into()),
        (13, "_".into()),
        (20, "Press START to enter SETTINGS".into()),
        (21, "10/11/26-H700-AXP2202-8821CS-00".into()),
    ]
    .into()
}

fn main() {
    let Some(out) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: bootscreens OUT_DIR");
        std::process::exit(2);
    };
    let logo_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/logo/omega-zero-logo-bios.png");
    let logo =
        read_png(&logo_path).expect("read the BIOS logo; make it with assets/logo/bioslogo.sh");
    let faces: Vec<(u32, UndoFace)> = lines()
        .into_iter()
        .map(|(row, text)| (row, opening_line(&text)))
        .collect();
    for (w, h) in PANELS {
        let mut screen = Screen::black(w, h);
        let (x0, y0) = terminal_origin(w, h);
        for (row, face) in &faces {
            screen.put(face, x0, y0 + row * OPENING_PITCH - OPENING_RISE);
        }
        let right = x0 + 32 * OPENING_ADVANCE + LOGO_OVERHANG;
        screen.put(&logo, right - logo.w, y0);
        let path = out.join(format!("bootlogo-{w}x{h}.bmp"));
        std::fs::write(&path, screen.bmp()).expect("write the boot image");
        println!("{}", path.display());
    }
}

struct Screen {
    w: u32,
    h: u32,
    rgb: Vec<[u8; 3]>,
}

impl Screen {
    fn black(w: u32, h: u32) -> Screen {
        Screen {
            w,
            h,
            rgb: vec![[0; 3]; (w * h) as usize],
        }
    }

    /// `face`'s opaque pixels, its top left at (`x`, `y`), clipped to the screen.
    fn put(&mut self, face: &UndoFace, x: u32, y: u32) {
        for fy in 0..face.h.min(self.h.saturating_sub(y)) {
            for fx in 0..face.w.min(self.w.saturating_sub(x)) {
                let i = ((fy * face.w + fx) * 4) as usize;
                if face.rgba[i + 3] >= 128 {
                    let p = &face.rgba[i..i + 3];
                    self.rgb[((y + fy) * self.w + x + fx) as usize] = [p[0], p[1], p[2]];
                }
            }
        }
    }

    /// The screen as the uncompressed bottom-up 24-bit bmp the bootloader draws.
    fn bmp(&self) -> Vec<u8> {
        let stride = (self.w as usize * 3).next_multiple_of(4);
        let size = HEADER + stride * self.h as usize;
        let mut b = vec![0u8; HEADER];
        b[0..2].copy_from_slice(b"BM");
        b[2..6].copy_from_slice(&(size as u32).to_le_bytes());
        b[10..14].copy_from_slice(&(HEADER as u32).to_le_bytes());
        b[14..18].copy_from_slice(&40u32.to_le_bytes());
        b[18..22].copy_from_slice(&self.w.to_le_bytes());
        b[22..26].copy_from_slice(&self.h.to_le_bytes());
        b[26..28].copy_from_slice(&1u16.to_le_bytes());
        b[28..30].copy_from_slice(&24u16.to_le_bytes());
        b[34..38].copy_from_slice(&((stride * self.h as usize) as u32).to_le_bytes());
        b[38..42].copy_from_slice(&3780u32.to_le_bytes());
        b[42..46].copy_from_slice(&3780u32.to_le_bytes());
        for row in self.rgb.chunks_exact(self.w as usize).rev() {
            let start = b.len();
            for [r, g, bl] in row {
                b.extend_from_slice(&[*bl, *g, *r]);
            }
            b.resize(start + stride, 0);
        }
        b
    }
}

/// A png as four bytes a pixel, top row first.
fn read_png(path: &Path) -> Option<UndoFace> {
    let file = std::fs::File::open(path).ok()?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(file));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba {
        return None;
    }
    buf.truncate(info.buffer_size());
    Some(UndoFace {
        rgba: buf,
        w: info.width,
        h: info.height,
    })
}
