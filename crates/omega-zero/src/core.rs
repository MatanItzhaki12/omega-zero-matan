use std::path::{Path, PathBuf};

use omega_retro::{LibretroCore, MockCore, RetroCore};
use omega_store::Core;

use crate::root;

const CORE_ENV: &str = "OMEGA_CORE";

pub fn dylib_name(core: Core) -> String {
    format!(
        "{}_libretro.{}",
        core.library(),
        std::env::consts::DLL_EXTENSION
    )
}

fn candidates(root: &Path, core: Core) -> Vec<PathBuf> {
    if let Some(named) = std::env::var_os(CORE_ENV) {
        return vec![PathBuf::from(named)];
    }
    let name = dylib_name(core);
    let mut paths = vec![root.join("System").join(&name)];
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        paths.push(dir.join(&name));
        paths.push(omega_retro::host_core(&dir.join("vendor"), core.library()));
    }
    // a computer's own cores, from the repo, never the console's beside them in vendor/
    paths.push(omega_retro::host_core(Path::new("vendor"), core.library()));
    paths
}

pub struct Opened {
    pub core: Box<dyn RetroCore>,
    pub named: bool,
}

/// What a game is opened with besides its core: a handheld's link cable mode, its screen's
/// colour correction and the player it is on a cable. A SNES or PlayStation game has none of it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Setup<'a> {
    Handheld {
        serial: &'a str,
        colour: bool,
        link: Option<u8>,
    },
    Console,
}

impl<'a> Setup<'a> {
    /// The serial mode, colour correction and cable player the core's options are set from.
    fn handheld(self) -> (&'a str, bool, Option<u8>) {
        match self {
            Setup::Handheld {
                serial,
                colour,
                link,
            } => (serial, colour, link),
            Setup::Console => ("auto", false, None),
        }
    }
}

/// Loads gpSP for good, with room kept beside it for its code cache (see omega_retro::gpsp):
/// the first of the places a game's gpSP is opened from that has it.
pub fn keep_gpsp_room(root: &Path) {
    if let Some(path) = candidates(root, Core::Gpsp)
        .into_iter()
        .find(|p| p.is_file())
    {
        omega_retro::gpsp::keep_room(&path);
    }
}

/// Opens `core` for a game, from the content root's System folder or beside omega-zero, with the
/// options omega-zero sets on it; the mock test pattern when it is nowhere.
pub fn open(root: &Path, core: Core, setup: Setup) -> Opened {
    let paths = candidates(root, core);
    match open_named(root, core, setup, &paths) {
        Some(core) => Opened { core, named: true },
        None => {
            report_missing(core, &paths);
            Opened {
                core: Box::new(stand_in(core)),
                named: false,
            }
        }
    }
}

/// The mock test pattern that stands in for a core that is nowhere, drawing its system's usual
/// picture, so a SNES or PlayStation game without its core is still laid out as one.
fn stand_in(core: Core) -> MockCore {
    match core {
        Core::Mgba | Core::Gpsp => MockCore::new(),
        Core::Snes9x | Core::Supafaust => MockCore::sized(256, 224),
        Core::PcsxRearmed => MockCore::sized(320, 240),
        Core::Ppsspp => MockCore::sized(480, 272),
    }
}

/// `open` for a handheld game.
pub fn open_core(root: &Path, core: Core, serial: &str, colour: bool, link: Option<u8>) -> Opened {
    open(
        root,
        core,
        Setup::Handheld {
            serial,
            colour,
            link,
        },
    )
}

pub fn open_core_for(
    root: &Path,
    core: Core,
    serial: &str,
    colour: bool,
    paths: &[PathBuf],
) -> Box<dyn RetroCore> {
    let setup = Setup::Handheld {
        serial,
        colour,
        link: None,
    };
    open_named(root, core, setup, paths).unwrap_or_else(|| {
        report_missing(core, paths);
        Box::new(stand_in(core))
    })
}

