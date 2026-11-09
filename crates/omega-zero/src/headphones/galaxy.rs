//! Galaxy Buds' protocol, over RFCOMM, as GalaxyBudsClient worked it out
//! (github.com/timschneeb/GalaxyBudsClient, GPL-3.0): each message is framed by 0xFD and 0xDD,
//! its length in two bytes little-endian after the first, then its id, what it carries and a
//! CRC-16 (CCITT, XMODEM's) of the id and what it carries. The Buds say their battery as they
//! connect and as it changes, and their noise control as it changes; the handheld sets noise
//! control. The first Galaxy Buds, of 2019, frame theirs otherwise, and are not taken.

use std::io::Read;
use std::sync::Mutex;
use std::time::Duration;

use super::{Battery, Noise, Phones};

const SOM: u8 = 0xFD;
const EOM: u8 = 0xDD;

const STATUS_UPDATED: u8 = 96;
const EXTENDED_STATUS_UPDATED: u8 = 97;
const NOISE_CONTROLS_UPDATE: u8 = 119;
const NOISE_CONTROLS: u8 = 120;
const SET_AMBIENT_MODE: u8 = 128;
const AMBIENT_MODE_UPDATED: u8 = 129;
const SET_NOISE_REDUCTION: u8 = 152;
const NOISE_REDUCTION_MODE_UPDATE: u8 = 155;

/// The service the Buds+ and every pair since talk on, which SDP gives the channel of.
pub const SPP_UUID: &str = "2e73a4ad-332d-41fc-90e2-16bef06523f2";

/// Which Galaxy Buds, by their name: what noise control each has, and how it is set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    /// Ambient sound, on or off.
    BudsPlus,
    /// Noise cancelling, on or off.
    BudsLive,
    BudsPro,
    Buds2,
    Buds2Pro,
    BudsFe,
    Buds3,
    Buds3Pro,
    /// One named otherwise, since: taken as the newest are.
    Newer,
}

impl Model {
    /// The model a device by this name, in lower case, is.
    pub fn of(lower: &str) -> Option<Model> {
        let at = lower.find("galaxy buds")?;
        let rest = lower[at + "galaxy buds".len()..].trim_start();
        let is = |s: &str| rest.starts_with(s);
        Some(if is("3 pro") || is("3pro") {
            Model::Buds3Pro
        } else if is("3") {
            Model::Buds3
        } else if is("2 pro") || is("2pro") {
            Model::Buds2Pro
        } else if is("2") {
            Model::Buds2
        } else if is("pro") {
            Model::BudsPro
        } else if is("fe") {
            Model::BudsFe
        } else if is("live") {
            Model::BudsLive
        } else if is("+") || is("plus") {
            Model::BudsPlus
        } else if rest.is_empty() || is("(") {
            // the first Galaxy Buds, which frame their messages otherwise
            return None;
        } else {
            Model::Newer
        })
    }

    pub fn noise_modes(self) -> &'static [Noise] {
        match self {
            Model::BudsPlus => &[Noise::Transparency, Noise::Off],
            Model::BudsLive => &[Noise::Cancellation, Noise::Off],
            Model::Buds3Pro | Model::Newer => &[
                Noise::Cancellation,
                Noise::Adaptive,
                Noise::Transparency,
                Noise::Off,
            ],
            _ => &[Noise::Cancellation, Noise::Transparency, Noise::Off],
        }
    }
}

/// CRC-16/XMODEM, which the Buds check each message by.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &b in data {
        crc ^= u16::from(b) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// A message to the Buds: `id` carrying `payload`, framed.
pub fn frame(id: u8, payload: &[u8]) -> Vec<u8> {
    let size = (1 + payload.len() + 2) as u16;
    let mut body = vec![id];
    body.extend_from_slice(payload);
    let crc = crc16(&body);
    let mut out = vec![SOM];
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc.to_le_bytes());
    out.push(EOM);
    out
}

/// The messages that set `mode` on Buds of `model`.
pub fn set_noise(model: Model, mode: Noise) -> Vec<Vec<u8>> {
    match model {
        Model::BudsPlus => vec![frame(
            SET_AMBIENT_MODE,
            &[u8::from(mode == Noise::Transparency)],
        )],
        Model::BudsLive => vec![frame(
            SET_NOISE_REDUCTION,
            &[u8::from(mode == Noise::Cancellation)],
        )],
        _ => vec![frame(NOISE_CONTROLS, &[noise_byte(mode)])],
    }
}

