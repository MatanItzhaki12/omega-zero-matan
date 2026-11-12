//! Settings > Save Backup: the SAVE BACKUP panel, and the two it opens, FIND NAS and FOLDER.
//!
//! SAVE BACKUP switches it on, and holds the NAS, who signs in to it, their password, the folder
//! saves go to, and Back Up Now with where the copying stands. FIND NAS lists the NASes that
//! answer on this Wi-Fi, and a row to type one's address. FOLDER lists the NAS's shares, then
//! the folders in the one opened: A opens one, B goes back up, Use This Folder picks the one
//! open, and New Folder makes one there on the keyboard.
//!
//! The NAS is asked on threads of their own, a job each, and their answers taken as they come
//! (`poll_backup`), so the panel never waits on the network.

use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::Duration;

use omega_ui::Toast;

use crate::app::{App, Phase};
use crate::nas::{Found, Login, Nas, NasFail};
use crate::nav::Dir;
use crate::options::{OptionRow, OptionsPage, OptionsScreen};
use crate::save_backup::{Differs, SaveBackup, Status, Target};

/// One question to the NAS, answered on a thread of its own.
pub struct Job<T> {
    rx: Receiver<T>,
}

impl<T: Send + 'static> Job<T> {
    pub fn spawn(f: impl FnOnce() -> T + Send + 'static) -> Job<T> {
        let (tx, rx) = channel();
        let _ = std::thread::Builder::new()
            .name("omega-nas".into())
            .spawn(move || {
                let _ = tx.send(f());
            });
        Job { rx }
    }

    /// The answer, once it has come; a job whose thread died answers with `gone`.
    fn poll(&self, gone: impl FnOnce() -> T) -> Option<T> {
        match self.rx.try_recv() {
            Ok(answer) => Some(answer),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(gone()),
        }
    }
}

/// FIND NAS while it is open: the NASes found, and the look under way.
#[derive(Default)]
pub struct Finding {
    pub found: Vec<Found>,
    job: Option<Job<std::io::Result<Vec<Found>>>>,
    /// The handheld is on no network.
    offline: bool,
}

/// FOLDER while it is open: the share and the folders in it open, what is in the folder open,
/// and the question to the NAS under way.
#[derive(Default)]
pub struct Browsing {
    pub share: Option<String>,
    pub path: Vec<String>,
    pub entries: Vec<String>,
    job: Option<Job<Result<Vec<String>, NasFail>>>,
    fail: Option<NasFail>,
}

impl Browsing {
    /// The folder open, as a path in its share.
    pub fn dir(&self) -> String {
        self.path.join("/")
    }

    /// The folder open as the panel prints it: the share, and the path in it.
    pub fn shown(&self) -> String {
        match &self.share {
            Some(share) => [share.as_str()]
                .into_iter()
                .chain(self.path.iter().map(String::as_str))
                .collect::<Vec<_>>()
                .join("/"),
            None => String::new(),
        }
    }
}

/// FROM BACKUP while it is open: the saves on the NAS unlike the handheld's, the comparing under
/// way, and the one A was pressed on once, which a second A copies.
#[derive(Default)]
pub struct Restoring {
    pub found: Vec<Differs>,
    job: Option<Job<Result<Vec<Differs>, NasFail>>>,
    fail: Option<NasFail>,
    armed: Option<u16>,
}

/// What Save Backup's panels are doing.
#[derive(Default)]
pub struct BackupUi {
    pub finding: Finding,
    pub browsing: Browsing,
    pub restoring: Restoring,
}

/// How often, at most, Save Backup asks Wi-Fi to join again while saves wait on a network.
const WAKE_EVERY_MS: omega_ui::Millis = 30_000;

/// How many characters of a value a row has room for beside its name.
const VALUE_CHARS: usize = 18;

/// `text` as a row has room for: its end kept, where a path's last folder is, its start cut.
pub fn fit_end(text: &str) -> String {
    let n = text.chars().count();
    match n > VALUE_CHARS {
        true => format!(
            "…{}",
            text.chars().skip(n - (VALUE_CHARS - 1)).collect::<String>()
        ),
        false => text.to_string(),
    }
}

