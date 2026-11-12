//! Settings › Saves, a panel at a time:
//!
//! - **SAVES**: NAS Backup (SAVE BACKUP: saves copied to a NAS as they are made, and brought
//!   back), Search Games (a name typed on the keyboard), and each system with games that have
//!   anything on the card.
//! - **GAMES**: a system's games, or those the search found, each with how big its save is.
//! - **GAME**: a game's Save and its Save States, each a panel of its own.
//! - **SAVE**: the save, Back Up Now, each backup kept, and Delete Save.
//! - **BACKUP**: a backup kept: Restore This Backup, Delete This Backup.
//! - **STATES**: the game's save states, the resume point written as it stops among them.
//!
//! Everything is done with A. Whatever can lose something asks first, in a panel over the rows
//! with B NO and A YES on the bar; and restoring a backup or deleting the save keeps the save as
//! it was as a backup of its own (`save_manager`). Settings opens only from the shelf, so no game
//! these touch is in the slot.

use omega_input::{Action, Btn};
use omega_store::Platform;
use omega_ui::{Draw, Toast};

use crate::app::{App, Phase};
use crate::favorites::ShelfKind;
use crate::nav::Dir;
use crate::options::{OptionRow, OptionsPage, OptionsScreen, Step};
use crate::save_manager::{self, Game, GameSaves, StateKind};
use crate::screens::lone::Lone;

/// The title over a question Saves asks.
pub const QUESTION_TITLE: &str = "SAVES";

/// What a question Saves asks does on A YES.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    RestoreBackup(u16),
    DeleteBackup(u16),
    DeleteState(u16),
    DeleteSave,
}

impl Answer {
    /// The question, in the panel.
    pub fn question(self) -> &'static str {
        match self {
            Answer::RestoreBackup(_) => "Restore this backup?",
            Answer::DeleteBackup(_) => "Delete this backup?",
            Answer::DeleteState(_) => "Delete this state?",
            Answer::DeleteSave => "Delete this save?",
        }
    }

    /// What it means, in the note under the rows while it is asked.
    pub fn detail(self) -> &'static str {
        match self {
            Answer::RestoreBackup(_) => "The save now is kept as a backup first",
            Answer::DeleteBackup(_) => "This copy cannot be brought back",
            Answer::DeleteState(_) => "This state cannot be brought back",
            Answer::DeleteSave => "The game starts over; a backup is kept first",
        }
    }
}

/// Which games GAMES lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Listed {
    #[default]
    None,
    System(Platform),
    /// The games whose names hold what was typed, any case.
    Search(String),
}

/// What the SAVES panels are showing.
#[derive(Default)]
pub struct SavesUi {
    /// The games with anything on the card, the name each is listed by, and how big its
    /// save is, by system and name.
    pub games: Vec<(Game, String, u64)>,
    /// Which GAMES lists, and the places in `games` of those it does.
    pub listed: Listed,
    pub shown: Vec<usize>,
    /// The game GAME is open on, by its place in `games`, and what it has.
    pub open: Option<usize>,
    pub look: GameSaves,
    /// The backup BACKUP is open on, by its place in `look.backups`.
    pub kept: Option<u16>,
    /// The question up over the rows, and its panel.
    pub asking: Option<Answer>,
    pub panel: Option<Lone>,
}

impl App {
    /// A on Saves in Settings: SAVES, the card looked over for every game with anything on it.
    pub(crate) fn open_saves(&mut self) {
        self.list_saved_games();
        self.go(
            Phase::Options(OptionsScreen::new(OptionsPage::Saves)),
            Dir::Deeper,
        );
    }

