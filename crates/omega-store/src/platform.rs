use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Platform {
    #[default]
    Gba,
    Gb,
    Gbc,
    Snes,
    Ps1,
    /// The PlayStation Portable, on PPSSPP, which draws on the GPU.
    Psp,
    /// The Nintendo DS, on melonDS: two screens, the lower one touched, shown on one.
    Nds,
}

impl Platform {
    pub const ALL: [Platform; 7] = [
        Platform::Gba,
        Platform::Gb,
        Platform::Gbc,
        Platform::Snes,
        Platform::Ps1,
        Platform::Psp,
        Platform::Nds,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Platform::Gba => "Game Boy Advance",
            Platform::Gb => "Game Boy",
            Platform::Gbc => "Game Boy Color",
            Platform::Snes => "Super Nintendo",
            Platform::Ps1 => "PlayStation",
            Platform::Psp => "PlayStation Portable",
            Platform::Nds => "Nintendo DS",
        }
    }

    pub fn dir_name(self) -> &'static str {
        match self {
            Platform::Gba => "GBA",
            Platform::Gb => "GB",
            Platform::Gbc => "GBC",
            Platform::Snes => "SNES",
            Platform::Ps1 => "PS1",
            Platform::Psp => "PSP",
            Platform::Nds => "NDS",
        }
    }

    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Platform::Gba => &["gba"],
            Platform::Gb | Platform::Gbc => &["gb", "gbc"],
            Platform::Snes => &["sfc", "smc"],
            Platform::Ps1 => &["cue", "chd", "pbp", "m3u"],
            Platform::Psp => &["iso", "cso", "chd", "pbp"],
            Platform::Nds => &["nds"],
        }
    }

    pub fn picture(self) -> (u32, u32) {
        match self {
            Platform::Gba => (240, 160),
            Platform::Gb | Platform::Gbc => (160, 144),
            Platform::Snes => (256, 224),
            Platform::Ps1 => (320, 240),
            Platform::Psp => (480, 272),
            // one screen of the two: the view a DS game is shown in
            Platform::Nds => (256, 192),
        }
    }

    /// Whether the frontend keeps L1/R1 while a game of this system plays, for its picture
    /// size: only the Game Boys, whose games have no shoulder buttons.
    pub fn owns_shoulders(self) -> bool {
        matches!(self, Platform::Gb | Platform::Gbc)
    }

    /// Whether held X/Y are turbo A/B: on the handhelds, whose games have no X or Y.
    pub fn turbo_xy(self) -> bool {
        matches!(self, Platform::Gba | Platform::Gb | Platform::Gbc)
    }

    /// Whether the core's colour correction for a handheld's screen applies.
    pub fn colour_correction(self) -> bool {
        matches!(self, Platform::Gba | Platform::Gb | Platform::Gbc)
    }

    /// Whether held L2 rewinds: not on the PlayStations or the DS, whose states are too big for
    /// the rewind buffer, and whose L2 is the game's (the PlayStation's), the stick's (the
    /// PSP's) or the touch screen's (the DS's).
    pub fn rewinds(self) -> bool {
        !matches!(self, Platform::Ps1 | Platform::Psp | Platform::Nds)
    }

    /// Whether a game runs faster while R2 is held: not a PSP game, which PPSSPP paces on a
    /// thread of its own whatever it is asked.
    pub fn fast_forwards(self) -> bool {
        self != Platform::Psp
    }

    /// Whether held L2 turns the D-pad into the analog stick: the PSP's, which the handheld
    /// has none of.
    pub fn stick_on_l2(self) -> bool {
        self == Platform::Psp
    }

    /// Whether held L2 is a cursor on the touch screen: the DS's, which the handheld has no
    /// touch screen for.
    pub fn touch_on_l2(self) -> bool {
        self == Platform::Nds
    }

    /// Whether a game has two screens to show on the handheld's one: the DS's.
    pub fn two_screens(self) -> bool {
        self == Platform::Nds
    }

    /// Whether L2/R2 belong to the game rather than to rewind and fast-forward: a PlayStation
    /// game's, and a PSP game's, whose L2 turns the D-pad into the stick.
    pub fn game_owns_l2_r2(self) -> bool {
        matches!(self, Platform::Ps1 | Platform::Psp)
    }

    pub fn accepts(self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| {
                self.extensions()
                    .iter()
                    .any(|k| ext.eq_ignore_ascii_case(k))
            })
    }
}
