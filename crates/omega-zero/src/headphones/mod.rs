//! Headphones' own controls, for the AirPods and Galaxy Buds connected: noise control, what
//! else they let be set, and the battery of each bud and the case, shown in Settings >
//! Bluetooth (START on them) and on the game menu's HEADPHONES page.
//!
//! Each make talks over a channel of its own beside the audio connection: AirPods over
//! Apple's L2CAP protocol (`airpods`), as LibrePods worked it out; Galaxy Buds over Samsung's
//! RFCOMM one (`galaxy`), as GalaxyBudsClient did. AirPods also say their battery in the
//! Bluetooth LE beacon they send out, which OpenPods reads (`beacon`): it stands in where
//! the channel says none. The channel is opened once bluetoothd has them connected and read
//! on a thread of its own; what they say is kept for the menus.

pub mod airpods;
pub mod beacon;
pub mod galaxy;
mod sock;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use omega_input::Millis;

/// Which headphones, by what their name says: each make's protocol and what it can set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    AirPods,
    Buds(galaxy::Model),
}

impl Kind {
    /// The headphones a device by this name are, if the handheld knows how to talk to them.
    /// AirPods keep "AirPods" in any name their owner gives them; Galaxy Buds name themselves
    /// "Galaxy Buds2 Pro (1A2B)" and the like.
    pub fn of(name: &str) -> Option<Kind> {
        let lower = name.to_ascii_lowercase();
        if lower.contains("airpods") {
            return Some(Kind::AirPods);
        }
        galaxy::Model::of(&lower).map(Kind::Buds)
    }

    /// What the menus call them.
    pub fn word(self) -> &'static str {
        match self {
            Kind::AirPods => "AirPods",
            Kind::Buds(_) => "Galaxy Buds",
        }
    }
}

/// What the headphones let in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Noise {
    Off,
    Cancellation,
    /// Transparency, or Samsung's Ambient sound.
    Transparency,
    Adaptive,
}

impl Noise {
    /// As a menu row says it.
    pub fn word(self, kind: Kind) -> &'static str {
        match (self, kind) {
            (Noise::Off, _) => "OFF",
            (Noise::Cancellation, _) => "NOISE CANCEL",
            (Noise::Transparency, Kind::Buds(_)) => "AMBIENT",
            (Noise::Transparency, Kind::AirPods) => "TRANSPARENCY",
            (Noise::Adaptive, _) => "ADAPTIVE",
        }
    }

    /// What a toast says when the headphones are switched to it.
    pub fn told(self, kind: Kind) -> &'static str {
        match (self, kind) {
            (Noise::Off, _) => "Noise control off",
            (Noise::Cancellation, _) => "Noise cancelling",
            (Noise::Transparency, Kind::Buds(_)) => "Ambient sound",
            (Noise::Transparency, Kind::AirPods) => "Transparency",
            (Noise::Adaptive, _) => "Adaptive",
        }
    }

    /// The mode beside this one among `modes`.
    pub fn step(self, modes: &[Noise], right: bool) -> Noise {
        let n = modes.len();
        if n == 0 {
            return self;
        }
        let at = modes.iter().position(|m| *m == self).unwrap_or(0);
        modes[if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
    }
}

/// A battery: its level, in percent, and whether it is charging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    pub level: u8,
    pub charging: bool,
}

/// The headphones as they last said they were.
#[derive(Clone, Debug, PartialEq)]
pub struct Phones {
    pub kind: Kind,
    pub left: Option<Battery>,
    pub right: Option<Battery>,
    pub case: Option<Battery>,
    pub noise: Option<Noise>,
    /// AirPods' conversational awareness: talking turns the sound down and lets the room in.
    pub conversation: Option<bool>,
    /// How much of the room Adaptive lets in, 0 to 100, on AirPods that have it.
    pub adaptive_level: Option<u8>,
    /// From conversational awareness: how far the sound is turned down while the wearer
    /// talks, 1.0 for not at all.
    pub duck: f32,
    /// The battery came from the AirPods' beacon, which says it to the nearest ten.
    pub beacon: bool,
    /// The headphones have said something on their channel: until then they have no menu.
    pub heard: bool,
    /// The noise control mode last set from here, and when: the headphones say back each
    /// mode a step at a time, so the ones they say just after are older steps, not theirs.
    pub noise_pending: Option<(Noise, std::time::Instant)>,
}