fn noise_byte(mode: Noise) -> u8 {
    match mode {
        Noise::Off => 0,
        Noise::Cancellation => 1,
        Noise::Transparency => 2,
        Noise::Adaptive => 3,
    }
}

fn noise_of(byte: u8) -> Option<Noise> {
    Some(match byte {
        0 => Noise::Off,
        1 => Noise::Cancellation,
        2 => Noise::Transparency,
        3 => Noise::Adaptive,
        _ => return None,
    })
}

/// One message from the Buds: its id and what it carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub id: u8,
    pub payload: Vec<u8>,
}

/// Takes the messages whole at the front of `buf` out of it: bytes before a start, and a
/// message whose checksum is wrong, are dropped; a message not all come yet is left.
pub fn unframe(buf: &mut Vec<u8>) -> Vec<Message> {
    let mut out = Vec::new();
    loop {
        match buf.iter().position(|&b| b == SOM) {
            Some(0) => {}
            Some(at) => {
                buf.drain(..at);
            }
            None => {
                buf.clear();
                return out;
            }
        }
        if buf.len() < 3 {
            return out;
        }
        let size = (u16::from_le_bytes([buf[1], buf[2]]) & 0x3ff) as usize;
        let total = 3 + size + 1;
        if size < 3 {
            buf.drain(..1);
            continue;
        }
        if buf.len() < total {
            return out;
        }
        let body = &buf[3..3 + size - 2];
        let crc = u16::from_le_bytes([buf[3 + size - 2], buf[3 + size - 1]]);
        if buf[total - 1] == EOM && crc16(body) == crc {
            out.push(Message {
                id: body[0],
                payload: body[1..].to_vec(),
            });
            buf.drain(..total);
        } else {
            buf.drain(..1);
        }
    }
}

/// Takes in what a message from the Buds said.
pub fn take(p: &mut Phones, m: &Message) {
    let b = &m.payload;
    let battery = |level: u8, charging: bool| (level <= 100).then_some(Battery { level, charging });
    match m.id {
        // revision, left, right, coupled, main, placement, case, charging
        STATUS_UPDATED if b.len() >= 7 => {
            let charging = b.get(7).copied().unwrap_or(0);
            p.left = battery(b[1], charging & 0x10 != 0);
            p.right = battery(b[2], charging & 0x04 != 0);
            p.case = battery(b[6], charging & 0x01 != 0).or(p.case);
        }
        // revision, ear type, left, right, coupled, main, placement, case
        EXTENDED_STATUS_UPDATED if b.len() >= 8 => {
            p.left = battery(b[2], false);
            p.right = battery(b[3], false);
            p.case = battery(b[7], false).or(p.case);
        }
        NOISE_CONTROLS_UPDATE if !b.is_empty() => {
            if let Some(mode) = noise_of(b[0]) {
                p.noise = Some(mode);
            }
        }
        AMBIENT_MODE_UPDATED if !b.is_empty() => {
            p.noise = Some(if b[0] != 0 {
                Noise::Transparency
            } else {
                Noise::Off
            });
        }
        NOISE_REDUCTION_MODE_UPDATE if !b.is_empty() => {
            p.noise = Some(if b[0] != 0 {
                Noise::Cancellation
            } else {
                Noise::Off
            });
        }
        _ => {}
    }
}

/// Reads `sock` until it closes.
pub fn read(mut sock: std::fs::File, phones: &Mutex<Phones>) {
    super::sock::read_timeout(&sock, None);
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let n = match sock.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        buf.extend_from_slice(&chunk[..n]);
        for m in unframe(&mut buf) {
            if let Ok(mut p) = phones.lock() {
                take(&mut p, &m);
            }
        }
    }
}

/// The RFCOMM channel `sdptool` says the Buds at `mac` talk on, where it is on the handheld.
fn sdp_channel(mac: &str) -> Option<u8> {
    let out = std::process::Command::new("sdptool")
        .args(["search", "--bdaddr", mac, SPP_UUID])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    channel_in(&String::from_utf8_lossy(&out.stdout))
}

/// The first `Channel: N` in what `sdptool` printed.
pub fn channel_in(text: &str) -> Option<u8> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("Channel:"))
        .and_then(|n| n.trim().parse().ok())
}

