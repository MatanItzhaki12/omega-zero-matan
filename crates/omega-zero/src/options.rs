//! The GAMEPLAY and SHELF panels Settings opens from CONFIGS: Slot's settings omega-zero did not
//! have, in a lone panel each, as THEME is. Left and Right change the row the cursor is on, A
//! steps it on, and B or MENU go back to Settings. Each row's note, under the rows, says what it
//! does.
//!
//! GAMEPLAY: Fast Forward's speed, its Sound and Rumble, which stood in CONFIGS, then Load State
//! on Insert, Save State on Eject, Rewind, Turbo Buttons, Frame Blending and the Game Boy's
//! Palette. Its last row, Emulators, opens EMULATORS: the core each system's games run on unless
//! one was switched by hand in the core picker, and a row a system to put those back on it.
//!
//! SHELF: Recents, Game Titles, the Clock's 12 or 24 hours, Lid Power Off, the Boot Animation,
//! the opening's terminal, off unless switched on, and the Menu Sounds and Menu Music.
//!
//! ACHIEVEMENTS, opened from CONFIGS' RetroAchievements: switching them on, the account and
//! Hardcore. Its Account row signs in on the keyboard, as Labels signs in to ScreenScraper: A
//! asks for the user name and then the password, and Y signs out.

use omega_input::{Action, Btn};
use omega_store::{Blending, Core, Platform, LID_OFF_MINUTES, MENU_VOLUME_MAX};
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::row::{panel_height, row_face, row_rect, Row};
use omega_ui::{Draw, Keyboard, QuickValue, Toast, UndoFace};

use crate::app::{App, Phase};
use crate::nav::Dir;
use crate::screens::lone::{notes, notes_height, over, rows_height, Lone, NOTE_GAP, NOTE_X};
use crate::screens::settings::{ff_next, SettingsRow};

/// How wide a panel is, in art pixels: room for Load State on Insert and a palette's name.
pub const PANEL_W: u32 = 268;

/// How many rows a panel shows at once, GAMEPLAY's all: a longer list, as a NAS's folders can
/// be, shows those round the lit one.
pub const VIEW_ROWS: usize = 13;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OptionsPage {
    /// How games are drawn, and the screen's brightness and blue light.
    Display,
    Gameplay,
    Shelf,
    /// RetroAchievements, opened from CONFIGS.
    Achievements,
    /// Each system's default core, opened from GAMEPLAY's last row.
    Emulators,
    /// The name and the relay of online play, opened from GAMEPLAY.
    Online,
    /// Saves, opened from CONFIGS: NAS Backup, Search Games, and each system with games that
    /// have anything on the card.
    Saves,
    /// A system's games with anything on the card, or those a search found.
    SavesGames,
    /// A game: its Save and its Save States.
    GameSaves,
    /// A game's save, Back Up Now, its backups and Delete Save.
    SaveMenu,
    /// A game's save states, the resume point among them.
    StatesMenu,
    /// Save Backup, opened from SAVES' NAS Backup: saves copied to a NAS.
    Backup,
    /// The NASes on this Wi-Fi, opened from SAVE BACKUP's NAS.
    Nas,
    /// The NAS's shares and folders, opened from SAVE BACKUP's Folder.
    Folder,
    /// The saves on the NAS that differ from the handheld's, opened from Sync From Backup.
    Restore,
    /// The headphones connected, opened from Bluetooth with START on them.
    Headphones,
    /// The menu sounds and the shelf's tune, and how loud each is, opened from SHELF.
    Sound,
}

/// The systems with more than one core to run on, so a default to pick.
pub const EMULATOR_SYSTEMS: [Platform; 2] = [Platform::Gba, Platform::Snes];

impl OptionsPage {
    /// One of Settings › Saves' own panels, whose rows A acts on in `saves_settings`.
    pub fn is_saves(self) -> bool {
        matches!(
            self,
            OptionsPage::Saves
                | OptionsPage::SavesGames
                | OptionsPage::GameSaves
                | OptionsPage::SaveMenu
                | OptionsPage::StatesMenu
        )
    }

