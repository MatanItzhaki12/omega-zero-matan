//! Plays a DS game headless through its Fulldex layouts, as a game in omega-zero is played,
//! to check them by eye: each `--every` frames the view laid out is written as a PNG, named by
//! its frame and the scene it is, and the scenes the run went through are listed at the end.
//!
//! `cargo run -p omega-zero --example ds_layouts -- CORE ROM OUT [--state FILE] [--state-out FILE]
//! [--system DIR] [--script FILE] [--frames N] [--every N] [--layout focus|picture|...]
//! [--no-fulldex]`
//!
//! The script's lines are `FROM TO KEY...`: the keys held from frame FROM to before TO, among A B
//! X Y L R START SELECT UP DOWN LEFT RIGHT L2 R2, counted from the run's start. `--state-out`
//! saves the game's state at the run's end, for the next run to go on from with `--state`.
//! `--no-fulldex` shows the layout alone, as Fulldex switched off does.

use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Path, PathBuf};

use omega_retro::{ButtonMask, LibretroCore, RetroCore};
use omega_store::Core;
use omega_zero::ds_screen::DsScreens;

const USAGE: &str = "usage: ds_layouts CORE ROM OUT [--state FILE] [--state-out FILE] \
[--system DIR] [--script FILE] [--frames N] [--every N] [--layout NAME] [--no-fulldex]";

/// libretro's joypad bits, as the script names them.
const KEYS: [(&str, u16); 14] = [
    ("B", 1 << 0),
    ("Y", 1 << 1),
    ("SELECT", 1 << 2),
    ("START", 1 << 3),
    ("UP", 1 << 4),
    ("DOWN", 1 << 5),
    ("LEFT", 1 << 6),
    ("RIGHT", 1 << 7),
    ("A", 1 << 8),
    ("X", 1 << 9),
    ("L", 1 << 10),
    ("R", 1 << 11),
    ("L2", 1 << 12),
    ("R2", 1 << 13),
];

struct Step {
    from: u32,
    to: u32,
    keys: u16,
}

fn script(path: &Path) -> Result<Vec<Step>, Box<dyn Error>> {
    let mut steps = Vec::new();
    for line in std::fs::read_to_string(path)?.lines() {
        let mut w = line.split_whitespace();
        let (Some(from), Some(to)) = (w.next(), w.next()) else {
            continue;
        };
        if from.starts_with('#') {
            continue;
        }
        let mut keys = 0;
        for k in w {
            keys |= KEYS
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, b)| *b)
                .ok_or_else(|| format!("no key {k}"))?;
        }
        steps.push(Step {
            from: from.parse()?,
            to: to.parse()?,
            keys,
        });
    }
    Ok(steps)
}

fn png(path: &Path, w: u32, h: u32, xrgb: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut rgba = Vec::with_capacity(xrgb.len());
    for px in xrgb.chunks_exact(4) {
        rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
    }
    let file = std::fs::File::create(path)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&rgba)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut it = std::env::args().skip(1);
    let mut positional = Vec::new();
    let (mut state, mut system, mut steps) = (None::<PathBuf>, None::<PathBuf>, Vec::new());
    let mut state_out = None::<PathBuf>;
    let (mut frames, mut every) = (600u32, 60u32);
    let mut arrangement = omega_ds::Arrangement::default();
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(USAGE);
        match arg.as_str() {
            "--state" => state = Some(value()?.into()),
            "--state-out" => state_out = Some(value()?.into()),
            "--system" => system = Some(value()?.into()),
            "--script" => steps = script(Path::new(&value()?))?,
            "--frames" => frames = value()?.parse()?,
            "--every" => every = value()?.parse()?,
            "--layout" => arrangement.layout = omega_ds::Layout::parse(&value()?).ok_or(USAGE)?,
            "--no-fulldex" => arrangement.fulldex = false,
            _ => positional.push(PathBuf::from(arg)),
        }
    }
    let [core_path, rom, out] = <[PathBuf; 3]>::try_from(positional).map_err(|_| USAGE)?;
    std::fs::create_dir_all(&out)?;
    let system =
        system.unwrap_or_else(|| rom.parent().unwrap_or_else(|| Path::new(".")).to_path_buf());
    let mut core = LibretroCore::open_with(&core_path, &system, &system)?;
    for (k, v) in omega_zero::core::core_options(Core::MelonDs, "", true, false) {
        core.set_option(k, &v);
    }
    core.load(&rom)?;
    if let Some(state) = state {
        core.run_frame(ButtonMask(0));
        core.unserialize(&std::fs::read(state)?)?;
    }
    let mut screens = DsScreens::start(&mut core, &rom, arrangement);
    let mut seen: BTreeMap<&'static str, u32> = BTreeMap::new();
    let mut view = Vec::new();
    for frame in 0..frames {
        let held = steps
            .iter()
            .filter(|s| (s.from..s.to).contains(&frame))
            .fold(0, |k, s| k | s.keys);
        let input = screens.input(&mut core, ButtonMask(held));
        core.run_frame(input);
        screens.ran();
        let Some((w, h)) = screens.view(&core, &mut view) else {
            continue;
        };
        *seen.entry(screens.scene()).or_default() += 1;
        if (frame + 1) % every == 0 {
            let name = format!("f{:05}-{}.png", frame + 1, screens.scene());
            png(&out.join(name), w, h, &view)?;
        }
    }
    for (scene, n) in seen {
        println!("{scene}: {n} frames");
    }
    if let Some(path) = state_out {
        std::fs::write(path, core.serialize()?)?;
    }
    Ok(())
}