    /// The games on the card with a save, a backup or a state, by system and then name.
    fn list_saved_games(&mut self) {
        self.saves_ui = SavesUi::default();
        let Some(root) = self.root.clone() else {
            return;
        };
        let mut games: Vec<(Game, String, u64)> = self
            .shelves
            .iter()
            .filter(|(kind, _)| matches!(kind, ShelfKind::System(_)))
            .flat_map(|(_, shelf)| shelf.carts.iter())
            .filter_map(|cart| {
                let game = Game::of(cart);
                let look = save_manager::look(&root, &game);
                (!look.is_empty())
                    .then(|| (game, omega_ui::clean_label(&cart.stem), look.save_bytes))
            })
            .collect();
        games.sort_by(|(a, an, _), (b, bn, _)| {
            (system_order(a.platform), an.to_lowercase())
                .cmp(&(system_order(b.platform), bn.to_lowercase()))
        });
        crate::log!("saves: {} games with saves or states", games.len());
        self.saves_ui.games = games;
    }

    /// The systems with games in SAVES, as the shelves stand.
    fn saved_systems(&self) -> Vec<Platform> {
        Platform::ALL
            .into_iter()
            .filter(|p| self.saves_ui.games.iter().any(|(g, ..)| g.platform == *p))
            .collect()
    }

    /// GAMES, on `listed`.
    fn open_games(&mut self, listed: Listed) {
        let ui = &mut self.saves_ui;
        ui.shown = (0..ui.games.len())
            .filter(|&i| {
                let (game, name, _) = &ui.games[i];
                match &listed {
                    Listed::None => false,
                    Listed::System(p) => game.platform == *p,
                    Listed::Search(text) => name.to_lowercase().contains(&text.to_lowercase()),
                }
            })
            .collect();
        crate::log!("saves: {:?} lists {} games", listed, ui.shown.len());
        ui.listed = listed;
        self.go(
            Phase::Options(OptionsScreen::new(OptionsPage::SavesGames)),
            Dir::Deeper,
        );
    }

    /// What the keyboard typed for Search Games: GAMES on what holds it.
    pub fn search_typed(&mut self, typed: &str) {
        let text = typed.trim();
        if text.is_empty() {
            return;
        }
        self.open_games(Listed::Search(text.to_string()));
    }

    /// A on a game in GAMES: GAME, on what it has.
    fn open_game_saves(&mut self, shown: usize) {
        let Some(root) = self.root.clone() else {
            return self.refuse();
        };
        let Some(&i) = self.saves_ui.shown.get(shown) else {
            return self.refuse();
        };
        self.saves_ui.look = save_manager::look(&root, &self.saves_ui.games[i].0);
        self.saves_ui.open = Some(i);
        self.go(
            Phase::Options(OptionsScreen::new(OptionsPage::GameSaves)),
            Dir::Deeper,
        );
    }

    /// A panel of GAME's, opened deeper.
    fn open_page(&mut self, page: OptionsPage) {
        self.go(Phase::Options(OptionsScreen::new(page)), Dir::Deeper);
    }

    /// A on a backup in SAVE: BACKUP, to restore it or delete it.
    fn open_kept(&mut self, i: u16) {
        if self.saves_ui.look.backups.get(i as usize).is_none() {
            return self.refuse();
        }
        self.saves_ui.kept = Some(i);
        self.open_page(OptionsPage::BackupKept);
    }

    /// The game GAME is open on.
    fn open_game(&self) -> Option<&Game> {
        self.saves_ui
            .open
            .and_then(|i| self.saves_ui.games.get(i))
            .map(|(game, ..)| game)
    }

    /// What GAME's game has, looked at again after a change; and its save's size in GAMES.
    fn look_again(&mut self) {
        let (Some(root), Some(game)) = (self.root.clone(), self.open_game().cloned()) else {
            return;
        };
        self.saves_ui.look = save_manager::look(&root, &game);
        let bytes = self.saves_ui.look.save_bytes;
        if let Some(entry) = self
            .saves_ui
            .open
            .and_then(|i| self.saves_ui.games.get_mut(i))
        {
            entry.2 = bytes;
        }
    }