    pub fn title(self) -> &'static str {
        match self {
            OptionsPage::Display => "DISPLAY",
            OptionsPage::Gameplay => "GAMEPLAY",
            OptionsPage::Shelf => "SHELF",
            OptionsPage::Achievements => "ACHIEVEMENTS",
            OptionsPage::Emulators => "EMULATORS",
            OptionsPage::Online => "ONLINE PLAY",
            OptionsPage::Saves => "SAVES",
            OptionsPage::SavesGames => "GAMES",
            OptionsPage::GameSaves => "GAME",
            OptionsPage::SaveMenu => "SAVE",
            OptionsPage::StatesMenu => "STATES",
            OptionsPage::Backup => "SAVE BACKUP",
            OptionsPage::Nas => "FIND NAS",
            OptionsPage::Folder => "FOLDER",
            OptionsPage::Restore => "FROM BACKUP",
            OptionsPage::Headphones => "HEADPHONES",
            OptionsPage::Sound => "SOUND",
        }
    }

    pub fn rows(self) -> &'static [OptionRow] {
        match self {
            // as the game's menu has them, in the same order
            OptionsPage::Display => &[
                OptionRow::ScreenFilter,
                OptionRow::Palette,
                OptionRow::ColourCorrection,
                OptionRow::Blending,
                OptionRow::Brightness,
                OptionRow::BlueLight,
                OptionRow::PspScreen,
            ],
            OptionsPage::Gameplay => &[
                OptionRow::FastForward,
                OptionRow::FastForwardSound,
                OptionRow::Rewind,
                OptionRow::Turbo,
                OptionRow::RunAhead,
                OptionRow::Rumble,
                OptionRow::LoadOnInsert,
                OptionRow::SaveOnEject,
                OptionRow::OnlinePlay,
                OptionRow::Emulators,
            ],
            OptionsPage::Online => &[OptionRow::OnlineName, OptionRow::Relay],
            OptionsPage::Backup => &[
                OptionRow::BackupOn,
                OptionRow::BackupNas,
                OptionRow::BackupUser,
                OptionRow::BackupPassword,
                OptionRow::BackupFolder,
                OptionRow::BackupNow,
                OptionRow::BackupRestore,
            ],
            // what the card has: `App::page_rows`
            OptionsPage::Saves => &[OptionRow::SavesNas],
            OptionsPage::SavesGames => &[OptionRow::SavesNoMatch],
            OptionsPage::GameSaves => &[OptionRow::GameSaveMenu],
            OptionsPage::SaveMenu => &[OptionRow::GameSave],
            OptionsPage::StatesMenu => &[OptionRow::GameNoStates],
            // what the NAS has: `App::page_rows`
            OptionsPage::Nas => &[OptionRow::NasAddress],
            OptionsPage::Folder => &[OptionRow::FolderUse],
            OptionsPage::Restore => &[OptionRow::RestoreItem(0)],
            // what the headphones have: `App::page_rows`
            OptionsPage::Headphones => &[],
            OptionsPage::Achievements => {
                &[OptionRow::Cheevos, OptionRow::Account, OptionRow::Hardcore]
            }
            OptionsPage::Emulators => &[
                OptionRow::DefaultCore(Platform::Gba),
                OptionRow::Switched(Platform::Gba),
                OptionRow::DefaultCore(Platform::Snes),
                OptionRow::Switched(Platform::Snes),
            ],
            OptionsPage::Shelf => &[
                OptionRow::Recents,
                OptionRow::ShelfSort,
                OptionRow::Titles,
                OptionRow::Clock,
                OptionRow::LidOff,
                OptionRow::BootAnimation,
                OptionRow::Sound,
                OptionRow::Theme,
                OptionRow::Labels,
            ],
            OptionsPage::Sound => &[
                OptionRow::MenuSounds,
                OptionRow::SoundsVolume,
                OptionRow::MenuMusic,
                OptionRow::MusicVolume,
            ],
        }
    }

    /// The Settings row that opens it, or the panel it opens from.
    pub fn settings_row(self) -> SettingsRow {
        match self {
            OptionsPage::Display => SettingsRow::Display,
            OptionsPage::Gameplay | OptionsPage::Emulators | OptionsPage::Online => {
                SettingsRow::Gameplay
            }
            OptionsPage::Shelf | OptionsPage::Sound => SettingsRow::Shelf,
            OptionsPage::Achievements => SettingsRow::Achievements,
            OptionsPage::Saves
            | OptionsPage::SavesGames
            | OptionsPage::GameSaves
            | OptionsPage::SaveMenu
            | OptionsPage::StatesMenu
            | OptionsPage::Backup
            | OptionsPage::Nas
            | OptionsPage::Folder
            | OptionsPage::Restore => SettingsRow::Backup,
            OptionsPage::Headphones => SettingsRow::Bluetooth,
        }
    }

    /// The panel and row B goes back to, for a panel opened from another; none for one
    /// Settings opens.
    pub fn parent(self) -> Option<(OptionsPage, OptionRow)> {
        match self {
            OptionsPage::Emulators => Some((OptionsPage::Gameplay, OptionRow::Emulators)),
            OptionsPage::Sound => Some((OptionsPage::Shelf, OptionRow::Sound)),
            OptionsPage::Online => Some((OptionsPage::Gameplay, OptionRow::OnlinePlay)),
            OptionsPage::SavesGames => Some((OptionsPage::Saves, OptionRow::SavesSearch)),
            OptionsPage::GameSaves => Some((OptionsPage::SavesGames, OptionRow::SavesGame(0))),
            OptionsPage::SaveMenu => Some((OptionsPage::GameSaves, OptionRow::GameSaveMenu)),
            OptionsPage::StatesMenu => Some((OptionsPage::GameSaves, OptionRow::GameStatesMenu)),
            OptionsPage::Backup => Some((OptionsPage::Saves, OptionRow::SavesNas)),
            OptionsPage::Nas => Some((OptionsPage::Backup, OptionRow::BackupNas)),
            OptionsPage::Folder => Some((OptionsPage::Backup, OptionRow::BackupFolder)),
            OptionsPage::Restore => Some((OptionsPage::Backup, OptionRow::BackupRestore)),
            OptionsPage::Display
            | OptionsPage::Gameplay
            | OptionsPage::Shelf
            | OptionsPage::Achievements
            | OptionsPage::Saves
            | OptionsPage::Headphones => None,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OptionRow {
    /// DISPLAY: the screen filter, colour correction, and the screen's brightness and blue light
    /// filter, as the game's menu has them.
    ScreenFilter,
    ColourCorrection,
    Brightness,
    BlueLight,
    /// Fast Forward's speed, which stood in CONFIGS.
    FastForward,
    FastForwardSound,
    Rumble,
    LoadOnInsert,
    SaveOnEject,
    Rewind,
    Turbo,
    RunAhead,
    Blending,
    Palette,
    Recents,
    Titles,
    Clock,
    LidOff,
    BootAnimation,
    /// SHELF: the games by name or by how long each was played.
    ShelfSort,
    /// SAVES: the same, for the games under each system.
    SavesSort,
    /// SHELF's way into the SOUND panel.
    Sound,
    /// SHELF: the THEME and LABELS screens, which stood in Settings.
    Theme,
    Labels,
    MenuSounds,
    SoundsVolume,
    MenuMusic,
    MusicVolume,
    /// GAMEPLAY's way into the EMULATORS panel.
    Emulators,
    /// The core a system's games run on unless switched by hand.
    DefaultCore(Platform),
    /// How many of a system's games were switched to another core by hand: A puts them back.
    Switched(Platform),
    Cheevos,
    /// The RetroAchievements account: A signs in on the keyboard, Y signs out.
    Account,
    Hardcore,
    /// GAMEPLAY's way into the ONLINE PLAY panel.
    OnlinePlay,
    /// The name online rooms show: A types it on the keyboard.
    OnlineName,
    /// The relay an online room is opened through.
    Relay,
    /// How a PSP game is shown unless set for itself: Fit, Actual or Stretch.
    PspScreen,
    /// SAVES' way into SAVE BACKUP, the keyboard for a search, Auto Backup, Back Up All Saves,
    /// a system's games, and the room on the card.
    SavesNas,
    SavesSearch,
    SavesAutoBackup,
    SavesBackUpAll,
    SavesSystem(Platform),
    SavesCardSpace,
    /// A game in GAMES, by its place in the list: A opens GAME. And the row of a list with none.
    SavesGame(u16),
    SavesNoMatch,
    /// GAME's ways into SAVE and STATES.
    GameSaveMenu,
    GameStatesMenu,
    /// SAVE: the save and Back Up Now, then under the Backups heading each backup kept by its
    /// place. X deletes the save or a backup, and Y restores a backup.
    GameSave,
    GameBackUp,
    GameBackupsHeading,
    GameBackup(u16),
    /// STATES: a state by its place, and the row of a game with none. X deletes a state, and Y
    /// makes it the one the game resumes from.
    GameState(u16),
    GameNoStates,
    /// Save Backup, on or off.
    BackupOn,
    /// The NAS: A opens FIND NAS.
    BackupNas,
    /// Who signs in to the NAS, and their password: A types each.
    BackupUser,
    BackupPassword,
    /// The folder saves go to: A opens FOLDER.
    BackupFolder,
    /// Where the copying stands: A copies now.
    BackupNow,
    /// The saves on the NAS that differ from the handheld's: A opens FROM BACKUP.
    BackupRestore,
    /// A save on the NAS that differs from the handheld's, by its place in the list.
    RestoreItem(u16),
    /// A NAS found on the network, by its place in the list.
    NasFound(u8),
    /// A NAS's address, typed on the keyboard.
    NasAddress,
    /// FOLDER: the folder open picked, a folder made in it, and a share or folder in it.
    FolderUse,
    FolderNew,
    FolderItem(u16),
    /// A row of HEADPHONES.
    Hp(crate::headphones_menu::HpRow),
}

impl OptionRow {
    /// A heading over the rows under it, drawn dim and passed over by the cursor.
    pub fn heading(self) -> bool {
        matches!(self, OptionRow::GameBackupsHeading)
    }

    /// A opens the keyboard or a panel on the row rather than stepping it: the row ends in a ›.
    pub fn opens(self) -> bool {
        matches!(
            self,
            OptionRow::Account
                | OptionRow::Emulators
                | OptionRow::Sound
                | OptionRow::Theme
                | OptionRow::Labels
                | OptionRow::OnlinePlay
                | OptionRow::OnlineName
                | OptionRow::BackupNas
                | OptionRow::BackupUser
                | OptionRow::BackupPassword
                | OptionRow::BackupFolder
                | OptionRow::BackupRestore
                | OptionRow::SavesNas
                | OptionRow::SavesSearch
                | OptionRow::SavesSystem(_)
                | OptionRow::SavesGame(_)
                | OptionRow::GameSaveMenu
                | OptionRow::GameStatesMenu
                | OptionRow::NasAddress
                | OptionRow::FolderNew
                | OptionRow::FolderItem(_)
        )
    }

    /// Left and Right step the row's value, which stands between a ◀ and a ▶ while it is lit.
    pub fn carets(self) -> bool {
        !self.opens()
            && !matches!(
                self,
                OptionRow::Switched(_)
                    | OptionRow::BackupNow
                    | OptionRow::NasFound(_)
                    | OptionRow::FolderUse
                    | OptionRow::RestoreItem(_)
                    | OptionRow::SavesNoMatch
                    | OptionRow::SavesBackUpAll
                    | OptionRow::SavesCardSpace
                    | OptionRow::GameSave
                    | OptionRow::GameBackUp
                    | OptionRow::GameBackup(_)
                    | OptionRow::GameState(_)
                    | OptionRow::GameNoStates
                    | OptionRow::GameBackupsHeading
            )
            && !matches!(self, OptionRow::Hp(row) if !row.steps())
    }

    pub fn label(self) -> &'static str {
        match self {
            OptionRow::ScreenFilter => "Screen Filter",
            OptionRow::ColourCorrection => "Colour Correction",
            OptionRow::Brightness => "Brightness",
            OptionRow::BlueLight => "Blue Light Filter",
            OptionRow::FastForward => "Fast Forward",
            OptionRow::FastForwardSound => "Fast Forward Sound",
            OptionRow::Rumble => "Rumble",
            OptionRow::LoadOnInsert => "Load State on Insert",
            OptionRow::SaveOnEject => "Save State on Eject",
            OptionRow::Rewind => "Rewind",
            OptionRow::Turbo => "Turbo Buttons",
            OptionRow::RunAhead => "Run-Ahead",
            OptionRow::Blending => "Frame Blending",
            OptionRow::Palette => "Game Boy Palette",
            OptionRow::Recents => "Recents",
            OptionRow::ShelfSort | OptionRow::SavesSort => "Sort Games",
            OptionRow::Titles => "Game Titles",
            OptionRow::Clock => "Clock",
            OptionRow::LidOff => "Lid Power Off",
            OptionRow::BootAnimation => "Boot Animation",
            OptionRow::Sound => "Sound",
            OptionRow::Theme => "Theme",
            OptionRow::Labels => "Labels",
            OptionRow::MenuSounds => "Menu Sounds",
            OptionRow::SoundsVolume => "Sounds Volume",
            OptionRow::MenuMusic => "Menu Music",
            OptionRow::MusicVolume => "Music Volume",
            OptionRow::Emulators => "Emulators",
            OptionRow::DefaultCore(Platform::Snes) => "SNES Default",
            OptionRow::DefaultCore(_) => "GBA Default",
            OptionRow::Switched(Platform::Snes) => "SNES Switched Games",
            OptionRow::Switched(_) => "GBA Switched Games",
            OptionRow::Cheevos => "Achievements",
            OptionRow::Account => "Account",
            OptionRow::Hardcore => "Hardcore",
            OptionRow::OnlinePlay => "Online Play",
            OptionRow::OnlineName => "Name",
            OptionRow::Relay => "Relay",
            OptionRow::PspScreen => "PSP Screen",
            OptionRow::SavesNas => "NAS Backup",
            OptionRow::SavesSearch => "Search Games",
            OptionRow::SavesAutoBackup => "Auto Backup",
            OptionRow::SavesBackUpAll => "Back Up All Saves",
            OptionRow::SavesCardSpace => "Card Space",
            OptionRow::SavesNoMatch => "No Games Found",
            OptionRow::GameSaveMenu => "Save",
            OptionRow::GameStatesMenu => "Save States",
            OptionRow::GameSave => "Current Save",
            OptionRow::GameBackUp => "Back Up Now",
            OptionRow::GameNoStates => "No Save States",
            OptionRow::GameBackupsHeading => "Backups",
            // a system's, a game's, a backup's or a state's own: `App::option_label`
            OptionRow::SavesSystem(_)
            | OptionRow::SavesGame(_)
            | OptionRow::GameBackup(_)
            | OptionRow::GameState(_) => "",
            OptionRow::BackupOn => "Save Backup",
            OptionRow::BackupNas => "NAS",
            OptionRow::BackupUser => "User",
            OptionRow::BackupPassword => "Password",
            OptionRow::BackupFolder => "Folder",
            OptionRow::BackupNow => "Back Up Now",
            OptionRow::BackupRestore => "Sync From Backup",
            // a NAS's, a folder's or a game's own name: `App::option_label`
            OptionRow::NasFound(_) | OptionRow::FolderItem(_) | OptionRow::RestoreItem(_) => "",
            OptionRow::NasAddress => "Type Its Address",
            OptionRow::FolderUse => "Use This Folder",
            OptionRow::FolderNew => "New Folder",
            OptionRow::Hp(row) => row.label(),
        }
    }

    /// What the row does, in the note under the rows while it is lit.
    pub fn note(self) -> &'static str {
        match self {
            OptionRow::ScreenFilter => "LCD, a grid between pixels or none",
            OptionRow::ColourCorrection => "Colours as the old screens had",
            OptionRow::Brightness => "SELECT+Up and Down do this too",
            OptionRow::BlueLight => "Warms the picture for the dark",
            OptionRow::FastForward => "How fast fast-forward plays",
            OptionRow::FastForwardSound => "Sound while fast-forwarding",
            OptionRow::Rumble => "For games with a rumble pak",
            OptionRow::LoadOnInsert => "A picks up where you left off",
            OptionRow::SaveOnEject => "Ejecting saves where you were",
            OptionRow::Rewind => "Hold L2 to go back",
            OptionRow::Turbo => "Hold X and Y for turbo A and B",
            OptionRow::RunAhead => "A frame sooner on gpSP",
            OptionRow::Blending => "Smooths flicker; ghosting is mGBA's",
            OptionRow::Palette => "Original Game Boy games only",
            OptionRow::Recents => "A shelf of your last 10 games",
            OptionRow::ShelfSort => "By name, or the most played first",
            OptionRow::SavesSort => "Each system's games by name or play time",
            OptionRow::Titles => "The cart's name over the shelf",
            OptionRow::Clock => "12 or 24-hour time",
            OptionRow::LidOff => "A shut lid powers off after this",
            OptionRow::BootAnimation => "The terminal before the shelf",
            OptionRow::Sound => "Menu sounds and music",
            OptionRow::Theme => "The background, its colour and the font",
            OptionRow::Labels => "Cart labels: ScreenScraper and the holo",
            OptionRow::MenuSounds => "Clicks and chimes in the menus",
            OptionRow::SoundsVolume => "How loud the menu sounds are",
            OptionRow::MenuMusic => "A soft tune on the shelf",
            OptionRow::MusicVolume => "How loud the tune is",
            OptionRow::Emulators => "Each system's default core",
            OptionRow::DefaultCore(_) => "Games not switched by hand",
            OptionRow::Switched(_) => "A puts them on the default",
            OptionRow::Cheevos => "Check games for achievements",
            OptionRow::Account => "A signs in, Y signs out",
            OptionRow::Hardcore => "No state loads or rewind, from the next game",
            OptionRow::OnlinePlay => "Your name and relay for online rooms",
            OptionRow::OnlineName => "What the other player sees",
            OptionRow::Relay => "The nearest is the quickest",
            OptionRow::PspScreen => "Fit fills the width; each game can differ",
            OptionRow::SavesNas => "Saves copied to a NAS, and back",
            OptionRow::SavesSearch => "Find a game by its name",
            OptionRow::SavesAutoBackup => "How many automatic backups each game keeps",
            OptionRow::SavesBackUpAll => "Keeps a copy of every save on the handheld",
            OptionRow::SavesCardSpace => "The room left on the card",
            OptionRow::SavesSystem(_) => "Its games' saves and states",
            OptionRow::SavesGame(_) => "Its save and its states",
            OptionRow::SavesNoMatch => "No games here",
            OptionRow::GameSaveMenu => "Its save and backups",
            OptionRow::GameStatesMenu => "Its save states",
            OptionRow::GameSave => "X deletes it",
            OptionRow::GameBackUp => "Keeps a copy of the save on the handheld",
            OptionRow::GameBackup(_) => "Y restores it, X deletes it",
            OptionRow::GameState(_) => "X deletes it",
            OptionRow::GameNoStates => "Saved states and the resume point show here",
            OptionRow::GameBackupsHeading => "Copies of the save, kept on the handheld",
            OptionRow::BackupOn => "Saves copy to a NAS as they're made",
            OptionRow::BackupNas => "A Samba share on this Wi-Fi",
            OptionRow::BackupUser => "None signs in as a guest",
            OptionRow::BackupPassword => "The user's password",
            OptionRow::BackupFolder => "Where saves go on the NAS",
            OptionRow::BackupNow => "A copies every save now",
            OptionRow::BackupRestore => "Saves on the NAS unlike the handheld's",
            OptionRow::RestoreItem(_) => "A copies it to the handheld",
            OptionRow::Hp(row) => row.note(),
            OptionRow::NasFound(_) => "A picks this NAS",
            OptionRow::NasAddress => "Its name or address, as 192.168.1.5",
            OptionRow::FolderUse => "Saves go to the folder open",
            OptionRow::FolderNew => "A names a folder to make here",
            OptionRow::FolderItem(_) => "A opens it, B goes back up",
        }
    }
}

