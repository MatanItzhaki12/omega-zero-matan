//! Settings, opened with MENU on the shelf: a modal holding CONFIGS, whose values left and
//! right change in place, and GENERAL, whose rows open a screen of their own.

use omega_input::{Action, Btn};
use omega_store::FF_SPEEDS;
use omega_ui::{date_time_text_in, Draw, QuickValue, RowShown, SettingsFaces, SettingsMenu, Toast};

use crate::options::OptionRow;

pub use omega_ui::{SettingsGroup, SettingsRow};

use crate::app::{App, Phase};
use crate::background_screen::BackgroundScreen;
use crate::bluetooth_screen::{BluetoothScreen, Summary as BluetoothSummary};
use crate::label_screen::LabelsScreen;
use crate::nav::Dir;
use crate::options::{OptionsPage, OptionsScreen};
use crate::screens::clock::clock_screen;
use crate::wifi_screen::{Summary, WifiScreen};

impl App {
    /// The lit row, while Settings is open.
    pub fn settings_row(&self) -> Option<SettingsRow> {
        match self.phase {
            Phase::Settings { row } => Some(row),
            _ => None,
        }
    }

    /// What a config row is set to; a row that opens a screen shows none of these.
    pub fn settings_value(&self, row: SettingsRow) -> Option<QuickValue> {
        match row {
            SettingsRow::ColourCorrection => Some(QuickValue::flag(self.state.colour_correction)),
            SettingsRow::ScreenFilter => Some(QuickValue::filter(self.state.screen_filter)),
            SettingsRow::Gameplay
            | SettingsRow::Shelf
            | SettingsRow::Labels
            | SettingsRow::Achievements
            | SettingsRow::Backup
            | SettingsRow::DateTime
            | SettingsRow::Network
            | SettingsRow::Bluetooth
            | SettingsRow::Background
            | SettingsRow::Update
            | SettingsRow::About => None,
        }
    }

    /// What a row says on its right: a config's value, the date and time, the network joined
    /// or the Bluetooth device connected (in capitals, as flow.html prints them), else ON, or
    /// OFF with the radio off.
    pub fn settings_text(&self, row: SettingsRow) -> Option<String> {
        match row {
            SettingsRow::DateTime => {
                Some(date_time_text_in(self.wall_secs(), self.state.clock_24h))
            }
            SettingsRow::Network => Some(match self.wifi_summary() {
                Summary::Off => QuickValue::Off.text().to_string(),
                Summary::On => QuickValue::On.text().to_string(),
                Summary::Joined(name) => name.to_uppercase(),
            }),
            SettingsRow::Bluetooth => Some(match self.bluetooth_summary() {
                BluetoothSummary::Off => QuickValue::Off.text().to_string(),
                BluetoothSummary::On => QuickValue::On.text().to_string(),
                BluetoothSummary::Connected(name) => name.to_uppercase(),
            }),
            SettingsRow::Update => Some(self.updater.state().text()),
            _ => self.settings_value(row).map(|v| v.text().to_string()),
        }
    }

    /// Every row Settings shows, as it shows it now: what each says and which is lit. Its
    /// faces are baked from this.
    pub fn settings_shown(&self) -> Vec<RowShown> {
        let lit = self.settings_row();
        self.settings_rows()
            .into_iter()
            .map(|row| RowShown {
                row,
                value: self.settings_text(row),
                lit: lit == Some(row),
            })
            .collect()
    }

    pub fn set_settings_faces(&mut self, faces: SettingsFaces) {
        self.settings_faces = Some(faces);
    }

    pub fn settings_faces(&self) -> Option<&SettingsFaces> {
        self.settings_faces.as_ref()
    }

    pub(crate) fn open_settings(&mut self) {
        self.go(
            Phase::Settings {
                row: SettingsRow::ALL[0],
            },
            Dir::Aside,
        );
    }

    /// The rows Settings lists on this handheld: Network only where there is a Wi-Fi radio,
    /// Bluetooth only where there is Bluetooth. Update is always there, the modal reaching up
    /// for it (`settings_rect`), unless a screen too short for it leaves GENERAL no room.
    pub fn settings_rows(&self) -> Vec<SettingsRow> {
        let mut rows: Vec<SettingsRow> = SettingsRow::ALL
            .into_iter()
            .filter(|r| match r {
                SettingsRow::Network => self.wifi.present(),
                SettingsRow::Bluetooth => self.bluetooth.present(),
                _ => true,
            })
            .collect();
        let general = rows
            .iter()
            .filter(|r| r.group() == SettingsGroup::General)
            .count() as u32;
        if general > omega_ui::general_room(omega_ui::configs_in(&rows)) {
            rows.retain(|r| *r != SettingsRow::Update);
        }
        rows
    }