impl App {
    /// Save Backup's thread, for the card at the root: started at boot.
    pub(crate) fn start_save_backup(&mut self) {
        if let Some(root) = &self.root {
            self.save_backup = Some(SaveBackup::start(root.clone(), self.backup_target()));
        }
    }

    /// Where saves go, while Save Backup is on and has a NAS and a share to go to.
    pub fn backup_target(&self) -> Option<Target> {
        let s = &self.state;
        (s.backup && !s.backup_host.is_empty() && !s.backup_share.is_empty()).then(|| Target {
            login: self.backup_login(),
            share: s.backup_share.clone(),
            dir: s.backup_dir.clone(),
        })
    }

    fn backup_login(&self) -> Login {
        Login {
            host: self.state.backup_host.clone(),
            user: self.state.backup_user.clone(),
            password: self.state.backup_password.clone(),
        }
    }

    /// The settings changed: the thread copies where they say from now on.
    fn backup_retarget(&mut self) {
        let target = self.backup_target();
        if let Some(backup) = &self.save_backup {
            backup.set_target(target);
        }
    }

    /// Saves were written, or the network came back: the thread looks for saves to copy.
    pub(crate) fn backup_pass(&self) {
        if let Some(backup) = &self.save_backup {
            backup.pass();
        }
    }

    pub fn backup_status(&self) -> Status {
        self.save_backup
            .as_ref()
            .map(SaveBackup::status)
            .unwrap_or_default()
    }

    /// Each frame: the answers of the NAS's jobs, and a pass once a network is joined.
    pub(crate) fn poll_backup(&mut self) {
        let joined = self.wifi.status().joined();
        if joined && !self.backup_joined {
            self.backup_pass();
        }
        self.backup_joined = joined;
        self.toast_backup();
        self.wake_for_backup();
        let finding = &mut self.backup_ui.finding;
        if let Some(found) = finding
            .job
            .as_ref()
            .and_then(|j| j.poll(|| Err(std::io::Error::other("the look ended"))))
        {
            finding.job = None;
            match found {
                Ok(found) => finding.found = found,
                Err(_) => finding.offline = true,
            }
        }
        let restoring = &mut self.backup_ui.restoring;
        if let Some(compared) = restoring
            .job
            .as_ref()
            .and_then(|j| j.poll(|| Err(NasFail::Other("the NAS's answer was lost".into()))))
        {
            restoring.job = None;
            match compared {
                Ok(found) => restoring.found = found,
                Err(e) => restoring.fail = Some(e),
            }
        }
        let browsing = &mut self.backup_ui.browsing;
        if let Some(listed) = browsing
            .job
            .as_ref()
            .and_then(|j| j.poll(|| Err(NasFail::Other("the NAS's answer was lost".into()))))
        {
            browsing.job = None;
            match listed {
                Ok(entries) => browsing.entries = entries,
                Err(e) => browsing.fail = Some(e),
            }
        }
    }

    /// Saves waiting on a network: Wi-Fi, while it is on, is asked to join a saved network
    /// again, every half minute at most.
    fn wake_for_backup(&mut self) {
        if !self.backup_status().offline || !self.wifi.enabled() {
            return;
        }
        let now = self.now();
        if self
            .backup_woke_at
            .is_some_and(|at| now.saturating_sub(at) < WAKE_EVERY_MS)
        {
            return;
        }
        self.backup_woke_at = Some(now);
        crate::log!("save backup: no network: asking Wi-Fi to join");
        self.wifi.wake(now);
    }

    /// A toast at the top of the screen once a pass has copied saves; and once a pass has
    /// failed, but only the first since saves last copied, so a NAS that stays away is told of
    /// once rather than at every try.
    fn toast_backup(&mut self) {
        if self.save_backup.is_none() {
            return;
        }
        let status = self.backup_status();
        let (copied, failed) = self.backup_seen;
        self.backup_seen = (status.copied, status.failed);
        if status.copied > copied {
            self.backup_fail_told = false;
            self.hud
                .toast(Toast::Backup("Saves backed up to the NAS"), self.now());
        } else if status.failed > failed && !self.backup_fail_told {
            self.backup_fail_told = true;
            let text = match status.fail {
                Some(NasFail::Login) => "Save backup failed: login refused",
                Some(NasFail::Denied) => "Save backup failed: folder refused",
                _ => "Save backup failed: NAS away, will retry",
            };
            self.hud.toast(Toast::Backup(text), self.now());
        }
    }

