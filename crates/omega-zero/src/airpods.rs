//! AirPods' own controls: noise control, conversational awareness, and the battery of each bud
//! and the case. Apple's devices talk to AirPods over an L2CAP channel of their own, PSM 0x1001,
//! in a protocol LibrePods worked out (github.com/librepods-org/librepods, GPL-3.0, whose
//! `docs/AAP Definitions.md` this follows): a handshake, a packet that turns on the features
//! newer AirPods keep for Apple's own devices, and a request for notifications, after which
//! the AirPods say their battery, noise control mode, which bud is in an ear, and when their
//! wearer starts and stops talking.
//!
//! The channel is opened beside the audio connection, once bluetoothd has the AirPods
//! connected, and read on a thread of its own; what it says is kept for the menus to show.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Opens the conversation; nothing is answered before it.
pub const HANDSHAKE: [u8; 16] = [
    0x00, 0x00, 0x04, 0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
/// Turns on what newer AirPods keep for Apple's own devices: Adaptive, and conversational
/// awareness while sound plays.
pub const SET_FEATURES: [u8; 14] = [
    0x04, 0x00, 0x04, 0x00, 0x4d, 0x00, 0xd7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
/// Asks for the battery, the noise control mode, ear detection and the rest as they change.
pub const REQUEST_NOTIFICATIONS: [u8; 10] =
    [0x04, 0x00, 0x04, 0x00, 0x0f, 0x00, 0xff, 0xff, 0xff, 0xff];

const HEADER: [u8; 4] = [0x04, 0x00, 0x04, 0x00];
const HANDSHAKE_ACK: [u8; 4] = [0x01, 0x00, 0x04, 0x00];
const FEATURES_ACK: [u8; 6] = [0x04, 0x00, 0x04, 0x00, 0x2b, 0x00];
/// A setting: noise control, conversational awareness and the like, `09 00 <setting> <value>`.
const SETTING: u8 = 0x09;
const NOISE_CONTROL: u8 = 0x0d;
const CONVERSATION: u8 = 0x28;
const BATTERY: u8 = 0x04;
const EARS: u8 = 0x06;
const SPEAKING: u8 = 0x4b;

/// What the AirPods let in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Noise {
    Off,
    Cancellation,
    Transparency,
    Adaptive,
}

impl Noise {
    pub const ALL: [Noise; 4] = [
        Noise::Cancellation,
        Noise::Adaptive,
        Noise::Transparency,
        Noise::Off,
    ];

    fn byte(self) -> u8 {
        match self {
            Noise::Off => 1,
            Noise::Cancellation => 2,
            Noise::Transparency => 3,
            Noise::Adaptive => 4,
        }
    }

    fn of(byte: u8) -> Option<Noise> {
        Some(match byte {
            1 => Noise::Off,
            2 => Noise::Cancellation,
            3 => Noise::Transparency,
            4 => Noise::Adaptive,
            _ => return None,
        })
    }

    /// As a menu row says it.
    pub fn word(self) -> &'static str {
        match self {
            Noise::Off => "OFF",
            Noise::Cancellation => "NOISE CANCEL",
            Noise::Transparency => "TRANSPARENCY",
            Noise::Adaptive => "ADAPTIVE",
        }
    }

    /// The mode beside this one in the row's order.
    pub fn step(self, right: bool) -> Noise {
        let at = Noise::ALL.iter().position(|n| *n == self).unwrap_or(0);
        let n = Noise::ALL.len();
        Noise::ALL[if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
    }
}

/// The packet that sets the noise control mode.
pub fn set_noise(mode: Noise) -> [u8; 11] {
    setting(NOISE_CONTROL, mode.byte())
}

/// The packet that turns conversational awareness on or off.
pub fn set_conversation(on: bool) -> [u8; 11] {
    setting(CONVERSATION, if on { 1 } else { 2 })
}

fn setting(which: u8, value: u8) -> [u8; 11] {
    [
        0x04, 0x00, 0x04, 0x00, SETTING, 0x00, which, value, 0x00, 0x00, 0x00,
    ]
}

/// A battery: its level, in percent, and whether it is charging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    pub level: u8,
    pub charging: bool,
}

