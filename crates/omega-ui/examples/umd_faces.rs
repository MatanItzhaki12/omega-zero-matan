//! Draws each label PNG in a folder on a UMD's face, as the shelf shows it, for checking a
//! label's fit by eye: `cargo run -p omega-ui --example umd_faces -- <labels dir> <out dir>`.

use std::path::PathBuf;

use omega_store::{Cart, Platform, ShellChoice};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [labels, out] = &args[..] else {
        eprintln!("usage: umd_faces <labels dir> <out dir>");
        std::process::exit(2);
    };
    std::fs::create_dir_all(out).expect("the out dir");
    let mut entries: Vec<_> = std::fs::read_dir(labels)
        .expect("the labels dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    entries.sort();
    for path in entries {
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let cart = Cart {
            platform: Platform::Psp,
            stem: stem.clone(),
            rom: PathBuf::from("none.iso"),
            label: Some(path.clone()),
            title: stem.clone(),
            code: String::new(),
            shell: ShellChoice::parse(""),
        };
        let face = omega_ui::cart_face(&cart);
        let file = std::fs::File::create(PathBuf::from(out).join(format!("{stem}.png")))
            .expect("the face file");
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), face.w, face.h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .and_then(|mut w| w.write_image_data(&face.rgba))
            .expect("the face");
    }
}