impl Phones {
    pub fn new(kind: Kind) -> Phones {
        Phones {
            kind,
            left: None,
            right: None,
            case: None,
            noise: None,
            conversation: None,
            adaptive_level: None,
            duck: 1.0,
            beacon: false,
            heard: false,
            noise_pending: None,
        }
    }

    /// Takes in a noise control mode the headphones say: not one older than the one just set
    /// from here, which they are still on their way to.
    pub fn said_noise(&mut self, mode: Noise) {
        if let Some((want, at)) = self.noise_pending {
            if at.elapsed() < NOISE_SETTLE && mode != want {
                return;
            }
            self.noise_pending = None;
        }
        self.noise = Some(mode);
    }

    /// The noise control modes these headphones have, in the order the row steps them.
    pub fn noise_modes(&self) -> &'static [Noise] {
        match self.kind {
            Kind::AirPods => &[
                Noise::Cancellation,
                Noise::Adaptive,
                Noise::Transparency,
                Noise::Off,
            ],
            Kind::Buds(model) => model.noise_modes(),
        }
    }

    pub fn has_conversation(&self) -> bool {
        self.kind == Kind::AirPods
    }

    pub fn has_adaptive_level(&self) -> bool {
        self.kind == Kind::AirPods
    }

    /// The batteries as a row says them: `L 80% R 75% CASE 40%`, a `+` for charging.
    pub fn battery_text(&self) -> String {
        let part = |name: &str, b: Option<Battery>| {
            b.map(|b| format!("{name} {}%{}", b.level, if b.charging { "+" } else { "" }))
        };
        let parts: Vec<String> = [
            part("L", self.left),
            part("R", self.right),
            part("CASE", self.case),
        ]
        .into_iter()
        .flatten()
        .collect();
        match parts.is_empty() {
            true => "…".to_string(),
            false => parts.join(" "),
        }
    }

    /// The buds' batteries as a row says them: `L 80% R 75%`, a `+` for charging.
    pub fn buds_text(&self) -> String {
        let part = |name: &str, b: Option<Battery>| {
            b.map(|b| format!("{name} {}%{}", b.level, if b.charging { "+" } else { "" }))
        };
        let parts: Vec<String> = [part("L", self.left), part("R", self.right)]
            .into_iter()
            .flatten()
            .collect();
        match parts.is_empty() {
            true => "…".to_string(),
            false => parts.join(" "),
        }
    }

    /// The case's battery as its row says it: what it last said, the buds out of it.
    pub fn case_text(&self) -> String {
        match self.case {
            Some(b) => format!("{}%{}", b.level, if b.charging { "+" } else { "" }),
            None => "…".to_string(),
        }
    }

    /// The lowest bud not charging, for the low battery notice.
    pub fn lowest(&self) -> Option<u8> {
        [self.left, self.right]
            .into_iter()
            .flatten()
            .filter(|b| !b.charging)
            .map(|b| b.level)
            .min()
    }
}

/// How long after the noise control mode is set from here the modes the headphones say back
/// are taken for the steps on their way to it.
pub const NOISE_SETTLE: std::time::Duration = std::time::Duration::from_millis(2_500);

/// How low a bud goes before the handheld says so, highest first.
pub const LOW: [u8; 3] = [20, 10, 5];

/// What the low battery notice says at each of `LOW`.
pub fn low_text(at: u8) -> &'static str {
    match at {
        20 => "Battery low: 20%",
        10 => "Battery low: 10%",
        _ => "Battery almost empty",
    }
}

/// Says each step of `LOW` once as a bud goes under it, and again only once it has charged.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LowNotice {
    /// The lowest step said since the buds were last above it.
    said: Option<u8>,
}

impl LowNotice {
    /// The step to say now for buds at `level`, none for nothing new.
    pub fn check(&mut self, level: Option<u8>) -> Option<u8> {
        let level = level?;
        // charged back above where it was said: the steps are said again as it runs down
        if self.said.is_some_and(|s| level > s + 5) {
            self.said = None;
        }
        let step = LOW.iter().copied().filter(|s| level <= *s).min()?;
        match self.said {
            Some(s) if s <= step => None,
            _ => {
                self.said = Some(step);
                Some(step)
            }
        }
    }
}

/// An open channel to a pair of headphones, and what they said on it.
pub struct Link {
    pub mac: String,
    phones: Arc<Mutex<Phones>>,
    /// The reading thread is still reading: the channel is open.
    alive: Arc<AtomicBool>,
    out: Option<std::fs::File>,
    /// A noise control mode picked in the menu, and when: sent once the row has rested on it,
    /// so stepping past a mode never switches the headphones to it.
    queued: Option<(Noise, std::time::Instant)>,
}