/// Where a bud is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bud {
    InEar,
    OutOfEar,
    InCase,
}

/// What a packet from the AirPods says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Said {
    HandshakeDone,
    FeaturesDone,
    Battery {
        left: Option<Battery>,
        right: Option<Battery>,
        case: Option<Battery>,
    },
    Noise(Noise),
    Conversation(bool),
    /// The bud with the microphone, then the other.
    Ears(Bud, Bud),
    /// How loud the sound should be while the wearer talks: 1 or 2 talking, up to 8 or 9 for
    /// not, from conversational awareness.
    Speaking(u8),
    Other,
}

fn bud(byte: u8) -> Bud {
    match byte {
        0 => Bud::InEar,
        2 => Bud::InCase,
        _ => Bud::OutOfEar,
    }
}

/// What `data`, one packet from the AirPods, says.
pub fn parse(data: &[u8]) -> Said {
    if data.starts_with(&HANDSHAKE_ACK) {
        return Said::HandshakeDone;
    }
    if data.starts_with(&FEATURES_ACK) {
        return Said::FeaturesDone;
    }
    if !data.starts_with(&HEADER) || data.len() < 6 {
        return Said::Other;
    }
    let body = &data[6..];
    match data[4] {
        BATTERY => {
            let count = body.first().copied().unwrap_or(0) as usize;
            let (mut left, mut right, mut case) = (None, None, None);
            for part in body.get(1..).unwrap_or(&[]).chunks_exact(5).take(count) {
                // [component] 01 [level] [status] 01
                let status = part[3];
                // a part that is not there: a bud in its case, or the case away
                if status == 4 {
                    continue;
                }
                let battery = Some(Battery {
                    level: part[2].min(100),
                    charging: status == 1,
                });
                match part[0] {
                    4 => left = battery,
                    2 => right = battery,
                    8 => case = battery,
                    _ => {}
                }
            }
            Said::Battery { left, right, case }
        }
        SETTING if body.len() >= 2 => match body[0] {
            NOISE_CONTROL => Noise::of(body[1]).map_or(Said::Other, Said::Noise),
            CONVERSATION => match body[1] {
                1 => Said::Conversation(true),
                2 => Said::Conversation(false),
                _ => Said::Other,
            },
            _ => Said::Other,
        },
        EARS if body.len() >= 2 => Said::Ears(bud(body[0]), bud(body[1])),
        // 4b 00 02 00 01 [level]
        SPEAKING if body.len() >= 4 => Said::Speaking(body[3]),
        _ => Said::Other,
    }
}

/// The AirPods as they last said they were.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pods {
    /// The channel is open and the AirPods answered the handshake.
    pub talking: bool,
    pub left: Option<Battery>,
    pub right: Option<Battery>,
    pub case: Option<Battery>,
    pub noise: Option<Noise>,
    pub conversation: Option<bool>,
    pub ears: Option<(Bud, Bud)>,
    /// From conversational awareness: how far the sound is turned down while the wearer talks,
    /// 1.0 for not at all.
    pub duck: f32,
}

impl Pods {
    pub fn fresh() -> Pods {
        Pods {
            duck: 1.0,
            ..Pods::default()
        }
    }

