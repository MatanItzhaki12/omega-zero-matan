//! How the two screens are shown on the handheld's one, as the player sets it.

use crate::scene::{Piece, Scene};
use crate::screen::{BOTTOM, H, TOP, W};

/// A DS game's screen layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    /// One view made of both screens, laid out for what the game is doing: for a game with
    /// layouts of its own (Platinum); Focus for any other.
    #[default]
    OneScreen,
    /// One screen at full size, the top one but while only the touch screen has anything on it.
    Focus,
    /// One screen at full size, the other small in its lower right corner.
    Picture,
    /// Both, the top one above the touch screen.
    Stacked,
    /// Both, beside each other.
    SideBySide,
}

impl Layout {
    /// Every layout, in the order the menus step through them.
    pub const ALL: [Layout; 5] = [
        Layout::OneScreen,
        Layout::Focus,
        Layout::Picture,
        Layout::Stacked,
        Layout::SideBySide,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Layout::OneScreen => "one-screen",
            Layout::Focus => "focus",
            Layout::Picture => "picture",
            Layout::Stacked => "stacked",
            Layout::SideBySide => "side-by-side",
        }
    }

    pub fn parse(s: &str) -> Option<Layout> {
        Layout::ALL
            .into_iter()
            .find(|l| l.as_str().eq_ignore_ascii_case(s.trim()))
    }

    /// As a menu prints it.
    pub fn word(self) -> &'static str {
        match self {
            Layout::OneScreen => "ONE SCREEN",
            Layout::Focus => "FOCUS",
            Layout::Picture => "PICTURE IN PICTURE",
            Layout::Stacked => "STACKED",
            Layout::SideBySide => "SIDE BY SIDE",
        }
    }

    /// The layout after this one, or before it.
    pub fn step(self, right: bool) -> Layout {
        let n = Layout::ALL.len();
        let i = Layout::ALL.iter().position(|l| *l == self).unwrap_or(0);
        Layout::ALL[if right { (i + 1) % n } else { (i + n - 1) % n }]
    }
}

/// How big the small screen of Picture in Picture is, as a share of the view.
pub const PICTURE_SHARE: f32 = 0.34;

/// Picture in Picture: `big` filling the view, the other screen small in its lower right corner.
pub fn picture(big: usize) -> Scene {
    let small = if big == TOP { BOTTOM } else { TOP };
    let (w, h) = (W as f32 * PICTURE_SHARE, H as f32 * PICTURE_SHARE);
    Scene {
        name: "picture",
        pieces: vec![
            Piece::full(big),
            Piece::full(small)
                .scaled(PICTURE_SHARE)
                .at(W as f32 - w - 3.0, H as f32 - h - 3.0)
                .bordered(),
        ],
        back: crate::scene::Back::Colour(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_layout_is_kept_by_a_name_that_brings_it_back() {
        for l in Layout::ALL {
            assert_eq!(Layout::parse(l.as_str()), Some(l));
        }
        assert_eq!(Layout::parse("Stacked "), Some(Layout::Stacked));
        assert_eq!(Layout::parse("upside-down"), None);
        assert_eq!(Layout::default(), Layout::OneScreen);
    }

    #[test]
    fn the_layouts_step_round_both_ways() {
        assert_eq!(Layout::SideBySide.step(true), Layout::OneScreen);
        assert_eq!(Layout::OneScreen.step(false), Layout::SideBySide);
        assert_eq!(Layout::Focus.step(true), Layout::Picture);
    }
}