    /// The rows of the SAVES panels, which are what the card has.
    pub(crate) fn saves_rows(&self, page: OptionsPage) -> Option<Vec<OptionRow>> {
        let ui = &self.saves_ui;
        let look = &ui.look;
        let index = |n: usize| 0..n.min(u16::MAX as usize);
        Some(match page {
            OptionsPage::Saves => [OptionRow::SavesNas, OptionRow::SavesSearch]
                .into_iter()
                .chain(self.saved_systems().into_iter().map(OptionRow::SavesSystem))
                .collect(),
            OptionsPage::SavesGames if ui.shown.is_empty() => vec![OptionRow::SavesNoMatch],
            OptionsPage::SavesGames => index(ui.shown.len())
                .map(|i| OptionRow::SavesGame(i as u16))
                .collect(),
            OptionsPage::GameSaves => vec![OptionRow::GameSaveMenu, OptionRow::GameStatesMenu],
            OptionsPage::SaveMenu => {
                let mut rows = vec![OptionRow::GameSave];
                if !look.save.is_empty() {
                    rows.push(OptionRow::GameBackUp);
                }
                rows.extend(index(look.backups.len()).map(|i| OptionRow::GameBackup(i as u16)));
                if !look.save.is_empty() {
                    rows.push(OptionRow::GameDelete);
                }
                rows
            }
            OptionsPage::StatesMenu if look.states.is_empty() => vec![OptionRow::GameNoStates],
            OptionsPage::StatesMenu => index(look.states.len())
                .map(|i| OptionRow::GameState(i as u16))
                .collect(),
            OptionsPage::BackupKept => vec![OptionRow::KeptRestore, OptionRow::KeptDelete],
            _ => return None,
        })
    }

    /// A row's name, where it is a system's, a game's, a backup's or a state's.
    pub(crate) fn saves_label(&self, row: OptionRow) -> Option<String> {
        let ui = &self.saves_ui;
        let now = omega_store::format_stamp(self.wall_secs());
        let when = |stamp: &str| omega_ui::Polaroids::relative_time(stamp, &now);
        Some(match row {
            OptionRow::SavesSystem(p) => system(p).to_string(),
            OptionRow::SavesGame(i) => ui.games.get(*ui.shown.get(i as usize)?)?.1.clone(),
            OptionRow::GameBackup(i) => {
                format!("Backup, {}", when(ui.look.backups.get(i as usize)?))
            }
            OptionRow::GameState(i) => match &ui.look.states.get(i as usize)?.kind {
                StateKind::Resume => "Resume Point".to_string(),
                StateKind::SetAside(_) => "Old Resume Point".to_string(),
                StateKind::Saved(stamp) => format!("Saved State, {}", when(stamp)),
            },
            _ => return None,
        })
    }