/// What the keyboard asks for while signing in: the user name, then the password.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Step {
    User,
    Password,
    /// The name for online rooms, typed on its own.
    OnlineName,
    /// Saves: what to look for in the games' names.
    SearchGames,
    /// Save Backup: the NAS's user and password, a NAS's address, and a new folder's name.
    BackupUser,
    BackupPassword,
    NasAddress,
    NewFolder,
}

impl Step {
    /// Typing nothing is an answer: no user is a guest, no password is none.
    fn may_be_empty(self) -> bool {
        matches!(self, Step::BackupUser | Step::BackupPassword)
    }
}

/// The keyboard up over the panel, signing in to RetroAchievements.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignIn {
    pub step: Step,
    /// The user name typed, once the password is being asked for.
    pub user: String,
    pub keys: Keyboard,
}

impl SignIn {
    pub fn prompt(&self) -> String {
        match self.step {
            Step::User => "RetroAchievements user name".to_string(),
            Step::OnlineName => "Your name in online rooms".to_string(),
            Step::BackupUser => "The NAS's user, or none for a guest".to_string(),
            Step::BackupPassword => "The NAS user's password".to_string(),
            Step::NasAddress => "The NAS's name or address".to_string(),
            Step::NewFolder => "The new folder's name".to_string(),
            Step::SearchGames => "Part of a game's name".to_string(),
            Step::Password => format!("RetroAchievements password for {}", self.user),
        }
    }
}

