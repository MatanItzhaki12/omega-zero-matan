//! The THEME screen, opened from Settings: what the shelf is drawn over, shown behind the list
//! as it is changed, and the font the bar's actions write their words in.

use omega_input::{Action, Btn};
use omega_store::{BackdropStyle, Font, BACKDROP_COLOURS};
use omega_ui::backdrop_colour_name;

use crate::list_view::{LegendHint, ListView, RowLabel, RowValue, ViewRow};

/// The shelf's background as it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Backdrop {
    pub style: BackdropStyle,
    pub moving: bool,
    pub colour: u8,
}

impl Backdrop {
    /// What the settings come to on a card that does or does not have a wallpaper: one that
    /// never picked shows the wallpaper if there is one, as omega-zero always did, else the grid.
    pub fn resolve(
        picked: Option<BackdropStyle>,
        moving: bool,
        colour: u8,
        wallpaper: bool,
    ) -> Self {
        let style = match picked {
            Some(BackdropStyle::Wallpaper) | None if wallpaper => BackdropStyle::Wallpaper,
            Some(BackdropStyle::Wallpaper) | None => BackdropStyle::Grid,
            Some(style) => style,
        };
        Backdrop {
            style,
            moving,
            colour: colour.min(BACKDROP_COLOURS.len() as u8 - 1),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sel {
    /// The background's style, on the row named Background.
    Style,
    Motion,
    Colour,
    Font,
}

impl Sel {
    /// The panel's rows, top to bottom.
    pub const ALL: [Sel; 4] = [Sel::Style, Sel::Motion, Sel::Colour, Sel::Font];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundScreen {
    pub sel: Sel,
}

impl Default for BackgroundScreen {
    fn default() -> Self {
        BackgroundScreen { sel: Sel::Style }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Stay,
    Close,
    Changed(Backdrop),
    Font(Font),
}

/// The styles a card offers: the wallpaper only where there is one.
fn styles(wallpaper: bool) -> Vec<BackdropStyle> {
    BackdropStyle::ALL
        .into_iter()
        .filter(|s| *s != BackdropStyle::Wallpaper || wallpaper)
        .collect()
}

fn step<T: PartialEq + Copy>(items: &[T], at: T, forward: bool) -> T {
    let n = items.len();
    let i = items.iter().position(|x| *x == at).unwrap_or(0);
    items[if forward {
        (i + 1) % n
    } else {
        (i + n - 1) % n
    }]
}

impl BackgroundScreen {
    /// Whether a style has a use for a row: a picture has no colour, a plain colour does not
    /// move. A row it has none for stays on the panel, faint, so the panel keeps its size.
    /// Every style has a use for the font, which is the bar's.
    fn applies(look: Backdrop, sel: Sel) -> bool {
        match sel {
            Sel::Style | Sel::Font => true,
            Sel::Motion => look.style.moves(),
            Sel::Colour => look.style.coloured(),
        }
    }

    /// The rows the cursor can stand on for this style.
    fn rows(look: Backdrop) -> Vec<Sel> {
        Sel::ALL
            .into_iter()
            .filter(|sel| Self::applies(look, *sel))
            .collect()
    }

    pub fn input(
        &mut self,
        look: Backdrop,
        font: Font,
        wallpaper: bool,
        action: Action,
    ) -> Outcome {
        let rows = Self::rows(look);
        if !rows.contains(&self.sel) {
            self.sel = Sel::Style;
        }
        let forward = match action {
            Action::GbaDown(Btn::B) | Action::QuickMenu => return Outcome::Close,
            Action::GbaDown(Btn::Up) => {
                self.sel = step(&rows, self.sel, false);
                return Outcome::Stay;
            }
            Action::GbaDown(Btn::Down) => {
                self.sel = step(&rows, self.sel, true);
                return Outcome::Stay;
            }
            Action::GbaDown(Btn::Left) => false,
            Action::GbaDown(Btn::Right | Btn::A) => true,
            _ => return Outcome::Stay,
        };
        let mut next = look;
        match self.sel {
            Sel::Style => next.style = step(&styles(wallpaper), look.style, forward),
            Sel::Motion => next.moving = !look.moving,
            Sel::Colour => {
                let colours: Vec<u8> = (0..BACKDROP_COLOURS.len() as u8).collect();
                next.colour = step(&colours, look.colour, forward);
            }
            Sel::Font => return Outcome::Font(step(&Font::ALL, font, forward)),
        }
        Outcome::Changed(next)
    }

    pub fn view(&self, look: Backdrop, font: Font) -> ListView {
        let at = Sel::ALL.iter().position(|r| *r == self.sel).unwrap_or(0);
        let row = |sel: &Sel| match sel {
            Sel::Style => ViewRow {
                label: RowLabel::Setting("Background"),
                value: Some(RowValue::Choice(look.style.label())),
                dim: false,
            },
            Sel::Motion => ViewRow {
                label: RowLabel::Setting("Motion"),
                value: Some(RowValue::Choice(if look.moving {
                    "Moving"
                } else {
                    "Still"
                })),
                dim: !Self::applies(look, Sel::Motion),
            },
            Sel::Colour => ViewRow {
                label: RowLabel::Setting("Color"),
                value: Some(RowValue::Choice(backdrop_colour_name(look.colour))),
                dim: !Self::applies(look, Sel::Colour),
            },
            Sel::Font => ViewRow {
                label: RowLabel::Setting("Font"),
                value: Some(RowValue::Choice(font.label())),
                dim: false,
            },
        };
        // the panel holds its rows and nothing under them, as flow.html draws it
        ListView {
            rows: Sel::ALL.iter().map(row).collect(),
            lit: Some(at),
            status: Vec::new(),
            legend: vec![LegendHint::Back, LegendHint::Change],
        }
    }
}
