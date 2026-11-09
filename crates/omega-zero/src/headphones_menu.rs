//! The HEADPHONES menu, for the AirPods or Galaxy Buds connected: the same rows in Settings >
//! Bluetooth (START on them) and on the game menu. Noise control, AirPods' conversational
//! awareness and how much of the room their Adaptive lets in, and each bud's and the case's
//! battery.

use crate::app::App;
use crate::headphones::{Kind, Noise};

/// A row of the HEADPHONES menu.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HpRow {
    Noise,
    Conversation,
    Adaptive,
    /// The buds' batteries.
    Battery,
    /// The case's.
    Case,
}

/// How far a step of Adaptive's level goes.
const ADAPTIVE_STEP: u8 = 10;

impl HpRow {
    pub fn label(self) -> &'static str {
        match self {
            HpRow::Noise => "Noise Control",
            HpRow::Conversation => "Conversation Awareness",
            HpRow::Adaptive => "Adaptive Level",
            HpRow::Battery => "Buds Battery",
            HpRow::Case => "Case Battery",
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            HpRow::Noise => "What the headphones let in",
            HpRow::Conversation => "Talking lowers the sound",
            HpRow::Adaptive => "How much of the room Adaptive lets in",
            HpRow::Battery => "Left and right; + is charging",
            HpRow::Case => "As it last said, the buds out of it",
        }
    }

    /// Left and Right step it: all but the battery, which only says.
    pub fn steps(self) -> bool {
        !matches!(self, HpRow::Battery | HpRow::Case)
    }
}

impl App {
    /// The headphones connected and talking to the handheld, by what they are.
    pub fn headphones_kind(&self) -> Option<Kind> {
        self.phones.phones().map(|p| p.kind)
    }

    /// The rows the headphones connected have.
    pub fn headphones_rows(&self) -> Vec<HpRow> {
        let Some(p) = self.phones.phones() else {
            return Vec::new();
        };
        [
            (!p.noise_modes().is_empty()).then_some(HpRow::Noise),
            p.has_conversation().then_some(HpRow::Conversation),
            p.has_adaptive_level().then_some(HpRow::Adaptive),
            Some(HpRow::Battery),
            Some(HpRow::Case),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// What `row` says now; … for what the headphones have not said yet.
    pub fn headphones_value(&self, row: HpRow) -> String {
        let Some(p) = self.phones.phones() else {
            return "…".to_string();
        };
        let wait = || "…".to_string();
        match row {
            HpRow::Noise => p.noise.map_or_else(wait, |n| n.word(p.kind).to_string()),
            HpRow::Conversation => match p.conversation {
                Some(true) => "ON".to_string(),
                Some(false) => "OFF".to_string(),
                None => wait(),
            },
            HpRow::Adaptive => p.adaptive_level.map_or_else(wait, |l| format!("{l}%")),
            HpRow::Battery => p.buds_text(),
            HpRow::Case => p.case_text(),
        }
    }

    /// Steps `row` on, or back, on the headphones themselves.
    pub(crate) fn change_headphones(&mut self, row: HpRow, right: bool) {
        let Some(p) = self.phones.phones() else {
            return;
        };
        let Some(link) = self.phones.link() else {
            return;
        };
        match row {
            HpRow::Noise => {
                let modes = p.noise_modes();
                let to = match p.noise {
                    Some(n) => n.step(modes, right),
                    None => modes.first().copied().unwrap_or(Noise::Off),
                };
                link.set_noise(to);
            }
            HpRow::Conversation => link.set_conversation(!p.conversation.unwrap_or(false)),
            HpRow::Adaptive => {
                let now = p.adaptive_level.unwrap_or(50);
                let to = match right {
                    true => now.saturating_add(ADAPTIVE_STEP).min(100),
                    false => now.saturating_sub(ADAPTIVE_STEP),
                };
                link.set_adaptive_level(to);
            }
            HpRow::Battery | HpRow::Case => {}
        }
    }

    /// For tests: headphones connected as `phones` says, on a channel that talks to nothing.
    pub fn stand_in_headphones(&mut self, mac: &str, phones: crate::headphones::Phones) {
        self.phones.stand_in(mac, phones);
    }

    /// Headphones gone while their menu is open: the game's menu goes back to MENU, Settings
    /// back to Bluetooth, rather than showing an empty page.
    pub(crate) fn leave_gone_headphones(&mut self) {
        if self.headphones_kind().is_some() {
            return;
        }
        if let Some(menu) = &mut self.pause_menu {
            if menu.page == crate::pause_menu::PausePage::Headphones {
                menu.page = crate::pause_menu::PausePage::Main;
                menu.row = crate::pause_menu::PauseRow::Resume;
            }
        }
        if let crate::app::Phase::Options(screen) = &self.phase {
            if screen.page == crate::options::OptionsPage::Headphones {
                self.go(
                    crate::app::Phase::Bluetooth(Default::default()),
                    crate::nav::Dir::Back,
                );
            }
        }
    }

    /// Says the mode the headphones were switched to on themselves.
    pub(crate) fn tell_headphones_noise(&mut self) {
        if let Some((mode, kind)) = self.phones.noise_switched() {
            self.hud
                .toast(omega_ui::Toast::Headphones(mode.told(kind)), self.now());
        }
    }

    /// Says when the headphones' buds run low, once a step of `headphones::LOW`.
    pub(crate) fn tell_low_headphones(&mut self) {
        if let Some(step) = self.phones.low_battery() {
            let text = crate::headphones::low_text(step);
            self.hud
                .toast(omega_ui::Toast::Headphones(text), self.now());
        }
    }
}