    /// The rows of a panel whose rows are what the NAS has.
    pub(crate) fn backup_rows(&self, page: OptionsPage) -> Option<Vec<OptionRow>> {
        match page {
            OptionsPage::Nas => Some(
                (0..self.backup_ui.finding.found.len().min(u8::MAX as usize))
                    .map(|i| OptionRow::NasFound(i as u8))
                    .chain([OptionRow::NasAddress])
                    .collect(),
            ),
            OptionsPage::Restore => Some(
                (0..self.backup_ui.restoring.found.len().min(u16::MAX as usize))
                    .map(|i| OptionRow::RestoreItem(i as u16))
                    .collect(),
            ),
            OptionsPage::Folder => {
                let b = &self.backup_ui.browsing;
                let items = (0..b.entries.len().min(u16::MAX as usize))
                    .map(|i| OptionRow::FolderItem(i as u16));
                Some(match b.share {
                    None => items.collect(),
                    Some(_) => [OptionRow::FolderUse, OptionRow::FolderNew]
                        .into_iter()
                        .chain(items)
                        .collect(),
                })
            }
            _ => None,
        }
    }

    /// A row's name, where it is the NAS's or a folder's.
    pub(crate) fn backup_label(&self, row: OptionRow) -> Option<String> {
        match row {
            OptionRow::NasFound(i) => self
                .backup_ui
                .finding
                .found
                .get(i as usize)
                .map(Found::shown),
            OptionRow::FolderItem(i) => self.backup_ui.browsing.entries.get(i as usize).cloned(),
            OptionRow::RestoreItem(i) => self
                .backup_ui
                .restoring
                .found
                .get(i as usize)
                .map(Differs::name),
            _ => None,
        }
    }

    /// What a Save Backup row is set to, as its panel prints it.
    pub(crate) fn backup_text(&self, row: OptionRow) -> String {
        let s = &self.state;
        match row {
            OptionRow::BackupOn => if s.backup { "ON" } else { "OFF" }.to_string(),
            OptionRow::BackupNas => match (s.backup_name.as_str(), s.backup_host.as_str()) {
                (_, "") => "FIND".to_string(),
                ("", host) => fit_end(host),
                (name, _) => fit_end(name),
            },
            OptionRow::BackupUser if s.backup_user.is_empty() => "GUEST".to_string(),
            OptionRow::BackupUser => fit_end(&s.backup_user),
            OptionRow::BackupPassword if s.backup_password.is_empty() => "NONE".to_string(),
            OptionRow::BackupPassword => "SET".to_string(),
            OptionRow::BackupFolder if s.backup_share.is_empty() => "PICK".to_string(),
            OptionRow::BackupFolder => fit_end(
                &[s.backup_share.as_str(), s.backup_dir.as_str()]
                    .into_iter()
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>()
                    .join("/"),
            ),
            OptionRow::BackupNow => self.backup_word().to_string(),
            OptionRow::NasFound(i) => self
                .backup_ui
                .finding
                .found
                .get(i as usize)
                .filter(|f| f.name.is_some())
                .map(|f| f.addr.to_string())
                .unwrap_or_default(),
            OptionRow::FolderUse => fit_end(&self.backup_ui.browsing.shown()),
            OptionRow::RestoreItem(i) => match self.backup_ui.restoring.found.get(i as usize) {
                Some(d) if d.on_card => d.system().to_string(),
                Some(_) => "NEW".to_string(),
                None => String::new(),
            },
            _ => String::new(),
        }
    }

    /// Back Up Now's value: off, waiting to be set up, copying, or where it stands.
    pub(crate) fn backup_word(&self) -> String {
        if !self.state.backup {
            return "OFF".to_string();
        }
        if self.backup_target().is_none() {
            return "SET UP".to_string();
        }
        let status = self.backup_status();
        match (status.busy, status.waiting) {
            (true, _) => "COPYING".to_string(),
            (false, n) if n > 0 && status.offline => "NO WI-FI".to_string(),
            (false, 0) if status.done_at.is_some() => "UP TO DATE".to_string(),
            (false, 0) => "READY".to_string(),
            (false, n) => format!("{n} WAITING"),
        }
    }

