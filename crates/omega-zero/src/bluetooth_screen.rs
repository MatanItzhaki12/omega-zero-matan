//! The Bluetooth screen, opened from Settings: switching Bluetooth on and off, the devices paired,
//! and those in reach once X has looked for them, which it pairs with, connects, disconnects and
//! forgets.

use omega_input::{Action, Btn, Millis};
use omega_ui::network::{network_window, networks_shown};

use crate::bluetooth::{
    BluetoothJob, BluetoothJobs, BluetoothNews, Device, DeviceKind, Fail, Status, LOOK_MS,
    REFRESH_MS,
};
use crate::wifi_screen::SettingLine;

/// How often the status is asked for while nothing on screen names a device, on top of
/// `REFRESH_MS`.
pub const IDLE_REFRESH_MS: Millis = 8_000;

/// How long a pairing or a connection that failed is said under the list.
pub const NOTE_MS: Millis = 5_000;
/// How long a job for a device may take before it is called failed. The script gives up on a
/// pairing, its trust and its connection well within this.
pub const DOING_MS: Millis = 90_000;

/// How closely the radio is being watched, which sets how often it is asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Watch {
    Idle,
    /// Something on screen names the device connected: the status is kept fresh.
    Status,
    /// The Bluetooth screen is open: X may look for devices, and what a look finds is kept until
    /// it is left.
    Devices,
}

/// What went wrong last, said under the list: a radio that did not come up until Bluetooth is
/// switched again, a device that would not pair or connect for a moment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Note {
    NoRadio,
    /// The device's name, or its address, as `Device::shown` has it.
    NoPair(String),
    NoConnect(String),
}

/// What the handheld is doing with a device, asked from the screen and not yet answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Doing {
    Pairing,
    Connecting,
    Disconnecting,
    Forgetting,
}

/// A device as the list shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceState {
    Connected,
    Connecting,
    Pairing,
    Paired,
    /// Found by a look and never paired.
    Found,
}

/// A device the list offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub mac: String,
    /// Its name in its own case, or the address of a paired device with no name.
    pub name: String,
    pub kind: DeviceKind,
    pub paired: bool,
    pub state: DeviceState,
}

/// What Settings says beside Bluetooth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Summary {
    Off,
    On,
    Connected(String),
}

/// The radio as the app knows it, and the jobs that change it.
pub struct Bluetooth {
    jobs: Box<dyn BluetoothJobs>,
    enabled: bool,
    status: Status,
    /// The device a job was asked for, what for and since when, until the job answers.
    doing: Option<(String, Doing, Millis)>,
    /// The note, and until when it is said.
    note: Option<(Note, Millis)>,
    refreshing: bool,
    refresh_at: Millis,
    /// Until when the look under way discovers devices.
    looking: Option<Millis>,
    /// X has looked for devices since the screen was opened.
    looked: bool,
    /// Every device a look has found with a name, until the screen is left. bluetoothd forgets
    /// a device only found within half a minute of discovery stopping, while the list still
    /// offers it; the script looks for it again to pair with it.
    found: Vec<Device>,
}

impl Bluetooth {
    pub fn new(jobs: Box<dyn BluetoothJobs>) -> Self {
        Bluetooth {
            jobs,
            enabled: false,
            status: Status::default(),
            doing: None,
            note: None,
            refreshing: false,
            refresh_at: 0,
            looking: None,
            looked: false,
            found: Vec::new(),
        }
    }

