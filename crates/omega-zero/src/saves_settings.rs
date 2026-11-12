//! Settings › Saves: the SAVES panel, and the GAME panel it opens for each game.
//!
//! SAVES has NAS Backup first, which opens SAVE BACKUP (saves copied to a NAS as they are made,
//! and brought back from it), then every game with a save, a backup or a state on the card, by
//! system and name. GAME shows a game's save, Back Up Save to keep a copy of it on the handheld,
//! each backup kept (A puts it back, Y deletes it), each save state, the resume state written as
//! the game stops among them (Y deletes it), and Delete Save. What can lose something asks
//! first, its note saying so, and acts on the same press again; and putting a backup back or
//! deleting the save keeps the save as it was as a backup of its own (`save_manager`).
//!
//! Settings opens only from the shelf, so no game these touch is in the slot.

use omega_input::Btn;
use omega_store::Platform;
use omega_ui::Toast;

use crate::app::{App, Phase};
use crate::favorites::ShelfKind;
use crate::nav::Dir;
use crate::options::{OptionRow, OptionsPage, OptionsScreen};
use crate::save_manager::{self, Game, GameSaves, StateKind};

/// What the SAVES panels are showing.
#[derive(Default)]
pub struct SavesUi {
    /// The games with anything on the card, and the name each is listed by.
    pub games: Vec<(Game, String)>,
    /// The game GAME is open on, by its place in `games`, and what it has.
    pub open: Option<usize>,
    pub look: GameSaves,
    /// The row and button pressed once on something that asks first: the same again acts.
    pub armed: Option<(OptionRow, Btn)>,
}

