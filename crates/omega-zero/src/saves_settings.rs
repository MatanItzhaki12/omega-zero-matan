//! Settings › Saves, a panel at a time:
//!
//! - **SAVES**: NAS Backup (SAVE BACKUP: saves copied to a NAS as they are made, and brought
//!   back), Search Games (a name typed on the keyboard), Auto Backup (a save backed up by itself
//!   as its game goes in), Back Up All Saves, each system with games that have anything on the
//!   card, and the room left on the card.
//! - **GAMES**: a system's games, or those the search found, each with how big its save is.
//! - **GAME**: a game's Save and its Save States, each a panel of its own.
//! - **SAVE**: the Current Save and Back Up Now, then under the Backups heading each backup kept,
//!   named for when it was made.
//! - **STATES**: the game's save states, the resume point written as it stops among them.
//!
//! A opens and backs up; on a save, a backup or a state, X deletes it and Y restores it (a
//! backup put back as the save, a state made the one the game resumes from). Each asks first, in
//! a panel over the rows with B NO and A YES on the bar; and restoring a backup or deleting the
//! save keeps the save as it was as a backup of its own (`save_manager`). Settings opens only from the shelf, so no game
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
    ResumeFrom(u16),
    DeleteSave,
    DeleteAllStates,
}

impl Answer {
    /// The question, in the panel.
    pub fn question(self) -> &'static str {
        match self {
            Answer::RestoreBackup(_) => "Restore this backup?",
            Answer::DeleteBackup(_) => "Delete this backup?",
            Answer::DeleteState(_) => "Delete this state?",
            Answer::ResumeFrom(_) => "Resume from this state?",
            Answer::DeleteSave => "Delete this save?",
            Answer::DeleteAllStates => "Delete every state?",
        }
    }

    /// What it means, in the note under the rows while it is asked.
    pub fn detail(self) -> &'static str {
        match self {
            Answer::RestoreBackup(_) => "The save now is kept as a backup first",
            Answer::DeleteBackup(_) => "This copy cannot be brought back",
            Answer::DeleteState(_) => "This state cannot be brought back",
            Answer::ResumeFrom(_) => "The game picks up here when it goes in",
            Answer::DeleteSave => "The game starts over; a backup is kept first",
            Answer::DeleteAllStates => "Its resume point and saved states all go",
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

/// The room on the card, free and in all, measured on a thread of its own. The first `df`
/// after a boot has the kernel count a FAT card's free clusters through its whole table, near a
/// second on the handheld's, which held SAVES' slide in; so boot starts it, SAVES shows what the
/// last one found, and each opening measures again behind it.
#[derive(Clone, Default)]
pub struct CardRoom(std::sync::Arc<CardRoomHeld>);

#[derive(Default)]
pub struct CardRoomHeld {
    room: std::sync::Mutex<Option<(u64, u64)>>,
    measuring: std::sync::atomic::AtomicBool,
}

impl CardRoom {
    /// The room last measured, none before the first is done.
    pub fn get(&self) -> Option<(u64, u64)> {
        *self
            .0
            .room
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Measures it again on a thread, unless one is at it already; `warm` too, first, as the
    /// boot's look over every game's save folders, so SAVES' own finds them read.
    pub fn measure(&self, root: &std::path::Path, warm: Vec<Game>) {
        use std::sync::atomic::Ordering;
        if self.0.measuring.swap(true, Ordering::AcqRel) {
            return;
        }
        let (me, root) = (self.clone(), root.to_path_buf());
        let spawned = std::thread::Builder::new()
            .name("card-room".into())
            .spawn(move || {
                if !warm.is_empty() {
                    let _ = save_manager::index(&root, &warm);
                }
                let room = save_manager::card_space(&root);
                *me.0
                    .room
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = room;
                me.0.measuring.store(false, Ordering::Release);
            });
        if spawned.is_err() {
            self.0.measuring.store(false, Ordering::Release);
        }
    }
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
    /// The backup the keyboard is naming, by its place in `look.backups`.
    pub renaming: Option<u16>,
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

    /// At boot, behind the shelf: the card's room measured, and every game's save folders read
    /// once, so the first SAVES of a boot opens as quickly as the next.
    pub(crate) fn warm_saves(&self, root: &std::path::Path) {
        let all: Vec<Game> = self
            .shelves
            .iter()
            .filter(|(kind, _)| matches!(kind, ShelfKind::System(_)))
            .flat_map(|(_, shelf)| shelf.carts.iter())
            .map(Game::of)
            .collect();
        self.card_room.measure(root, all);
    }

    /// The games on the card with a save, a backup or a state, by system and then name.
    fn list_saved_games(&mut self) {
        self.saves_ui = SavesUi::default();
        let Some(root) = self.root.clone() else {
            return;
        };
        let began = std::time::Instant::now();
        let carts: Vec<&omega_store::Cart> = self
            .shelves
            .iter()
            .filter(|(kind, _)| matches!(kind, ShelfKind::System(_)))
            .flat_map(|(_, shelf)| shelf.carts.iter())
            .collect();
        let all: Vec<Game> = carts.iter().map(|c| Game::of(c)).collect();
        // each folder read once for all the games in it, so the panel slides in on time
        let games: Vec<(Game, String, u64)> = save_manager::index(&root, &all)
            .into_iter()
            .map(|(i, bytes)| (all[i].clone(), omega_ui::clean_label(&carts[i].stem), bytes))
            .collect();
        crate::log!(
            "saves: looked over {} games in {} ms",
            all.len(),
            began.elapsed().as_millis()
        );
        // measured behind the panel: the row shows the last room found, and the new one as it
        // comes
        self.card_room.measure(&root, Vec::new());
        crate::log!("saves: {} games with saves or states", games.len());
        self.saves_ui.games = games;
        self.sort_saved_games();
    }

    /// The games Saves lists, by system and then as Saves › Sort Games asks: by name, or the
    /// most played first and the rest by name.
    pub(crate) fn sort_saved_games(&mut self) {
        let by_play = self.state.saves_by_play;
        let log = &self.play_log;
        let played = |g: &Game| {
            if by_play {
                log.of(g.platform, &g.stem).map_or(0, |p| p.secs)
            } else {
                0
            }
        };
        self.saves_ui.games.sort_by_cached_key(|(g, name, _)| {
            (
                system_order(g.platform),
                std::cmp::Reverse(played(g)),
                name.to_lowercase(),
            )
        });
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

    /// A on a backup: the keyboard slides in, its name typed, to name it.
    fn rename_backup(&mut self, i: u16) {
        let Some(stamp) = self.saves_ui.look.backups.get(i as usize) else {
            return self.refuse();
        };
        let name = self
            .open_game()
            .zip(self.root.as_deref())
            .and_then(|(game, root)| save_manager::backup_name(root, game, stamp))
            .unwrap_or_default();
        self.saves_ui.renaming = Some(i);
        self.open_typing(Step::RenameBackup, name);
    }

    /// What the keyboard typed for a backup's name: the backup renamed, or going by its date
    /// again where the name is blank.
    pub fn rename_typed(&mut self, typed: &str) {
        let Some(i) = self.saves_ui.renaming.take() else {
            return;
        };
        let (Some(root), Some(game)) = (self.root.clone(), self.open_game().cloned()) else {
            return self.refuse();
        };
        let Some(stamp) = self.saves_ui.look.backups.get(i as usize).cloned() else {
            return self.refuse();
        };
        match save_manager::rename_backup(&root, &game, &stamp, typed) {
            Ok(()) => {
                crate::log!("saves: backup {stamp} of {} named {typed:?}", game.stem);
                self.saves_ui.look = save_manager::look(&root, &game);
            }
            Err(e) => {
                crate::log!("saves: rename {stamp}: {e}");
                self.saves_toast("Could not rename it");
            }
        }
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
            OptionsPage::Saves => [
                OptionRow::SavesNas,
                OptionRow::SavesSearch,
                OptionRow::SavesSort,
                OptionRow::SavesAutoBackup,
                OptionRow::SavesBackUpAll,
            ]
            .into_iter()
            .chain(self.saved_systems().into_iter().map(OptionRow::SavesSystem))
            .chain(self.card_room.get().map(|_| OptionRow::SavesCardSpace))
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
                if !look.backups.is_empty() {
                    rows.push(OptionRow::GameBackupsHeading);
                }
                rows.extend(index(look.backups.len()).map(|i| OptionRow::GameBackup(i as u16)));
                rows
            }
            OptionsPage::StatesMenu if look.states.is_empty() => vec![OptionRow::GameNoStates],
            OptionsPage::StatesMenu => index(look.states.len())
                .map(|i| OptionRow::GameState(i as u16))
                .collect(),
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
                let stamp = ui.look.backups.get(i as usize)?;
                let made = made_at(stamp, self.state.clock_24h);
                match self.open_game().zip(self.root.as_deref()) {
                    Some((game, root)) => match save_manager::backup_name(root, game, stamp) {
                        Some(name) => name,
                        None if save_manager::is_auto(root, game, stamp) => {
                            format!("{made} (Auto)")
                        }
                        None => made,
                    },
                    None => made,
                }
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
            OptionRow::SavesAutoBackup => match self.state.auto_backups {
                0 => "OFF".to_string(),
                n => format!("KEEP {n}"),
            },
            OptionRow::SavesCardSpace => match self.card_room.get() {
                Some((free, _)) => format!("{} FREE", size(free)),
                None => String::new(),
            },
            OptionRow::SavesSystem(p) => {
                let n = ui.games.iter().filter(|(g, ..)| g.platform == p).count();
                format!("{n} {}", if n == 1 { "GAME" } else { "GAMES" })
            }
            OptionRow::SavesGame(i) => {
                let what = match ui.shown.get(i as usize).and_then(|&i| ui.games.get(i)) {
                    Some((_, _, 0)) => "STATES".to_string(),
                    Some((_, _, bytes)) => size(*bytes),
                    None => return String::new(),
                };
                // a search lists every system's games: each says which it is
                match (
                    &ui.listed,
                    ui.shown.get(i as usize).and_then(|&i| ui.games.get(i)),
                ) {
                    (Listed::Search(_), Some((game, ..))) => {
                        format!("{} {what}", game.platform.dir_name())
                    }
                    _ => what,
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
            OptionRow::GameBackupsHeading => look.backups.len().to_string(),
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
            OptionRow::SavesGame(i) => {
                let (game, ..) = ui.games.get(*ui.shown.get(i as usize)?)?;
                self.played_note(game)
                    .unwrap_or_else(|| "Its save and its states".to_string())
            }
            OptionRow::SavesCardSpace => match self.card_room.get() {
                Some((free, total)) => format!("{} free of {}", size(free), size(total)),
                None => return None,
            },
            OptionRow::GameStatesMenu if !look.states.is_empty() => {
                format!("{}: X deletes every state", name()?)
            }
            OptionRow::SavesNoMatch => match &ui.listed {
                Listed::Search(text) => format!("No game's name holds \"{text}\""),
                _ => "No games here".to_string(),
            },
            OptionRow::GameSaveMenu | OptionRow::GameStatesMenu => {
                match self.open_game().and_then(|game| self.played_note(game)) {
                    Some(played) => format!("{}: {}", name()?, played.to_lowercase()),
                    None => name()?,
                }
            }
            OptionRow::GameSave if look.save.is_empty() => "No save on the handheld".to_string(),
            OptionRow::GameSave => match look.save_written {
                Some(at) => {
                    let secs = at
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs());
                    format!("Saved {}: X deletes it", crate::backup_settings::ago(secs))
                }
                None => "X deletes it".to_string(),
            },
            OptionRow::GameBackup(i) => {
                let stamp = look.backups.get(i as usize)?;
                let named = self
                    .open_game()
                    .zip(self.root.as_deref())
                    .and_then(|(game, root)| save_manager::backup_name(root, game, stamp));
                match named {
                    // the name stands for the date on the row: the note keeps it
                    Some(_) => format!(
                        "{}: A renames, Y restores, X deletes",
                        made_at(stamp, self.state.clock_24h)
                    ),
                    None => "A renames it, Y restores it, X deletes it".to_string(),
                }
            }
            OptionRow::GameState(i) => match look.states.get(i as usize) {
                Some(s) if s.kind == StateKind::Resume => {
                    format!("Where it stopped, on {}: X deletes it", s.core.name())
                }
                Some(s) => format!("On {}: Y resumes from it, X deletes it", s.core.name()),
                None => return None,
            },
            _ => return None,
        })
    }

    /// A, X or Y on a SAVES panel's row: A opens and backs up, X deletes and Y restores.
    pub(crate) fn saves_press(&mut self, row: OptionRow, btn: Btn) {
        let look = &self.saves_ui.look;
        match (btn, row) {
            (Btn::X, OptionRow::GameSave) if !look.save.is_empty() => {
                return self.ask(Answer::DeleteSave)
            }
            (Btn::X, OptionRow::GameBackup(i)) => return self.ask(Answer::DeleteBackup(i)),
            (Btn::Y, OptionRow::GameBackup(i)) => return self.ask(Answer::RestoreBackup(i)),
            (Btn::X, OptionRow::GameState(i)) => return self.ask(Answer::DeleteState(i)),
            (Btn::X, OptionRow::GameStatesMenu) if !look.states.is_empty() => {
                return self.ask(Answer::DeleteAllStates)
            }
            // the resume point is already where the game picks up
            (Btn::Y, OptionRow::GameState(i))
                if look
                    .states
                    .get(i as usize)
                    .is_some_and(|s| s.kind != StateKind::Resume) =>
            {
                return self.ask(Answer::ResumeFrom(i))
            }
            (Btn::A, _) => {}
            _ => return self.refuse(),
        }
        match row {
            OptionRow::SavesNas => {
                // a look at where it stands is a reason to try the NAS again now
                self.backup_pass();
                self.open_page(OptionsPage::Backup);
            }
            OptionRow::SavesSearch => self.open_typing(Step::SearchGames, String::new()),
            OptionRow::SavesAutoBackup | OptionRow::SavesSort => self.change_option(row, true),
            OptionRow::SavesBackUpAll => self.back_up_all(),
            OptionRow::SavesSystem(p) => self.open_games(Listed::System(p)),
            OptionRow::SavesGame(i) => self.open_game_saves(i as usize),
            OptionRow::GameSaveMenu => self.open_page(OptionsPage::SaveMenu),
            OptionRow::GameStatesMenu => self.open_page(OptionsPage::StatesMenu),
            OptionRow::GameBackUp => self.back_up_open_game(),
            OptionRow::GameBackup(i) => self.rename_backup(i),
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
            crate::screens::question::dim_behind(out);
            panel.draw(out);
        }
    }

    /// What Saves says of how long `game` has been played, and when last; none for a game
    /// not played since the handheld kept count.
    fn played_note(&self, game: &Game) -> Option<String> {
        let p = self
            .played(game.platform, &game.stem)
            .filter(|p| p.secs > 0)?;
        let last = p
            .last
            .map(|at| format!(", last {}", crate::backup_settings::ago(at)))
            .unwrap_or_default();
        Some(format!(
            "Played {}{last}",
            crate::play_log::played_text(p.secs)
        ))
    }

    /// Back Up All Saves: a copy of every game's save, kept on the handheld.
    fn back_up_all(&mut self) {
        let Some(root) = self.root.clone() else {
            return self.refuse();
        };
        let stamp = omega_store::format_stamp(self.wall_secs());
        let began = std::time::Instant::now();
        let (mut kept, mut failed) = (0, 0);
        for (game, ..) in &self.saves_ui.games {
            match save_manager::back_up(&root, game, &stamp) {
                Ok(true) => kept += 1,
                Ok(false) => {}
                Err(e) => {
                    failed += 1;
                    crate::log!("saves: back up {}: {e}", game.stem);
                }
            }
        }
        crate::log!(
            "saves: backed up {kept} saves in {} ms, {failed} failed",
            began.elapsed().as_millis()
        );
        self.saves_toast(match (kept, failed) {
            (_, f) if f > 0 => "Some saves could not be backed up",
            (0, _) => "No saves to back up",
            _ => "Every save backed up",
        });
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
            Answer::ResumeFrom(i) => match look.states.get(i as usize) {
                Some(state) => (
                    save_manager::resume_from(&root, &game, state, &now),
                    "The game resumes from it",
                ),
                None => return self.refuse(),
            },
            Answer::DeleteSave => (
                save_manager::delete_save(&root, &game, &now),
                "Save deleted",
            ),
            Answer::DeleteAllStates => (
                look.states
                    .iter()
                    .try_for_each(|s| save_manager::delete_state(&root, &game, s)),
                "Every state deleted",
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

/// When a backup stamped `stamp` was made, as its row names it: `12 Nov 2026, 16:40`, or
/// `4:40 PM` on a 12-hour clock.
pub fn made_at(stamp: &str, clock_24h: bool) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let part = |from: usize, to: usize| stamp.get(from..to).and_then(|p| p.parse::<u32>().ok());
    let (Some(year), Some(month), Some(day), Some(hour), Some(min)) = (
        part(0, 4),
        part(5, 7),
        part(8, 10),
        part(11, 13),
        part(14, 16),
    ) else {
        return stamp.to_string();
    };
    let month = MONTHS.get(month.wrapping_sub(1) as usize).unwrap_or(&"");
    let time = match clock_24h {
        true => format!("{hour:02}:{min:02}"),
        false => {
            let half = if hour < 12 { "AM" } else { "PM" };
            let h = match hour % 12 {
                0 => 12,
                h => h,
            };
            format!("{h}:{min:02} {half}")
        }
    };
    format!("{day} {month} {year}, {time}")
}

/// A size as the panel prints it: bytes, KB, MB or GB.
pub fn size(bytes: u64) -> String {
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * MB;
    match bytes {
        b if b < 1024 => format!("{b} B"),
        b if b < MB => format!("{} KB", b.div_ceil(1024)),
        b if b < GB => format!("{:.1} MB", b as f64 / MB as f64),
        b => format!("{:.1} GB", b as f64 / GB as f64),
    }
}