    pub(crate) fn settings_input(&mut self, row: SettingsRow, action: Action) {
        let rows = self.settings_rows();
        let row = match action {
            Action::GbaDown(Btn::Up) => row.up_in(&rows),
            Action::GbaDown(Btn::Down) => row.down_in(&rows),
            Action::GbaDown(Btn::Left) => return self.change_setting(row, false),
            Action::GbaDown(Btn::Right) => return self.change_setting(row, true),
            Action::GbaDown(Btn::A) if row == SettingsRow::Update => return self.press_update(),
            Action::GbaDown(Btn::A) => return self.open_settings_row(row),
            Action::GbaDown(Btn::B | Btn::Start) | Action::QuickMenu => {
                // what Update said goes with Settings: the version shows again when it opens
                self.updater.settle();
                return self.go(Phase::Shelf, Dir::FromAside);
            }
            _ => return,
        };
        self.phase = Phase::Settings { row };
    }

    fn open_settings_row(&mut self, row: SettingsRow) {
        let to = match row {
            SettingsRow::Gameplay => Phase::Options(OptionsScreen::new(OptionsPage::Gameplay)),
            SettingsRow::Shelf => Phase::Options(OptionsScreen::new(OptionsPage::Shelf)),
            SettingsRow::Labels => Phase::Labels(LabelsScreen::default()),
            SettingsRow::Achievements => {
                Phase::Options(OptionsScreen::new(OptionsPage::Achievements))
            }
            SettingsRow::Backup => return self.open_saves(),
            SettingsRow::DateTime => clock_screen(self.utc_secs(), self.state.utc_offset_min, true),
            SettingsRow::Network => Phase::Network(WifiScreen::default()),
            SettingsRow::Bluetooth => Phase::Bluetooth(BluetoothScreen::default()),
            SettingsRow::Background => Phase::Background(BackgroundScreen::default()),
            SettingsRow::About => Phase::About,
            SettingsRow::ColourCorrection | SettingsRow::ScreenFilter | SettingsRow::Update => {
                return
            }
        };
        self.go(to, Dir::Deeper);
    }

    pub(crate) fn change_setting(&mut self, row: SettingsRow, right: bool) {
        if row == SettingsRow::Update {
            return self.switch_update_channel();
        }
        let s = &mut self.state;
        match row {
            SettingsRow::ColourCorrection => {
                s.colour_correction = !s.colour_correction;
                self.colour_pending = Some(s.colour_correction);
            }
            SettingsRow::ScreenFilter => s.screen_filter = s.screen_filter.step(right),
            SettingsRow::Gameplay
            | SettingsRow::Shelf
            | SettingsRow::Labels
            | SettingsRow::Achievements
            | SettingsRow::Backup
            | SettingsRow::DateTime
            | SettingsRow::Network
            | SettingsRow::Bluetooth
            | SettingsRow::Background
            | SettingsRow::Update
            | SettingsRow::About => return,
        }
        self.persist();
    }

    /// Left or right on Update: the other channel, the releases or the test builds. What the one
    /// before had found is forgotten, and the other's newest looked for quietly: it shows as
    /// INSTALL, as any update found does, and nothing shows when there is none.
    fn switch_update_channel(&mut self) {
        if self.updater.state().busy() {
            return self.refuse();
        }
        self.state.test_updates = !self.state.test_updates;
        self.persist();
        let channel = self.update_channel();
        crate::log!("update: switched to {channel:?}");
        self.updater.forget();
        // off any network, the check as Wi-Fi joins one looks on the channel picked
        match self.online() {
            true => self.updater.check_quietly(self.root.clone(), channel),
            false => self.update_checked = false,
        }
        self.update_told = false;
        let said = match channel {
            crate::update::Channel::Test => "Updates from test builds",
            crate::update::Channel::Stable => "Updates from releases",
        };
        let now = self.now();
        self.hud.toast(Toast::Update(said), now);
    }

    /// A on Update: checks for a newer release, installs the one found, or restarts onto the
    /// one installed; while it is checking or downloading, A is refused.
    fn press_update(&mut self) {
        use crate::update::Update;
        match self.updater.state() {
            Update::Idle | Update::Latest | Update::Failed(_) => {
                self.updater.check(self.root.clone(), self.update_channel())
            }
            // the card's System folder: the running program's own path is the loader it starts
            // through
            Update::Available { .. } => match &self.root {
                Some(root) => self.updater.install(root.clone()),
                None => self.refuse(),
            },
            Update::Installed(_) => self.begin_restart(),
            Update::Checking | Update::Downloading(_) => self.refuse(),
        }
    }

    /// Steps Fast Forward's speed, which GAMEPLAY and the in-game menu both hold.
    pub(crate) fn step_ff_speed(&mut self, right: bool) {
        self.change_option(OptionRow::FastForward, right);
    }

    /// The modal with `row` lit, over the backdrop, which shows round it.
    pub(crate) fn draw_settings(&self, row: SettingsRow, out: &mut Vec<Draw>) {
        SettingsMenu {
            row,
            rows: &self.settings_rows(),
            faces: self.settings_faces.as_ref(),
        }
        .draw(out)
    }
}

pub(crate) fn ff_next(from: u8, right: bool) -> u8 {
    let at = FF_SPEEDS.iter().position(|&v| v == from).unwrap_or(0);
    // OFF stands below 2X, as on Slot
    let to = if right { at + 1 } else { at.saturating_sub(1) };
    FF_SPEEDS[to.min(FF_SPEEDS.len() - 1)]
}