impl App {
    /// A on Saves in Settings: SAVES, with every game that has anything on the card.
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
        let mut games: Vec<(Game, String)> = self
            .shelves
            .iter()
            .filter(|(kind, _)| matches!(kind, ShelfKind::System(_)))
            .flat_map(|(_, shelf)| shelf.carts.iter())
            .map(|cart| (Game::of(cart), omega_ui::clean_label(&cart.stem)))
            .filter(|(game, _)| save_manager::has_any(&root, game))
            .collect();
        games.sort_by(|(a, an), (b, bn)| {
            (system_order(a.platform), an.to_lowercase())
                .cmp(&(system_order(b.platform), bn.to_lowercase()))
        });
        crate::log!("saves: {} games with saves or states", games.len());
        self.saves_ui.games = games;
    }

    /// A on a game in SAVES: GAME, on what it has.
    fn open_game_saves(&mut self, i: usize) {
        let Some(root) = self.root.clone() else {
            return self.refuse();
        };
        let Some((game, _)) = self.saves_ui.games.get(i) else {
            return self.refuse();
        };
        self.saves_ui.look = save_manager::look(&root, game);
        self.saves_ui.open = Some(i);
        self.saves_ui.armed = None;
        self.go(
            Phase::Options(OptionsScreen::new(OptionsPage::GameSaves)),
            Dir::Deeper,
        );
    }

    /// The game GAME is open on.
    fn open_game(&self) -> Option<&Game> {
        self.saves_ui
            .open
            .and_then(|i| self.saves_ui.games.get(i))
            .map(|(game, _)| game)
    }

    /// What GAME's game has, looked at again after a change.
    fn look_again(&mut self) {
        let (Some(root), Some(game)) = (self.root.clone(), self.open_game().cloned()) else {
            return;
        };
        self.saves_ui.look = save_manager::look(&root, &game);
    }

    /// The rows of SAVES and GAME, which are what the card has.
    pub(crate) fn saves_rows(&self, page: OptionsPage) -> Option<Vec<OptionRow>> {
        let ui = &self.saves_ui;
        match page {
            OptionsPage::Saves => Some(
                [OptionRow::SavesNas]
                    .into_iter()
                    .chain(
                        (0..ui.games.len().min(u16::MAX as usize))
                            .map(|i| OptionRow::SavesGame(i as u16)),
                    )
                    .collect(),
            ),
            OptionsPage::GameSaves => {
                let look = &ui.look;
                let mut rows = vec![OptionRow::GameSave];
                if !look.save.is_empty() {
                    rows.push(OptionRow::GameBackUp);
                }
                rows.extend(
                    (0..look.backups.len().min(u16::MAX as usize))
                        .map(|i| OptionRow::GameBackup(i as u16)),
                );
                rows.extend(
                    (0..look.states.len().min(u16::MAX as usize))
                        .map(|i| OptionRow::GameState(i as u16)),
                );
                if !look.save.is_empty() {
                    rows.push(OptionRow::GameDelete);
                }
                Some(rows)
            }
            _ => None,
        }
    }

    /// A row's name, where it is a game's, a backup's or a state's.
    pub(crate) fn saves_label(&self, row: OptionRow) -> Option<String> {
        let ui = &self.saves_ui;
        let now = omega_store::format_stamp(self.wall_secs());
        Some(match row {
            OptionRow::SavesGame(i) => ui.games.get(i as usize)?.1.clone(),
            OptionRow::GameSave => ui.games.get(ui.open?)?.1.clone(),
            OptionRow::GameBackup(i) => format!(
                "Backup, {}",
                omega_ui::Polaroids::relative_time(ui.look.backups.get(i as usize)?, &now)
            ),
            OptionRow::GameState(i) => {
                let state = ui.look.states.get(i as usize)?;
                match &state.kind {
                    StateKind::Resume => "Resume State".to_string(),
                    StateKind::SetAside(_) => "Old Resume State".to_string(),
                    StateKind::Saved(stamp) => {
                        format!("State, {}", omega_ui::Polaroids::relative_time(stamp, &now))
                    }
                }
            }
            _ => return None,
        })
    }

    /// What a SAVES or GAME row is set to, as its panel prints it.
    pub(crate) fn saves_text(&self, row: OptionRow) -> String {
        let ui = &self.saves_ui;
        match row {
            OptionRow::SavesNas => self.backup_word(),
            OptionRow::SavesGame(i) => ui
                .games
                .get(i as usize)
                .map(|(g, _)| g.platform.dir_name().to_string())
                .unwrap_or_default(),
            OptionRow::GameSave if ui.look.save.is_empty() => "NO SAVE".to_string(),
            OptionRow::GameSave => size(ui.look.save_bytes),
            OptionRow::GameState(i) => ui
                .look
                .states
                .get(i as usize)
                .map(|s| s.core.name().to_uppercase())
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// A SAVES or GAME row's note: what it does, or what pressing it again will.
    pub(crate) fn saves_note(&self, row: OptionRow) -> Option<String> {
        let ui = &self.saves_ui;
        let armed = |btn: Btn| ui.armed == Some((row, btn));
        Some(match row {
            OptionRow::SavesNas => "Saves copied to a NAS, and back".to_string(),
            OptionRow::SavesGame(_) => "Its save, backups and states".to_string(),
            OptionRow::GameSave if ui.look.save.is_empty() => "No save on the handheld".to_string(),
            OptionRow::GameSave => match ui.look.save_written {
                Some(at) => {
                    let secs = at
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs());
                    format!("Saved {}", crate::backup_settings::ago(secs))
                }
                None => "Its save on the handheld".to_string(),
            },
            OptionRow::GameBackUp => "A keeps a copy on the handheld".to_string(),
            OptionRow::GameBackup(_) if armed(Btn::A) => {
                "A again puts it back; the save is kept".to_string()
            }
            OptionRow::GameBackup(_) if armed(Btn::Y) => "Y again deletes the backup".to_string(),
            OptionRow::GameBackup(_) => "A puts it back, Y deletes it".to_string(),
            OptionRow::GameState(_) if armed(Btn::Y) => "Y again deletes the state".to_string(),
            OptionRow::GameState(i) => match ui.look.states.get(i as usize) {
                Some(s) if s.kind == StateKind::Resume => {
                    format!("Where it stopped, {}: Y deletes it", size(s.bytes))
                }
                Some(s) => format!("{}: Y deletes it", size(s.bytes)),
                None => return None,
            },
            OptionRow::GameDelete if armed(Btn::A) => {
                "A again deletes it; a backup is kept".to_string()
            }
            OptionRow::GameDelete => "The game starts anew".to_string(),
            _ => return None,
        })
    }

    /// A or Y on a SAVES or GAME row.
    pub(crate) fn saves_press(&mut self, row: OptionRow, btn: Btn) {
        match (row, btn) {
            (OptionRow::SavesNas, Btn::A) => {
                // a look at where it stands is a reason to try the NAS again now
                self.backup_pass();
                self.go(
                    Phase::Options(OptionsScreen::new(OptionsPage::Backup)),
                    Dir::Deeper,
                );
            }
            (OptionRow::SavesGame(i), Btn::A) => self.open_game_saves(i as usize),
            (OptionRow::GameBackUp, Btn::A) => self.back_up_open_game(),
            (OptionRow::GameBackup(_) | OptionRow::GameDelete, Btn::A)
            | (OptionRow::GameBackup(_) | OptionRow::GameState(_), Btn::Y) => {
                if self.saves_ui.armed != Some((row, btn)) {
                    self.saves_ui.armed = Some((row, btn));
                    return;
                }
                self.saves_ui.armed = None;
                self.saves_act(row, btn);
            }
            _ => self.refuse(),
        }
    }

    /// The cursor moved: what was pressed once to ask is let go of.
    pub(crate) fn saves_disarm(&mut self) {
        self.saves_ui.armed = None;
    }

    /// Back Up Save: a copy of the save as it is, kept on the handheld.
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

    /// What a press asked twice for: a backup put back or deleted, a state deleted, the save
    /// deleted.
    fn saves_act(&mut self, row: OptionRow, btn: Btn) {
        let (Some(root), Some(game)) = (self.root.clone(), self.open_game().cloned()) else {
            return self.refuse();
        };
        let now = omega_store::format_stamp(self.wall_secs());
        let look = &self.saves_ui.look;
        let (done, said) = match (row, btn) {
            (OptionRow::GameBackup(i), Btn::A) => match look.backups.get(i as usize) {
                Some(stamp) => (
                    save_manager::restore(&root, &game, stamp, &now),
                    "Backup put back",
                ),
                None => return self.refuse(),
            },
            (OptionRow::GameBackup(i), Btn::Y) => match look.backups.get(i as usize) {
                Some(stamp) => (
                    save_manager::delete_backup(&root, &game, stamp),
                    "Backup deleted",
                ),
                None => return self.refuse(),
            },
            (OptionRow::GameState(i), Btn::Y) => match look.states.get(i as usize) {
                Some(state) => (
                    save_manager::delete_state(&root, &game, state),
                    "State deleted",
                ),
                None => return self.refuse(),
            },
            (OptionRow::GameDelete, Btn::A) => (
                save_manager::delete_save(&root, &game, &now),
                "Save deleted",
            ),
            _ => return self.refuse(),
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

/// A size as the panel prints it: bytes, KB or MB.
fn size(bytes: u64) -> String {
    match bytes {
        b if b < 1024 => format!("{b} B"),
        b if b < 1024 * 1024 => format!("{} KB", b.div_ceil(1024)),
        b => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
    }
}