/// An options panel while it is open, the row the cursor is on, and the keyboard while it is
/// up for the account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionsScreen {
    pub page: OptionsPage,
    pub row: OptionRow,
    pub typing: Option<SignIn>,
}

impl OptionsScreen {
    pub fn new(page: OptionsPage) -> Self {
        OptionsScreen {
            page,
            row: page.rows().first().copied().unwrap_or(OptionRow::FolderUse),
            typing: None,
        }
    }
}

/// What a panel shows: its title, each row's name and value, the lit row and its note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionsView {
    pub title: &'static str,
    /// Each row in view, its name and its value.
    pub lines: Vec<(OptionRow, String, String)>,
    pub lit: usize,
    /// The lit row's note: what it does, or for RetroAchievements who is signed in.
    pub note: String,
    /// The list goes on past the rows in view: above them, and below.
    pub above: bool,
    pub below: bool,
}

/// How Sort Games says which order it is.
fn sort_word(by_play: bool) -> &'static str {
    if by_play {
        "PLAY TIME"
    } else {
        "NAME"
    }
}

/// A panel in `pal`, every value between carets while its row is lit, and the lit row's note.
pub fn options_panel_face(view: &OptionsView, pal: &Palette) -> UndoFace {
    let held = rows_height(view.lines.len());
    let mut panel = panel_face(
        PANEL_W,
        panel_height(held + notes_height(1)),
        view.title,
        true,
        pal,
    );
    for (i, (row, label, value)) in view.lines.iter().enumerate() {
        let [x, y, w, _] = row_rect(panel.well, i as u32);
        let face = row_face(
            w,
            &Row {
                label,
                value: (!value.is_empty()).then_some(value.as_str()),
                lit: i == view.lit,
                carets: row.carets(),
                opens: row.opens(),
                dim: row.heading(),
                caret_room: false,
            },
            pal,
        );
        over(&mut panel.face, &face, x, y);
    }
    // a list longer than the panel says so at the end of the note's line, clear of the rows: an
    // arrow up for rows above, one down for rows below, in the dim colour the note is in
    let [wx, wy, ww, _] = panel.well;
    let right = wx as i32 + ww as i32;
    let y = wy as i32 + (held + NOTE_GAP) as i32 + 3;
    if view.above {
        scroll_mark(&mut panel.face, right - 20, y, true, pal.dim);
    }
    if view.below {
        scroll_mark(&mut panel.face, right - 11, y, false, pal.dim);
    }
    // the note stops short of the arrows
    let note = match view.above || view.below {
        true => crate::screens::lone::fitted(&view.note, ww.saturating_sub(NOTE_X + 24)),
        false => view.note.clone(),
    };
    notes(&mut panel, &[note], held + NOTE_GAP, pal);
    panel.face
}

