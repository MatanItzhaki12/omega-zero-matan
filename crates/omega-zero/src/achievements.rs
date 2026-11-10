//! RetroAchievements: with Settings > RetroAchievements switched on, omega-zero signs
//! in to the player's account and every Game Boy, Game Boy Color, GBA and SNES game is checked
//! for achievements as it plays, each one earned said in a toast. MENU over a game says, under
//! Settings > Achievements, which game RetroAchievements took the ROM for, and lists its
//! achievements.
//!
//! The player signs in on the keyboard, from Settings > RetroAchievements > Account,
//! or by writing Config/retroachievements.ini by hand:
//!
//! ```text
//! username = MyName
//! password = MyPassword
//! ```
//!
//! On the first sign-in the password is swapped for the token the site hands back, as every
//! RetroAchievements emulator keeps it, and the file then holds only the name and the token.
//!
//! Hardcore, on the same panel, plays as the site's hardcore asks: no state is loaded, no game
//! rewinds and none picks up from where it was put away; unlocks count as hardcore.
//!
//! A sign-in the network refuses, as at a boot before Wi-Fi is up, is tried again, sooner at
//! first and then each minute, and the game in play has its achievements loaded once it is in.
//! What the site answered is kept in Config/retroachievements/, so a game played before signs
//! in and loads with no network, and unlocks earned offline are sent once it is back.

use std::path::Path;

use omega_input::Millis;
use omega_store::{atomic_write, Platform, CONFIG_DIR};
use omega_ui::Toast;

use crate::app::App;

pub const ACCOUNT_FILE: &str = "retroachievements.ini";

/// The longest a title is shown in a toast before it is cut short.
const TOAST_CHARS: usize = 30;

/// Where the site's answers and the unlocks still to send are kept, in Config.
pub const STORE_DIR: &str = "retroachievements";

/// How long after the first sign-in the network refused the next is tried; each wait after is
/// twice the last, up to `RETRY_MAX_MS`.
pub const RETRY_FIRST_MS: Millis = 5_000;
pub const RETRY_MAX_MS: Millis = 60_000;

/// The wait before try `n` again, counting from 0.
pub fn retry_after(n: u32) -> Millis {
    RETRY_FIRST_MS
        .saturating_mul(1 << n.min(16))
        .min(RETRY_MAX_MS)
}

/// The player's RetroAchievements account, as Config/retroachievements.ini holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub user: String,
    pub password: Option<String>,
    pub token: Option<String>,
}

impl Account {
    pub fn load(root: &Path) -> Option<Account> {
        let fields = omega_store::ini::read(root, &format!("{CONFIG_DIR}/{ACCOUNT_FILE}"));
        let field = |k: &str| fields.get(k).filter(|v| !v.is_empty()).cloned();
        Some(Account {
            user: field("username")?,
            password: field("password"),
            token: field("token"),
        })
    }

    /// Keeps the name and password typed on the keyboard, until the site swaps the password
    /// for a token on the sign-in they are for.
    pub fn keep_password(root: &Path, user: &str, password: &str) -> std::io::Result<()> {
        let text = format!(
            "# RetroAchievements account, typed in Settings > RetroAchievements >\n\
             # Account. omega-zero swaps the password for a token as soon as the site lets it\n\
             # sign in.\n\
             username = {user}\npassword = {password}\n"
        );
        atomic_write(&root.join(CONFIG_DIR).join(ACCOUNT_FILE), text.as_bytes())
    }

    /// Signing out forgets the account, and its token with it.
    pub fn forget(root: &Path) {
        let _ = std::fs::remove_file(root.join(CONFIG_DIR).join(ACCOUNT_FILE));
    }

    /// Keeps the name and the token the site handed back, and drops the password.
    pub fn keep_token(root: &Path, user: &str, token: &str) -> std::io::Result<()> {
        let text = format!(
            "# RetroAchievements account. omega-zero swapped the password for this token on\n\
             # its first sign-in; to sign in as someone else, put username and password back.\n\
             username = {user}\ntoken = {token}\n"
        );
        atomic_write(&root.join(CONFIG_DIR).join(ACCOUNT_FILE), text.as_bytes())
    }
}