    /// What a SAVES panel's row is set to, as its panel prints it.
    pub(crate) fn saves_text(&self, row: OptionRow) -> String {
        let ui = &self.saves_ui;
        let look = &ui.look;
        match row {
            OptionRow::SavesNas => self.backup_word(),
            OptionRow::SavesSystem(p) => {
                let n = ui.games.iter().filter(|(g, ..)| g.platform == p).count();
                format!("{n} {}", if n == 1 { "GAME" } else { "GAMES" })
            }
            OptionRow::SavesGame(i) => {
                match ui.shown.get(i as usize).and_then(|&i| ui.games.get(i)) {
                    Some((_, _, 0)) => "STATES".to_string(),
                    Some((_, _, bytes)) => size(*bytes),
                    None => String::new(),
                }
            }
            OptionRow::GameSaveMenu if look.save.is_empty() && look.backups.is_empty() => {
                "NONE".to_string()
            }
            OptionRow::GameSaveMenu if look.save.is_empty() => "BACKUPS".to_string(),
            OptionRow::GameSaveMenu | OptionRow::GameSave if !look.save.is_empty() => {
                size(look.save_bytes)
            }
            OptionRow::GameSave => "NO SAVE".to_string(),
            OptionRow::GameStatesMenu => match look.states.len() {
                0 => "NONE".to_string(),
                n => n.to_string(),
            },
            OptionRow::GameState(i) => look
                .states
                .get(i as usize)
                .map(|s| size(s.bytes))
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// A SAVES panel's row's note; while a question is up, what answering yes means.
    pub(crate) fn saves_note(&self, row: OptionRow) -> Option<String> {
        let ui = &self.saves_ui;
        let look = &ui.look;
        if let Some(answer) = ui.asking {
            return Some(answer.detail().to_string());
        }
        let name = || {
            ui.open
                .and_then(|i| ui.games.get(i))
                .map(|(_, n, _)| n.clone())
        };
        Some(match row {
            OptionRow::SavesNoMatch => match &ui.listed {
                Listed::Search(text) => format!("No game's name holds \"{text}\""),
                _ => "No games here".to_string(),
            },
            OptionRow::GameSaveMenu | OptionRow::GameStatesMenu => name()?,
            OptionRow::GameSave if look.save.is_empty() => "No save on the handheld".to_string(),
            OptionRow::GameSave => match look.save_written {
                Some(at) => {
                    let secs = at
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs());
                    format!("Saved {}", crate::backup_settings::ago(secs))
                }
                None => "Its save on the handheld".to_string(),
            },
            OptionRow::GameState(i) => match look.states.get(i as usize) {
                Some(s) if s.kind == StateKind::Resume => {
                    format!("Where it stopped, on {}: A deletes it", s.core.name())
                }
                Some(s) => format!("On {}: A deletes it", s.core.name()),
                None => return None,
            },
            _ => return None,
        })
    }

    /// A on a SAVES panel's row.
    pub(crate) fn saves_press(&mut self, row: OptionRow) {
        match row {
            OptionRow::SavesNas => {
                // a look at where it stands is a reason to try the NAS again now
                self.backup_pass();
                self.open_page(OptionsPage::Backup);
            }
            OptionRow::SavesSearch => self.open_typing(Step::SearchGames, String::new()),
            OptionRow::SavesSystem(p) => self.open_games(Listed::System(p)),
            OptionRow::SavesGame(i) => self.open_game_saves(i as usize),
            OptionRow::GameSaveMenu => self.open_page(OptionsPage::SaveMenu),
            OptionRow::GameStatesMenu => self.open_page(OptionsPage::StatesMenu),
            OptionRow::GameBackUp => self.back_up_open_game(),
            OptionRow::GameBackup(i) => self.open_kept(i),
            OptionRow::GameState(i) => self.ask(Answer::DeleteState(i)),
            OptionRow::GameDelete => self.ask(Answer::DeleteSave),
            OptionRow::KeptRestore => match self.saves_ui.kept {
                Some(i) => self.ask(Answer::RestoreBackup(i)),
                None => self.refuse(),
            },
            OptionRow::KeptDelete => match self.saves_ui.kept {
                Some(i) => self.ask(Answer::DeleteBackup(i)),
                None => self.refuse(),
            },
            _ => self.refuse(),
        }
    }

    /// Whether a question is up over the SAVES panels, and which.
    pub fn saves_asking(&self) -> Option<Answer> {
        self.saves_ui.asking
    }

    pub fn set_saves_question(&mut self, panel: Lone) {
        self.saves_ui.panel = Some(panel);
    }

    fn ask(&mut self, answer: Answer) {
        self.saves_ui.asking = Some(answer);
    }

    /// A press while a question is up: A does it, B lets it go, and the rest wait.
    pub(crate) fn saves_answer(&mut self, action: Action) {
        let Some(answer) = self.saves_ui.asking else {
            return;
        };
        match action {
            Action::GbaDown(Btn::A) => {
                self.saves_ui.asking = None;
                self.saves_act(answer);
            }
            Action::GbaDown(Btn::B) | Action::QuickMenu => self.saves_ui.asking = None,
            _ => {}
        }
    }

    /// The question over the rows, while it is up.
    pub(crate) fn draw_saves_question(&self, out: &mut Vec<Draw>) {
        if let (Some(_), Some(panel)) = (self.saves_ui.asking, &self.saves_ui.panel) {
            panel.draw(out);
        }
    }

    /// Back Up Now: a copy of the save as it is, kept on the handheld.
    fn back_up_open_game(&mut self) {
        let (Some(root), Some(game)) = (self.root.clone(), self.open_game().cloned()) else {
            return self.refuse();
        };
        let stamp = omega_store::format_stamp(self.wall_secs());
        match save_manager::back_up(&root, &game, &stamp) {
            Ok(true) => self.saves_toast("Save backed up"),
            Ok(false) => return self.refuse(),
            Err(e) => {
                crate::log!("saves: back up {}: {e}", game.stem);
                self.saves_toast("Could not back up the save");
            }
        }
        self.look_again();
    }

    /// What a question answered yes asked for.
    fn saves_act(&mut self, answer: Answer) {
        let (Some(root), Some(game)) = (self.root.clone(), self.open_game().cloned()) else {
            return self.refuse();
        };
        let now = omega_store::format_stamp(self.wall_secs());
        let look = &self.saves_ui.look;
        let stamp = |i: u16| look.backups.get(i as usize).cloned();
        let (done, said) = match answer {
            Answer::RestoreBackup(i) => match stamp(i) {
                Some(s) => (
                    save_manager::restore(&root, &game, &s, &now),
                    "Backup restored",
                ),
                None => return self.refuse(),
            },
            Answer::DeleteBackup(i) => match stamp(i) {
                Some(s) => (
                    save_manager::delete_backup(&root, &game, &s),
                    "Backup deleted",
                ),
                None => return self.refuse(),
            },
            Answer::DeleteState(i) => match look.states.get(i as usize) {
                Some(state) => (
                    save_manager::delete_state(&root, &game, state),
                    "State deleted",
                ),
                None => return self.refuse(),
            },
            Answer::DeleteSave => (
                save_manager::delete_save(&root, &game, &now),
                "Save deleted",
            ),
        };
        match done {
            Ok(()) => self.saves_toast(said),
            Err(e) => {
                crate::log!("saves: {said} for {}: {e}", game.stem);
                self.saves_toast("Could not change the save");
            }
        }
        self.look_again();
        // the backup BACKUP was open on is done with: back to SAVE, on the save
        if matches!(answer, Answer::RestoreBackup(_) | Answer::DeleteBackup(_)) {
            self.saves_ui.kept = None;
            self.go(
                Phase::Options(OptionsScreen::new(OptionsPage::SaveMenu)),
                Dir::Back,
            );
        }
    }

    fn saves_toast(&mut self, said: &'static str) {
        let now = self.now();
        self.hud.toast(Toast::Saves(said), now);
    }
}

/// Where a system's games stand in SAVES: as the shelves stand.
fn system_order(p: Platform) -> usize {
    Platform::ALL
        .iter()
        .position(|q| *q == p)
        .unwrap_or(usize::MAX)
}

/// A system's name, as SAVES lists it.
fn system(p: Platform) -> &'static str {
    match p {
        Platform::Gba => "Game Boy Advance",
        Platform::Gb => "Game Boy",
        Platform::Gbc => "Game Boy Color",
        Platform::Snes => "SNES",
        Platform::Ps1 => "PlayStation",
        Platform::Psp => "PSP",
    }
}

/// A size as the panel prints it: bytes, KB or MB.
fn size(bytes: u64) -> String {
    match bytes {
        b if b < 1024 => format!("{b} B"),
        b if b < 1024 * 1024 => format!("{} KB", b.div_ceil(1024)),
        b => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
    }
}