/// A small arrow, 7 art pixels wide and 4 tall, from `(x, y)`: pointing up, or down.
fn scroll_mark(face: &mut UndoFace, x: i32, y: i32, up: bool, colour: [u8; 3]) {
    for row in 0..4 {
        let half = if up { row } else { 3 - row };
        omega_ui::kit::pixel::fill(face, [x + 3 - half, y + row, 2 * half + 1, 1], colour);
    }
}

fn on_off(on: bool) -> String {
    if on { "ON" } else { "OFF" }.to_string()
}

/// A menu volume one up or down, held between 1 and the top rather than wrapping round.
fn volume_step(at: u8, right: bool) -> u8 {
    match right {
        true => (at + 1).min(MENU_VOLUME_MAX),
        false => at.saturating_sub(1).max(1),
    }
}

fn step<T: Copy + PartialEq>(items: &[T], at: T, right: bool) -> T {
    let n = items.len();
    let i = items.iter().position(|x| *x == at).unwrap_or(0);
    items[if right { (i + 1) % n } else { (i + n - 1) % n }]
}

/// A relay's name as the panel prints it, by its place in RetroArch's list.
pub fn relay_word(relay: u8) -> &'static str {
    match crate::netplay_lobby::RELAYS.get(relay as usize).copied() {
        Some("nyc") => "NEW YORK",
        Some("madrid") => "MADRID",
        Some("saopaulo") => "SAO PAULO",
        Some("singapore") => "SINGAPORE",
        _ => "MADRID",
    }
}

impl App {
    /// What `row` is set to, as its panel prints it.
    pub fn option_text(&self, row: OptionRow) -> String {
        let s = &self.state;
        match row {
            OptionRow::ScreenFilter => QuickValue::filter(s.screen_filter).text().to_string(),
            OptionRow::ColourCorrection => QuickValue::flag(s.colour_correction).text().to_string(),
            OptionRow::Brightness => s.brightness.to_string(),
            OptionRow::BlueLight => match s.blue_light {
                0 => "OFF".to_string(),
                n => n.to_string(),
            },
            OptionRow::FastForward => QuickValue::speed(s.ff_speed)
                .map(|v| v.text().to_string())
                .unwrap_or_default(),
            OptionRow::FastForwardSound => on_off(s.ff_sound),
            OptionRow::Rumble => on_off(s.rumble),
            OptionRow::LoadOnInsert => on_off(s.auto_load),
            OptionRow::SaveOnEject => on_off(s.eject_save),
            OptionRow::Rewind => on_off(s.rewind),
            OptionRow::Turbo => on_off(s.turbo),
            OptionRow::RunAhead => on_off(s.run_ahead),
            OptionRow::Blending => match s.blending {
                Blending::Off => "OFF",
                Blending::Mix => "MIX",
                Blending::Ghosting => "GHOSTING",
            }
            .to_string(),
            OptionRow::Palette => s.gb_colors.label().to_uppercase(),
            OptionRow::Recents => if s.show_recents { "SHOW" } else { "HIDE" }.to_string(),
            OptionRow::ShelfSort => sort_word(s.shelf_by_play).to_string(),
            OptionRow::SavesSort => sort_word(s.saves_by_play).to_string(),
            OptionRow::Titles => if s.show_titles { "SHOW" } else { "HIDE" }.to_string(),
            OptionRow::Clock => if s.clock_24h { "24-HOUR" } else { "12-HOUR" }.to_string(),
            OptionRow::LidOff => format!("{} MIN", s.lid_off_min),
            OptionRow::BootAnimation => on_off(s.boot_animation),
            OptionRow::Sound | OptionRow::Theme | OptionRow::Labels => String::new(),
            OptionRow::MenuSounds => on_off(s.menu_sounds),
            OptionRow::SoundsVolume => format!("{} / {MENU_VOLUME_MAX}", s.menu_sounds_volume),
            OptionRow::MenuMusic => on_off(s.menu_music),
            OptionRow::MusicVolume => format!("{} / {MENU_VOLUME_MAX}", s.menu_music_volume),
            // a row that opens a panel shows only its ›
            OptionRow::Emulators => String::new(),
            OptionRow::DefaultCore(platform) => self.default_core(platform).name().to_uppercase(),
            OptionRow::Switched(platform) => match self.switched_games(platform) {
                0 => "NONE".to_string(),
                n => n.to_string(),
            },
            OptionRow::Cheevos => on_off(s.cheevos),
            OptionRow::Account => match &self.cheevos.signed_in {
                Some(name) => name.clone(),
                None => "SIGN IN".to_string(),
            },
            OptionRow::Hardcore => on_off(s.cheevos_hardcore),
            OptionRow::OnlinePlay => String::new(),
            OptionRow::OnlineName => self.online_name(),
            OptionRow::Relay => relay_word(s.relay).to_string(),
            OptionRow::PspScreen => self.psp_screen().word().to_string(),
            OptionRow::BackupOn
            | OptionRow::BackupNas
            | OptionRow::BackupUser
            | OptionRow::BackupPassword
            | OptionRow::BackupFolder
            | OptionRow::BackupNow
            | OptionRow::BackupRestore
            | OptionRow::RestoreItem(_)
            | OptionRow::NasFound(_)
            | OptionRow::NasAddress
            | OptionRow::FolderUse
            | OptionRow::FolderNew
            | OptionRow::FolderItem(_) => self.backup_text(row),
            OptionRow::SavesNas
            | OptionRow::SavesSearch
            | OptionRow::SavesAutoBackup
            | OptionRow::SavesBackUpAll
            | OptionRow::SavesCardSpace
            | OptionRow::SavesSystem(_)
            | OptionRow::SavesGame(_)
            | OptionRow::SavesNoMatch
            | OptionRow::GameSaveMenu
            | OptionRow::GameStatesMenu
            | OptionRow::GameSave
            | OptionRow::GameBackUp
            | OptionRow::GameBackup(_)
            | OptionRow::GameState(_)
            | OptionRow::GameNoStates
            | OptionRow::GameBackupsHeading => self.saves_text(row),
            OptionRow::Hp(row) => self.headphones_value(row),
        }
    }