/// The console a system's games are checked as; none for one RetroAchievements does not read
/// here: the PlayStation, whose core's memory is not mapped for it.
pub fn console_of(platform: Platform) -> Option<u32> {
    match platform {
        Platform::Gb => Some(omega_cheevos::CONSOLE_GB),
        Platform::Gbc => Some(omega_cheevos::CONSOLE_GBC),
        Platform::Gba => Some(omega_cheevos::CONSOLE_GBA),
        Platform::Snes => Some(omega_cheevos::CONSOLE_SNES),
        Platform::Psp => Some(omega_cheevos::CONSOLE_PSP),
        Platform::Ps1 => None,
    }
}

/// `text` cut to what a toast holds, kept for the toast's life: the few toasts a session shows.
fn shown(text: &str) -> &'static str {
    let cut: String = match text.chars().count() > TOAST_CHARS {
        true => {
            let mut t: String = text.chars().take(TOAST_CHARS - 1).collect();
            t.push('…');
            t
        }
        false => text.to_string(),
    };
    Box::leak(cut.into_boxed_str())
}

/// Where the session with RetroAchievements stands, as the app follows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cheevos {
    /// Signing in was asked for, and its answer not yet told.
    pub signing_in: bool,
    /// The name signed in as.
    pub signed_in: Option<String>,
    /// Why the last sign-in was refused.
    pub refused: Option<String>,
    /// The game whose summary is still to be told when its achievements load.
    pub told_game: bool,
    /// The game's achievements as the in-game menu's list last read them.
    pub list: Vec<omega_cheevos::Achievement>,
    /// When to try again what the network refused, the sign-in or the game's achievements, and
    /// how many times it has been tried.
    pub retry_at: Option<Millis>,
    pub retries: u32,
}

impl App {
    /// Hardcore is on, for a game played with achievements: no state loads and no rewinding.
    pub fn hardcore(&self) -> bool {
        self.state.cheevos && self.state.cheevos_hardcore && self.cheevos.signed_in.is_some()
    }

    /// The console the game in the handheld is checked as, when achievements are on.
    pub fn achievements_console(&self) -> u32 {
        match self.state.cheevos {
            true => console_of(self.platform).unwrap_or(0),
            false => 0,
        }
    }

    /// Starts the client and signs in, as the boot does and as switching achievements on does.
    pub(crate) fn start_achievements(&mut self) {
        if !self.state.cheevos {
            return;
        }
        let Some(root) = self.root.clone() else {
            return;
        };
        omega_cheevos::start(self.state.cheevos_hardcore, env!("CARGO_PKG_VERSION"));
        omega_cheevos::set_store(&root.join(CONFIG_DIR).join(STORE_DIR));
        let Some(account) = Account::load(&root) else {
            self.cheevos.refused = Some(format!("No account in Config/{ACCOUNT_FILE}"));
            return;
        };
        match (&account.token, &account.password) {
            (Some(token), _) => omega_cheevos::login_with_token(&account.user, token),
            (None, Some(password)) => omega_cheevos::login_with_password(&account.user, password),
            (None, None) => {
                self.cheevos.refused = Some("No password or token in the account".into());
                return;
            }
        }
        self.cheevos.signing_in = true;
        self.cheevos.refused = None;
    }

    /// Switching achievements off signs out; a game already started keeps its own until it is
    /// put away.
    pub(crate) fn stop_achievements(&mut self) {
        omega_cheevos::logout();
        self.cheevos = Cheevos::default();
    }