/// The folder a core reads its system files from: BIOS/, where a player puts a BIOS, or for
/// PPSSPP System/, where its support files ship in System/PPSSPP/, the fonts PSP games draw
/// their text with among them. PPSSPP looks in the system folder's PPSSPP/; given BIOS/ it found
/// nothing, and every game's menus came up without a word on them.
pub fn system_dir(root: &Path, core: Core) -> PathBuf {
    match core {
        Core::Ppsspp => root.join("System"),
        _ => root::bios_dir(root),
    }
}

fn open_named(
    root: &Path,
    core: Core,
    setup: Setup,
    paths: &[PathBuf],
) -> Option<Box<dyn RetroCore>> {
    // a core put away a moment ago may still be closing on its emulator's thread
    crate::emu::wait_closed();
    let system = system_dir(root, core);
    let saves = root::saves_dir(root);
    let (serial, colour, link) = setup.handheld();
    for path in paths {
        if !path.exists() {
            continue;
        }
        // the options are there before the core starts, for one that reads them as it does
        let options = core_options(core, serial, root::has_real_bios(root), colour);
        let early: Vec<(&str, &str)> = options.iter().map(|(k, v)| (*k, v.as_str())).collect();
        match LibretroCore::open_with_options(path, &system, &saves, &early) {
            Ok(mut opened) => {
                apply_core_options(&mut opened, core, serial, root::has_real_bios(root), colour);
                if let Some(player) = link {
                    apply_link_options(&mut opened, core, player);
                }
                eprintln!("omega-zero: core {}", path.display());
                return Some(Box::new(opened));
            }
            Err(e) => eprintln!("omega-zero: {}: {e}", path.display()),
        }
    }
    None
}

