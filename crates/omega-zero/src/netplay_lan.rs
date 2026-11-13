//! RetroArch's netplay on a local network, as RetroArch finds its rooms there: whoever looks
//! sends "RANQ" to UDP port 55435 of every host on the network, and each room that is not
//! opened through a relay answers with an ad, "RANS", saying who hosts it, what they play and
//! the TCP port to reach them on. The link itself is RetroArch's netplay, as online, but
//! straight between the two, with no relay or lobby in between.
//!
//! The ad is RetroArch's `struct ad_packet`: four words in network order (the magic, the
//! game's checksum, the port, and the passwords set: 1 a player's, 2 a spectator's), then the
//! host's name in 32 bytes, the frontend, core, core version and RetroArch version in 32
//! each, and the game and subsystem in 256 each, 688 bytes in all.

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::netplay::{CORE_NAME, CORE_VERSION};
use crate::netplay_lobby::Room;

/// The UDP port RetroArch asks for rooms on, and the TCP port its rooms are opened on.
pub const DISCOVERY_PORT: u16 = 55435;

const QUERY: u32 = 0x5241_4e51;
const RESPONSE: u32 = 0x5241_4e53;

const NICK_LEN: usize = 32;
const STR_LEN: usize = 32;
const LONG_LEN: usize = 256;
/// An ad's length: RetroArch takes none of another.
pub const AD_LEN: usize = 16 + NICK_LEN + 4 * STR_LEN + 2 * LONG_LEN;

const RETROARCH_VERSION: &str = "1.22.2";

/// What a room says of itself on the network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ad {
    pub nick: String,
    pub game: String,
    pub crc: u32,
    pub port: u16,
    pub password: bool,
}

impl Ad {
    /// The ad as RetroArch reads it, the core gpSP's as RetroArch names it.
    pub fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(AD_LEN);
        out.extend(RESPONSE.to_be_bytes());
        out.extend(self.crc.to_be_bytes());
        out.extend(u32::from(self.port).to_be_bytes());
        out.extend(u32::from(self.password).to_be_bytes());
        for (text, len) in [
            (self.nick.as_str(), NICK_LEN),
            ("unix aarch64", STR_LEN),
            (CORE_NAME, STR_LEN),
            (CORE_VERSION, STR_LEN),
            (RETROARCH_VERSION, STR_LEN),
            (self.game.as_str(), LONG_LEN),
            ("N/A", LONG_LEN),
        ] {
            out.extend(field(text, len));
        }
        out
    }
}

/// `text` in a field of `len` bytes, cut on a character's edge so a NUL always ends it.
fn field(text: &str, len: usize) -> Vec<u8> {
    let mut end = text.len().min(len - 1);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = text.as_bytes()[..end].to_vec();
    out.resize(len, 0);
    out
}

fn text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(bytes[at..at + 4].try_into().expect("four bytes"))
}

/// The room an ad from `from` tells of, as the lobby would list it: reached straight, with no
/// relay. None for what is not an ad.
pub fn parse_ad(bytes: &[u8], from: IpAddr) -> Option<Room> {
    if bytes.len() != AD_LEN || word(bytes, 0) != RESPONSE {
        return None;
    }
    let port = word(bytes, 8);
    let port = u16::try_from(port).ok().filter(|p| *p != 0)?;
    let mut at = 16;
    let mut next = |len: usize| {
        let s = text(&bytes[at..at + len]);
        at += len;
        s
    };
    let nick = next(NICK_LEN);
    let _frontend = next(STR_LEN);
    let core = next(STR_LEN);
    let _core_version = next(STR_LEN);
    let _version = next(STR_LEN);
    let game = next(LONG_LEN);
    Some(Room {
        username: nick,
        game_name: game,
        game_crc: word(bytes, 4),
        core_name: core,
        ip: from.to_string(),
        port,
        has_password: word(bytes, 12) & 1 != 0,
        mitm_ip: String::new(),
        mitm_port: 0,
        mitm_session: String::new(),
        connectable: true,
        country: String::new(),
    })
}

/// An address on this network, as RetroArch answers only those: a private one, or this
/// handheld's own.
fn local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.to_ipv4_mapped().is_some_and(|v4| local(IpAddr::V4(v4))),
    }
}

/// A room's ad answered to whoever asks on the network, until it is dropped.
pub struct Advertiser {
    stop: Arc<AtomicBool>,
}

impl Advertiser {
    /// Answers on RetroArch's own port.
    pub fn start(ad: Ad) -> io::Result<Advertiser> {
        Advertiser::start_on(DISCOVERY_PORT, ad)
    }