    /// A Save Backup row's note, where it says more than the row's own.
    pub(crate) fn backup_note(&self, row: OptionRow) -> Option<String> {
        let finding = &self.backup_ui.finding;
        let browsing = &self.backup_ui.browsing;
        let restoring = &self.backup_ui.restoring;
        Some(match row {
            OptionRow::RestoreItem(_) if restoring.job.is_some() => {
                "Comparing with the NAS".to_string()
            }
            OptionRow::RestoreItem(_) if restoring.fail.is_some() => restoring
                .fail
                .as_ref()
                .map_or("", NasFail::line)
                .to_string(),
            OptionRow::RestoreItem(_) if restoring.found.is_empty() => {
                "Every save matches the NAS".to_string()
            }
            OptionRow::RestoreItem(i) if restoring.armed == Some(i) => {
                "A again replaces the handheld's".to_string()
            }
            OptionRow::RestoreItem(i) => match restoring.found.get(i as usize) {
                Some(d) if d.on_card => "Unlike the handheld's: A copies it".to_string(),
                _ => "Not on the handheld: A copies it".to_string(),
            },
            OptionRow::BackupNow => {
                let status = self.backup_status();
                if !self.state.backup {
                    "Switch Save Backup on first".to_string()
                } else if self.backup_target().is_none() {
                    "Pick a NAS and a folder first".to_string()
                } else if status.offline && status.waiting > 0 && !self.wifi.enabled() {
                    "Wi-Fi is off: saves wait for it".to_string()
                } else if status.offline && status.waiting > 0 {
                    "Saves wait for Wi-Fi to join".to_string()
                } else if let Some(fail) = &status.fail {
                    format!("{}: saves wait", fail.line())
                } else if let Some(at) = status.done_at {
                    format!("Copied {}", ago(at))
                } else {
                    "A copies every save now".to_string()
                }
            }
            OptionRow::NasFound(_) | OptionRow::NasAddress if finding.job.is_some() => {
                "Looking on this Wi-Fi".to_string()
            }
            OptionRow::NasFound(_) | OptionRow::NasAddress if finding.offline => {
                "Join a Wi-Fi network first".to_string()
            }
            OptionRow::NasAddress if finding.found.is_empty() => {
                "None found: type its address".to_string()
            }
            OptionRow::FolderUse | OptionRow::FolderNew | OptionRow::FolderItem(_)
                if browsing.job.is_some() =>
            {
                "Opening the NAS".to_string()
            }
            OptionRow::FolderUse | OptionRow::FolderNew | OptionRow::FolderItem(_)
                if browsing.fail.is_some() =>
            {
                browsing.fail.as_ref().map_or("", NasFail::line).to_string()
            }
            OptionRow::FolderItem(_) if browsing.share.is_none() => "A opens the share".to_string(),
            _ => return None,
        })
    }

    /// A on a Save Backup row.
    pub(crate) fn backup_press(&mut self, row: OptionRow) {
        match row {
            OptionRow::BackupOn => self.change_backup(row),
            OptionRow::BackupNas => self.open_find_nas(),
            OptionRow::BackupFolder if self.state.backup_host.is_empty() => self.refuse(),
            OptionRow::BackupFolder => self.open_folders(),
            OptionRow::BackupNow if self.backup_target().is_none() => self.refuse(),
            OptionRow::BackupNow => self.backup_pass(),
            OptionRow::BackupRestore => self.open_restore(),
            OptionRow::RestoreItem(i) => self.restore_press(i),
            OptionRow::NasFound(i) => {
                let Some(found) = self.backup_ui.finding.found.get(i as usize).cloned() else {
                    return self.refuse();
                };
                self.pick_nas(found.addr.to_string(), found.name.unwrap_or_default());
            }
            OptionRow::FolderUse => {
                let b = &self.backup_ui.browsing;
                let Some(share) = b.share.clone() else {
                    return self.refuse();
                };
                self.state.backup_share = share;
                self.state.backup_dir = b.dir();
                self.persist();
                self.backup_retarget();
                self.back_to_backup(OptionRow::BackupNow);
            }
            OptionRow::FolderItem(i) => {
                let Some(name) = self.backup_ui.browsing.entries.get(i as usize).cloned() else {
                    return self.refuse();
                };
                let b = &mut self.backup_ui.browsing;
                match b.share {
                    None => b.share = Some(name),
                    Some(_) => b.path.push(name),
                }
                self.list_folder(None);
                self.light_option(OptionRow::FolderUse);
            }
            _ => {}
        }
    }

