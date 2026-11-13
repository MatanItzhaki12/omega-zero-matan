//! RetroArch's netplay lobby, lobby.libretro.com: the rooms open to join, the relays a host
//! behind a home router plays through, and a room of our own put up for others to find.
//!
//! It is plain HTTP, reached with curl as Settings > Update reaches GitHub. /list answers every
//! room as JSON, each room's fields in an object of its own (or wrapped in one called "fields");
//! /tunnel?name= answers a relay's address as `tunnel_addr=` and `tunnel_port=` lines; and /add
//! takes a room as a form, answering `id=` and `connectable=` lines. A room is put up again every
//! twenty seconds while it is open, or the lobby lets it go.
//!
//! The rooms worth showing are gpSP's, with no password, that the lobby could reach.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::json::Json;
use crate::netplay::{CORE_NAME, CORE_VERSION};

pub const LOBBY: &str = "http://lobby.libretro.com/";

/// RetroArch's relays, by the names the lobby knows them by, the first its default.
pub const RELAYS: [&str; 4] = ["nyc", "madrid", "saopaulo", "singapore"];

/// The port a room is said to be on: RetroArch's own. Through a relay no one connects to it.
pub const NETPLAY_PORT: u16 = 55435;

/// How often a room is put up again while it is open.
pub const ANNOUNCE_EVERY: Duration = Duration::from_secs(20);

/// How long the lobby is given to answer.
const ANSWER_SECS: &str = "15";

/// The RetroArch a room says it is hosted by, the one whose protocol is spoken.
const RETROARCH_VERSION: &str = "1.22.2";

/// A room in the lobby.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Room {
    pub username: String,
    pub game_name: String,
    pub game_crc: u32,
    pub core_name: String,
    pub ip: String,
    pub port: u16,
    pub has_password: bool,
    /// The relay it is reached through, and its session there: empty when it is reached
    /// directly.
    pub mitm_ip: String,
    pub mitm_port: u16,
    pub mitm_session: String,
    pub connectable: bool,
    pub country: String,
}

impl Room {
    /// A room a handheld can join: gpSP's, and reachable. One with a password takes it.
    pub fn joinable(&self) -> bool {
        self.core_name.eq_ignore_ascii_case(CORE_NAME) && self.connectable
    }

    /// The relay session to give before joining, when the room is reached through one.
    pub fn relay(&self) -> Option<(String, u16, [u8; 12])> {
        let session = decode_base64(&self.mitm_session)?;
        let session: [u8; 12] = session.try_into().ok()?;
        (!self.mitm_ip.is_empty() && self.mitm_port != 0 && session.iter().any(|&b| b != 0))
            .then(|| (self.mitm_ip.clone(), self.mitm_port, session))
    }
}