    /// What the RetroAchievements and Account rows' note says: who is signed in, or what is
    /// missing.
    pub fn achievements_note(&self) -> String {
        if !self.state.cheevos {
            return "Off. A on Account signs in and turns it on".into();
        }
        match (&self.cheevos.signed_in, &self.cheevos.refused) {
            (Some(name), _) => format!("Signed in as {name}"),
            (None, Some(why)) => why.clone(),
            (None, None) if self.cheevos.signing_in => "Signing in…".into(),
            (None, None) if self.cheevos.retry_at.is_some() => {
                "No network yet: trying again".into()
            }
            (None, None) => "A on Account signs in".into(),
        }
    }

    /// The keyboard's name and password: kept on the card, achievements switched on, and the
    /// sign-in under way, the password to be swapped for a token once it is let in.
    pub(crate) fn sign_in_achievements(&mut self, user: &str, password: &str) {
        let Some(root) = self.root.clone() else {
            return;
        };
        if let Err(e) = Account::keep_password(&root, user, password) {
            eprintln!("omega-zero: {ACCOUNT_FILE}: {e}");
        }
        omega_cheevos::logout();
        self.cheevos = Cheevos::default();
        self.state.cheevos = true;
        self.persist();
        self.start_achievements();
    }

    /// Y on Account: signs out and forgets the account.
    pub(crate) fn sign_out_achievements(&mut self) {
        if let Some(root) = &self.root {
            Account::forget(root);
        }
        self.stop_achievements();
        let now = self.now();
        self.hud.toast(Toast::Cheevos("Signed out"), now);
    }

    /// Whether there is an account to sign out of, for the bar's Y SIGN OUT.
    pub fn achievements_account(&self) -> bool {
        self.root.as_deref().and_then(Account::load).is_some()
    }

    /// Follows the client: hands it the network's answers, and tells the player what happened.
    pub(crate) fn tick_achievements(&mut self) {
        if !self.state.cheevos {
            return;
        }
        omega_cheevos::pump();
        let now = self.now();
        if self.cheevos.retry_at.is_some_and(|at| now >= at) {
            self.cheevos.retry_at = None;
            self.retry_achievements();
        }
        if self.cheevos.signing_in {
            match omega_cheevos::login() {
                omega_cheevos::Login::In { name, token } => {
                    self.cheevos.signing_in = false;
                    self.cheevos.signed_in = Some(name.clone());
                    self.cheevos.retries = 0;
                    self.cheevos.retry_at = None;
                    // in after the game began: its achievements load now
                    if self.achievements_wanted() {
                        self.reload_achievements();
                    }
                    if let (Some(root), Some(account)) =
                        (&self.root, self.root.as_deref().and_then(Account::load))
                    {
                        // the password is swapped for the token, once
                        if account.password.is_some() && !token.is_empty() {
                            if let Err(e) = Account::keep_token(root, &account.user, &token) {
                                eprintln!("omega-zero: {ACCOUNT_FILE}: {e}");
                            }
                        }
                    }
                    self.hud
                        .toast(Toast::Cheevos(shown(&format!("Signed in as {name}"))), now);
                }
                omega_cheevos::Login::Unreachable(why) => {
                    self.cheevos.signing_in = false;
                    eprintln!("omega-zero: cheevos: no network to sign in: {why}");
                    if self.cheevos.retries == 0 {
                        self.hud.toast(Toast::Cheevos("Waiting for network"), now);
                    }
                    self.retry_later(now);
                }
                omega_cheevos::Login::Refused(why) => {
                    self.cheevos.signing_in = false;
                    self.cheevos.refused = Some(why.clone());
                    eprintln!("omega-zero: cheevos: sign-in refused: {why}");
                    self.hud.toast(Toast::Cheevos("Sign-in failed"), now);
                }
                _ => {}
            }
        }
        // the game's achievements could not load for want of a network: tried again
        if self.cheevos.signed_in.is_some()
            && self.cheevos.retry_at.is_none()
            && omega_cheevos::game_unreachable()
        {
            self.retry_later(now);
        }
        if !self.cheevos.told_game {
            if let Some(s) = omega_cheevos::summary() {
                self.cheevos.told_game = true;
                omega_cheevos::trace(&format!(
                    "{}: {} of {} unlocked",
                    s.title, s.unlocked, s.total
                ));
                if s.total > 0 {
                    let line = format!("{} of {} unlocked", s.unlocked, s.total);
                    self.hud.toast(Toast::Cheevos(shown(&line)), now);
                }
            }
        }
        for event in omega_cheevos::take_events() {
            use omega_cheevos::EventKind as E;
            let toast = match event.kind {
                E::Unlocked => Toast::Achievement(shown(&event.title)),
                E::Mastered => Toast::Cheevos(shown(&format!("Mastered {}", event.title))),
                E::LeaderboardSubmitted => {
                    Toast::Cheevos(shown(&format!("{}: {}", event.title, event.description)))
                }
                E::Disconnected => Toast::Cheevos("Unlocks waiting for network"),
                E::Reconnected => Toast::Cheevos("Unlocks sent"),
                E::Synced => match event.points {
                    1 => Toast::Cheevos("1 offline unlock sent"),
                    n => Toast::Cheevos(shown(&format!("{n} offline unlocks sent"))),
                },
                E::ServerError => {
                    eprintln!(
                        "omega-zero: cheevos: {}: {}",
                        event.title, event.description
                    );
                    continue;
                }
                E::LeaderboardStarted | E::LeaderboardFailed => continue,
            };
            self.hud.toast(toast, now);
        }
    }