    /// Takes in what a packet said; what it answers, if anything.
    pub fn take(&mut self, said: Said) -> Option<&'static [u8]> {
        match said {
            Said::HandshakeDone => {
                self.talking = true;
                return Some(&SET_FEATURES);
            }
            Said::FeaturesDone => return Some(&REQUEST_NOTIFICATIONS),
            Said::Battery { left, right, case } => {
                self.left = left;
                self.right = right;
                // the case says nothing once both buds are out of it: keep what it last said
                if case.is_some() {
                    self.case = case;
                }
            }
            Said::Noise(n) => self.noise = Some(n),
            Said::Conversation(on) => self.conversation = Some(on),
            Said::Ears(a, b) => self.ears = Some((a, b)),
            Said::Speaking(level) => {
                self.duck = match level {
                    0..=2 => 0.3,
                    8.. => 1.0,
                    // 3 says the talking stopped; the levels between rise back to full
                    n => 0.3 + 0.7 * f32::from(n - 2) / 6.0,
                }
            }
            Said::Other => {}
        }
        None
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
}

/// Whether a device by this name is AirPods: Apple names them so, and keeps the name a user
/// gives them ("Matan's AirPods Pro").
pub fn is_airpods(name: &str) -> bool {
    name.to_ascii_lowercase().contains("airpods")
}

/// An open channel to a pair of AirPods, and what they said on it.
pub struct Link {
    pub mac: String,
    pods: Arc<Mutex<Pods>>,
    /// The reading thread is still reading: the channel is open.
    alive: Arc<AtomicBool>,
    #[cfg(target_os = "linux")]
    out: Option<std::fs::File>,
}