    pub fn present(&self) -> bool {
        self.jobs.present()
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn note(&self) -> Option<&Note> {
        self.note.as_ref().map(|(note, _)| note)
    }

    /// The device a job is under way for, and what it is doing with it.
    pub fn doing(&self) -> Option<(&str, Doing)> {
        self.doing
            .as_ref()
            .map(|(mac, what, _)| (mac.as_str(), *what))
    }

    /// A look for devices is under way.
    pub fn looking(&self) -> bool {
        self.looking.is_some()
    }

    /// At boot: brings Bluetooth up if it was left on. The radio comes up on the worker thread,
    /// so the first frame never waits for it.
    pub fn start(&mut self, enabled: bool) {
        self.enabled = enabled && self.present();
        if self.enabled {
            self.jobs.ask(BluetoothJob::Up);
        }
    }

    pub fn set_enabled(&mut self, on: bool, now: Millis) {
        if on == self.enabled || !self.present() {
            return;
        }
        self.enabled = on;
        self.note = None;
        self.doing = None;
        // a switch lets the looks go; going down stops a discovery under way too
        self.unlook();
        if on {
            self.jobs.ask(BluetoothJob::Up);
            self.refresh_at = now;
        } else {
            self.jobs.ask(BluetoothJob::Down);
            self.status = Status::default();
        }
    }

    /// X: one look for devices, discovery on for `LOOK_MS` and then off. Nothing while a look is
    /// under way, while Bluetooth is off, or while a device pairs or connects, which a discovery
    /// would only slow down. What an earlier look found and bluetoothd has since forgotten goes.
    pub fn look(&mut self, now: Millis) {
        let busy = matches!(self.doing(), Some((_, Doing::Pairing | Doing::Connecting)));
        if !self.enabled || self.looking.is_some() || busy {
            return;
        }
        if matches!(self.note(), Some(Note::NoPair(_) | Note::NoConnect(_))) {
            self.note = None;
        }
        self.looking = Some(now + LOOK_MS);
        self.looked = true;
        self.found.clear();
        self.remember();
        self.jobs.ask(BluetoothJob::Scan(true));
    }

    /// Lets the looks go, as the screen is left or Bluetooth switched: the devices they found are
    /// offered no more.
    fn unlook(&mut self) {
        self.looking = None;
        self.looked = false;
        self.found.clear();
    }

    /// Keeps every device the status has found with a name, while the screen has looked.
    fn remember(&mut self) {
        if !self.looked {
            return;
        }
        for d in &self.status.devices {
            if d.paired || d.name.is_empty() {
                continue;
            }
            match self.found.iter_mut().find(|known| known.mac == d.mac) {
                Some(known) => *known = d.clone(),
                None => self.found.push(d.clone()),
            }
        }
    }

    /// Asks a job for device `mac`, unless Bluetooth is off or another is under way: whether it
    /// did.
    fn ask(&mut self, mac: &str, doing: Doing, now: Millis) -> bool {
        if !self.enabled || self.doing.is_some() {
            return false;
        }
        self.note = None;
        self.doing = Some((mac.to_string(), doing, now));
        self.jobs.ask(match doing {
            Doing::Pairing => BluetoothJob::Pair(mac.to_string()),
            Doing::Connecting => BluetoothJob::Connect(mac.to_string()),
            Doing::Disconnecting => BluetoothJob::Disconnect(mac.to_string()),
            Doing::Forgetting => BluetoothJob::Forget(mac.to_string()),
        });
        true
    }

    /// Pairs with a device found, then trusts and connects it. The script stops a look under way
    /// first, as discovery slows a pairing down, and the look is over.
    pub fn pair(&mut self, mac: &str, now: Millis) {
        if self.ask(mac, Doing::Pairing, now) {
            self.looking = None;
        }
    }

    pub fn connect(&mut self, mac: &str, now: Millis) {
        self.ask(mac, Doing::Connecting, now);
    }

    pub fn disconnect(&mut self, mac: &str, now: Millis) {
        self.ask(mac, Doing::Disconnecting, now);
    }

    /// Forgets a paired device, even while it connects: the job waits its turn behind the
    /// connection. The list lets the device go at once, as bluetoothd does.
    pub fn forget(&mut self, mac: &str, now: Millis) {
        if !self.enabled {
            return;
        }
        self.note = None;
        if self.doing.as_ref().is_none_or(|(m, ..)| m == mac) {
            self.doing = Some((mac.to_string(), Doing::Forgetting, now));
        }
        self.status.devices.retain(|d| d.mac != mac);
        self.found.retain(|d| d.mac != mac);
        self.jobs.ask(BluetoothJob::Forget(mac.to_string()));
    }

    /// Takes what the radio has said since the last tick, lets a note that has been said long
    /// enough go, ends a look that has looked long enough or whose screen is left, and keeps the
    /// status fresh for a screen that names the device connected or lists the devices. Nothing
    /// ever looks for devices by itself.
    pub fn tick(&mut self, now: Millis, watch: Watch) {
        for news in self.jobs.news() {
            self.take(news, now);
        }
        if self.note.as_ref().is_some_and(|(_, until)| now >= *until) {
            self.note = None;
        }
        if let Some((mac, doing, _)) = self
            .doing
            .take_if(|(.., since)| now.saturating_sub(*since) > DOING_MS)
        {
            let name = self.name_of(&mac);
            let note = match doing {
                Doing::Pairing => Some(Note::NoPair(name)),
                Doing::Connecting => Some(Note::NoConnect(name)),
                Doing::Disconnecting | Doing::Forgetting => None,
            };
            if let Some(note) = note {
                self.note = Some((note, now + NOTE_MS));
            }
        }
        if !self.enabled {
            return;
        }
        let left = watch < Watch::Devices;
        if self.looking.is_some_and(|until| left || now >= until) {
            self.looking = None;
            self.jobs.ask(BluetoothJob::Scan(false));
        }
        if left {
            self.unlook();
        }
        // nothing on screen names a device: asked now and then all the same, so AirPods that
        // connect by themselves have their controls in the game's menu
        let due = match watch {
            Watch::Idle => self.refresh_at + IDLE_REFRESH_MS,
            _ => self.refresh_at,
        };
        if !self.refreshing && now >= due {
            self.refreshing = true;
            self.jobs.ask(BluetoothJob::Refresh);
        }
    }

    fn take(&mut self, news: BluetoothNews, now: Millis) {
        match news {
            BluetoothNews::Started(true) => {
                if self.note() == Some(&Note::NoRadio) {
                    self.note = None;
                }
            }
            BluetoothNews::Started(false) => {
                if self.enabled {
                    self.note = Some((Note::NoRadio, Millis::MAX));
                }
            }
            BluetoothNews::Status(status) => {
                self.refreshing = false;
                self.refresh_at = now + REFRESH_MS;
                if self.enabled {
                    self.status = status;
                    self.remember();
                }
            }
            BluetoothNews::Done { job, result } => {
                let what = match job {
                    BluetoothJob::Pair(_) => Doing::Pairing,
                    BluetoothJob::Connect(_) => Doing::Connecting,
                    BluetoothJob::Disconnect(_) => Doing::Disconnecting,
                    BluetoothJob::Forget(_) => Doing::Forgetting,
                    _ => return,
                };
                let mac = job.device().unwrap_or_default();
                // the job asked last for the device: one it replaced settles nothing
                let Some((asked, doing, _)) =
                    self.doing.take_if(|(m, d, _)| m == mac && *d == what)
                else {
                    return;
                };
                let name = self.name_of(&asked);
                let note = match (doing, result) {
                    (_, Ok(())) => return,
                    (Doing::Pairing, Err(Fail::Connect)) | (Doing::Connecting, Err(_)) => {
                        Note::NoConnect(name)
                    }
                    (Doing::Pairing, Err(_)) => Note::NoPair(name),
                    (Doing::Disconnecting | Doing::Forgetting, Err(_)) => return,
                };
                self.note = Some((note, now + NOTE_MS));
            }
        }
    }

    /// The device `mac` as a note names it.
    fn name_of(&self, mac: &str) -> String {
        self.status
            .devices
            .iter()
            .chain(&self.found)
            .find(|d| d.mac == mac)
            .map_or(mac, Device::shown)
            .to_string()
    }

    fn state_of(&self, d: &Device) -> DeviceState {
        match self.doing() {
            Some((mac, Doing::Pairing)) if mac == d.mac => DeviceState::Pairing,
            Some((mac, Doing::Connecting)) if mac == d.mac && !d.connected => {
                DeviceState::Connecting
            }
            _ if d.connected => DeviceState::Connected,
            _ if d.paired => DeviceState::Paired,
            _ => DeviceState::Found,
        }
    }

    /// The devices to offer, while Bluetooth is on: the paired ones first, those connected
    /// leading, then those the looks since the screen was opened have found with a name, each
    /// by name. A device only found is offered while a job is under way for it, too. A device
    /// being forgotten is gone already, whatever a status from before says.
    pub fn entries(&self) -> Vec<Entry> {
        if !self.enabled {
            return Vec::new();
        }
        let forgetting = |d: &Device| self.doing() == Some((d.mac.as_str(), Doing::Forgetting));
        let doing = |d: &Device| self.doing().is_some_and(|(mac, _)| mac == d.mac);
        let mut shown: Vec<&Device> = self
            .status
            .devices
            .iter()
            .filter(|d| d.paired || doing(d))
            .collect();
        for d in &self.found {
            if !shown.iter().any(|s| s.mac == d.mac) {
                shown.push(d);
            }
        }
        let mut out: Vec<Entry> = shown
            .into_iter()
            .filter(|d| !forgetting(d))
            .map(|d| Entry {
                mac: d.mac.clone(),
                name: d.shown().to_string(),
                kind: d.kind,
                paired: d.paired,
                state: self.state_of(d),
            })
            .collect();
        out.sort_by_cached_key(|e| {
            let rank = match e.state {
                DeviceState::Connected => 0,
                _ if e.paired => 1,
                _ => 2,
            };
            (rank, e.name.to_lowercase(), e.mac.clone())
        });
        out
    }

    /// The headphones connected, by their address, for the sound to go to: none while
    /// Bluetooth is off or none are.
    pub fn connected_audio(&self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        self.status
            .devices
            .iter()
            .find(|d| {
                d.connected && (d.kind == DeviceKind::Audio || crate::airpods::is_airpods(&d.name))
            })
            .map(|d| d.mac.clone())
    }

    /// The AirPods connected, by their address: none while Bluetooth is off or no AirPods are.
    pub fn connected_airpods(&self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        self.status
            .devices
            .iter()
            .find(|d| d.connected && crate::airpods::is_airpods(&d.name))
            .map(|d| d.mac.clone())
    }

    pub fn summary(&self) -> Summary {
        if !self.enabled {
            return Summary::Off;
        }
        match self
            .entries()
            .into_iter()
            .find(|e| e.state == DeviceState::Connected)
        {
            Some(e) => Summary::Connected(e.name),
            None => Summary::On,
        }
    }

    /// The notes under the devices, a line each, short enough for the list's panel at 640x480:
    /// that Bluetooth is off, why a pairing or a connection went wrong, that devices are being
    /// looked for, that X looks for them while the screen has not, or that a look found none.
    pub fn notes(&self) -> Vec<String> {
        if !self.present() {
            return vec!["This handheld has no Bluetooth.".into()];
        }
        if !self.enabled {
            return vec!["Bluetooth is off.".into()];
        }
        match self.note() {
            Some(Note::NoRadio) => return vec!["The Bluetooth radio did not start.".into()],
            Some(Note::NoPair(name)) => return vec![said("Could not pair with ", name)],
            Some(Note::NoConnect(name)) => return vec![said("Could not connect to ", name)],
            None => {}
        }
        if self.looking() {
            return vec!["Looking for devices…".into()];
        }
        if !self.looked {
            return vec!["Press X to look for devices.".into()];
        }
        if self.found.is_empty() {
            return vec!["No devices found.".into()];
        }
        Vec::new()
    }
}

/// The most letters a note has: the list's panel at 640x480 holds 43.
const NOTE_LETTERS: usize = 43;

/// `lead`, then a device's name, then a full stop, the name cut short with an ellipsis so that
/// the note fits the list's panel.
fn said(lead: &str, name: &str) -> String {
    let room = NOTE_LETTERS - lead.chars().count() - 1;
    if name.chars().count() <= room {
        return format!("{lead}{name}.");
    }
    let cut: String = name.chars().take(room - 1).collect();
    format!("{lead}{}….", cut.trim_end())
}

/// A row of the Bluetooth screen, held by what it is rather than where it is, so a device found
/// or lost that reorders the list leaves the cursor on the same device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sel {
    Switch,
    Device(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BluetoothScreen {
    pub sel: Sel,
    /// Where the cursor was, for when its device drops out of the list.
    pub at: usize,
    /// The first device in view, in a list longer than its panel.
    pub first: usize,
}

impl Default for BluetoothScreen {
    fn default() -> Self {
        BluetoothScreen {
            sel: Sel::Switch,
            at: 0,
            first: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Stay,
    Close,
}

/// A device in the DEVICES list, as a kit list row shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceLine {
    /// Its name, in its own case, or the address of a paired device with no name.
    pub name: String,
    /// What it is to the handheld: CONNECTED, CONNECTING, PAIRING or PAIRED; nothing for a
    /// device only found.
    pub value: Option<&'static str>,
    pub kind: DeviceKind,
}

/// The row under the cursor: BLUETOOTH's switch, or one of the devices in view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lit {
    Switch,
    Device(usize),
}

/// What the Bluetooth modal shows, for its faces to be made from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BluetoothView {
    /// The BLUETOOTH panel's one row.
    pub switch: SettingLine,
    /// The devices in view, top first: as many as DEVICES holds, from the first in view on.
    pub devices: Vec<DeviceLine>,
    pub lit: Lit,
    /// The notes under the devices, a line each.
    pub notes: Vec<String>,
}

impl BluetoothScreen {
    fn rows(&self, bt: &Bluetooth) -> Vec<Sel> {
        std::iter::once(Sel::Switch)
            .chain(bt.entries().into_iter().map(|e| Sel::Device(e.mac)))
            .collect()
    }