    /// A game goes in: its summary is told once its achievements load.
    pub(crate) fn achievements_game_starts(&mut self) {
        self.cheevos.told_game = false;
    }

    /// Tries again, after the wait the tries so far have earned.
    fn retry_later(&mut self, now: Millis) {
        self.cheevos.retry_at = Some(now + retry_after(self.cheevos.retries));
        self.cheevos.retries = self.cheevos.retries.saturating_add(1);
    }

    /// What the network refused, tried again: the sign-in, or the game's achievements.
    fn retry_achievements(&mut self) {
        match self.cheevos.signed_in.is_some() {
            false => {
                let retries = self.cheevos.retries;
                self.start_achievements();
                self.cheevos.retries = retries;
            }
            true if omega_cheevos::game_unreachable() => self.reload_achievements(),
            true => {}
        }
    }

    /// A game is in play whose achievements are not loaded, nor loading.
    fn achievements_wanted(&self) -> bool {
        matches!(self.phase, crate::app::Phase::Playing { .. })
            && self.achievements_console() != 0
            && !matches!(
                omega_cheevos::game_load(),
                omega_cheevos::GameLoad::Loaded | omega_cheevos::GameLoad::Pending
            )
    }

    /// Loads the achievements of the game in play again.
    fn reload_achievements(&mut self) {
        let Some(rom) = self.seated_cart().map(|c| c.rom.clone()) else {
            return;
        };
        self.cheevos.told_game = false;
        omega_cheevos::reload_game(self.achievements_console(), &rom);
    }

    /// Hardcore switched: the client takes it at once. Switched on in a game, the game starts
    /// over, as RetroAchievements asks, so nothing done before it counts.
    pub(crate) fn switch_hardcore(&mut self) {
        if !self.state.cheevos {
            return;
        }
        omega_cheevos::start(self.state.cheevos_hardcore, env!("CARGO_PKG_VERSION"));
        if self.state.cheevos_hardcore && matches!(self.phase, crate::app::Phase::Playing { .. }) {
            self.restart_game();
        }
    }

    /// The game in play as RetroAchievements has it, once it is found there.
    pub fn achievements_found(&self) -> Option<omega_cheevos::Summary> {
        if !self.state.cheevos || self.cheevos.signed_in.is_none() {
            return None;
        }
        omega_cheevos::summary().filter(|s| s.id != 0)
    }