impl Link {
    /// Opens the channel to the AirPods at `mac` and reads it on a thread of its own: none
    /// where it would not open.
    #[cfg(target_os = "linux")]
    pub fn open(mac: &str) -> Option<Link> {
        use std::io::{Read, Write};
        let sock = l2cap::open(mac, 0x1001)?;
        let mut writer = sock.try_clone().ok()?;
        let mut reader = sock;
        writer.write_all(&HANDSHAKE).ok()?;
        let pods = Arc::new(Mutex::new(Pods::fresh()));
        let theirs = pods.clone();
        let mut answer = writer.try_clone().ok()?;
        let alive = Arc::new(AtomicBool::new(true));
        let still = alive.clone();
        std::thread::Builder::new()
            .name("omega-airpods".into())
            .spawn(move || {
                let mut buf = [0u8; 1024];
                loop {
                    let n = match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    let said = parse(&buf[..n]);
                    let reply = theirs.lock().ok().and_then(|mut p| p.take(said));
                    if let Some(reply) = reply {
                        if answer.write_all(reply).is_err() {
                            break;
                        }
                    }
                }
                if let Ok(mut p) = theirs.lock() {
                    *p = Pods::fresh();
                }
                still.store(false, Ordering::Release);
            })
            .ok()?;
        Some(Link {
            mac: mac.to_string(),
            pods,
            alive,
            out: Some(writer),
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn open(_mac: &str) -> Option<Link> {
        None
    }

    /// A link that talks to nothing, holding `pods`: for the menus' tests.
    pub fn stand_in(mac: &str, pods: Pods) -> Link {
        Link {
            mac: mac.to_string(),
            pods: Arc::new(Mutex::new(pods)),
            alive: Arc::new(AtomicBool::new(true)),
            #[cfg(target_os = "linux")]
            out: None,
        }
    }

    pub fn pods(&self) -> Pods {
        self.pods
            .lock()
            .map(|p| p.clone())
            .unwrap_or_else(|_| Pods::fresh())
    }

    /// Whether the channel is still open.
    pub fn open_still(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    fn send(&mut self, packet: &[u8]) {
        #[cfg(target_os = "linux")]
        if let Some(out) = &mut self.out {
            use std::io::Write;
            let _ = out.write_all(packet);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = packet;
    }

    /// Sets the noise control mode, shown at once: the AirPods say it back as they change.
    pub fn set_noise(&mut self, mode: Noise) {
        self.send(&set_noise(mode));
        if let Ok(mut p) = self.pods.lock() {
            p.noise = Some(mode);
        }
    }

    pub fn set_conversation(&mut self, on: bool) {
        self.send(&set_conversation(on));
        if let Ok(mut p) = self.pods.lock() {
            p.conversation = Some(on);
            if !on {
                p.duck = 1.0;
            }
        }
    }
}

/// How long after a channel would not open it is tried again.
pub const RETRY_MS: omega_input::Millis = 5_000;

/// Keeps a channel open to the AirPods connected, if any: opened on a thread of its own, as
/// opening waits on the radio, and opened again should it close while they stay connected.
#[derive(Default)]
pub struct Keeper {
    link: Option<Link>,
    opening: Option<(String, std::sync::mpsc::Receiver<Option<Link>>)>,
    /// The AirPods a channel last would not open to, and when.
    failed: Option<(String, omega_input::Millis)>,
}

impl Keeper {
    /// Keeps the channel to the AirPods at `want`, the ones connected, or closes it for none.
    pub fn keep(&mut self, want: Option<&str>, now: omega_input::Millis) {
        if let Some((mac, rx)) = &self.opening {
            match rx.try_recv() {
                Ok(link) => {
                    if link.is_none() {
                        self.failed = Some((mac.clone(), now));
                    }
                    self.link = link;
                    self.opening = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.opening = None,
            }
        }
        if self
            .link
            .as_ref()
            .is_some_and(|l| Some(l.mac.as_str()) != want || !l.open_still())
        {
            self.link = None;
        }
        let Some(mac) = want else {
            self.failed = None;
            return;
        };
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
            .name("omega-airpods-open".into())
            .spawn(move || {
                let _ = tx.send(Link::open(&theirs));
            });
        match spawned {
            Ok(_) => self.opening = Some((mac.to_string(), rx)),
            Err(_) => self.failed = Some((mac.to_string(), now)),
        }
    }

    /// The channel, while one is open.
    pub fn link(&mut self) -> Option<&mut Link> {
        self.link.as_mut()
    }

    pub fn pods(&self) -> Option<Pods> {
        self.link.as_ref().map(Link::pods)
    }

    /// For the menus' tests: AirPods as `pods` says, on a channel that talks to nothing.
    pub fn stand_in(&mut self, mac: &str, pods: Pods) {
        self.link = Some(Link::stand_in(mac, pods));
    }
}

#[cfg(target_os = "linux")]
mod l2cap {
    use std::os::fd::FromRawFd;

    const AF_BLUETOOTH: i32 = 31;
    const SOCK_SEQPACKET: i32 = 5;
    const BTPROTO_L2CAP: i32 = 0;
    const SOCK_CLOEXEC: i32 = 0o2000000;

    /// The kernel's sockaddr_l2: the family, the PSM little-endian, the address with its
    /// bytes the other way round, the channel and the address type (0, classic Bluetooth).
    #[repr(C)]
    struct SockaddrL2 {
        family: u16,
        psm: u16,
        bdaddr: [u8; 6],
        cid: u16,
        bdaddr_type: u8,
    }

    extern "C" {
        fn socket(domain: i32, kind: i32, protocol: i32) -> i32;
        fn connect(fd: i32, addr: *const SockaddrL2, len: u32) -> i32;
        fn close(fd: i32) -> i32;
    }

    /// `mac` as the kernel takes it: its six bytes, last first.
    pub fn bdaddr(mac: &str) -> Option<[u8; 6]> {
        let bytes: Vec<u8> = mac
            .split(':')
            .map(|b| u8::from_str_radix(b, 16))
            .collect::<Result<_, _>>()
            .ok()?;
        let mut out: [u8; 6] = bytes.try_into().ok()?;
        out.reverse();
        Some(out)
    }

    /// A channel to `psm` on the device at `mac`, connected: blocks until it is or is refused.
    pub fn open(mac: &str, psm: u16) -> Option<std::fs::File> {
        let addr = SockaddrL2 {
            family: AF_BLUETOOTH as u16,
            psm: psm.to_le(),
            bdaddr: bdaddr(mac)?,
            cid: 0,
            bdaddr_type: 0,
        };
        // SAFETY: a socket of the kernel's own, closed again where connect fails, and handed to
        // a File, which closes it, where it succeeds; connect reads addr for len bytes
        unsafe {
            let fd = socket(AF_BLUETOOTH, SOCK_SEQPACKET | SOCK_CLOEXEC, BTPROTO_L2CAP);
            if fd < 0 {
                return None;
            }
            let len = std::mem::size_of::<SockaddrL2>() as u32;
            if connect(fd, &addr, len) != 0 {
                close(fd);
                return None;
            }
            Some(std::fs::File::from_raw_fd(fd))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    #[test]
    fn the_battery_packet_gives_each_bud_and_the_case() {
        let said = parse(&hex(
            "04 00 04 00 04 00 03 02 01 64 02 01 04 01 63 01 01 08 01 11 02 01",
        ));
        assert_eq!(
            said,
            Said::Battery {
                right: Some(Battery {
                    level: 100,
                    charging: false
                }),
                left: Some(Battery {
                    level: 99,
                    charging: true
                }),
                case: Some(Battery {
                    level: 17,
                    charging: false
                }),
            }
        );
        let mut pods = Pods::fresh();
        pods.take(said);
        assert_eq!(pods.battery_text(), "L 99%+ R 100% CASE 17%");
    }

    #[test]
    fn a_part_that_is_away_is_left_out() {
        let said = parse(&hex("04 00 04 00 04 00 02 02 01 50 02 01 08 01 00 04 01"));
        assert_eq!(
            said,
            Said::Battery {
                left: None,
                right: Some(Battery {
                    level: 80,
                    charging: false
                }),
                case: None,
            }
        );
    }

    #[test]
    fn noise_control_is_read_and_written_the_same_way() {
        for mode in Noise::ALL {
            assert_eq!(parse(&set_noise(mode)), Said::Noise(mode));
        }
        assert_eq!(
            set_noise(Noise::Cancellation).to_vec(),
            hex("04 00 04 00 09 00 0D 02 00 00 00")
        );
        assert_eq!(parse(&set_conversation(true)), Said::Conversation(true));
        assert_eq!(parse(&set_conversation(false)), Said::Conversation(false));
    }

    #[test]
    fn the_handshake_is_answered_with_the_features_then_the_notifications() {
        let mut pods = Pods::fresh();
        assert_eq!(
            pods.take(parse(&hex("01 00 04 00 00 00"))),
            Some(&SET_FEATURES[..])
        );
        assert!(pods.talking);
        assert_eq!(
            pods.take(parse(&hex("04 00 04 00 2b 00 01"))),
            Some(&REQUEST_NOTIFICATIONS[..])
        );
    }

    #[test]
    fn talking_turns_the_sound_down_until_it_stops() {
        let mut pods = Pods::fresh();
        pods.take(parse(&hex("04 00 04 00 4B 00 02 00 01 01")));
        assert!(pods.duck < 0.5);
        pods.take(parse(&hex("04 00 04 00 4B 00 02 00 01 08")));
        assert_eq!(pods.duck, 1.0);
    }

    #[test]
    fn ears_say_where_each_bud_is() {
        assert_eq!(
            parse(&hex("04 00 04 00 06 00 00 02")),
            Said::Ears(Bud::InEar, Bud::InCase)
        );
    }

    #[test]
    fn airpods_are_known_by_their_name() {
        assert!(is_airpods("Matan's AirPods Pro"));
        assert!(is_airpods("AIRPODS"));
        assert!(!is_airpods("Pixel Buds"));
    }

    #[test]
    fn the_noise_row_steps_round() {
        assert_eq!(Noise::Off.step(true), Noise::Cancellation);
        assert_eq!(Noise::Cancellation.step(false), Noise::Off);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn an_address_goes_to_the_kernel_last_byte_first() {
        assert_eq!(
            l2cap::bdaddr("AA:BB:CC:DD:EE:FF"),
            Some([0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa])
        );
        assert_eq!(l2cap::bdaddr("nope"), None);
    }
}
