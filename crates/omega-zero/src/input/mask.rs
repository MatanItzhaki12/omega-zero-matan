use omega_input::{Action, Btn, Keys};
use omega_retro::ButtonMask;

const DPAD: u16 = ButtonMask::UP | ButtonMask::DOWN | ButtonMask::LEFT | ButtonMask::RIGHT;

/// How L2 brings up what the handheld has none of: on the PSP the D-pad as its analog stick,
/// on the DS a cursor on its touch screen.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Stick {
    /// The D-pad is the D-pad.
    #[default]
    Off,
    /// While L2 is held, the D-pad is the stick.
    HoldL2,
    /// Each press of L2 switches the D-pad between itself and the stick.
    ToggleL2,
}

impl Stick {
    pub fn as_str(self) -> &'static str {
        match self {
            Stick::Off => "off",
            Stick::HoldL2 => "hold",
            Stick::ToggleL2 => "toggle",
        }
    }

    pub fn parse(s: &str) -> Option<Stick> {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Stick::Off),
            "hold" => Some(Stick::HoldL2),
            "toggle" => Some(Stick::ToggleL2),
            _ => None,
        }
    }
}

/// Each game's stick, where it was changed from its system's: `Config/psp_stick.ini`.
pub const STICK_FILE: &str = "Config/psp_stick.ini";

/// How L2 works in the game `stem` of `platform`: as it was set for that game, else
/// `fallback` on a system whose games want a stick or a touch screen, else not at all.
pub fn stick_for(
    root: &std::path::Path,
    stem: &str,
    platform: omega_store::Platform,
    fallback: Stick,
) -> Stick {
    if !platform.stick_on_l2() && !platform.touch_on_l2() {
        return Stick::Off;
    }
    omega_store::ini::value(root, STICK_FILE, stem)
        .as_deref()
        .and_then(Stick::parse)
        .unwrap_or(fallback)
}

pub fn write_stick(root: &std::path::Path, stem: &str, stick: Stick) -> std::io::Result<()> {
    omega_store::ini::write(root, STICK_FILE, stem, stick.as_str())
}

/// How far the stick goes, either way: all the way.
const FULL: i16 = i16::MAX;

/// The buttons held for the game, as the core reads them.
#[derive(Default)]
pub struct Pad {
    mask: u16,
    /// L2 and R2 are the game's, as the PlayStation's are: only then do they reach its pad.
    shoulders: bool,
    stick: Stick,
    /// L2 is held, and, for a stick L2 switches, whether the D-pad is the stick now.
    l2: bool,
    toggled: bool,
    /// L2 brings up a touch cursor rather than a stick: the DS's. The game's pad is then
    /// handed L2 while the cursor is on, the D-pad left in it for the cursor to take.
    touch: bool,
}

impl Pad {
    pub fn apply(&mut self, action: Action) {
        match action {
            Action::GbaDown(Btn::L2) if self.stick != Stick::Off => {
                self.l2 = true;
                self.toggled = !self.toggled;
            }
            Action::GbaUp(Btn::L2) if self.stick != Stick::Off => self.l2 = false,
            Action::GbaDown(b) => {
                if let Some(bit) = self.bit(b) {
                    self.mask |= bit;
                }
            }
            Action::GbaUp(b) => {
                if let Some(bit) = self.bit(b) {
                    self.mask &= !bit;
                }
            }
            _ => {}
        }
    }

    /// What the buttons do for the game being played: where L2 and R2 stop being the game's,
    /// any still held are let go.
    pub fn set_keys(&mut self, keys: Keys) {
        self.shoulders = keys.game_l2_r2;
        if !self.shoulders {
            self.mask &= !(ButtonMask::L2 | ButtonMask::R2);
        }
    }

    /// How the D-pad stands in for the stick from now on: for the game being played.
    pub fn set_stick(&mut self, stick: Stick) {
        if stick != self.stick {
            self.stick = stick;
            self.toggled = false;
        }
    }

    /// Whether L2 is a touch cursor, the DS's, rather than a stick.
    pub fn set_touch(&mut self, touch: bool) {
        self.touch = touch;
    }

    /// Whether the D-pad is the stick now.
    pub fn stick_active(&self) -> bool {
        match self.stick {
            Stick::Off => false,
            Stick::HoldL2 => self.l2,
            Stick::ToggleL2 => self.toggled,
        }
    }

    /// The buttons as the game's pad has them: without the D-pad while it is the stick.
    pub fn mask(&self) -> ButtonMask {
        match (self.stick_active(), self.touch) {
            (true, true) => ButtonMask(self.mask | ButtonMask::L2),
            (true, false) => ButtonMask(self.mask & !DPAD),
            (false, _) => ButtonMask(self.mask),
        }
    }

