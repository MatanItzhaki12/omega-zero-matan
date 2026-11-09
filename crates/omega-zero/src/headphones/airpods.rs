//! AirPods' protocol, over an L2CAP channel of Apple's own, PSM 0x1001, as LibrePods worked it
//! out (github.com/librepods-org/librepods, GPL-3.0, whose `docs/AAP Definitions.md` this
//! follows): a handshake, a packet that turns on what newer AirPods keep for Apple's own
//! devices, and a request for notifications, after which the AirPods say their battery, noise
//! control mode, which bud is in an ear, and when their wearer starts and stops talking.

use std::io::{Read, Write};
use std::sync::Mutex;

use super::{Battery, Noise, Phones};

pub const PSM: u16 = 0x1001;

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
const ADAPTIVE_LEVEL: u8 = 0x2e;
const BATTERY: u8 = 0x04;
const EARS: u8 = 0x06;
const SPEAKING: u8 = 0x4b;

fn noise_byte(mode: Noise) -> u8 {
    match mode {
        Noise::Off => 1,
        Noise::Cancellation => 2,
        Noise::Transparency => 3,
        Noise::Adaptive => 4,
    }
}

fn noise_of(byte: u8) -> Option<Noise> {
    Some(match byte {
        1 => Noise::Off,
        2 => Noise::Cancellation,
        3 => Noise::Transparency,
        4 => Noise::Adaptive,
        _ => return None,
    })
}

fn setting(which: u8, value: u8) -> [u8; 11] {
    [
        0x04, 0x00, 0x04, 0x00, SETTING, 0x00, which, value, 0x00, 0x00, 0x00,
    ]
}

/// The packet that sets the noise control mode.
pub fn set_noise(mode: Noise) -> [u8; 11] {
    setting(NOISE_CONTROL, noise_byte(mode))
}

/// The packet that turns conversational awareness on or off.
pub fn set_conversation(on: bool) -> [u8; 11] {
    setting(CONVERSATION, if on { 1 } else { 2 })
}

/// The packet that sets how much of the room Adaptive lets in, 0 to 100.
pub fn set_adaptive_level(level: u8) -> [u8; 11] {
    setting(ADAPTIVE_LEVEL, level.min(100))
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
    AdaptiveLevel(u8),
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
            NOISE_CONTROL => noise_of(body[1]).map_or(Said::Other, Said::Noise),
            CONVERSATION => match body[1] {
                1 => Said::Conversation(true),
                2 => Said::Conversation(false),
                _ => Said::Other,
            },
            ADAPTIVE_LEVEL if body[1] <= 100 => Said::AdaptiveLevel(body[1]),
            _ => Said::Other,
        },
        EARS if body.len() >= 2 => Said::Ears(bud(body[0]), bud(body[1])),
        // 4b 00 02 00 01 [level]
        SPEAKING if body.len() >= 4 => Said::Speaking(body[3]),
        _ => Said::Other,
    }
}

/// Takes in what a packet said; what it answers, if anything.
pub fn take(p: &mut Phones, said: Said) -> Option<&'static [u8]> {
    if said != Said::Other {
        p.heard = true;
    }
    match said {
        Said::HandshakeDone => return Some(&SET_FEATURES),
        Said::FeaturesDone => return Some(&REQUEST_NOTIFICATIONS),
        Said::Battery { left, right, case } => {
            p.left = left;
            p.right = right;
            // the case says nothing once both buds are out of it: keep what it last said
            if case.is_some() {
                p.case = case;
            }
            p.beacon = false;
        }
        Said::Noise(n) => p.said_noise(n),
        Said::Conversation(on) => p.conversation = Some(on),
        Said::AdaptiveLevel(level) => p.adaptive_level = Some(level),
        Said::Ears(..) => {}
        Said::Speaking(level) => {
            p.duck = match level {
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

/// Opens the conversation on `sock` and reads it until it closes. The packets that open it are
/// sent again as the AirPods answer each, but also sent straight after the handshake, for
/// AirPods whose answer to it is not the one LibrePods saw.
pub fn read(mut sock: std::fs::File, mut answer: std::fs::File, phones: &Mutex<Phones>) {
    for packet in [&HANDSHAKE[..], &SET_FEATURES, &REQUEST_NOTIFICATIONS] {
        if answer.write_all(packet).is_err() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    // the notifications asked for again each second until the AirPods say something of
    // their own: some take the first ask before they are ready for it
    super::sock::read_timeout(&sock, Some(std::time::Duration::from_secs(1)));
    let mut asked = 1;
    let mut buf = [0u8; 1024];
    loop {
        let n = match sock.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => n,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                let told = phones.lock().map(|p| p.left.is_some() || p.noise.is_some());
                if told.unwrap_or(true) || asked >= 8 {
                    super::sock::read_timeout(&sock, None);
                } else {
                    asked += 1;
                    for packet in [&SET_FEATURES[..], &REQUEST_NOTIFICATIONS] {
                        if answer.write_all(packet).is_err() {
                            return;
                        }
                    }
                }
                continue;
            }
            Err(_) => return,
        };
        let said = parse(&buf[..n]);
        let reply = phones.lock().ok().and_then(|mut p| take(&mut p, said));
        if let Some(reply) = reply {
            if answer.write_all(reply).is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headphones::Kind;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    #[test]
    fn the_battery_packet_gives_each_bud_and_the_case() {
        let mut p = Phones::new(Kind::AirPods);
        take(
            &mut p,
            parse(&hex(
                "04 00 04 00 04 00 03 02 01 64 02 01 04 01 63 01 01 08 01 11 02 01",
            )),
        );
        assert_eq!(p.battery_text(), "L 99%+ R 100% CASE 17%");
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
    fn settings_are_read_and_written_the_same_way() {
        for mode in [
            Noise::Off,
            Noise::Cancellation,
            Noise::Transparency,
            Noise::Adaptive,
        ] {
            assert_eq!(parse(&set_noise(mode)), Said::Noise(mode));
        }
        assert_eq!(
            set_noise(Noise::Cancellation).to_vec(),
            hex("04 00 04 00 09 00 0D 02 00 00 00")
        );
        assert_eq!(parse(&set_conversation(true)), Said::Conversation(true));
        assert_eq!(parse(&set_conversation(false)), Said::Conversation(false));
        assert_eq!(parse(&set_adaptive_level(40)), Said::AdaptiveLevel(40));
    }

    #[test]
    fn the_handshake_is_answered_with_the_features_then_the_notifications() {
        let mut p = Phones::new(Kind::AirPods);
        assert_eq!(
            take(&mut p, parse(&hex("01 00 04 00 00 00"))),
            Some(&SET_FEATURES[..])
        );
        assert_eq!(
            take(&mut p, parse(&hex("04 00 04 00 2b 00 01"))),
            Some(&REQUEST_NOTIFICATIONS[..])
        );
    }

    #[test]
    fn talking_turns_the_sound_down_until_it_stops() {
        let mut p = Phones::new(Kind::AirPods);
        take(&mut p, parse(&hex("04 00 04 00 4B 00 02 00 01 01")));
        assert!(p.duck < 0.5);
        take(&mut p, parse(&hex("04 00 04 00 4B 00 02 00 01 08")));
        assert_eq!(p.duck, 1.0);
    }

    #[test]
    fn ears_say_where_each_bud_is() {
        assert_eq!(
            parse(&hex("04 00 04 00 06 00 00 02")),
            Said::Ears(Bud::InEar, Bud::InCase)
        );
    }
}