    /// Whether the game in play was found on RetroAchievements, in a word for the in-game
    /// menu's Game row, and its note: the title and number it was taken for, or why not.
    pub fn achievements_game(&self) -> (String, String) {
        let (value, note) = if !self.state.cheevos {
            ("OFF", "Switch RetroAchievements on below".to_string())
        } else if self.cheevos.signed_in.is_none() {
            ("SIGNED OUT", self.achievements_note())
        } else if let Some(s) = self.achievements_found() {
            ("FOUND", format!("{} (game {})", s.title, s.id))
        } else {
            match omega_cheevos::game_load() {
                omega_cheevos::GameLoad::Pending => (
                    "LOOKING",
                    "Asking RetroAchievements for this game".to_string(),
                ),
                omega_cheevos::GameLoad::Refused(why) if !why.is_empty() => ("NOT FOUND", why),
                omega_cheevos::GameLoad::Refused(_) | omega_cheevos::GameLoad::Loaded => (
                    "NOT FOUND",
                    "RetroAchievements does not know this ROM".to_string(),
                ),
                omega_cheevos::GameLoad::None => (
                    "NOT STARTED",
                    "Achievements start with the next game".to_string(),
                ),
            }
        };
        (value.to_string(), note)
    }

    /// Reads the game's achievements again, for the in-game menu's list.
    pub(crate) fn read_achievements(&mut self) {
        self.cheevos.list = match self.achievements_found() {
            Some(_) => omega_cheevos::achievements(),
            None => Vec::new(),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_account_takes_a_password_once_and_keeps_the_token() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join(CONFIG_DIR)).unwrap();
        let file = d.path().join(CONFIG_DIR).join(ACCOUNT_FILE);
        assert_eq!(Account::load(d.path()), None);
        std::fs::write(&file, "username = Ash\r\npassword = pikachu\r\n").unwrap();
        assert_eq!(
            Account::load(d.path()),
            Some(Account {
                user: "Ash".into(),
                password: Some("pikachu".into()),
                token: None
            })
        );
        Account::keep_token(d.path(), "Ash", "t0k3n").unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(!text.contains("pikachu"), "the password was kept: {text}");
        assert_eq!(
            Account::load(d.path()),
            Some(Account {
                user: "Ash".into(),
                password: None,
                token: Some("t0k3n".into())
            })
        );
    }

    #[test]
    fn the_keyboards_password_is_kept_until_the_token_and_signing_out_forgets_it() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join(CONFIG_DIR)).unwrap();
        Account::keep_password(d.path(), "Misty", "starmie").unwrap();
        assert_eq!(
            Account::load(d.path()),
            Some(Account {
                user: "Misty".into(),
                password: Some("starmie".into()),
                token: None
            })
        );
        Account::forget(d.path());
        assert_eq!(Account::load(d.path()), None);
    }

    #[test]
    fn the_handhelds_and_the_snes_have_achievements_and_the_playstation_none_here() {
        assert_eq!(console_of(Platform::Gba), Some(5));
        assert_eq!(console_of(Platform::Gb), Some(4));
        assert_eq!(console_of(Platform::Gbc), Some(6));
        assert_eq!(console_of(Platform::Snes), Some(3));
        assert_eq!(console_of(Platform::Ps1), None);
        assert_eq!(console_of(Platform::Psp), Some(41));
    }

    #[test]
    fn a_refused_sign_in_waits_longer_each_time_up_to_a_minute() {
        let waits: Vec<Millis> = (0..6).map(retry_after).collect();
        assert_eq!(waits, [5_000, 10_000, 20_000, 40_000, 60_000, 60_000]);
        assert_eq!(retry_after(u32::MAX), RETRY_MAX_MS);
    }

    #[test]
    fn a_long_title_is_cut_for_its_toast() {
        let t = shown("The Legend of Zelda: A Link to the Past and Four Swords");
        assert_eq!(t.chars().count(), TOAST_CHARS);
        assert!(t.ends_with('…'));
        assert_eq!(shown("Short"), "Short");
    }
}