/// Opens the channel to the Buds at `mac`: the one SDP names, else each in turn until one
/// answers as Buds do, with a message framed as theirs, which they send as they connect.
pub fn open(mac: &str) -> std::io::Result<std::fs::File> {
    if let Some(ch) = sdp_channel(mac) {
        return super::sock::rfcomm(mac, ch);
    }
    let mut last = std::io::Error::from(std::io::ErrorKind::NotFound);
    for ch in 1..=30u8 {
        let mut sock = match super::sock::rfcomm(mac, ch) {
            Ok(sock) => sock,
            Err(e) => {
                last = e;
                continue;
            }
        };
        super::sock::read_timeout(&sock, Some(Duration::from_millis(2500)));
        let mut buf = Vec::new();
        let mut chunk = [0u8; 256];
        if let Ok(n) = sock.read(&mut chunk) {
            buf.extend_from_slice(&chunk[..n]);
            if !unframe(&mut buf).is_empty() {
                eprintln!("omega-zero: headphones: Galaxy Buds talk on channel {ch}");
                return Ok(sock);
            }
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headphones::Kind;

    #[test]
    fn the_checksum_is_xmodem_s() {
        assert_eq!(crc16(b"123456789"), 0x31c3);
    }

    #[test]
    fn a_message_comes_back_out_of_its_frame() {
        let mut buf = vec![0x00, 0x11];
        buf.extend(frame(NOISE_CONTROLS_UPDATE, &[1]));
        let half = frame(STATUS_UPDATED, &[13, 80, 75, 1, 0, 0x33, 40, 0x10]);
        buf.extend(&half[..5]);
        let got = unframe(&mut buf);
        assert_eq!(
            got,
            [Message {
                id: NOISE_CONTROLS_UPDATE,
                payload: vec![1]
            }]
        );
        buf.extend(&half[5..]);
        let got = unframe(&mut buf);
        assert_eq!(got[0].id, STATUS_UPDATED);
        let mut p = Phones::new(Kind::Buds(Model::Buds2Pro));
        take(&mut p, &got[0]);
        assert_eq!(p.battery_text(), "L 80%+ R 75% CASE 40%");
        assert!(buf.is_empty());
    }

    #[test]
    fn a_message_with_a_bad_checksum_is_dropped() {
        let mut m = frame(NOISE_CONTROLS_UPDATE, &[2]);
        m[4] ^= 1;
        let mut buf = m;
        assert!(unframe(&mut buf).is_empty());
    }

    #[test]
    fn noise_control_is_set_as_each_model_takes_it() {
        assert_eq!(
            set_noise(Model::Buds2Pro, Noise::Transparency),
            [frame(NOISE_CONTROLS, &[2])]
        );
        assert_eq!(
            set_noise(Model::BudsPlus, Noise::Transparency),
            [frame(SET_AMBIENT_MODE, &[1])]
        );
        assert_eq!(
            set_noise(Model::BudsLive, Noise::Off),
            [frame(SET_NOISE_REDUCTION, &[0])]
        );
    }

    #[test]
    fn models_are_known_by_their_name() {
        assert_eq!(Model::of("galaxy buds3 pro (9f3c)"), Some(Model::Buds3Pro));
        assert_eq!(Model::of("galaxy buds2 (1a2b)"), Some(Model::Buds2));
        assert_eq!(Model::of("galaxy buds+ (1a2b)"), Some(Model::BudsPlus));
        assert_eq!(Model::of("galaxy buds live (1a2b)"), Some(Model::BudsLive));
        assert_eq!(Model::of("galaxy buds fe (1a2b)"), Some(Model::BudsFe));
        assert_eq!(Model::of("galaxy buds core (1a2b)"), Some(Model::Newer));
        assert_eq!(
            Model::of("galaxy buds (1a2b)"),
            None,
            "the first, framed otherwise"
        );
        assert_eq!(Model::of("matan's airpods"), None);
        assert_eq!(Model::of("pixel buds pro"), None);
    }

    #[test]
    fn sdptool_names_the_channel() {
        let text = "Searching for 2e73a4ad on 1A:2B ...\nService Name: SPP\n  Protocol Descriptor List:\n    \"RFCOMM\" (0x0003)\n      Channel: 7\n";
        assert_eq!(channel_in(text), Some(7));
    }
}