fn text(f: &Json, key: &str) -> String {
    match f.get(key) {
        Some(Json::Text(s)) => s.clone(),
        Some(Json::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn number(f: &Json, key: &str) -> u64 {
    f.get(key).and_then(Json::count).unwrap_or(0)
}

fn flag(f: &Json, key: &str, missing: bool) -> bool {
    match f.get(key) {
        None => missing,
        Some(Json::Bool(b)) => *b,
        Some(Json::Number(n)) => *n != 0.0,
        Some(Json::Text(s)) => matches!(s.trim(), "1" | "true" | "True"),
        _ => false,
    }
}

/// The rooms in a /list answer, every one, joinable or not.
pub fn parse_rooms(answer: &str) -> Vec<Room> {
    let Some(list) = Json::parse(answer) else {
        return Vec::new();
    };
    list.list()
        .iter()
        .map(|item| item.get("fields").unwrap_or(item))
        .map(|f| Room {
            username: text(f, "username"),
            game_name: text(f, "game_name"),
            game_crc: match f.get("game_crc") {
                Some(Json::Number(n)) => *n as u32,
                _ => u32::from_str_radix(text(f, "game_crc").trim(), 16).unwrap_or(0),
            },
            core_name: text(f, "core_name"),
            ip: text(f, "ip"),
            port: number(f, "port") as u16,
            has_password: flag(f, "has_password", false),
            mitm_ip: text(f, "mitm_ip"),
            mitm_port: number(f, "mitm_port") as u16,
            mitm_session: text(f, "mitm_session"),
            connectable: flag(f, "connectable", true),
            country: text(f, "country"),
        })
        .collect()
}

/// The `key=value` lines /tunnel and /add answer.
pub fn parse_lines(answer: &str) -> Vec<(String, String)> {
    answer
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(k, _)| !k.trim().is_empty())
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

fn line<'a>(lines: &'a [(String, String)], key: &str) -> Option<&'a str> {
    lines
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// `pairs` as a form: every byte but the unreserved ones written as %XX.
pub fn form(pairs: &[(&str, String)]) -> String {
    let enc = |s: &str| {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect::<String>()
    };
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", enc(k), enc(v)))
        .collect::<Vec<_>>()
        .join("&")
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `bytes` in base64, padded: how the lobby carries a relay session.
pub fn encode_base64(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            match i <= chunk.len() {
                true => out.push(BASE64[(n >> (18 - 6 * i) & 0x3f) as usize] as char),
                false => out.push('='),
            }
        }
    }
    out
}

pub fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut n, mut bits) = (0u32, 0);
    for c in text.trim().bytes().take_while(|&c| c != b'=') {
        let v = BASE64.iter().position(|&b| b == c)? as u32;
        n = n << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((n >> bits) as u8);
        }
    }
    Some(out)
}

/// The lobby's answer to `path`, with `body` posted as a form when there is one: its status and
/// what it said.
fn ask(base: &str, path: &str, body: Option<String>) -> std::io::Result<(u16, String)> {
    let url = format!("{base}{path}");
    let mut args = vec![
        "-s",
        "-m",
        ANSWER_SECS,
        "-A",
        "omega-zero-matan",
        "-w",
        "\n%{http_code}",
    ];
    if body.is_some() {
        args.extend([
            "-H",
            "Content-Type: application/x-www-form-urlencoded",
            "--data-binary",
            "@-",
        ]);
    }
    let mut child = Command::new("curl")
        .args(&args)
        .arg(&url)
        .stdin(match body {
            Some(_) => Stdio::piped(),
            None => Stdio::null(),
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    if let (Some(body), Some(mut stdin)) = (body, child.stdin.take()) {
        stdin.write_all(body.as_bytes())?;
    }
    let mut out = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        stdout.read_to_string(&mut out)?;
    }
    child.wait()?;
    let (said, status) = out
        .trim_end()
        .rsplit_once('\n')
        .unwrap_or_else(|| ("", out.trim()));
    let status = status.trim().parse().unwrap_or(0);
    match status {
        200..=299 => Ok((status, said.to_string())),
        0 => Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "the lobby could not be reached",
        )),
        _ => Err(std::io::Error::other(format!(
            "the lobby answered {status}"
        ))),
    }
}

/// Every room open now, joinable or not.
pub fn rooms(base: &str) -> std::io::Result<Vec<Room>> {
    ask(base, "list", None).map(|(_, said)| parse_rooms(&said))
}

/// Where the relay called `name` is.
pub fn tunnel(base: &str, name: &str) -> std::io::Result<(String, u16)> {
    let (_, said) = ask(base, &format!("tunnel?name={}", form_value(name)), None)?;
    let lines = parse_lines(&said);
    let addr = line(&lines, "tunnel_addr").unwrap_or_default();
    let port = line(&lines, "tunnel_port").and_then(|p| p.parse::<u16>().ok());
    match (addr.is_empty(), port) {
        (false, Some(port)) if port != 0 => Ok((addr.to_string(), port)),
        _ => Err(std::io::Error::other(format!("no relay called {name}"))),
    }
}

fn form_value(v: &str) -> String {
    form(&[("v", v.to_string())])
        .trim_start_matches("v=")
        .to_string()
}