fn report_missing(core: Core, paths: &[PathBuf]) {
    eprintln!(
        "omega-zero: no {} core found, running the mock test pattern instead. Looked in: {}",
        core.as_str(),
        paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
}

pub fn colour_option(which: Core, on: bool) -> Option<(&'static str, &'static str)> {
    match which {
        Core::Mgba => Some(("mgba_color_correction", if on { "Auto" } else { "OFF" })),
        Core::Gpsp => Some((
            "gpsp_color_correction",
            if on { "enabled" } else { "disabled" },
        )),
        Core::Snes9x | Core::Supafaust | Core::PcsxRearmed | Core::Ppsspp => None,
    }
}

pub fn apply_link_options(core: &mut LibretroCore, which: Core, player: u8) {
    if which != Core::Mgba {
        return;
    }
    core.set_option("mgba_link", "on");
    core.set_option("mgba_link_player", &player.to_string());
    eprintln!("omega-zero: core: link mode on, player {player}");
}

pub fn apply_core_options(
    core: &mut LibretroCore,
    which: Core,
    serial: &str,
    bios: bool,
    colour: bool,
) {
    for (key, value) in core_options(which, serial, bios, colour) {
        core.set_option(key, &value);
    }
}

/// Every option omega-zero sets on `which`, in the order it sets them, each spelt as that core
/// declares it. `serial`, the BIOS and `colour` only reach the handhelds' cores.
///
/// Frame skipping is asked for wherever the core has it, so fast-forward can skip frames by
/// reporting an underrun: mGBA and gpSP name it `_frameskip`, pcsx_rearmed
/// `_frameskip_type`; Snes9x has none and skips a frame the frontend says not to draw, and
/// Supafaust skips none. The SNES picture is kept to its 224 lines rather than the 239 a few
/// games show, so it is exactly twice that on every canvas. Supafaust is left to run its
/// threads wherever the system puts them: by itself it would pin the emulator thread to the
/// second CPU and its PPU thread to the first, where the render thread and the kernel's
/// interrupts run. The PlayStation runs its own BIOS from BIOS/ when one is there and its
/// built-in one otherwise, and keeps memory card 1 as the game's save, in Saves/PS1; the
/// second card, which pcsx_rearmed would keep in a file of its own in Saves, is left out.
pub fn core_options(
    which: Core,
    serial: &str,
    bios: bool,
    colour: bool,
) -> Vec<(&'static str, String)> {
    let mut set: Vec<(&'static str, String)> = Vec::new();
    let mut put = |key: &'static str, value: &str| set.push((key, value.to_string()));
    match which {
        Core::Mgba => {
            put("mgba_frameskip", "auto");
            put("mgba_sgb_borders", "OFF");
            put("mgba_gb_colors_preset", "1");
            put("mgba_gb_colors", "GBC Dark Green →A");
        }
        Core::Gpsp => {
            put("gpsp_frameskip", "auto");
            put("gpsp_serial", serial);
            if bios {
                put("gpsp_boot_mode", "bios");
            }
        }
        Core::Snes9x => put("snes9x_overscan", "enabled"),
        Core::Supafaust => {
            put("supafaust_thread_affinity_emu", "0x0");
            put("supafaust_thread_affinity_ppu", "0x0");
        }
        Core::PcsxRearmed => {
            put("pcsx_rearmed_frameskip_type", "auto");
            put("pcsx_rearmed_bios", "auto");
            put("pcsx_rearmed_memcard1", "libretro");
            put("pcsx_rearmed_memcard2", "none");
        }
        // the H700 runs PSP games at their own 480x272, recompiled, skipping frames it has
        // no time for, on the OpenGL ES context the frontend offers; Cross confirms, as on a PSP
        // sold outside Japan. Names and values as PPSSPP 1.20's core declares them
        Core::Ppsspp => {
            put("ppsspp_backend", "opengl");
            put("ppsspp_cpu_core", "JIT");
            put("ppsspp_internal_resolution", "480x272");
            put("ppsspp_auto_frameskip", "enabled");
            // the GPU queues no frame ahead of the one shown: a frame of input delay the less
            put("ppsspp_inflight_frames", "No buffer");
            put("ppsspp_fast_memory", "enabled");
            put("ppsspp_texture_scaling_level", "disabled");
            put("ppsspp_button_preference", "Cross");
            put("ppsspp_psp_model", "psp_2000_3000");
        }
    }
    if let Some((key, value)) = colour_option(which, colour) {
        put(key, value);
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn each_core_spells_colour_correction_its_own_way() {
        assert_eq!(
            colour_option(Core::Mgba, true),
            Some(("mgba_color_correction", "Auto"))
        );
        assert_eq!(
            colour_option(Core::Mgba, false),
            Some(("mgba_color_correction", "OFF"))
        );
        assert_eq!(
            colour_option(Core::Gpsp, true),
            Some(("gpsp_color_correction", "enabled"))
        );
        assert_eq!(
            colour_option(Core::Gpsp, false),
            Some(("gpsp_color_correction", "disabled"))
        );
    }

    fn keys(which: Core) -> Vec<&'static str> {
        core_options(which, "auto", true, true)
            .into_iter()
            .map(|(k, _)| k)
            .collect()
    }

    #[test]
    fn every_option_is_spelt_with_its_own_core_s_prefix() {
        for which in Core::ALL {
            let prefix = format!("{}_", which.as_str());
            for key in keys(which) {
                assert!(key.starts_with(&prefix), "{key} set on {which:?}");
            }
        }
    }

    #[test]
    fn frame_skipping_is_asked_for_by_each_core_s_own_name_for_it() {
        let skip = |which| {
            core_options(which, "auto", false, false)
                .into_iter()
                .find(|(k, _)| k.contains("frameskip"))
        };
        assert_eq!(skip(Core::Mgba), Some(("mgba_frameskip", "auto".into())));
        assert_eq!(skip(Core::Gpsp), Some(("gpsp_frameskip", "auto".into())));
        assert_eq!(
            skip(Core::PcsxRearmed),
            Some(("pcsx_rearmed_frameskip_type", "auto".into())),
            "pcsx_rearmed's _frameskip is an interval, not a mode"
        );
        assert_eq!(skip(Core::Snes9x), None, "Snes9x has no frame skipping");
        assert_eq!(
            skip(Core::Supafaust),
            None,
            "Supafaust has no frame skipping"
        );
    }

    #[test]
    fn the_consoles_take_no_handheld_options() {
        for which in [Core::Snes9x, Core::Supafaust, Core::PcsxRearmed] {
            assert_eq!(
                core_options(which, "rfu", true, true),
                core_options(which, "auto", false, false),
                "{which:?} took a serial mode, a BIOS boot or colour correction"
            );
        }
        assert!(core_options(Core::PcsxRearmed, "auto", false, false)
            .contains(&("pcsx_rearmed_memcard1", "libretro".into())));
    }

    #[test]
    fn a_missing_console_core_is_stood_in_for_at_its_system_s_size() {
        assert_eq!(stand_in(Core::Mgba).video_size(), (240, 160));
        assert_eq!(stand_in(Core::Snes9x).video_size(), (256, 224));
        assert_eq!(stand_in(Core::Supafaust).video_size(), (256, 224));
        assert_eq!(stand_in(Core::PcsxRearmed).video_size(), (320, 240));
        assert_eq!(stand_in(Core::Ppsspp).video_size(), (480, 272));
    }

    #[test]
    fn ppsspp_reads_its_fonts_from_system_and_the_others_their_bios_from_bios() {
        let root = Path::new("/card");
        assert_eq!(
            system_dir(root, Core::Ppsspp).join("PPSSPP/flash0/font"),
            Path::new("/card/System/PPSSPP/flash0/font")
        );
        assert_eq!(system_dir(root, Core::PcsxRearmed), root::bios_dir(root));
        assert_eq!(system_dir(root, Core::Gpsp), root::bios_dir(root));
    }

    #[test]
    fn ppsspp_draws_with_opengl_at_the_psp_s_own_size() {
        let set = core_options(Core::Ppsspp, "", false, false);
        for want in [
            ("ppsspp_backend", "opengl"),
            ("ppsspp_cpu_core", "JIT"),
            ("ppsspp_internal_resolution", "480x272"),
            ("ppsspp_auto_frameskip", "enabled"),
            ("ppsspp_inflight_frames", "No buffer"),
            ("ppsspp_button_preference", "Cross"),
        ] {
            assert!(
                set.contains(&(want.0, want.1.into())),
                "{want:?} is not set"
            );
        }
        // every key is one PPSSPP 1.20's core declares
        assert!(set.iter().all(|(k, _)| k.starts_with("ppsspp_")));
    }

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn candidates_search_the_named_cores_own_filename_only() {
        let _g = lock();
        std::env::remove_var(CORE_ENV);

        let root = Path::new("/root");
        let mgba = candidates(root, Core::Mgba);
        let gpsp = candidates(root, Core::Gpsp);
        assert_ne!(mgba, gpsp, "the two cores searched the same paths");
        assert!(!mgba.is_empty());
        assert!(!gpsp.is_empty());
        for path in &mgba {
            let name = path.file_name().unwrap().to_str().unwrap();
            assert!(name.starts_with("mgba_libretro"), "{name} is not mGBA's");
        }
        for path in &gpsp {
            let name = path.file_name().unwrap().to_str().unwrap();
            assert!(name.starts_with("gpsp_libretro"), "{name} is not gpSP's");
        }
    }

    #[test]
    fn candidates_search_the_roots_own_system_directory() {
        let _g = lock();
        std::env::remove_var(CORE_ENV);
        let root = Path::new("/some/content/root");
        assert_eq!(
            candidates(root, Core::Gpsp)[0],
            root.join("System").join(dylib_name(Core::Gpsp)),
        );
    }

    #[test]
    fn the_env_override_ignores_which_core_was_asked_for() {
        let _g = lock();
        std::env::set_var(CORE_ENV, "/dev/null/named-core");
        let root = Path::new("/root");
        let mgba = candidates(root, Core::Mgba);
        let gpsp = candidates(root, Core::Gpsp);
        std::env::remove_var(CORE_ENV);
        assert_eq!(mgba, vec![PathBuf::from("/dev/null/named-core")]);
        assert_eq!(
            mgba, gpsp,
            "the override stopped winning for one of the cores"
        );
    }
}
