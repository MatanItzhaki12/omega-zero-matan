//! What the HUD says just happened, each under the title of what it is about, drawn as the
//! kit's toast (kit::toast).

use crate::kit::palette::Palette;
use crate::kit::toast::toast_face;
use crate::plate::UndoFace;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Toast {
    StateSaved,
    StateLoaded,
    NeedsGpsp,
    NoLink,
    LinkEnded,
    PeerEnded,
    BiosMismatch,
    ColourOn,
    ColourOff,
    /// SELECT + MENU on a PlayStation game on one disc: there is no other to swap in.
    OneDisc,
    /// The DISC screen swapped disc `n`, counting from 1, into the drive.
    Disc(u8),
    /// SELECT+A or SELECT+B in a game moved to this screen filter.
    Filter(omega_store::ScreenFilter),
    /// Y on the shelf starred the cart, or took its star off.
    Favorited,
    Unfavorited,
    /// The in-game menu's Undo Save took the state just saved back, or Undo Load the load.
    SaveUndone,
    LoadUndone,
    /// RetroAchievements: an achievement earned, by its title.
    Achievement(&'static str),
    /// RetroAchievements: signing in, a game's progress, a mastery.
    Cheevos(&'static str),
    /// The in-game menu's Restart started the game over.
    Restarted,
    /// The emulator a game or a system runs on: set back to the default, or how to.
    Emulator(&'static str),
    /// The switcher's X, A to confirm, deleted the save it showed.
    StateDeleted,
    /// Save Backup: saves copied to the NAS, a copy that failed, or one brought back.
    Backup(&'static str),
    /// The headphones connected: their buds running low.
    Headphones(&'static str),
    /// A newer omega-zero to install.
    Update(&'static str),
    /// SELECT+X in a game kept its screen in Screenshots.
    Screenshot,
    /// X on Recents took the game off it.
    RecentRemoved,
    /// Settings › Saves backed a save up, put one back, or deleted one or a state.
    Saves(&'static str),
}

/// What the toast says once disc `n` is in, for every disc a PlayStation game can have.
const DISC_IN: [&str; 9] = [
    "Disc 1 inserted",
    "Disc 2 inserted",
    "Disc 3 inserted",
    "Disc 4 inserted",
    "Disc 5 inserted",
    "Disc 6 inserted",
    "Disc 7 inserted",
    "Disc 8 inserted",
    "Disc 9 inserted",
];

impl Toast {
    /// Every toast there is, a disc's once, as disc 1.
    pub const ALL: [Toast; 27] = [
        Toast::StateSaved,
        Toast::StateLoaded,
        Toast::NeedsGpsp,
        Toast::NoLink,
        Toast::LinkEnded,
        Toast::PeerEnded,
        Toast::BiosMismatch,
        Toast::ColourOn,
        Toast::ColourOff,
        Toast::OneDisc,
        Toast::Disc(1),
        Toast::Filter(omega_store::ScreenFilter::Lcd),
        Toast::Favorited,
        Toast::Unfavorited,
        Toast::SaveUndone,
        Toast::LoadUndone,
        Toast::Achievement(""),
        Toast::Cheevos(""),
        Toast::Restarted,
        Toast::Emulator(""),
        Toast::StateDeleted,
        Toast::Backup(""),
        Toast::Headphones(""),
        Toast::Update(""),
        Toast::Screenshot,
        Toast::RecentRemoved,
        Toast::Saves(""),
    ];

    /// Its place in `ALL`: every disc's toast is the one place.
    pub fn index(self) -> usize {
        match self {
            Toast::StateSaved => 0,
            Toast::StateLoaded => 1,
            Toast::NeedsGpsp => 2,
            Toast::NoLink => 3,
            Toast::LinkEnded => 4,
            Toast::PeerEnded => 5,
            Toast::BiosMismatch => 6,
            Toast::ColourOn => 7,
            Toast::ColourOff => 8,
            Toast::OneDisc => 9,
            Toast::Disc(_) => 10,
            Toast::Filter(_) => 11,
            Toast::Favorited => 12,
            Toast::Unfavorited => 13,
            Toast::SaveUndone => 14,
            Toast::LoadUndone => 15,
            Toast::Achievement(_) => 16,
            Toast::Cheevos(_) => 17,
            Toast::Restarted => 18,
            Toast::Emulator(_) => 19,
            Toast::StateDeleted => 20,
            Toast::Backup(_) => 21,
            Toast::Headphones(_) => 22,
            Toast::Update(_) => 23,
            Toast::Screenshot => 24,
            Toast::RecentRemoved => 25,
            Toast::Saves(_) => 26,
        }
    }

    /// What it is about, in the toast's title tab.
    pub fn label(self) -> &'static str {
        match self {
            Toast::StateSaved
            | Toast::StateLoaded
            | Toast::SaveUndone
            | Toast::LoadUndone
            | Toast::StateDeleted => "SAVE STATE",
            Toast::NeedsGpsp
            | Toast::NoLink
            | Toast::LinkEnded
            | Toast::PeerEnded
            | Toast::BiosMismatch => "LINK",
            Toast::ColourOn | Toast::ColourOff => "COLOUR",
            Toast::OneDisc | Toast::Disc(_) => "DISC",
            Toast::Filter(_) => "FILTER",
            Toast::Favorited | Toast::Unfavorited => "FAVORITES",
            Toast::Achievement(_) => "ACHIEVEMENT",
            Toast::Cheevos(_) => "RETROACHIEVEMENTS",
            Toast::Restarted => "GAME",
            Toast::Emulator(_) => "EMULATOR",
            Toast::Backup(_) => "SAVE BACKUP",
            Toast::Headphones(_) => "HEADPHONES",
            Toast::Update(_) => "UPDATE",
            Toast::Screenshot => "SCREENSHOT",
            Toast::RecentRemoved => "RECENTS",
            Toast::Saves(_) => "SAVES",
        }
    }

    /// The line in the toast's well, in its own case.
    pub fn text(self) -> &'static str {
        match self {
            Toast::StateSaved => "State saved",
            Toast::StateLoaded => "State loaded",
            Toast::NeedsGpsp => "Please switch to gpSP",
            Toast::NoLink => "No link support",
            Toast::LinkEnded => "Link ended",
            Toast::PeerEnded => "Link was ended",
            Toast::BiosMismatch => "BIOS does not match",
            Toast::ColourOn => "Correction on",
            Toast::ColourOff => "Correction off",
            Toast::OneDisc => "Only one disc",
            Toast::Disc(n) => DISC_IN
                .get(usize::from(n).wrapping_sub(1))
                .copied()
                .unwrap_or("Disc inserted"),
            Toast::Filter(omega_store::ScreenFilter::Lcd) => "LCD",
            Toast::Filter(omega_store::ScreenFilter::Grid) => "Grid",
            Toast::Filter(omega_store::ScreenFilter::Off) => "Off",
            Toast::Favorited => "Added",
            Toast::Unfavorited => "Removed",
            Toast::SaveUndone => "Save undone",
            Toast::LoadUndone => "Load undone",
            Toast::Achievement(title)
            | Toast::Cheevos(title)
            | Toast::Emulator(title)
            | Toast::Backup(title)
            | Toast::Headphones(title)
            | Toast::Update(title)
            | Toast::Saves(title) => title,
            Toast::Restarted => "Restarted",
            Toast::StateDeleted => "State deleted",
            Toast::Screenshot => "Saved",
            Toast::RecentRemoved => "Removed",
        }
    }

    /// The toast as the kit draws it, in `pal`.
    pub fn face(self, pal: &Palette) -> UndoFace {
        toast_face(self.label(), self.text(), pal)
    }
}
