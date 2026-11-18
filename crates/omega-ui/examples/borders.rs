//! Draws each system's built-in border on a 720x480 screen around its picture, a grey stand-in
//! where the game goes, for checking them by eye: `cargo run -p omega-ui --example borders --
//! <out dir>`.

use omega_store::Platform;

fn main() {
    let Some(out) = std::env::args().nth(1) else {
        eprintln!("usage: borders <out dir>");
        std::process::exit(2);
    };
    std::fs::create_dir_all(&out).expect("the out dir");
    let (w, h) = (720u32, 480u32);
    for (platform, picture) in [
        (Platform::Gb, [120.0, 24.0, 480.0, 432.0]),
        (Platform::Gbc, [120.0, 24.0, 480.0, 432.0]),
        (Platform::Snes, [104.0, 16.0, 512.0, 448.0]),
        (Platform::Ps1, [40.0, 0.0, 640.0, 480.0]),
        (Platform::Psp, [0.0, 36.0, 720.0, 408.0]),
        (Platform::Nds, [104.0, 48.0, 512.0, 384.0]),
        (Platform::Nds, [40.0, 0.0, 640.0, 480.0]),
        (Platform::Nds, [200.0, 0.0, 320.0, 480.0]),
    ] {
        let Some(face) = omega_ui::border_face(platform, w, h, picture) else {
            println!("{} {picture:?}: no room for a border", platform.dir_name());
            continue;
        };
        let mut rgba = face.rgba;
        // the clear picture as a grey, so the PNG shows where it goes
        for px in rgba.chunks_exact_mut(4).filter(|p| p[3] == 0) {
            px.copy_from_slice(&[0x60, 0x68, 0x78, 255]);
        }
        let name = format!(
            "{}/{}-{}x{}.png",
            out,
            platform.dir_name(),
            picture[2],
            picture[3]
        );
        let file = std::fs::File::create(&name).expect("the png");
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .and_then(|mut wr| wr.write_image_data(&rgba))
            .expect("written");
        println!("{name}");
    }
}
