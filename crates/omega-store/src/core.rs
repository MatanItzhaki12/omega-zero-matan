use std::collections::HashMap;
use std::path::Path;

use crate::Platform;

pub const SELECTED_CORE_FILE: &str = "Config/selected_core.ini";

/// Each system's default core, where Settings changed it: `GBA = gpsp`. A cart with no core of
/// its own in `SELECTED_CORE_FILE` runs on its system's.
pub const DEFAULT_CORE_FILE: &str = "Config/default_core.ini";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Core {
    #[default]
    Mgba,
    Gpsp,
    Snes9x,
    Supafaust,
    PcsxRearmed,
    Ppsspp,
    MelonDs,
}

impl Core {
    pub const ALL: [Core; 7] = [
        Core::Mgba,
        Core::Gpsp,
        Core::Snes9x,
        Core::Supafaust,
        Core::PcsxRearmed,
        Core::Ppsspp,
        Core::MelonDs,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Core::Mgba => "mgba",
            Core::Gpsp => "gpsp",
            Core::Snes9x => "snes9x",
            Core::Supafaust => "supafaust",
            Core::PcsxRearmed => "pcsx_rearmed",
            Core::Ppsspp => "ppsspp",
            Core::MelonDs => "melonds",
        }
    }

    /// The libretro name its library is built under, `<name>_libretro.so`.
    pub fn library(&self) -> &'static str {
        match self {
            Core::Supafaust => "mednafen_supafaust",
            other => other.as_str(),
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// The core's name as its chip in the core picker wears it, in its own case: as long as the
    /// chip has room for, six letters at most, so Supafaust is Faust.
    pub fn text(self) -> &'static str {
        match self {
            Core::Mgba => "mGBA",
            Core::Gpsp => "gpSP",
            Core::Snes9x => "Snes9x",
            Core::Supafaust => "Faust",
            Core::PcsxRearmed => "PCSX",
            Core::Ppsspp => "PPSSPP",
            Core::MelonDs => "melon",
        }
    }

    /// The core's whole name, in its own case, as About credits it.
    pub fn name(self) -> &'static str {
        match self {
            Core::Mgba => "mGBA",
            Core::Gpsp => "gpSP",
            Core::Snes9x => "Snes9x",
            Core::Supafaust => "Supafaust",
            Core::PcsxRearmed => "PCSX-ReARMed",
            Core::Ppsspp => "PPSSPP",
            Core::MelonDs => "melonDS",
        }
    }

    pub fn parse(s: &str) -> Option<Core> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mgba" => Some(Core::Mgba),
            "gpsp" => Some(Core::Gpsp),
            "snes9x" => Some(Core::Snes9x),
            "supafaust" => Some(Core::Supafaust),
            "pcsx_rearmed" => Some(Core::PcsxRearmed),
            "ppsspp" => Some(Core::Ppsspp),
            "melonds" => Some(Core::MelonDs),
            _ => None,
        }
    }

    pub fn runs(self, platform: Platform) -> bool {
        Core::for_platform(platform).contains(&self)
    }

    /// The cores that run `platform`, the default first, in the order the core picker's
    /// sockets show them.
    pub fn for_platform(platform: Platform) -> &'static [Core] {
        match platform {
            Platform::Gba => &[Core::Mgba, Core::Gpsp],
            Platform::Gb | Platform::Gbc => &[Core::Mgba],
            Platform::Snes => &[Core::Snes9x, Core::Supafaust],
            Platform::Ps1 => &[Core::PcsxRearmed],
            Platform::Psp => &[Core::Ppsspp],
            Platform::Nds => &[Core::MelonDs],
        }
    }

    pub fn default_for(platform: Platform) -> Core {
        Core::for_platform(platform)[0]
    }
}

pub fn read_selected_cores(root: &Path) -> HashMap<String, Core> {
    crate::ini::read(root, SELECTED_CORE_FILE)
        .into_iter()
        .filter_map(|(stem, name)| Core::parse(&name).map(|core| (stem, core)))
        .collect()
}

pub fn core_for(root: &Path, stem: &str) -> Core {
    crate::ini::value(root, SELECTED_CORE_FILE, stem)
        .as_deref()
        .and_then(Core::parse)
        .unwrap_or_default()
}

pub fn core_for_platform(root: &Path, stem: &str, platform: Platform) -> Core {
    switched_core(root, stem, platform).unwrap_or_else(|| default_core(root, platform))
}

/// The core the cart `stem` was switched to by hand, if it was, and to one that runs it.
pub fn switched_core(root: &Path, stem: &str, platform: Platform) -> Option<Core> {
    crate::ini::value(root, SELECTED_CORE_FILE, stem)
        .as_deref()
        .and_then(Core::parse)
        .filter(|core| core.runs(platform))
}

pub fn write_selected_core(root: &Path, stem: &str, core: Core) -> std::io::Result<()> {
    crate::ini::write(root, SELECTED_CORE_FILE, stem, core.as_str())
}

/// The cart `stem` runs on its system's default core again.
pub fn clear_selected_core(root: &Path, stem: &str) -> std::io::Result<()> {
    crate::ini::remove(root, SELECTED_CORE_FILE, stem)
}

/// `platform`'s default core: the one Settings set, else the first it lists.
pub fn default_core(root: &Path, platform: Platform) -> Core {
    crate::ini::value(root, DEFAULT_CORE_FILE, platform.dir_name())
        .as_deref()
        .and_then(Core::parse)
        .filter(|core| core.runs(platform))
        .unwrap_or_else(|| Core::default_for(platform))
}

pub fn write_default_core(root: &Path, platform: Platform, core: Core) -> std::io::Result<()> {
    crate::ini::write(root, DEFAULT_CORE_FILE, platform.dir_name(), core.as_str())
}