    /// The cursor's row now, settling it on a neighbour if its device has gone, and the list
    /// scrolled, a whole row at a time, so that a device under the cursor is in view.
    fn settle(&mut self, rows: &[Sel], bt: &Bluetooth) -> usize {
        let at = rows
            .iter()
            .position(|r| *r == self.sel)
            .unwrap_or(self.at.min(rows.len() - 1));
        self.sel = rows[at].clone();
        self.at = at;
        let in_view = networks_shown(1, bt.notes().len());
        // the cursor on the switch leaves the list where it was
        let lit = at.checked_sub(1).unwrap_or(self.first);
        self.first = network_window(lit, rows.len() - 1, self.first, in_view);
        at
    }

    fn entry(&self, bt: &Bluetooth) -> Option<Entry> {
        let Sel::Device(mac) = &self.sel else {
            return None;
        };
        bt.entries().into_iter().find(|e| e.mac == *mac)
    }

    /// Takes a press.
    pub fn input(&mut self, bt: &mut Bluetooth, action: Action, now: Millis) -> Outcome {
        let rows = self.rows(bt);
        let at = self.settle(&rows, bt);
        match action {
            Action::GbaDown(Btn::B) | Action::QuickMenu => return Outcome::Close,
            Action::GbaDown(Btn::Up) => self.sel = rows[(at + rows.len() - 1) % rows.len()].clone(),
            Action::GbaDown(Btn::Down) => self.sel = rows[(at + 1) % rows.len()].clone(),
            Action::GbaDown(Btn::Left | Btn::Right | Btn::A) if self.sel == Sel::Switch => {
                bt.set_enabled(!bt.enabled(), now);
            }
            Action::GbaDown(Btn::A) => {
                if let Some(e) = self.entry(bt) {
                    match e.state {
                        DeviceState::Found => bt.pair(&e.mac, now),
                        DeviceState::Paired => bt.connect(&e.mac, now),
                        DeviceState::Connected => bt.disconnect(&e.mac, now),
                        DeviceState::Pairing | DeviceState::Connecting => {}
                    }
                }
            }
            Action::GbaDown(Btn::Y) if self.forgets(bt) => {
                if let Some(e) = self.entry(bt) {
                    bt.forget(&e.mac, now);
                }
            }
            Action::GbaDown(Btn::X) => bt.look(now),
            _ => {}
        }
        let rows = self.rows(bt);
        self.settle(&rows, bt);
        Outcome::Stay
    }