    /// Left or right on a Save Backup row: Save Backup itself, on or off.
    pub(crate) fn change_backup(&mut self, row: OptionRow) {
        if row != OptionRow::BackupOn {
            return;
        }
        self.state.backup = !self.state.backup;
        self.persist();
        self.backup_retarget();
        if self.state.backup {
            self.backup_pass();
        }
    }

    /// B on FOLDER: up from the folder open, to the shares from a share's top, and from the
    /// shares back to SAVE BACKUP. False once there is nowhere further up in FOLDER.
    pub(crate) fn folder_back(&mut self) -> bool {
        let b = &mut self.backup_ui.browsing;
        if b.path.pop().is_some() {
            self.list_folder(None);
            return true;
        }
        if b.share.take().is_some() {
            self.list_folder(None);
            return true;
        }
        false
    }

    /// What typing on the keyboard for Save Backup set: the user, the password, a NAS's
    /// address, or a new folder's name.
    pub(crate) fn backup_typed(&mut self, step: crate::options::Step, typed: &str) {
        use crate::options::Step;
        match step {
            Step::BackupUser => self.state.backup_user = omega_store::kept_text(typed.trim()),
            Step::BackupPassword => self.state.backup_password = omega_store::kept_text(typed),
            Step::NasAddress => {
                let host = omega_store::kept_text(typed.trim());
                if host.is_empty() {
                    return;
                }
                return self.pick_nas(host, String::new());
            }
            Step::NewFolder => {
                let name: String = typed
                    .trim()
                    .chars()
                    .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
                    .collect();
                if name.is_empty() || name == "." || name == ".." {
                    return self.refuse();
                }
                return self.list_folder(Some(name));
            }
            _ => return,
        }
        self.persist();
        self.backup_retarget();
    }

    /// The NAS at `host`, called `name`, picked: its share and folder are another NAS's no
    /// more, so FOLDER is next.
    fn pick_nas(&mut self, host: String, name: String) {
        let s = &mut self.state;
        if s.backup_host != host {
            s.backup_share.clear();
            s.backup_dir.clear();
        }
        s.backup_host = host;
        s.backup_name = name;
        self.persist();
        self.backup_retarget();
        self.back_to_backup(OptionRow::BackupUser);
    }

    fn back_to_backup(&mut self, row: OptionRow) {
        self.go(
            Phase::Options(OptionsScreen {
                row,
                ..OptionsScreen::new(OptionsPage::Backup)
            }),
            Dir::Back,
        );
    }

    fn light_option(&mut self, row: OptionRow) {
        if let Phase::Options(screen) = &mut self.phase {
            screen.row = row;
        }
    }

    /// FROM BACKUP, the NAS asked which of its saves differ from the handheld's. The NAS and
    /// folder are the ones set up, whether Save Backup is on or not.
    fn open_restore(&mut self) {
        let s = &self.state;
        if s.backup_host.is_empty() || s.backup_share.is_empty() {
            return self.refuse();
        }
        let (Some(root), to) = (
            self.root.clone(),
            Target {
                login: self.backup_login(),
                share: s.backup_share.clone(),
                dir: s.backup_dir.clone(),
            },
        ) else {
            return self.refuse();
        };
        self.backup_ui.restoring = Restoring {
            job: Some(Job::spawn(move || {
                crate::save_backup::differing(&root, &to)
            })),
            ..Restoring::default()
        };
        self.go(
            Phase::Options(OptionsScreen {
                row: OptionRow::RestoreItem(0),
                ..OptionsScreen::new(OptionsPage::Restore)
            }),
            Dir::Deeper,
        );
    }

