//! What the shelf is drawn over: a pattern in one of a few colours, still or drifting, or a
//! picture of the player's own from Wallpapers/.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BackdropStyle {
    /// Lines crossing, like squared paper.
    Grid,
    /// Raised square plates, lit from the top left like a 16-bit floor.
    Plates,
    /// Dotted lines, a cross where they meet.
    Dotted,
    /// The grid's squares shaded from the top left in an ordered dither.
    Dither,
    /// The plates riveted at each corner.
    Rivets,
    /// Square pixels scattered like a 1-bit picture.
    Bits,
    /// Tufts of tall grass, as a route's, the Trainer style's.
    Grass,
    /// Capture balls, faint, in rows: the Trainer style's own.
    Capture,
    /// The colour alone.
    Solid,
    /// A .png from Wallpapers/ on the card.
    Wallpaper,
}

impl BackdropStyle {
    pub const ALL: [BackdropStyle; 10] = [
        BackdropStyle::Grid,
        BackdropStyle::Plates,
        BackdropStyle::Dotted,
        BackdropStyle::Dither,
        BackdropStyle::Rivets,
        BackdropStyle::Bits,
        BackdropStyle::Grass,
        BackdropStyle::Capture,
        BackdropStyle::Solid,
        BackdropStyle::Wallpaper,
    ];

    pub fn key(self) -> &'static str {
        match self {
            BackdropStyle::Grid => "grid",
            BackdropStyle::Plates => "plates",
            BackdropStyle::Dotted => "dotted",
            BackdropStyle::Dither => "dither",
            BackdropStyle::Rivets => "rivets",
            BackdropStyle::Bits => "bits",
            BackdropStyle::Grass => "grass",
            BackdropStyle::Capture => "capture",
            BackdropStyle::Solid => "solid",
            BackdropStyle::Wallpaper => "wallpaper",
        }
    }

    pub fn from_key(key: &str) -> Option<BackdropStyle> {
        Self::ALL.into_iter().find(|s| s.key() == key)
    }

    pub fn label(self) -> &'static str {
        match self {
            BackdropStyle::Grid => "Grid",
            BackdropStyle::Plates => "Plates",
            BackdropStyle::Dotted => "Dotted",
            BackdropStyle::Dither => "Dither",
            BackdropStyle::Rivets => "Rivets",
            BackdropStyle::Bits => "Bits",
            BackdropStyle::Grass => "Tall Grass",
            BackdropStyle::Capture => "Capture",
            BackdropStyle::Solid => "Solid",
            BackdropStyle::Wallpaper => "Wallpaper",
        }
    }

    /// A pattern that can drift.
    pub fn moves(self) -> bool {
        !matches!(self, BackdropStyle::Solid | BackdropStyle::Wallpaper)
    }

    /// Drawn in one of the palette's colours.
    pub fn coloured(self) -> bool {
        self != BackdropStyle::Wallpaper
    }
}

/// The colours a pattern comes in, by the name omega-zero.state keeps, darkest first; then
/// Trainer, a dark grey whose menus wear a capture ball's red, and one for each of the Game Boy
/// Advance's monster-catching games, a dark grey whose menus wear that game's colour. omega-ui
/// holds what each looks like, in this order.
pub const BACKDROP_COLOURS: [&str; 14] = [
    "night",
    "slate",
    "navy",
    "teal",
    "green",
    "plum",
    "crimson",
    "amber",
    "trainer",
    "ruby",
    "sapphire",
    "emerald",
    "firered",
    "leafgreen",
];

/// How far apart, in degrees round the hue circle, the custom colours stand.
pub const CUSTOM_STEP: u16 = 10;
/// How many custom colours there are: one every [`CUSTOM_STEP`] degrees.
pub const CUSTOM_HUES: u8 = (360 / CUSTOM_STEP) as u8;
/// Every colour, the named ones then the custom: a colour is kept as an index under this. A
/// custom colour comes after the named, the first at hue 0.
pub const COLOUR_COUNT: u8 = BACKDROP_COLOURS.len() as u8 + CUSTOM_HUES;

/// What omega-zero.state keeps a custom colour by, before its hue in degrees.
const CUSTOM_KEY: &str = "custom-";

/// The hue, in degrees, of custom colour `colour`, or nothing for a named one.
pub fn custom_hue(colour: u8) -> Option<u16> {
    let at = colour.checked_sub(BACKDROP_COLOURS.len() as u8)?;
    (at < CUSTOM_HUES).then(|| at as u16 * CUSTOM_STEP)
}

/// The custom colour of hue `hue` degrees, to the nearest step down.
pub fn custom_colour(hue: u16) -> u8 {
    BACKDROP_COLOURS.len() as u8 + ((hue % 360) / CUSTOM_STEP) as u8
}

/// The key omega-zero.state keeps colour `colour` by: a named colour's name, or `custom-` and
/// its hue. Out of range is the last.
pub fn colour_key(colour: u8) -> String {
    let colour = colour.min(COLOUR_COUNT - 1);
    match custom_hue(colour) {
        Some(hue) => format!("{CUSTOM_KEY}{hue}"),
        None => BACKDROP_COLOURS[colour as usize].to_string(),
    }
}

/// The colour omega-zero.state keeps by `key`, named or custom.
pub fn colour_index(key: &str) -> Option<u8> {
    if let Some(hue) = key.strip_prefix(CUSTOM_KEY) {
        let hue: u16 = hue.parse().ok()?;
        return (hue < 360 && hue.is_multiple_of(CUSTOM_STEP)).then(|| custom_colour(hue));
    }
    BACKDROP_COLOURS
        .iter()
        .position(|c| *c == key)
        .map(|i| i as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_colour_is_kept_by_a_key_that_brings_it_back() {
        for colour in 0..COLOUR_COUNT {
            assert_eq!(colour_index(&colour_key(colour)), Some(colour), "{colour}");
        }
        assert_eq!(colour_key(BACKDROP_COLOURS.len() as u8), "custom-0");
        assert_eq!(colour_key(COLOUR_COUNT - 1), "custom-350");
        assert_eq!(
            colour_key(u8::MAX),
            "custom-350",
            "out of range is the last"
        );
    }

    #[test]
    fn a_custom_key_off_the_steps_or_the_circle_is_no_colour() {
        for key in [
            "custom-5",
            "custom-360",
            "custom-",
            "custom-x",
            "custom--10",
        ] {
            assert_eq!(colour_index(key), None, "{key}");
        }
        assert_eq!(custom_hue(0), None, "Night is named");
        assert_eq!(
            custom_colour(370),
            custom_colour(10),
            "a hue past 360 goes round"
        );
    }
}