    /// A row's name: its own, or a NAS's or a folder's.
    pub fn option_label(&self, row: OptionRow) -> String {
        self.backup_label(row)
            .or_else(|| self.saves_label(row))
            .unwrap_or_else(|| row.label().to_string())
    }

    /// The rows `page` shows now: its own, or what the NAS has.
    pub fn page_rows(&self, page: OptionsPage) -> Vec<OptionRow> {
        if page == OptionsPage::Headphones {
            return self
                .headphones_rows()
                .into_iter()
                .map(OptionRow::Hp)
                .collect();
        }
        self.backup_rows(page)
            .or_else(|| self.saves_rows(page))
            .unwrap_or_else(|| page.rows().to_vec())
    }

    /// Steps `row` on, or back, and keeps it; a change a game in play can take reaches it.
    pub(crate) fn change_option(&mut self, row: OptionRow, right: bool) {
        if let OptionRow::Hp(row) = row {
            return self.change_headphones(row, right);
        }
        match row {
            OptionRow::ScreenFilter => return self.step_filter(right),
            OptionRow::ColourCorrection => return self.toggle_colour_correction(),
            OptionRow::Brightness => return self.step_light(false, right),
            OptionRow::BlueLight => return self.step_light(true, right),
            _ => {}
        }
        let s = &mut self.state;
        match row {
            OptionRow::ScreenFilter
            | OptionRow::ColourCorrection
            | OptionRow::Brightness
            | OptionRow::BlueLight => return,
            OptionRow::FastForward => {
                let to = ff_next(s.ff_speed, right);
                if to == s.ff_speed {
                    return;
                }
                s.ff_speed = to;
            }
            OptionRow::FastForwardSound => s.ff_sound = !s.ff_sound,
            OptionRow::Rumble => s.rumble = !s.rumble,
            OptionRow::LoadOnInsert => s.auto_load = !s.auto_load,
            OptionRow::SaveOnEject => s.eject_save = !s.eject_save,
            OptionRow::Rewind => s.rewind = !s.rewind,
            OptionRow::Turbo => s.turbo = !s.turbo,
            OptionRow::RunAhead => s.run_ahead = !s.run_ahead,
            OptionRow::Blending => {
                s.blending = step(&Blending::ALL, s.blending, right);
                self.queue_core_options();
            }
            OptionRow::Palette => {
                s.gb_colors = match right {
                    true => s.gb_colors.next(),
                    false => s.gb_colors.prev(),
                };
                self.queue_core_options();
            }
            OptionRow::Recents => {
                s.show_recents = !s.show_recents;
                self.refresh_recents();
            }
            OptionRow::ShelfSort => {
                s.shelf_by_play = !s.shelf_by_play;
                self.sort_shelves();
            }
            OptionRow::SavesSort => {
                s.saves_by_play = !s.saves_by_play;
                self.sort_saved_games();
            }
            OptionRow::Titles => s.show_titles = !s.show_titles,
            OptionRow::Clock => s.clock_24h = !s.clock_24h,
            OptionRow::BootAnimation => s.boot_animation = !s.boot_animation,
            OptionRow::MenuSounds => s.menu_sounds = !s.menu_sounds,
            OptionRow::SoundsVolume => {
                s.menu_sounds_volume = volume_step(s.menu_sounds_volume, right);
            }
            OptionRow::MenuMusic => s.menu_music = !s.menu_music,
            OptionRow::MusicVolume => s.menu_music_volume = volume_step(s.menu_music_volume, right),
            OptionRow::Cheevos => {
                s.cheevos = !s.cheevos;
                match s.cheevos {
                    true => self.start_achievements(),
                    false => self.stop_achievements(),
                }
            }
            // the keyboard signs in, not the arrows, and A opens the panel
            OptionRow::Account
            | OptionRow::Sound
            | OptionRow::Theme
            | OptionRow::Labels
            | OptionRow::Emulators
            | OptionRow::OnlinePlay
            | OptionRow::OnlineName => return,
            OptionRow::BackupOn => return self.change_backup(row),
            OptionRow::PspScreen => {
                let to = self
                    .psp_screen()
                    .step_in(&crate::video_mode::VideoMode::PSP, right);
                self.state.psp_screen = to.as_str().to_string();
            }
            OptionRow::BackupNas
            | OptionRow::BackupUser
            | OptionRow::BackupPassword
            | OptionRow::BackupFolder
            | OptionRow::BackupNow
            | OptionRow::BackupRestore
            | OptionRow::RestoreItem(_)
            | OptionRow::NasFound(_)
            | OptionRow::NasAddress
            | OptionRow::FolderUse
            | OptionRow::FolderNew
            | OptionRow::FolderItem(_)
            | OptionRow::SavesNas
            | OptionRow::SavesSearch
            | OptionRow::SavesBackUpAll
            | OptionRow::SavesCardSpace
            | OptionRow::SavesSystem(_)
            | OptionRow::SavesGame(_)
            | OptionRow::SavesNoMatch
            | OptionRow::GameSaveMenu
            | OptionRow::GameStatesMenu
            | OptionRow::GameSave
            | OptionRow::GameBackUp
            | OptionRow::GameBackup(_)
            | OptionRow::GameState(_)
            | OptionRow::GameNoStates
            | OptionRow::GameBackupsHeading
            | OptionRow::Hp(_) => return,
            OptionRow::SavesAutoBackup => {
                s.auto_backups = step(&omega_store::AUTO_BACKUPS, s.auto_backups, right);
            }
            OptionRow::Relay => {
                let n = crate::netplay_lobby::RELAYS.len() as u8;
                s.relay = match right {
                    true => (s.relay + 1) % n,
                    false => (s.relay + n - 1) % n,
                };
            }
            // kept in default_core.ini, not the state
            OptionRow::DefaultCore(platform) => return self.step_default_core(platform, right),
            // A alone puts them back, not the arrows
            OptionRow::Switched(_) => return,
            OptionRow::Hardcore => {
                s.cheevos_hardcore = !s.cheevos_hardcore;
                self.switch_hardcore();
            }
            OptionRow::LidOff => {
                s.lid_off_min = step(&LID_OFF_MINUTES, s.lid_off_min, right);
                let timeout = self.lid_timeout();
                if let Some(power) = &mut self.power {
                    power.set_timeout(timeout);
                }
            }
        }
        self.persist();
    }