    /// Answers on `port`, which a test takes at random.
    pub fn start_on(port: u16, ad: Ad) -> io::Result<Advertiser> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port))?;
        socket.set_read_timeout(Some(Duration::from_millis(200)))?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let answer = ad.bytes();
        std::thread::Builder::new()
            .name("omega-lan-ad".into())
            .spawn(move || {
                let mut buf = [0u8; 16];
                while !stopped.load(Ordering::Relaxed) {
                    let Ok((n, from)) = socket.recv_from(&mut buf) else {
                        continue;
                    };
                    if n == 4 && word(&buf, 0) == QUERY && local(from.ip()) {
                        let _ = socket.send_to(&answer, from);
                    }
                }
            })?;
        Ok(Advertiser { stop })
    }
}

impl Drop for Advertiser {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// The rooms on this network that answer within `wait`, each once.
pub fn discover(wait: Duration) -> io::Result<Vec<Room>> {
    discover_at(
        SocketAddr::from((Ipv4Addr::BROADCAST, DISCOVERY_PORT)),
        wait,
    )
}

/// As [`discover`], asking `target` alone, which a test points at its own room.
pub fn discover_at(target: SocketAddr, wait: Duration) -> io::Result<Vec<Room>> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(Duration::from_millis(100)))?;
    let query = QUERY.to_be_bytes();
    let until = Instant::now() + wait;
    let mut asked = Instant::now().checked_sub(wait).unwrap();
    let mut rooms: Vec<Room> = Vec::new();
    let mut buf = vec![0u8; AD_LEN + 1];
    while Instant::now() < until {
        // asked again now and then: a datagram can be lost
        if asked.elapsed() >= Duration::from_millis(400) {
            socket.send_to(&query, target)?;
            asked = Instant::now();
        }
        let Ok((n, from)) = socket.recv_from(&mut buf) else {
            continue;
        };
        let room = match parse_ad(&buf[..n], from.ip()) {
            Some(room) if local(from.ip()) => room,
            _ => continue,
        };
        if !rooms.iter().any(|r| r.ip == room.ip && r.port == room.port) {
            rooms.push(room);
        }
    }
    Ok(rooms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ad() -> Ad {
        Ad {
            nick: "Ash".into(),
            game: "Pokemon - Emerald Version (USA, Europe)".into(),
            crc: 0x1F1C_08FB,
            port: 55435,
            password: true,
        }
    }

    #[test]
    fn an_ad_is_retroarchs_688_bytes_and_reads_back() {
        let bytes = ad().bytes();
        assert_eq!(bytes.len(), 688);
        assert_eq!(&bytes[..4], b"RANS");
        let room = parse_ad(&bytes, "192.168.1.20".parse().unwrap()).expect("an ad");
        assert_eq!(room.username, "Ash");
        assert_eq!(room.game_name, "Pokemon - Emerald Version (USA, Europe)");
        assert_eq!(room.game_crc, 0x1F1C_08FB);
        assert_eq!(room.core_name, "gpSP");
        assert_eq!((room.ip.as_str(), room.port), ("192.168.1.20", 55435));
        assert!(room.has_password);
        assert!(
            room.relay().is_none(),
            "a room on the network is reached straight"
        );
        assert!(room.joinable());
    }

    #[test]
    fn what_is_not_an_ad_is_no_room() {
        let from: IpAddr = "192.168.1.20".parse().unwrap();
        assert_eq!(parse_ad(b"RANQ", from), None);
        let mut wrong = ad().bytes();
        wrong[3] = b'Q';
        assert_eq!(parse_ad(&wrong, from), None);
        let mut no_port = ad().bytes();
        no_port[8..12].copy_from_slice(&0u32.to_be_bytes());
        assert_eq!(parse_ad(&no_port, from), None);
    }

    #[test]
    fn a_long_name_is_cut_with_its_nul_kept() {
        let long = Ad {
            nick: "é".repeat(40),
            ..ad()
        };
        let room = parse_ad(&long.bytes(), "10.0.0.2".parse().unwrap()).unwrap();
        assert_eq!(room.username, "é".repeat(15));
    }

    #[test]
    fn only_this_network_is_answered() {
        assert!(local("192.168.1.2".parse().unwrap()));
        assert!(local("10.1.2.3".parse().unwrap()));
        assert!(local("127.0.0.1".parse().unwrap()));
        assert!(!local("8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn a_room_is_found_by_asking_where_it_answers() {
        let port = {
            let probe = UdpSocket::bind("127.0.0.1:0").unwrap();
            probe.local_addr().unwrap().port()
        };
        let _ad = Advertiser::start_on(port, ad()).expect("the ad's socket");
        let rooms = discover_at(
            SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
            Duration::from_millis(600),
        )
        .expect("the query");
        assert_eq!(rooms.len(), 1, "{rooms:?}");
        assert_eq!(rooms[0].username, "Ash");
        assert_eq!(rooms[0].ip, "127.0.0.1");
    }
}