    /// Whether Y forgets the device under the cursor: one paired, connected, connecting or not.
    pub fn forgets(&self, bt: &Bluetooth) -> bool {
        self.entry(bt).is_some_and(|e| e.paired)
    }

    /// Whether the bar offers X for a look: while Bluetooth is on, looking already or not.
    pub fn scans(&self, bt: &Bluetooth) -> bool {
        bt.enabled()
    }

    /// What the modal shows.
    pub fn view(&mut self, bt: &Bluetooth) -> BluetoothView {
        let rows = self.rows(bt);
        let at = self.settle(&rows, bt);
        let notes = bt.notes();
        let in_view = networks_shown(1, notes.len());
        let devices = bt
            .entries()
            .into_iter()
            .skip(self.first)
            .take(in_view)
            .map(|e| DeviceLine {
                name: e.name,
                value: match e.state {
                    DeviceState::Connected => Some("CONNECTED"),
                    DeviceState::Connecting => Some("CONNECTING"),
                    DeviceState::Pairing => Some("PAIRING"),
                    DeviceState::Paired => Some("PAIRED"),
                    DeviceState::Found => None,
                },
                kind: e.kind,
            })
            .collect();
        BluetoothView {
            switch: SettingLine {
                label: "BLUETOOTH",
                value: bt
                    .present()
                    .then_some(if bt.enabled() { "ON" } else { "OFF" }),
                carets: bt.present(),
                opens: false,
            },
            devices,
            lit: match at.checked_sub(1) {
                None => Lit::Switch,
                Some(i) => Lit::Device(i - self.first),
            },
            notes,
        }
    }
}