    /// What the open panel shows.
    pub fn options_view(&self) -> Option<OptionsView> {
        let Phase::Options(screen) = &self.phase else {
            return None;
        };
        if screen.typing.is_some() {
            return None;
        }
        let rows = self.page_rows(screen.page);
        let lit = rows.iter().position(|r| *r == screen.row).unwrap_or(0);
        // a long list shows the rows round the lit one
        let first = lit
            .saturating_sub(VIEW_ROWS - 1)
            .min(rows.len().saturating_sub(VIEW_ROWS));
        let row = rows.get(lit).copied().unwrap_or(screen.row);
        Some(OptionsView {
            title: screen.page.title(),
            lines: rows
                .iter()
                .skip(first)
                .take(VIEW_ROWS)
                .map(|r| (*r, self.option_label(*r), self.option_text(*r)))
                .collect(),
            lit: lit - first,
            above: first > 0,
            below: first + VIEW_ROWS < rows.len(),
            note: match row {
                OptionRow::Cheevos | OptionRow::Account => self.achievements_note(),
                row => self
                    .backup_note(row)
                    .or_else(|| self.saves_note(row))
                    .unwrap_or_else(|| row.note().to_string()),
            },
        })
    }

    pub(crate) fn options_input(&mut self, action: Action) {
        let Phase::Options(screen) = &mut self.phase else {
            return;
        };
        if screen.typing.is_some() {
            return self.sign_in_input(action);
        }
        // a question up over Saves' rows takes every press
        if self.saves_asking().is_some() {
            return self.saves_answer(action);
        }
        let Phase::Options(screen) = &mut self.phase else {
            return;
        };
        let page = screen.page;
        // a heading is passed over: the cursor stands only on rows that do something
        let rows: Vec<OptionRow> = self
            .page_rows(page)
            .into_iter()
            .filter(|r| !r.heading())
            .collect();
        let Phase::Options(screen) = &mut self.phase else {
            return;
        };
        // a row gone from a list that changed: the cursor is on its first
        if !rows.contains(&screen.row) {
            screen.row = rows.first().copied().unwrap_or(screen.row);
        }
        let row = screen.row;
        match action {
            Action::GbaDown(Btn::Up) if !rows.is_empty() => screen.row = step(&rows, row, false),
            Action::GbaDown(Btn::Down) if !rows.is_empty() => screen.row = step(&rows, row, true),
            Action::GbaDown(btn @ (Btn::A | Btn::X | Btn::Y)) if page.is_saves() => {
                self.saves_press(row, btn);
            }
            Action::GbaDown(Btn::A) if row == OptionRow::BackupUser => {
                self.open_typing(Step::BackupUser, self.state.backup_user.clone());
            }
            Action::GbaDown(Btn::A) if row == OptionRow::BackupPassword => {
                self.open_typing(Step::BackupPassword, self.state.backup_password.clone());
            }
            Action::GbaDown(Btn::A) if row == OptionRow::NasAddress => {
                self.open_typing(Step::NasAddress, self.state.backup_host.clone());
            }
            Action::GbaDown(Btn::A) if row == OptionRow::FolderNew => {
                self.open_typing(Step::NewFolder, String::new());
            }
            Action::GbaDown(Btn::A)
                if matches!(
                    row,
                    OptionRow::BackupOn
                        | OptionRow::BackupNas
                        | OptionRow::BackupFolder
                        | OptionRow::BackupNow
                        | OptionRow::BackupRestore
                        | OptionRow::RestoreItem(_)
                        | OptionRow::NasFound(_)
                        | OptionRow::FolderUse
                        | OptionRow::FolderItem(_)
                ) =>
            {
                self.backup_press(row);
            }
            Action::GbaDown(Btn::B) if page == OptionsPage::Folder && self.folder_back() => {}
            Action::GbaDown(Btn::A) if row == OptionRow::Account => self.open_sign_in(),
            Action::GbaDown(Btn::A) if row == OptionRow::OnlineName => self.open_online_name(),
            Action::GbaDown(Btn::A) if row == OptionRow::OnlinePlay => self.go(
                Phase::Options(OptionsScreen::new(OptionsPage::Online)),
                Dir::Deeper,
            ),
            Action::GbaDown(Btn::A) if row == OptionRow::Sound => self.go(
                Phase::Options(OptionsScreen::new(OptionsPage::Sound)),
                Dir::Deeper,
            ),
            Action::GbaDown(Btn::A) if row == OptionRow::Theme => self.go(
                Phase::Background(crate::background_screen::BackgroundScreen::default()),
                Dir::Deeper,
            ),
            Action::GbaDown(Btn::A) if row == OptionRow::Labels => self.go(
                Phase::Labels(crate::label_screen::LabelsScreen::default()),
                Dir::Deeper,
            ),
            Action::GbaDown(Btn::A) if row == OptionRow::Emulators => {
                self.count_switched_games();
                self.go(
                    Phase::Options(OptionsScreen::new(OptionsPage::Emulators)),
                    Dir::Deeper,
                );
            }
            Action::GbaDown(Btn::A) => match row {
                OptionRow::Switched(platform) => self.unswitch_games(platform),
                row => self.change_option(row, true),
            },
            Action::GbaDown(Btn::Y) if row == OptionRow::Account => self.sign_out_achievements(),
            Action::GbaDown(Btn::Left) => self.change_option(row, false),
            Action::GbaDown(Btn::Right) => self.change_option(row, true),
            // back to Bluetooth, on the headphones
            Action::GbaDown(Btn::B) | Action::QuickMenu if page == OptionsPage::Headphones => {
                let screen = match self.phones.link() {
                    Some(link) => crate::bluetooth_screen::BluetoothScreen::on(&link.mac),
                    None => crate::bluetooth_screen::BluetoothScreen::default(),
                };
                self.go(Phase::Bluetooth(screen), Dir::Back);
            }
            Action::GbaDown(Btn::B) | Action::QuickMenu => match page.parent() {
                Some((parent, row)) => self.go(
                    Phase::Options(OptionsScreen {
                        row,
                        ..OptionsScreen::new(parent)
                    }),
                    Dir::Back,
                ),
                None => self.go(
                    Phase::Settings {
                        row: page.settings_row(),
                    },
                    Dir::Back,
                ),
            },
            _ => {}
        }
    }

    /// The keyboard slides in over the panel for `step`, `text` already typed.
    pub(crate) fn open_typing(&mut self, step: Step, text: String) {
        let Phase::Options(screen) = &mut self.phase else {
            return;
        };
        let before = Phase::Options(screen.clone());
        screen.typing = Some(SignIn {
            step,
            user: String::new(),
            keys: Keyboard {
                text,
                ..Keyboard::default()
            },
        });
        self.slide_from(before, Dir::Deeper);
    }

    /// A on Name: the keyboard slides in over the panel, the name rooms show already typed.
    fn open_online_name(&mut self) {
        let name = self.online_name();
        let Phase::Options(screen) = &mut self.phase else {
            return;
        };
        let before = Phase::Options(screen.clone());
        screen.typing = Some(SignIn {
            step: Step::OnlineName,
            user: String::new(),
            keys: Keyboard {
                text: name,
                ..Keyboard::default()
            },
        });
        self.slide_from(before, Dir::Deeper);
    }