/// How long the noise control row must rest on a mode before the headphones are switched to it.
pub const NOISE_REST: std::time::Duration = std::time::Duration::from_millis(500);

impl Link {
    /// Opens the channel to the headphones at `mac` and reads it on a thread of its own: none
    /// where it would not open.
    pub fn open(mac: &str, kind: Kind) -> Option<Link> {
        let sock = match kind {
            Kind::AirPods => sock::l2cap(mac, airpods::PSM),
            Kind::Buds(_) => galaxy::open(mac),
        };
        let sock = match sock {
            Ok(sock) => sock,
            Err(e) => {
                eprintln!("omega-zero: headphones: {} at {mac}: {e}", kind.word());
                return None;
            }
        };
        eprintln!("omega-zero: headphones: {} at {mac} open", kind.word());
        let phones = Arc::new(Mutex::new(Phones::new(kind)));
        let alive = Arc::new(AtomicBool::new(true));
        let out = sock.try_clone().ok()?;
        let answer = sock.try_clone().ok()?;
        let (theirs, still) = (phones.clone(), alive.clone());
        std::thread::Builder::new()
            .name("omega-phones".into())
            .spawn(move || {
                match kind {
                    Kind::AirPods => airpods::read(sock, answer, &theirs),
                    Kind::Buds(_) => galaxy::read(sock, &theirs),
                }
                eprintln!("omega-zero: headphones: channel closed");
                still.store(false, Ordering::Release);
            })
            .ok()?;
        Some(Link {
            mac: mac.to_string(),
            phones,
            alive,
            out: Some(out),
            queued: None,
        })
    }

    /// A link that talks to nothing, holding `phones`: for the menus' tests.
    pub fn stand_in(mac: &str, phones: Phones) -> Link {
        Link {
            mac: mac.to_string(),
            phones: Arc::new(Mutex::new(phones)),
            alive: Arc::new(AtomicBool::new(true)),
            out: None,
            queued: None,
        }
    }

    pub fn phones(&self) -> Option<Phones> {
        self.phones.lock().ok().map(|p| p.clone())
    }