/// A room of ours, as the lobby is told of it.
#[derive(Clone, Debug)]
pub struct Announce {
    pub username: String,
    pub game_name: String,
    pub game_crc: u32,
    /// The relay it plays through, by name, and its session there.
    pub relay: String,
    pub session: [u8; 12],
    /// The room has a password.
    pub password: bool,
}

impl Announce {
    fn form(&self, players: u8) -> String {
        form(&[
            ("username", self.username.clone()),
            ("core_name", CORE_NAME.to_string()),
            ("core_version", CORE_VERSION.to_string()),
            ("game_name", self.game_name.clone()),
            ("game_crc", format!("{:08X}", self.game_crc)),
            ("port", NETPLAY_PORT.to_string()),
            ("mitm_server", self.relay.clone()),
            ("has_password", if self.password { "1" } else { "0" }.into()),
            ("has_spectate_password", "0".into()),
            ("force_mitm", "1".into()),
            ("retroarch_version", RETROARCH_VERSION.into()),
            ("frontend", "unix aarch64".into()),
            ("subsystem_name", "N/A".into()),
            ("mitm_session", encode_base64(&self.session)),
            ("mitm_custom_addr", String::new()),
            ("mitm_custom_port", "0".into()),
            ("player_count", players.to_string()),
            ("spectator_count", "0".into()),
        ])
    }
}

/// Puts `room` up in the lobby: the lobby's id for it, and whether it could reach it.
pub fn announce(base: &str, room: &Announce, players: u8) -> std::io::Result<(u64, bool)> {
    let (_, said) = ask(base, "add", Some(room.form(players)))?;
    let lines = parse_lines(&said);
    let id = line(&lines, "id").and_then(|v| v.parse().ok()).unwrap_or(0);
    let connectable =
        line(&lines, "connectable").is_none_or(|v| matches!(v, "1" | "true" | "True"));
    Ok((id, connectable))
}

/// A room kept up in the lobby, put up again every [`ANNOUNCE_EVERY`] until it is dropped.
pub struct Announcer {
    stop: Arc<AtomicBool>,
    players: Arc<AtomicU8>,
}

impl Announcer {
    pub fn start(base: &str, room: Announce) -> Announcer {
        let stop = Arc::new(AtomicBool::new(false));
        let players = Arc::new(AtomicU8::new(1));
        let (until, count, base) = (stop.clone(), players.clone(), base.to_string());
        let _ = std::thread::Builder::new()
            .name("omega-lobby".into())
            .spawn(move || {
                while !until.load(Ordering::Acquire) {
                    if let Err(e) = announce(&base, &room, count.load(Ordering::Relaxed)) {
                        crate::log!("netplay: the lobby: {e}");
                    }
                    // waits in short steps, so a room closed is let go at once
                    for _ in 0..ANNOUNCE_EVERY.as_secs() * 4 {
                        if until.load(Ordering::Acquire) {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(250));
                    }
                }
            });
        Announcer { stop, players }
    }

    /// How many play in the room now, the host among them.
    pub fn set_players(&self, n: u8) {
        self.players.store(n, Ordering::Relaxed);
    }
}