    /// A on a save in FROM BACKUP: the first press asks, the second puts it on the handheld.
    fn restore_press(&mut self, i: u16) {
        let restoring = &mut self.backup_ui.restoring;
        let Some(from) = restoring.found.get(i as usize).cloned() else {
            return self.refuse();
        };
        if restoring.armed != Some(i) {
            restoring.armed = Some(i);
            return;
        }
        restoring.armed = None;
        let Some(root) = self.root.clone() else {
            return self.refuse();
        };
        let stamp = omega_store::format_stamp(self.wall_secs());
        match crate::save_backup::restore(&root, &from, &stamp) {
            Ok(()) => {
                self.backup_ui.restoring.found.remove(i as usize);
                self.hud
                    .toast(Toast::Backup("Save copied from the NAS"), self.now());
            }
            Err(e) => {
                crate::log!("sync from backup: {}: {e}", from.rel);
                self.refuse();
            }
        }
    }

    /// FIND NAS, the look on the network begun.
    fn open_find_nas(&mut self) {
        self.backup_ui.finding = Finding {
            job: Some(Job::spawn(|| crate::nas::find(Duration::from_millis(700)))),
            ..Finding::default()
        };
        self.go(
            Phase::Options(OptionsScreen {
                row: OptionRow::NasAddress,
                ..OptionsScreen::new(OptionsPage::Nas)
            }),
            Dir::Deeper,
        );
    }

    /// FOLDER, at the folder picked before, the NAS asked what is there.
    fn open_folders(&mut self) {
        let s = &self.state;
        self.backup_ui.browsing = Browsing {
            share: (!s.backup_share.is_empty()).then(|| s.backup_share.clone()),
            path: s
                .backup_dir
                .split('/')
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect(),
            ..Browsing::default()
        };
        self.list_folder(None);
        let row = match self.backup_ui.browsing.share {
            Some(_) => OptionRow::FolderUse,
            None => OptionRow::FolderItem(0),
        };
        self.go(
            Phase::Options(OptionsScreen {
                row,
                ..OptionsScreen::new(OptionsPage::Folder)
            }),
            Dir::Deeper,
        );
    }

    /// Asks the NAS what is in the folder open: the shares, at the top. With `make`, a folder
    /// of that name is made there first, and opened.
    fn list_folder(&mut self, make: Option<String>) {
        let login = self.backup_login();
        let b = &mut self.backup_ui.browsing;
        if let Some(name) = &make {
            b.path.push(name.clone());
        }
        let (share, dir) = (b.share.clone(), b.dir());
        b.entries.clear();
        b.fail = None;
        b.job = Some(Job::spawn(move || {
            let mut nas = Nas::open(&login)?;
            match share {
                None => nas.shares(),
                Some(share) => {
                    if make.is_some() {
                        nas.make_folder(&share, &dir)?;
                    }
                    nas.folders(&share, &dir)
                }
            }
        }));
    }
}

/// How long ago `at`, in seconds since 1970, was, as a note says it.
pub(crate) fn ago(at: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(at, |d| d.as_secs());
    match now.saturating_sub(at) / 60 {
        0 => "just now".to_string(),
        1 => "a minute ago".to_string(),
        m if m < 60 => format!("{m} minutes ago"),
        m if m < 120 => "an hour ago".to_string(),
        m if m < 48 * 60 => format!("{} hours ago", m / 60),
        m => format!("{} days ago", m / (24 * 60)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_value_keeps_its_end() {
        assert_eq!(fit_end("Backups"), "Backups");
        let long = "Backups/Handhelds/RG SP/Saves";
        let fitted = fit_end(long);
        assert_eq!(fitted.chars().count(), VALUE_CHARS);
        assert!(fitted.ends_with("RG SP/Saves"), "{fitted}");
    }

    #[test]
    fn a_browsed_folder_reads_as_its_share_and_path() {
        let b = Browsing {
            share: Some("Backups".into()),
            path: vec!["RG SP".into(), "Saves".into()],
            ..Browsing::default()
        };
        assert_eq!(b.shown(), "Backups/RG SP/Saves");
        assert_eq!(b.dir(), "RG SP/Saves");
        assert_eq!(Browsing::default().shown(), "");
    }
}
