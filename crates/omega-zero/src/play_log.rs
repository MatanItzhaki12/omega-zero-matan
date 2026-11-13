//! How long each game has been played, and when it was last: kept in `Config/play_log.txt`, a
//! line a game, `gba/<stem>`, its seconds played and when it was last played (seconds since
//! 1970), split by tabs. Settings › Saves shows both.
//!
//! The time counts while a game is in the slot and on screen, not while its menu or the save
//! states are up over it. It is kept in memory as it is played and written as the cart comes
//! out, as the game autosaves and as the console powers off, so a write never lands mid-frame.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use omega_store::Platform;

/// The log, under the card's root.
pub const FILE: &str = "Config/play_log.txt";

/// A game's time played, and when it was last played.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Played {
    pub secs: u64,
    /// Seconds since 1970; none for a game in the log from before it kept this.
    pub last: Option<u64>,
}

/// Every game's time played.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayLog {
    games: HashMap<String, Played>,
    /// Played since the log was last written.
    dirty: bool,
}

fn key(platform: Platform, stem: &str) -> String {
    format!("{}/{stem}", platform.dir_name().to_ascii_lowercase())
}

fn path(root: &Path) -> PathBuf {
    root.join(FILE)
}

impl PlayLog {
    /// The log on the card at `root`; empty where there is none, or a line it cannot read.
    pub fn read(root: &Path) -> PlayLog {
        let text = std::fs::read_to_string(path(root)).unwrap_or_default();
        let games = text
            .lines()
            .filter_map(|line| {
                let mut parts = line.trim_end_matches('\r').split('\t');
                let game = parts.next().filter(|g| g.contains('/'))?;
                let secs = parts.next()?.parse().ok()?;
                let last = parts.next().and_then(|t| t.parse().ok());
                Some((game.to_string(), Played { secs, last }))
            })
            .collect();
        PlayLog {
            games,
            dirty: false,
        }
    }

    /// The log as its file holds it, the games in order.
    pub fn text(&self) -> String {
        let mut games: Vec<_> = self.games.iter().collect();
        games.sort_by(|a, b| a.0.cmp(b.0));
        games.into_iter().fold(String::new(), |mut out, (game, p)| {
            let _ = match p.last {
                Some(last) => writeln!(out, "{game}\t{}\t{last}", p.secs),
                None => writeln!(out, "{game}\t{}", p.secs),
            };
            out
        })
    }

    /// Writes the log to the card at `root`, through the card's writer, where it was played
    /// since.
    pub fn write(&mut self, root: &Path) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        if let Err(e) = crate::card_writer::write(&path(root), self.text().into_bytes()) {
            crate::log!("play log: {e}");
        }
    }

    /// How long `stem` has been played, and when last.
    pub fn of(&self, platform: Platform, stem: &str) -> Option<Played> {
        self.games.get(&key(platform, stem)).copied()
    }

    /// `secs` more of `stem` played, last at `now`.
    pub fn add(&mut self, platform: Platform, stem: &str, secs: u64, now: u64) {
        let p = self.games.entry(key(platform, stem)).or_default();
        p.secs += secs;
        p.last = Some(now);
        self.dirty = true;
    }
}

/// A time played, as Saves says it: `under a minute`, `25 min`, `3 h 5 min`, `41 h`.
pub fn played_text(secs: u64) -> String {
    let (h, m) = (secs / 3600, secs % 3600 / 60);
    match (h, m) {
        (0, 0) => "under a minute".to_string(),
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, _) if h >= 10 => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_played_reads_in_hours_and_minutes() {
        assert_eq!(played_text(30), "under a minute");
        assert_eq!(played_text(25 * 60), "25 min");
        assert_eq!(played_text(3 * 3600 + 5 * 60), "3 h 5 min");
        assert_eq!(played_text(2 * 3600), "2 h");
        assert_eq!(played_text(41 * 3600 + 59 * 60), "41 h");
    }

    #[test]
    fn the_log_is_read_back_as_it_was_written() {
        let mut log = PlayLog::default();
        log.add(Platform::Gba, "Emerald", 90, 1_700_000_000);
        log.add(Platform::Gba, "Emerald", 30, 1_700_000_100);
        log.add(Platform::Psp, "Patapon", 5, 1_700_000_200);
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("Config")).unwrap();
        std::fs::write(path(d.path()), log.text()).unwrap();
        let read = PlayLog::read(d.path());
        assert_eq!(
            read.of(Platform::Gba, "Emerald"),
            Some(Played {
                secs: 120,
                last: Some(1_700_000_100)
            })
        );
        assert_eq!(read.of(Platform::Psp, "Patapon").map(|p| p.secs), Some(5));
        assert_eq!(read.of(Platform::Gba, "Fusion"), None);
    }

    #[test]
    fn a_line_from_before_last_played_was_kept_still_reads() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("Config")).unwrap();
        std::fs::write(path(d.path()), "gba/Emerald\t600\nnot a line\n").unwrap();
        let read = PlayLog::read(d.path());
        assert_eq!(
            read.of(Platform::Gba, "Emerald"),
            Some(Played {
                secs: 600,
                last: None
            })
        );
    }
}