    /// The left stick, x and y, right and down positive: the D-pad held, while it is the stick.
    pub fn analog(&self) -> (i16, i16) {
        if !self.stick_active() || self.touch {
            return (0, 0);
        }
        let axis = |minus: u16, plus: u16| match (self.mask & minus != 0, self.mask & plus != 0) {
            (true, false) => -FULL,
            (false, true) => FULL,
            _ => 0,
        };
        (
            axis(ButtonMask::LEFT, ButtonMask::RIGHT),
            axis(ButtonMask::UP, ButtonMask::DOWN),
        )
    }

    pub fn clear(&mut self) {
        self.mask = 0;
        self.l2 = false;
    }

    fn bit(&self, btn: Btn) -> Option<u16> {
        Some(match btn {
            Btn::Up => ButtonMask::UP,
            Btn::Down => ButtonMask::DOWN,
            Btn::Left => ButtonMask::LEFT,
            Btn::Right => ButtonMask::RIGHT,
            Btn::A => ButtonMask::A,
            Btn::B => ButtonMask::B,
            Btn::X => ButtonMask::X,
            Btn::Y => ButtonMask::Y,
            Btn::L1 => ButtonMask::L,
            Btn::R1 => ButtonMask::R,
            Btn::L2 if self.shoulders => ButtonMask::L2,
            Btn::R2 if self.shoulders => ButtonMask::R2,
            Btn::Start => ButtonMask::START,
            Btn::Select => ButtonMask::SELECT,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(stick: Stick) -> Pad {
        let mut pad = Pad::default();
        pad.set_stick(stick);
        pad
    }

    #[test]
    fn held_l2_turns_the_dpad_into_the_stick_until_it_is_let_go() {
        let mut p = pad(Stick::HoldL2);
        p.apply(Action::GbaDown(Btn::Right));
        p.apply(Action::GbaDown(Btn::Up));
        assert_eq!(p.analog(), (0, 0));
        assert_ne!(p.mask().0 & ButtonMask::RIGHT, 0, "the D-pad is the D-pad");
        p.apply(Action::GbaDown(Btn::L2));
        assert_eq!(p.analog(), (FULL, -FULL));
        assert_eq!(
            p.mask().0 & DPAD,
            0,
            "the D-pad reached the game as well as the stick"
        );
        assert_eq!(
            p.mask().0 & ButtonMask::L2,
            0,
            "L2 is the stick's, not the game's"
        );
        p.apply(Action::GbaUp(Btn::L2));
        assert_eq!(p.analog(), (0, 0));
        assert_ne!(p.mask().0 & ButtonMask::UP, 0);
    }

    #[test]
    fn a_toggled_stick_stays_until_l2_is_pressed_again() {
        let mut p = pad(Stick::ToggleL2);
        p.apply(Action::GbaDown(Btn::L2));
        p.apply(Action::GbaUp(Btn::L2));
        p.apply(Action::GbaDown(Btn::Left));
        assert_eq!(p.analog(), (-FULL, 0), "let go of, L2 left the stick off");
        p.apply(Action::GbaDown(Btn::L2));
        assert_eq!(p.analog(), (0, 0));
        assert_ne!(p.mask().0 & ButtonMask::LEFT, 0);
    }

    #[test]
    fn a_touch_cursor_hands_the_game_l2_and_keeps_the_dpad_for_the_cursor() {
        let mut p = pad(Stick::HoldL2);
        p.set_touch(true);
        p.apply(Action::GbaDown(Btn::Right));
        assert_eq!(p.mask().0 & ButtonMask::L2, 0, "no cursor until L2");
        p.apply(Action::GbaDown(Btn::L2));
        assert_ne!(p.mask().0 & ButtonMask::L2, 0);
        assert_ne!(
            p.mask().0 & ButtonMask::RIGHT,
            0,
            "the cursor takes the D-pad"
        );
        assert_eq!(p.analog(), (0, 0), "a DS game has no stick");
        p.apply(Action::GbaUp(Btn::L2));
        assert_eq!(p.mask().0 & ButtonMask::L2, 0);
    }

    #[test]
    fn with_no_stick_the_dpad_and_l2_are_as_they_were() {
        let mut p = pad(Stick::Off);
        p.apply(Action::GbaDown(Btn::L2));
        p.apply(Action::GbaDown(Btn::Down));
        assert_eq!(p.analog(), (0, 0));
        assert_ne!(p.mask().0 & ButtonMask::DOWN, 0);
    }
}