impl Drop for Announcer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rooms_are_read_wrapped_in_fields_or_not_and_only_gpsps_open_ones_are_joinable() {
        let answer = r#"[
            {"fields": {"username": "Ash", "core_name": "gpSP", "game_name": "Pokemon - Emerald Version (USA, Europe)",
              "game_crc": "1F1C08FB", "ip": "1.2.3.4", "port": 55435, "has_password": false,
              "mitm_ip": "5.6.7.8", "mitm_port": 55435, "mitm_session": "AAECAwQFBgcICQoL",
              "connectable": true, "country": "us"}},
            {"username": "Misty", "core_name": "mGBA", "game_crc": "00000000", "connectable": true},
            {"username": "Brock", "core_name": "gpsp", "has_password": 1, "connectable": true},
            {"username": "Gary", "core_name": "gpSP", "connectable": false}
        ]"#;
        let rooms = parse_rooms(answer);
        assert_eq!(rooms.len(), 4);
        let ash = &rooms[0];
        assert_eq!(ash.username, "Ash");
        assert_eq!(ash.game_crc, 0x1F1C_08FB);
        assert_eq!(ash.port, 55435);
        assert_eq!(
            ash.relay(),
            Some((
                "5.6.7.8".into(),
                55435,
                [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
            ))
        );
        let joinable: Vec<_> = rooms
            .iter()
            .filter(|r| r.joinable())
            .map(|r| r.username.as_str())
            .collect();
        assert_eq!(
            joinable,
            ["Ash", "Brock"],
            "mGBA's and one unreachable: one with a password takes it"
        );
    }

    #[test]
    fn a_room_with_no_relay_session_is_reached_directly() {
        let rooms = parse_rooms(r#"[{"core_name": "gpSP", "mitm_session": "", "ip": "1.2.3.4"}]"#);
        assert_eq!(rooms[0].relay(), None);
        assert!(parse_rooms("not json").is_empty());
    }

    #[test]
    fn base64_goes_both_ways_as_the_lobby_writes_it() {
        for (bytes, text) in [
            (&b""[..], ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (
                &[0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
                "AAECAwQFBgcICQoL",
            ),
        ] {
            assert_eq!(encode_base64(bytes), text);
            assert_eq!(decode_base64(text).as_deref(), Some(bytes));
        }
        assert_eq!(decode_base64("not base64!"), None);
    }

    #[test]
    fn a_form_escapes_all_but_the_unreserved() {
        assert_eq!(
            form(&[
                ("game_name", "Pokémon - Emerald (USA)".into()),
                ("a", "b&c=d".into())
            ]),
            "game_name=Pok%C3%A9mon%20-%20Emerald%20%28USA%29&a=b%26c%3Dd"
        );
    }

    #[test]
    fn a_room_put_up_names_gpsp_its_relay_and_its_session() {
        let room = Announce {
            username: "RG SP".into(),
            game_name: "Emerald".into(),
            game_crc: 0x1F1C_08FB,
            relay: "madrid".into(),
            session: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            password: false,
        };
        let sent = room.form(1);
        for field in [
            "username=RG%20SP",
            "core_name=gpSP",
            "core_version=gpSP%20v1.0",
            "game_crc=1F1C08FB",
            "mitm_server=madrid",
            "mitm_session=AAECAwQFBgcICQoL",
            "force_mitm=1",
            "has_password=0",
            "player_count=1",
        ] {
            assert!(
                sent.split('&').any(|f| f == field),
                "{field} missing from {sent}"
            );
        }
    }

    #[test]
    fn the_lines_tunnel_and_add_answer_are_read() {
        let lines = parse_lines("tunnel_addr=1.2.3.4\r\ntunnel_port=55435\nnoise\n=x\n");
        assert_eq!(line(&lines, "tunnel_addr"), Some("1.2.3.4"));
        assert_eq!(line(&lines, "tunnel_port"), Some("55435"));
        assert_eq!(lines.len(), 2);
    }

    /// The real lobby, read only: its rooms read and a relay found. Never run by CI.
    #[test]
    #[ignore = "reaches lobby.libretro.com"]
    fn the_real_lobby_lists_rooms_and_finds_a_relay() {
        let rooms = rooms(LOBBY).expect("the lobby's list");
        let gpsp = rooms
            .iter()
            .filter(|r| r.core_name.eq_ignore_ascii_case(CORE_NAME))
            .count();
        let joinable = rooms.iter().filter(|r| r.joinable()).count();
        eprintln!(
            "{} rooms, {gpsp} of them gpSP's, {joinable} joinable",
            rooms.len()
        );
        for name in RELAYS {
            let (addr, port) = tunnel(LOBBY, name).expect("a relay's address");
            eprintln!("relay {name}: {addr}:{port}");
        }
    }
}