    /// Whether the channel is still open.
    pub fn open_still(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    fn send(&mut self, packets: &[Vec<u8>]) {
        use std::io::Write;
        if let Some(out) = &mut self.out {
            for p in packets {
                let _ = out.write_all(p);
            }
        }
    }

    fn change(&mut self, f: impl FnOnce(&mut Phones)) {
        if let Ok(mut p) = self.phones.lock() {
            f(&mut p);
        }
    }

    /// Picks the noise control mode: shown at once, and sent once the row rests on it for
    /// `NOISE_REST`.
    pub fn set_noise(&mut self, mode: Noise) {
        let now = std::time::Instant::now();
        self.queued = Some((mode, now));
        self.change(|p| {
            p.noise = Some(mode);
            p.noise_pending = Some((mode, now));
        });
    }

    /// Whether a mode picked in the menu is still waiting to be sent.
    pub fn noise_queued(&self) -> bool {
        self.queued.is_some()
    }

    /// Sends the mode picked once the row has rested on it.
    pub fn flush(&mut self) {
        let Some((mode, at)) = self.queued else {
            return;
        };
        if at.elapsed() < NOISE_REST {
            return;
        }
        self.queued = None;
        let Some(kind) = self.phones().map(|p| p.kind) else {
            return;
        };
        let packets = match kind {
            Kind::AirPods => vec![airpods::set_noise(mode).to_vec()],
            Kind::Buds(model) => galaxy::set_noise(model, mode),
        };
        self.send(&packets);
        // what they say back is weighed from now, when they were asked
        self.change(|p| p.noise_pending = Some((mode, std::time::Instant::now())));
    }

    pub fn set_conversation(&mut self, on: bool) {
        self.send(&[airpods::set_conversation(on).to_vec()]);
        self.change(|p| {
            p.conversation = Some(on);
            if !on {
                p.duck = 1.0;
            }
        });
    }

    pub fn set_adaptive_level(&mut self, level: u8) {
        let level = level.min(100);
        self.send(&[airpods::set_adaptive_level(level).to_vec()]);
        self.change(|p| p.adaptive_level = Some(level));
    }

    /// The battery from the AirPods' beacon, where their channel has said none.
    fn take_beacon(&mut self, found: beacon::Beacon) {
        self.change(|p| {
            // the channel says the buds: the beacon only fills in the case
            if p.left.is_some() && !p.beacon {
                p.case = found.case.or(p.case);
                return;
            }
            p.left = found.left;
            p.right = found.right;
            p.case = found.case.or(p.case);
            p.beacon = true;
        });
    }
}

/// How long after a channel would not open it is tried again.
pub const RETRY_MS: Millis = 5_000;
/// How long the channel may go without a battery before the AirPods' beacon is looked for, and
/// how often it is looked for again while the channel still says none.
pub const BEACON_MS: Millis = 8_000;
pub const BEACON_AGAIN_MS: Millis = 60_000;

type Opening = std::sync::mpsc::Receiver<Option<Link>>;

/// Keeps a channel open to the headphones connected, if any: opened on a thread of its own, as
/// opening waits on the radio, and opened again should it close while they stay connected.
#[derive(Default)]
pub struct Keeper {
    link: Option<Link>,
    opened_at: Millis,
    opening: Option<(String, Opening)>,
    /// The headphones a channel last would not open to, and when.
    failed: Option<(String, Millis)>,
    looking: Option<std::sync::mpsc::Receiver<Option<beacon::Beacon>>>,
    looked_at: Option<Millis>,
    low: LowNotice,
    /// The noise control mode last seen, for a toast when the headphones are switched.
    noise_seen: Option<Noise>,
}

impl Keeper {
    /// Keeps the channel to the headphones at `want`, the ones connected, of `kind`, or closes
    /// it for none.
    pub fn keep(&mut self, want: Option<(&str, Kind)>, now: Millis) {
        if let Some(link) = self.link.as_mut() {
            link.flush();
        }
        if let Some((mac, rx)) = &self.opening {
            match rx.try_recv() {
                Ok(link) => {
                    if link.is_none() {
                        self.failed = Some((mac.clone(), now));
                    }
                    self.link = link;
                    self.opened_at = now;
                    self.looked_at = None;
                    self.opening = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.opening = None,
            }
        }
        let mac = want.map(|(m, _)| m);
        if self
            .link
            .as_ref()
            .is_some_and(|l| Some(l.mac.as_str()) != mac || !l.open_still())
        {
            self.link = None;
            self.low = LowNotice::default();
        }
        let Some((mac, kind)) = want else {
            self.failed = None;
            return;
        };
        self.look_for_beacon(now);
        let waiting = self
            .failed
            .as_ref()
            .is_some_and(|(m, at)| m == mac && now.saturating_sub(*at) < RETRY_MS);
        if self.link.is_some() || waiting {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let theirs = mac.to_string();
        let spawned = std::thread::Builder::new()
            .name("omega-phones-open".into())
            .spawn(move || {
                let _ = tx.send(Link::open(&theirs, kind));
            });
        match spawned {
            Ok(_) => self.opening = Some((mac.to_string(), rx)),
            Err(_) => self.failed = Some((mac.to_string(), now)),
        }
    }

    /// AirPods whose channel has said no battery yet: their beacon is looked for, now and then.
    fn look_for_beacon(&mut self, now: Millis) {
        if let Some(rx) = &self.looking {
            match rx.try_recv() {
                Ok(found) => {
                    self.looking = None;
                    if let (Some(found), Some(link)) = (found, self.link.as_mut()) {
                        link.take_beacon(found);
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.looking = None,
            }
        }
        let Some(phones) = self.phones() else {
            return;
        };
        let lacking = phones.kind == Kind::AirPods
            && (phones.left.is_none() || phones.beacon || phones.case.is_none());
        let due = match self.looked_at {
            None => now.saturating_sub(self.opened_at) >= BEACON_MS,
            Some(at) => now.saturating_sub(at) >= BEACON_AGAIN_MS,
        };
        if !lacking || !due {
            return;
        }
        self.looked_at = Some(now);
        let (tx, rx) = std::sync::mpsc::channel();
        if std::thread::Builder::new()
            .name("omega-beacon".into())
            .spawn(move || {
                crate::spawn::stand_back();
                let _ = tx.send(beacon::look());
            })
            .is_ok()
        {
            self.looking = Some(rx);
        }
    }

    /// The channel, while one is open.
    pub fn link(&mut self) -> Option<&mut Link> {
        self.link.as_mut()
    }

    /// The headphones, once they have said something on their channel.
    pub fn phones(&self) -> Option<Phones> {
        self.link
            .as_ref()
            .and_then(Link::phones)
            .filter(|p| p.heard)
    }

    /// The mode the headphones were just switched to on themselves, by a stem or a touch: not
    /// one picked in the menu, nor the first they say as they connect.
    pub fn noise_switched(&mut self) -> Option<(Noise, Kind)> {
        let Some(p) = self.phones() else {
            self.noise_seen = None;
            return None;
        };
        let ours = p.noise_pending.is_some() || self.link.as_ref().is_some_and(Link::noise_queued);
        let before = std::mem::replace(&mut self.noise_seen, p.noise);
        match (before, p.noise) {
            (Some(was), Some(now)) if was != now && !ours => Some((now, p.kind)),
            _ => None,
        }
    }

    /// The low battery step to say now, if the buds just went under one.
    pub fn low_battery(&mut self) -> Option<u8> {
        let level = self.phones().and_then(|p| p.lowest());
        self.low.check(level)
    }

    /// For the menus' tests: headphones as `phones` says, on a channel that talks to nothing.
    pub fn stand_in(&mut self, mac: &str, mut phones: Phones) {
        phones.heard = true;
        self.link = Some(Link::stand_in(mac, phones));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headphones_are_known_by_their_name() {
        assert_eq!(Kind::of("Matan's AirPods Pro"), Some(Kind::AirPods));
        assert_eq!(
            Kind::of("Galaxy Buds2 Pro (1A2B)"),
            Some(Kind::Buds(galaxy::Model::Buds2Pro))
        );
        assert_eq!(Kind::of("WH-1000XM4"), None);
    }

    #[test]
    fn the_noise_row_steps_round_what_the_headphones_have() {
        let pods = Phones::new(Kind::AirPods);
        let modes = pods.noise_modes();
        assert_eq!(Noise::Off.step(modes, true), Noise::Cancellation);
        assert_eq!(Noise::Cancellation.step(modes, false), Noise::Off);
        let plus = Phones::new(Kind::Buds(galaxy::Model::BudsPlus));
        assert!(!plus.noise_modes().contains(&Noise::Cancellation));
    }

    #[test]
    fn low_battery_is_said_once_a_step_until_the_buds_charge() {
        let mut low = LowNotice::default();
        assert_eq!(low.check(Some(50)), None);
        assert_eq!(low.check(Some(20)), Some(20));
        assert_eq!(low.check(Some(18)), None, "said twice");
        assert_eq!(low.check(Some(9)), Some(10));
        assert_eq!(low.check(Some(4)), Some(5));
        assert_eq!(low.check(Some(3)), None);
        // charged, then run down again
        assert_eq!(low.check(Some(60)), None);
        assert_eq!(low.check(Some(19)), Some(20));
        assert_eq!(low.check(None), None);
    }

    #[test]
    fn a_mode_switched_on_the_headphones_is_told_once_and_one_picked_here_never() {
        let mut k = Keeper::default();
        let mut p = Phones::new(Kind::AirPods);
        p.noise = Some(Noise::Off);
        k.stand_in("AA:BB:CC:DD:EE:FF", p);
        assert_eq!(k.noise_switched(), None, "the mode they connect with");
        k.link()
            .unwrap()
            .change(|p| p.said_noise(Noise::Cancellation));
        assert_eq!(
            k.noise_switched(),
            Some((Noise::Cancellation, Kind::AirPods))
        );
        assert_eq!(k.noise_switched(), None, "told twice");
        k.link().unwrap().set_noise(Noise::Transparency);
        assert_eq!(
            k.noise_switched(),
            None,
            "a mode picked in the menu was told"
        );
    }

    #[test]
    fn a_mode_picked_in_the_menu_waits_for_the_row_to_rest() {
        let mut k = Keeper::default();
        k.stand_in("AA:BB:CC:DD:EE:FF", Phones::new(Kind::AirPods));
        let link = k.link().unwrap();
        link.set_noise(Noise::Adaptive);
        link.set_noise(Noise::Transparency);
        link.flush();
        assert!(link.noise_queued(), "sent before the row rested");
        assert_eq!(link.phones().unwrap().noise, Some(Noise::Transparency));
        std::thread::sleep(NOISE_REST);
        link.flush();
        assert!(!link.noise_queued(), "never sent");
    }

    #[test]
    fn the_battery_row_says_each_part_and_charging() {
        let mut p = Phones::new(Kind::AirPods);
        assert_eq!(p.battery_text(), "…");
        p.left = Some(Battery {
            level: 99,
            charging: true,
        });
        p.right = Some(Battery {
            level: 100,
            charging: false,
        });
        assert_eq!(p.battery_text(), "L 99%+ R 100%");
        assert_eq!(p.lowest(), Some(100), "a charging bud is not running low");
    }
}
