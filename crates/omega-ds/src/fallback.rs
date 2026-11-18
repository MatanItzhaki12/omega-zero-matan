//! What a game's layouts show for a screen none of them knows: mostly a moment between two
//! screens (a fade, a button pressed in a battle) in which none of the functions watched runs.
//! The last layout stays a moment longer; after that either screen alone while the other is
//! blank, else the top one. After Pokémon Fulldex's pickSceneBw2 (frontend/main.cpp, GPL-3.0),
//! which spreads both screens side by side there, as a 4:3 view would show at half their size.

use crate::scene::{Back, Scene};
use crate::screen::{Screens, BOTTOM, TOP};

/// How long the last known layout stays when nothing known runs, in frames.
const LAST_HOLD: u32 = 45;

/// How long either screen must stay alone before it is shown alone, in frames: a screen fading
/// out first flashes no layout.
const ALONE_HOLD: u32 = 6;

/// The layouts' memory of the screens they knew.
#[derive(Default)]
pub(crate) struct Fallback {
    /// The last scene a known screen had, and the frame it was picked in.
    last: Option<(Scene, u32)>,
    /// The screen shown alone while the other is blank, the one wanted, and since when.
    alone: (Option<usize>, Option<usize>, u32),
}

impl Fallback {
    /// Remembers the scene a known screen was given in `frame`.
    pub(crate) fn known(&mut self, sc: &Scene, frame: u32) {
        self.last = Some((sc.clone(), frame));
    }

    /// The last scene a known screen had.
    pub(crate) fn last(&self) -> Option<&Scene> {
        self.last.as_ref().map(|(sc, _)| sc)
    }

    /// The scene for a screen no layout knows; `in_world` says the overworld ran a moment ago,
    /// whose last layout then stays (a door, a cut scene).
    pub(crate) fn unknown(&mut self, frame: u32, s: &Screens, in_world: bool) -> Scene {
        let last = self.last.as_ref();
        if let Some((sc, at)) = last {
            if frame - at <= LAST_HOLD {
                return sc.clone();
            }
        }
        let (top_blank, bottom_blank) = (s.blank(TOP), s.blank(BOTTOM));
        if top_blank && bottom_blank {
            if let Some((sc, _)) = last {
                return sc.clone();
            }
            let mut sc = Scene::only("one-screen", TOP);
            sc.back = Back::Sample(TOP, 0, 0);
            return sc;
        }
        let alone = match (top_blank, bottom_blank) {
            (true, false) => Some(BOTTOM),
            (false, true) => Some(TOP),
            _ => None,
        };
        if alone != self.alone.1 {
            self.alone.1 = alone;
            self.alone.2 = frame;
        }
        if self.alone.1 != self.alone.0 && frame - self.alone.2 >= ALONE_HOLD {
            self.alone.0 = self.alone.1;
        }
        if let Some(screen) = self.alone.0 {
            return Scene::only("one-screen", screen);
        }
        if let Some((sc, _)) = last {
            if in_world {
                return sc.clone();
            }
        }
        // (the top screen whole, the touch screen a press of L2 away)
        Scene::only("top", TOP)
    }
}