    /// The name online rooms show: the one typed, else the RetroAchievements user's, else
    /// "RG SP".
    pub fn online_name(&self) -> String {
        if !self.state.online_name.is_empty() {
            return self.state.online_name.clone();
        }
        self.root
            .as_deref()
            .and_then(crate::achievements::Account::load)
            .map(|a| omega_store::online_name(&a.user))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "RG SP".to_string())
    }

    /// A on Account: the keyboard slides in over the panel, asking for the user name, the one
    /// signed in as already typed.
    fn open_sign_in(&mut self) {
        let user = self
            .root
            .as_deref()
            .and_then(crate::achievements::Account::load)
            .map(|a| a.user)
            .unwrap_or_default();
        let Phase::Options(screen) = &mut self.phase else {
            return;
        };
        let before = Phase::Options(screen.clone());
        screen.typing = Some(SignIn {
            step: Step::User,
            user: String::new(),
            keys: Keyboard {
                text: user,
                ..Keyboard::default()
            },
        });
        self.slide_from(before, Dir::Deeper);
    }

    /// A key on the keyboard: the user name, then the password, then signing in; B on an empty
    /// field, or MENU, puts the keyboard away.
    fn sign_in_input(&mut self, action: Action) {
        let Phase::Options(screen) = &mut self.phase else {
            return;
        };
        let before = Phase::Options(screen.clone());
        let Some(t) = &mut screen.typing else {
            return;
        };
        let keys = &mut t.keys;
        let mut submit = false;
        match action {
            Action::GbaDown(Btn::Up) => keys.up(),
            Action::GbaDown(Btn::Down) => keys.down(),
            Action::GbaDown(Btn::Left) => keys.left(),
            Action::GbaDown(Btn::Right) => keys.right(),
            Action::GbaDown(Btn::A) => submit = keys.press(),
            Action::GbaDown(Btn::B) if keys.text.is_empty() => {
                screen.typing = None;
                return self.slide_from(before, Dir::Back);
            }
            Action::GbaDown(Btn::B) => keys.delete(),
            Action::GbaDown(Btn::X) => keys.shift(),
            Action::GbaDown(Btn::Y) => keys.type_char(' '),
            Action::GbaDown(Btn::L1 | Btn::R1) => keys.symbols(),
            Action::GbaDown(Btn::Start) => submit = true,
            Action::QuickMenu => {
                screen.typing = None;
                return self.slide_from(before, Dir::Back);
            }
            _ => return,
        }
        if !submit || (t.keys.text.is_empty() && !t.step.may_be_empty()) {
            return;
        }
        let typed = std::mem::take(&mut t.keys.text);
        match t.step {
            step @ (Step::BackupUser
            | Step::BackupPassword
            | Step::NasAddress
            | Step::NewFolder) => {
                screen.typing = None;
                self.slide_from(before, Dir::Back);
                self.backup_typed(step, &typed);
            }
            Step::OnlineName => {
                screen.typing = None;
                self.slide_from(before, Dir::Back);
                self.state.online_name = omega_store::online_name(&typed);
                self.persist();
            }
            Step::SearchGames => {
                screen.typing = None;
                self.slide_from(before, Dir::Back);
                self.search_typed(&typed);
            }
            // from the user name to the password the keyboard stays where it is
            Step::User => {
                t.step = Step::Password;
                t.user = typed;
                t.keys = Keyboard::default();
            }
            Step::Password => {
                let user = std::mem::take(&mut t.user);
                screen.typing = None;
                self.slide_from(before, Dir::Back);
                self.sign_in_achievements(&user, &typed);
            }
        }
    }

    /// `platform`'s default core, as Settings set it.
    pub fn default_core(&self, platform: Platform) -> Core {
        match &self.root {
            Some(root) => omega_store::default_core(root, platform),
            None => Core::default_for(platform),
        }
    }

    /// Steps `platform`'s default core on to the next that runs it.
    fn step_default_core(&mut self, platform: Platform, right: bool) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let to = step(
            Core::for_platform(platform),
            self.default_core(platform),
            right,
        );
        if let Err(e) = omega_store::write_default_core(&root, platform, to) {
            crate::log!("core: could not write default_core.ini: {e}");
        }
        self.count_switched_games();
    }

    /// How many of `platform`'s games run on a core other than its default, switched by hand,
    /// as last counted.
    pub fn switched_games(&self, platform: Platform) -> usize {
        self.switched
            .iter()
            .find(|(p, _)| *p == platform)
            .map_or(0, |(_, stems)| stems.len())
    }

    /// Counts each system's games switched off its default by hand: on opening EMULATORS and
    /// after each change there, not each frame, since it reads the card.
    pub(crate) fn count_switched_games(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let picked = omega_store::read_selected_cores(&root);
        self.switched = EMULATOR_SYSTEMS
            .into_iter()
            .map(|platform| {
                let default = omega_store::default_core(&root, platform);
                let stems = self
                    .carts()
                    .filter(|c| c.platform == platform)
                    .filter(|c| {
                        picked
                            .get(&c.stem)
                            .is_some_and(|core| core.runs(platform) && *core != default)
                    })
                    .map(|c| c.stem.clone())
                    .collect();
                (platform, stems)
            })
            .collect();
    }

    /// A on a Switched row: every one of `platform`'s games switched by hand runs on its
    /// default again.
    fn unswitch_games(&mut self, platform: Platform) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let stems: Vec<String> = self
            .switched
            .iter()
            .find(|(p, _)| *p == platform)
            .map(|(_, stems)| stems.clone())
            .unwrap_or_default();
        if stems.is_empty() {
            return self.refuse();
        }
        for stem in &stems {
            if let Err(e) = omega_store::clear_selected_core(&root, stem) {
                crate::log!("core: could not write selected_core.ini: {e}");
            }
        }
        self.count_switched_games();
        let text = match stems.len() {
            1 => "1 game on the default".to_string(),
            n => format!("{n} games on the default"),
        };
        self.hud.toast(
            Toast::Emulator(Box::leak(text.into_boxed_str())),
            self.now(),
        );
    }

    pub fn set_options_panel(&mut self, title: &'static str, panel: Lone) {
        match self.options_panels.iter_mut().find(|(t, _)| *t == title) {
            Some((_, kept)) => *kept = panel,
            None => self.options_panels.push((title, panel)),
        }
    }

    /// `page`'s panel, as it was last baked: the one leaving a slide keeps its own face.
    pub(crate) fn draw_options(&self, page: OptionsPage, out: &mut Vec<Draw>) {
        let title = page.title();
        if let Some((_, panel)) = self.options_panels.iter().find(|(t, _)| *t == title) {
            panel.draw(out);
        }
    }
}
