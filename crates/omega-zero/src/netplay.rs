//! RetroArch's netplay, as a link channel: online play with RetroArch running gpSP, with a GBA
//! on a GB-Link adapter through its netplay bridge, or with another handheld on omega-zero.
//!
//! RetroArch plays a core that sends its own link packets, as gpSP does, in its netpacket mode:
//! no inputs or states are kept in step, the two ends say who they are and then carry the core's
//! packets back and forth. That is all of the protocol spoken here, as RetroArch 1.22 speaks it
//! (network/netplay/netplay_private.h and netplay_frontend.c), for two players: the host is
//! client 0 and the one who joins client 1, as a link between two handhelds always was.
//!
//! On the wire a connection opens with each end's 24-byte header; everything after it is a frame,
//! a command and the size of what follows it, both big-endian u32s, then that many bytes. A
//! link packet's frame carries one more u32 between the size and the packet, not counted in the
//! size: the client the packet is for on its way to the host, and the one it came from on its way
//! from it.
//!
//! The core is named as RetroArch names gpSP to the other end, "gpSP", and its version as gpSP
//! gives it for its link packets, "gpSP v1.0", so RetroArch and the GB-Link bridge take us for
//! one of their own. A room with a password takes it as RetroArch does: the host's header carries
//! a salt, and the one who joins answers with the SHA-256 of the salt, in eight capital hex
//! digits, and the password after it; a wrong one is dropped without a word.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use omega_retro::LinkChannel;

/// "RANP": a netplay header.
const MAGIC: u32 = 0x5241_4e50;
/// "FULL": the host has no room for another player.
const MAGIC_FULL: u32 = 0x4655_4c4c;
/// "DENY": the host has banned this address.
const MAGIC_DENY: u32 = 0x4445_4e59;
/// "POKE": the lobby asking a host for its header, to see it is there.
const MAGIC_POKE: u32 = 0x504f_4b45;

/// The protocols spoken: RetroArch 1.22's own, 7, down to 5.
const HIGH_PROTOCOL: u32 = 7;
const LOW_PROTOCOL: u32 = 5;

/// The platform RetroArch says it runs on: little-endian, with 64-bit size_t and long, as the
/// handheld's aarch64 Linux and the PCs it meets are.
const PLATFORM_MAGIC: u32 = (8 << 15) | 8;

/// zlib, which RetroArch says it can take; in netpacket mode no state is ever sent to take.
const COMPRESSION: u32 = 1;

/// The RetroArch whose version goes into the header's check.
const RETROARCH_VERSION: &str = "1.22.2";

pub const CORE_NAME: &str = "gpSP";
pub const CORE_VERSION: &str = "gpSP v1.0";

const CMD_NAK: u32 = 0x0001;
const CMD_DISCONNECT: u32 = 0x0002;
const CMD_NICK: u32 = 0x0020;
const CMD_PASSWORD: u32 = 0x0021;
const CMD_INFO: u32 = 0x0022;
const CMD_SYNC: u32 = 0x0023;
const CMD_PLAY: u32 = 0x0025;
const CMD_MODE: u32 = 0x0026;
const CMD_MODE_REFUSED: u32 = 0x0027;
const CMD_NETPACKET: u32 = 0x0048;
const CMD_PING_REQUEST: u32 = 0x1100;
const CMD_PING_RESPONSE: u32 = 0x1101;
const CMD_SETTING_ALLOW_PAUSING: u32 = 0x2000;
const CMD_SETTING_INPUT_LATENCY_FRAMES: u32 = 0x2001;

/// A link packet for every client but the sender.
const BROADCAST: u32 = 0xffff;

/// The client the one who joins is: the host is 0.
pub const JOINER: u16 = 1;

const NICK_LEN: usize = 32;
const PASSWORD_LEN: usize = 64;
const INFO_LEN: usize = 4 + 2 * NICK_LEN;
const MAX_INPUT_DEVICES: usize = 16;
const SYNC_LEN: usize =
    4 + 4 + MAX_INPUT_DEVICES * 4 + MAX_INPUT_DEVICES + MAX_INPUT_DEVICES * 4 + NICK_LEN;
const MODE_LEN: usize = 4 + 4 + 4 + MAX_INPUT_DEVICES + NICK_LEN;
const MODE_YOU: u32 = 0x8000_0000;
const MODE_PLAYING: u32 = 0x4000_0000;

/// The most a frame may carry: anything bigger is not RetroArch.
const MAX_FRAME: u32 = 256 * 1024;

/// How often the one who joined asks the host for a ping, as RetroArch does once it plays.
const PING_EVERY: Duration = Duration::from_secs(3);

/// How long a goodbye is given to leave before the connection is dropped.
const BYE: Duration = Duration::from_millis(100);

/// Why a connection did not become a link.
#[derive(Debug)]
pub enum NetplayFail {
    /// The host has no room for another player.
    Full,
    /// The host has banned this address.
    Banned,
    /// The room has a password, and none was given.
    Password,
    /// The room's password is not the one given.
    WrongPassword,
    /// The other end speaks no protocol in common, or something that is not netplay.
    Protocol(String),
    /// The other end runs another core: its name.
    Core(String),
    /// The host would not let this end play.
    Refused,
    /// The lobby looking in on a host, not a player: the host waits on for one.
    Probe,
    /// No answer in time.
    TimedOut,
    /// The connection went away.
    Io(std::io::Error),
}

impl std::fmt::Display for NetplayFail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetplayFail::Full => write!(f, "the room is full"),
            NetplayFail::Banned => write!(f, "the host turned this handheld away"),
            NetplayFail::Password => write!(f, "the room has a password"),
            NetplayFail::WrongPassword => write!(f, "the room's password is another"),
            NetplayFail::Protocol(why) => write!(f, "not a netplay we speak: {why}"),
            NetplayFail::Core(core) => write!(f, "the other player runs {core}, not gpSP"),
            NetplayFail::Refused => write!(f, "the host would not let this handheld play"),
            NetplayFail::Probe => write!(f, "the lobby looked in"),
            NetplayFail::TimedOut => write!(f, "no answer in time"),
            NetplayFail::Io(e) => write!(f, "{e}"),
        }
    }
}

impl From<std::io::Error> for NetplayFail {
    fn from(e: std::io::Error) -> NetplayFail {
        match e.kind() {
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => NetplayFail::TimedOut,
            _ => NetplayFail::Io(e),
        }
    }
}

/// RetroArch's check that both ends are builds that speak alike (netplay_impl_magic): its
/// version's characters, each shifted by its place, and the protocol after them.
pub fn impl_magic(version: &str, protocol: u32) -> u32 {
    let mut magic = 0u32;
    let mut i = 0;
    for c in version.bytes() {
        magic ^= u32::from(c) << (i & 0xf);
        i += 1;
    }
    magic ^ (protocol << (i & 0xf))
}

/// The answer to a room's `salt` for `password`, as RetroArch checks it: the SHA-256 of the salt
/// in eight capital hex digits and the password after it, in lower-case hex.
pub fn password_hash(salt: u32, password: &str) -> String {
    crate::update::sha256_hex(format!("{salt:08X}{password}").as_bytes())
}

/// A salt for a room with a password: never 0, which says a room has none.
fn salt() -> u32 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    (h.finish() as u32).max(1)
}

/// A header: the magic, the platform, the compression, then the joiner's highest protocol or the
/// host's password salt, the protocol, and the build check.
fn header(slot: u32, protocol: u32) -> [u8; 24] {
    let mut out = [0u8; 24];
    for (i, word) in [
        MAGIC,
        PLATFORM_MAGIC,
        COMPRESSION,
        slot,
        protocol,
        impl_magic(RETROARCH_VERSION, HIGH_PROTOCOL),
    ]
    .into_iter()
    .enumerate()
    {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn word(buf: &[u8], at: usize) -> u32 {
    buf.get(at..at + 4)
        .map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// `cmd` with `payload`, framed.
pub fn frame(cmd: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + payload.len());
    out.extend_from_slice(&cmd.to_be_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

/// A link packet, framed: `client` is the one it is for on its way to the host, the one it came
/// from on its way back.
pub fn packet_frame(client: u32, packet: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + packet.len());
    out.extend_from_slice(&CMD_NETPACKET.to_be_bytes());
    out.extend_from_slice(&(packet.len() as u32).to_be_bytes());
    out.extend_from_slice(&client.to_be_bytes());
    out.extend_from_slice(packet);
    out
}

/// A frame read off the wire: its command, the client a link packet's carries, and its payload.
struct Frame {
    cmd: u32,
    payload: Vec<u8>,
}

fn read_frame(r: &mut impl Read) -> std::io::Result<Frame> {
    let mut head = [0u8; 8];
    r.read_exact(&mut head)?;
    let cmd = word(&head, 0);
    let size = word(&head, 4);
    if size > MAX_FRAME {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("a frame of {size} bytes"),
        ));
    }
    if cmd == CMD_NETPACKET {
        let mut client = [0u8; 4];
        r.read_exact(&mut client)?;
    }
    let mut payload = vec![0u8; size as usize];
    r.read_exact(&mut payload)?;
    Ok(Frame { cmd, payload })
}

/// `text` in a fixed field of `len` bytes, cut to leave the NUL RetroArch's strlcpy leaves.
fn field(text: &str, len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    let mut end = text.len().min(len - 1);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    out[..end].copy_from_slice(&text.as_bytes()[..end]);
    out
}

fn read_field(buf: &[u8], at: usize, len: usize) -> String {
    let bytes = buf.get(at..at + len).unwrap_or_default();
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn info(crc: u32) -> Vec<u8> {
    let mut out = crc.to_be_bytes().to_vec();
    out.extend(field(CORE_NAME, NICK_LEN));
    out.extend(field(CORE_VERSION, NICK_LEN));
    out
}

fn sync(client: u32, nick: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(SYNC_LEN);
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&client.to_be_bytes());
    // every port a joypad, shared by no one, held by no client
    for _ in 0..MAX_INPUT_DEVICES {
        out.extend_from_slice(&1u32.to_be_bytes());
    }
    out.extend([0u8; MAX_INPUT_DEVICES]);
    out.extend([0u8; MAX_INPUT_DEVICES * 4]);
    out.extend(field(nick, NICK_LEN));
    out
}

fn mode(client: u32, nick: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(MODE_LEN);
    out.extend_from_slice(&1u32.to_be_bytes());
    out.extend_from_slice(&(MODE_YOU | MODE_PLAYING | (client & 0xffff)).to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend([0u8; MAX_INPUT_DEVICES]);
    out.extend(field(nick, NICK_LEN));
    out
}

/// "RATS": a relay session, asked for with no id and answered with the session's.
const RELAY_SESSION: u32 = 0x5241_5453;
/// "RATL": a player come to the relay, the id of their link; and the host's claim on it.
const RELAY_LINK: u32 = 0x5241_544c;
/// "RATA": the host asking for a player's address by their link's id, and the relay's answer.
const RELAY_ADDR: u32 = 0x5241_5441;
/// "RATP": the relay's ping, answered as it came.
const RELAY_PING: u32 = 0x5241_5450;

fn relay_id(magic: u32, id: &[u8; 12]) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..4].copy_from_slice(&magic.to_be_bytes());
    out[4..].copy_from_slice(id);
    out
}

/// A connection to `addr`, given up after `bound`.
fn connect(addr: &str, port: u16, bound: Duration) -> std::io::Result<TcpStream> {
    let mut last = None;
    for at in (addr, port).to_socket_addrs()? {
        match TcpStream::connect_timeout(&at, bound) {
            Ok(stream) => return Ok(stream),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("no address for {addr}"),
        )
    }))
}

/// Joins the room reached through the relay at `addr` as its `session`, as [`join`] does a room
/// reached directly: the relay is told the session first, and passes everything after it on.
pub fn join_relay(
    addr: &str,
    port: u16,
    session: [u8; 12],
    nick: &str,
    password: &str,
    bound: Duration,
) -> Result<Joined, NetplayFail> {
    let mut stream = connect(addr, port, bound)?;
    stream.write_all(&relay_id(RELAY_SESSION, &session))?;
    join_with(stream, nick, password, bound)
}

/// A room hosted through a relay: the session the lobby is told of, and the players the relay
/// brings, each on a connection of their own that then goes on as if they had reached this end
/// directly. The session's connection is kept the while, its pings answered, until this is
/// dropped.
pub struct RelayHost {
    pub session: [u8; 12],
    players: Receiver<TcpStream>,
    control: TcpStream,
}

impl RelayHost {
    /// Asks the relay at `addr` for a session.
    pub fn open(addr: &str, port: u16, bound: Duration) -> Result<RelayHost, NetplayFail> {
        let mut control = connect(addr, port, bound)?;
        control.set_nodelay(true)?;
        control.set_read_timeout(Some(bound))?;
        control.write_all(&relay_id(RELAY_SESSION, &[0; 12]))?;
        let mut answer = [0u8; 16];
        control.read_exact(&mut answer)?;
        let mut session = [0u8; 12];
        session.copy_from_slice(&answer[4..]);
        if word(&answer, 0) != RELAY_SESSION || session == [0; 12] {
            return Err(NetplayFail::Protocol("the relay gave no session".into()));
        }
        control.set_read_timeout(None)?;
        let (tx, players) = channel();
        let (mut reader, mut writer) = (control.try_clone()?, control.try_clone()?);
        let (addr, relay) = (addr.to_string(), port);
        std::thread::Builder::new()
            .name("omega-relay".into())
            .spawn(move || {
                // the players come to the relay, waiting for its say-so to go on
                let mut waiting: Vec<([u8; 12], TcpStream)> = Vec::new();
                let mut magic = [0u8; 4];
                while reader.read_exact(&mut magic).is_ok() {
                    match u32::from_be_bytes(magic) {
                        RELAY_PING => {
                            if writer.write_all(&RELAY_PING.to_be_bytes()).is_err() {
                                return;
                            }
                        }
                        RELAY_LINK => {
                            let mut id = [0u8; 12];
                            if reader.read_exact(&mut id).is_err() {
                                return;
                            }
                            // one connection a player, then their address asked for
                            let Ok(stream) = connect(&addr, relay, bound) else {
                                continue;
                            };
                            if writer.write_all(&relay_id(RELAY_ADDR, &id)).is_err() {
                                return;
                            }
                            waiting.push((id, stream));
                        }
                        RELAY_ADDR => {
                            let mut rest = [0u8; 28];
                            if reader.read_exact(&mut rest).is_err() {
                                return;
                            }
                            let Some(at) = waiting.iter().position(|(id, _)| id[..] == rest[..12])
                            else {
                                continue;
                            };
                            let (id, mut stream) = waiting.swap_remove(at);
                            // the link claimed, the relay passes the player on as they are
                            if stream.write_all(&relay_id(RELAY_LINK, &id)).is_ok()
                                && tx.send(stream).is_err()
                            {
                                return;
                            }
                        }
                        other => {
                            crate::log!("netplay: the relay said {other:#010x}");
                            return;
                        }
                    }
                }
            })?;
        Ok(RelayHost {
            session,
            players,
            control,
        })
    }

    /// The next player the relay brings, waited for at most `wait`.
    pub fn next_player(&self, wait: Duration) -> Option<TcpStream> {
        self.players.recv_timeout(wait).ok()
    }
}

impl Drop for RelayHost {
    fn drop(&mut self) {
        let _ = self.control.shutdown(Shutdown::Both);
    }
}

/// The connection as a link, once both ends have said who they are.
pub struct Joined {
    pub link: NetplayLink,
    /// The client this end plays as: the host 0, the one who joins as RetroArch numbered it.
    pub client: u16,
    /// The other player's name.
    pub peer: String,
}

/// Joins the room at the other end of `stream` as `nick`, the stream's first bytes already
/// given to a relay when the room is reached through one. Waits `bound` for the host to let
/// this end play.
pub fn join(stream: TcpStream, nick: &str, bound: Duration) -> Result<Joined, NetplayFail> {
    join_with(stream, nick, "", bound)
}

/// As [`join`], with `password` for a room that has one.
pub fn join_with(
    mut stream: TcpStream,
    nick: &str,
    password: &str,
    bound: Duration,
) -> Result<Joined, NetplayFail> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(bound))?;
    let deadline = Instant::now() + bound;
    stream.write_all(&header(HIGH_PROTOCOL, LOW_PROTOCOL))?;
    let mut head = [0u8; 24];
    stream.read_exact(&mut head)?;
    match word(&head, 0) {
        MAGIC => {}
        MAGIC_FULL => return Err(NetplayFail::Full),
        MAGIC_DENY => return Err(NetplayFail::Banned),
        other => return Err(NetplayFail::Protocol(format!("magic {other:#010x}"))),
    }
    let protocol = word(&head, 16);
    if !(LOW_PROTOCOL..=HIGH_PROTOCOL).contains(&protocol) {
        return Err(NetplayFail::Protocol(format!("protocol {protocol}")));
    }
    let salt = word(&head, 12);
    if salt != 0 && password.is_empty() {
        return Err(NetplayFail::Password);
    }
    stream.write_all(&frame(CMD_NICK, &field(nick, NICK_LEN)))?;
    if salt != 0 {
        stream.write_all(&frame(
            CMD_PASSWORD,
            password_hash(salt, password).as_bytes(),
        ))?;
    }
    let mut peer = String::new();
    let mut client = None;
    let mut let_in = salt == 0;
    loop {
        if Instant::now() >= deadline {
            return Err(NetplayFail::TimedOut);
        }
        let f = match read_frame(&mut stream) {
            Ok(f) => f,
            // RetroArch hangs up on a wrong password without a word
            Err(e) if !let_in && e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Err(NetplayFail::WrongPassword)
            }
            Err(e) => return Err(e.into()),
        };
        if f.cmd == CMD_INFO {
            let_in = true;
        }
        match f.cmd {
            CMD_NICK => peer = read_field(&f.payload, 0, NICK_LEN),
            // the host has no game in yet: say who we are and wait for its next
            CMD_INFO if f.payload.is_empty() => stream.write_all(&frame(CMD_INFO, &info(0)))?,
            CMD_INFO => {
                let core = read_field(&f.payload, 4, NICK_LEN);
                if !core.eq_ignore_ascii_case(CORE_NAME) {
                    return Err(NetplayFail::Core(core));
                }
                // the host's game's checksum back, so another game of the same series is let in
                stream.write_all(&frame(CMD_INFO, &info(word(&f.payload, 0))))?;
            }
            CMD_SYNC => {
                client = Some(word(&f.payload, 4) & 0x7fff_ffff);
                stream.write_all(&frame(CMD_PLAY, &[0; 4]))?;
            }
            CMD_MODE
                if word(&f.payload, 4) & (MODE_YOU | MODE_PLAYING) == MODE_YOU | MODE_PLAYING =>
            {
                let client = client.unwrap_or_else(|| word(&f.payload, 4) & 0xffff);
                let link = NetplayLink::wrap(stream, Role::Joiner, protocol)?;
                return Ok(Joined {
                    link,
                    client: client as u16,
                    peer,
                });
            }
            CMD_MODE_REFUSED => return Err(NetplayFail::Refused),
            CMD_NAK | CMD_DISCONNECT => {
                return Err(NetplayFail::Protocol("the host hung up".into()))
            }
            // pings and settings before the game starts are the host's to keep
            _ => {}
        }
    }
}

/// Takes the player at the other end of `stream` into this end's room as `nick`, hosting the game
/// whose checksum is `crc`. Waits `bound` for them to say who they are and ask to play.
pub fn host(
    stream: TcpStream,
    nick: &str,
    crc: u32,
    bound: Duration,
) -> Result<Joined, NetplayFail> {
    host_with(stream, nick, "", crc, bound)
}

/// As [`host`], the room kept for those who give `password`, when there is one.
pub fn host_with(
    mut stream: TcpStream,
    nick: &str,
    password: &str,
    crc: u32,
    bound: Duration,
) -> Result<Joined, NetplayFail> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(bound))?;
    let deadline = Instant::now() + bound;
    let mut magic = [0u8; 4];
    stream.read_exact(&mut magic)?;
    match u32::from_be_bytes(magic) {
        MAGIC => {}
        // the lobby asks for the header alone, to see the room is there
        MAGIC_POKE => {
            let _ = stream.write_all(&header(0, HIGH_PROTOCOL));
            return Err(NetplayFail::Probe);
        }
        other => return Err(NetplayFail::Protocol(format!("magic {other:#010x}"))),
    }
    let mut rest = [0u8; 20];
    stream.read_exact(&mut rest)?;
    // a joiner gives its highest protocol in the salt's place, an older one its only one
    let (highest, only) = (word(&rest, 8), word(&rest, 12));
    let protocol = if highest != 0 {
        highest.min(HIGH_PROTOCOL)
    } else {
        only
    };
    if !(LOW_PROTOCOL..=HIGH_PROTOCOL).contains(&protocol) {
        let _ = stream.write_all(&header(0, 0));
        return Err(NetplayFail::Protocol(format!("protocol {protocol}")));
    }
    let salt = if password.is_empty() { 0 } else { salt() };
    stream.write_all(&header(salt, protocol))?;
    let f = read_frame(&mut stream)?;
    if f.cmd != CMD_NICK {
        return Err(NetplayFail::Protocol(format!("{:#06x} for a name", f.cmd)));
    }
    let mut peer = read_field(&f.payload, 0, NICK_LEN);
    // two players of one name are told apart, as RetroArch tells them
    if peer == nick {
        peer = format!(
            "{} (2)",
            peer.chars().take(NICK_LEN - 5).collect::<String>()
        );
    }
    stream.write_all(&frame(CMD_NICK, &field(nick, NICK_LEN)))?;
    if salt != 0 {
        let f = read_frame(&mut stream)?;
        let given = String::from_utf8_lossy(&f.payload).into_owned();
        // a wrong one is dropped without a word, as RetroArch drops it
        if f.cmd != CMD_PASSWORD
            || f.payload.len() != PASSWORD_LEN
            || given != password_hash(salt, password)
        {
            return Err(NetplayFail::WrongPassword);
        }
    }
    stream.write_all(&frame(CMD_INFO, &info(crc)))?;
    loop {
        if Instant::now() >= deadline {
            return Err(NetplayFail::TimedOut);
        }
        let f = read_frame(&mut stream)?;
        match f.cmd {
            CMD_INFO if f.payload.len() == INFO_LEN => {
                let core = read_field(&f.payload, 4, NICK_LEN);
                if !core.eq_ignore_ascii_case(CORE_NAME) {
                    return Err(NetplayFail::Core(core));
                }
                break;
            }
            // a joiner with no game in yet: ours again, until it has one
            CMD_INFO => stream.write_all(&frame(CMD_INFO, &info(crc)))?,
            CMD_NAK | CMD_DISCONNECT => {
                return Err(NetplayFail::Protocol("the player hung up".into()))
            }
            _ => {}
        }
    }
    let client = u32::from(JOINER);
    stream.write_all(&frame(CMD_SYNC, &sync(client, &peer)))?;
    if protocol >= 6 {
        stream.write_all(&frame(CMD_SETTING_ALLOW_PAUSING, &0u32.to_be_bytes()))?;
        stream.write_all(&frame(CMD_SETTING_INPUT_LATENCY_FRAMES, &[0; 8]))?;
    }
    loop {
        if Instant::now() >= deadline {
            return Err(NetplayFail::TimedOut);
        }
        let f = read_frame(&mut stream)?;
        match f.cmd {
            CMD_PLAY => break,
            CMD_PING_REQUEST => stream.write_all(&frame(CMD_PING_RESPONSE, &[]))?,
            CMD_NAK | CMD_DISCONNECT => {
                return Err(NetplayFail::Protocol("the player hung up".into()))
            }
            _ => {}
        }
    }
    stream.write_all(&frame(CMD_MODE, &mode(client, &peer)))?;
    let link = NetplayLink::wrap(stream, Role::Host, protocol)?;
    Ok(Joined {
        link,
        client: 0,
        peer,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Host,
    Joiner,
}

enum Out {
    Packet(Vec<u8>),
    Frame(Vec<u8>),
    Bye(Sender<()>),
}

/// A netplay connection carrying the core's link packets, once both ends have said who they are.
pub struct NetplayLink {
    outbox: Sender<Out>,
    inbox: Receiver<Vec<u8>>,
    stream: TcpStream,
    closed: Arc<AtomicBool>,
    ended: Arc<AtomicBool>,
}

impl NetplayLink {
    fn wrap(stream: TcpStream, role: Role, protocol: u32) -> std::io::Result<NetplayLink> {
        stream.set_read_timeout(None)?;
        let mut reader = stream.try_clone()?;
        let mut writer = stream.try_clone()?;
        let (rtx, inbox) = channel();
        let (wtx, wrx) = channel::<Out>();
        let closed = Arc::new(AtomicBool::new(false));
        let ended = Arc::new(AtomicBool::new(false));
        let (reader_closed, reader_ended, answer) = (closed.clone(), ended.clone(), wtx.clone());

        std::thread::Builder::new()
            .name("omega-netplay-in".into())
            .spawn(move || loop {
                let Ok(f) = read_frame(&mut reader) else {
                    reader_closed.store(true, Ordering::Release);
                    return;
                };
                match f.cmd {
                    CMD_NETPACKET => {
                        if rtx.send(f.payload).is_err() {
                            return;
                        }
                    }
                    CMD_PING_REQUEST => {
                        let _ = answer.send(Out::Frame(frame(CMD_PING_RESPONSE, &[])));
                        // RetroArch's host asks back at once, to keep its own time
                        if role == Role::Host && protocol >= 6 {
                            let _ = answer.send(Out::Frame(frame(CMD_PING_REQUEST, &[])));
                        }
                    }
                    CMD_DISCONNECT | CMD_NAK => {
                        reader_ended.store(true, Ordering::Release);
                    }
                    // the other player leaving play: neither playing nor this end
                    CMD_MODE if word(&f.payload, 4) & (MODE_YOU | MODE_PLAYING) == 0 => {
                        reader_ended.store(true, Ordering::Release);
                    }
                    _ => {}
                }
            })?;

        std::thread::Builder::new()
            .name("omega-netplay-out".into())
            .spawn(move || {
                let mut next_ping = Instant::now() + PING_EVERY;
                loop {
                    let wait = next_ping.saturating_duration_since(Instant::now());
                    let out = match wrx.recv_timeout(wait) {
                        Ok(out) => out,
                        Err(RecvTimeoutError::Timeout) => {
                            next_ping = Instant::now() + PING_EVERY;
                            // the one who joined keeps the host's patience, as RetroArch does
                            if role == Role::Joiner && protocol >= 6 {
                                Out::Frame(frame(CMD_PING_REQUEST, &[]))
                            } else {
                                continue;
                            }
                        }
                        Err(RecvTimeoutError::Disconnected) => return,
                    };
                    let bytes = match out {
                        // to everyone from the one who joined, from client 0 from the host
                        Out::Packet(p) => match role {
                            Role::Joiner => packet_frame(BROADCAST, &p),
                            Role::Host => packet_frame(0, &p),
                        },
                        Out::Frame(f) => f,
                        Out::Bye(ack) => {
                            let _ = writer.write_all(&frame(CMD_DISCONNECT, &[]));
                            let _ = ack.send(());
                            continue;
                        }
                    };
                    if writer.write_all(&bytes).is_err() {
                        return;
                    }
                }
            })?;

        Ok(NetplayLink {
            outbox: wtx,
            inbox,
            stream,
            closed,
            ended,
        })
    }
}

impl Drop for NetplayLink {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

impl LinkChannel for NetplayLink {
    fn send(&mut self, _flags: i32, buf: &[u8]) {
        if buf.is_empty() {
            return;
        }
        let _ = self.outbox.send(Out::Packet(buf.to_vec()));
    }

    fn try_recv(&mut self) -> Option<Vec<u8>> {
        self.inbox.try_recv().ok()
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    fn send_end(&mut self) {
        let (ack, said) = channel();
        if self.outbox.send(Out::Bye(ack)).is_ok() {
            let _ = said.recv_timeout(BYE);
        }
    }

    fn peer_ended(&self) -> bool {
        self.ended.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    const BOUND: Duration = Duration::from_secs(5);

    fn pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let joiner = TcpStream::connect(addr).expect("connect");
        let (hosted, _) = listener.accept().expect("accept");
        (hosted, joiner)
    }

    fn recv(link: &mut NetplayLink) -> Vec<u8> {
        let deadline = Instant::now() + BOUND;
        loop {
            if let Some(p) = link.try_recv() {
                return p;
            }
            assert!(Instant::now() < deadline, "no packet came");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn retroarchs_build_check_is_its_own() {
        // netplay_impl_magic("1.22.2", 7), worked through by hand
        assert_eq!(impl_magic("1.22.2", 7), 0x455);
    }

    #[test]
    fn a_header_and_a_link_packet_are_laid_out_as_retroarch_lays_them() {
        let h = header(HIGH_PROTOCOL, LOW_PROTOCOL);
        assert_eq!(&h[0..4], b"RANP");
        assert_eq!(word(&h, 4), 0x0004_0008);
        assert_eq!(word(&h, 12), 7);
        assert_eq!(word(&h, 16), 5);
        assert_eq!(word(&h, 20), 0x455);
        assert_eq!(
            packet_frame(0xffff, b"MPK1"),
            [0, 0, 0, 0x48, 0, 0, 0, 4, 0, 0, 0xff, 0xff, b'M', b'P', b'K', b'1']
        );
        let i = info(0xdead_beef);
        assert_eq!(i.len(), INFO_LEN);
        assert_eq!(read_field(&i, 4, NICK_LEN), "gpSP");
        assert_eq!(read_field(&i, 4 + NICK_LEN, NICK_LEN), "gpSP v1.0");
        assert_eq!(sync(1, "x").len(), SYNC_LEN);
        assert_eq!(mode(1, "x").len(), MODE_LEN);
    }

    #[test]
    fn a_name_too_long_is_cut_to_leave_its_nul() {
        let f = field("a name far longer than thirty-one bytes", NICK_LEN);
        assert_eq!(f.len(), NICK_LEN);
        assert_eq!(f[NICK_LEN - 1], 0);
        assert_eq!(read_field(&f, 0, NICK_LEN).len(), NICK_LEN - 1);
    }

    /// Our host and our joiner meet, say who they are, and carry link packets both ways.
    #[test]
    fn a_joiner_and_a_host_meet_and_carry_packets_both_ways() {
        let (hosted, joiner) = pair();
        let hosted = std::thread::spawn(move || host(hosted, "Host", 0x1234_5678, BOUND));
        let mut joined = join(joiner, "Guest", BOUND).expect("the join failed");
        let mut hosting = hosted.join().unwrap().expect("the host failed");
        assert_eq!((joined.client, hosting.client), (1, 0));
        assert_eq!(
            (joined.peer.as_str(), hosting.peer.as_str()),
            ("Host", "Guest")
        );

        joined.link.send(0, b"MPK1 from the joiner");
        assert_eq!(recv(&mut hosting.link), b"MPK1 from the joiner");
        hosting.link.send(0, b"MPK1 from the host");
        assert_eq!(recv(&mut joined.link), b"MPK1 from the host");

        joined.link.send_end();
        let deadline = Instant::now() + BOUND;
        while !hosting.link.peer_ended() {
            assert!(Instant::now() < deadline, "the goodbye never arrived");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// A RetroArch host's frames, laid out by hand: the joiner answers each as RetroArch does and
    /// is let in as the client the host numbers it.
    #[test]
    fn a_joiner_follows_a_retroarch_host() {
        let (mut hosted, joiner) = pair();
        let ra = std::thread::spawn(move || {
            let mut theirs = [0u8; 24];
            hosted.read_exact(&mut theirs).unwrap();
            assert_eq!(&theirs[0..4], b"RANP");
            hosted.write_all(&header(0, 7)).unwrap();
            let nick = read_frame(&mut hosted).unwrap();
            assert_eq!(
                (nick.cmd, read_field(&nick.payload, 0, NICK_LEN)),
                (CMD_NICK, "Guest".into())
            );
            hosted
                .write_all(&frame(CMD_NICK, &field("RetroArch", NICK_LEN)))
                .unwrap();
            let mut ours = 0xcafe_f00du32.to_be_bytes().to_vec();
            ours.extend(field("gpSP", NICK_LEN));
            ours.extend(field("gpSP v1.0", NICK_LEN));
            hosted.write_all(&frame(CMD_INFO, &ours)).unwrap();
            let back = read_frame(&mut hosted).unwrap();
            assert_eq!(back.cmd, CMD_INFO);
            assert_eq!(
                word(&back.payload, 0),
                0xcafe_f00d,
                "the checksum was not echoed"
            );
            hosted
                .write_all(&frame(CMD_SYNC, &sync(3, "Guest")))
                .unwrap();
            hosted
                .write_all(&frame(CMD_SETTING_ALLOW_PAUSING, &0u32.to_be_bytes()))
                .unwrap();
            assert_eq!(read_frame(&mut hosted).unwrap().cmd, CMD_PLAY);
            hosted
                .write_all(&frame(CMD_MODE, &mode(3, "Guest")))
                .unwrap();
            hosted
                .write_all(&packet_frame(0, b"from client 0"))
                .unwrap();
            let mut head = [0u8; 12];
            hosted.read_exact(&mut head).unwrap();
            assert_eq!(word(&head, 0), CMD_NETPACKET);
            assert_eq!(
                word(&head, 8),
                BROADCAST,
                "a joiner's packet is for everyone"
            );
            let mut body = vec![0u8; word(&head, 4) as usize];
            hosted.read_exact(&mut body).unwrap();
            body
        });
        let mut joined = join(joiner, "Guest", BOUND).expect("the join failed");
        assert_eq!((joined.client, joined.peer.as_str()), (3, "RetroArch"));
        assert_eq!(recv(&mut joined.link), b"from client 0");
        joined.link.send(0, b"from the joiner");
        assert_eq!(ra.join().unwrap(), b"from the joiner");
    }

    #[test]
    fn a_host_running_another_core_is_refused() {
        let (mut hosted, joiner) = pair();
        std::thread::spawn(move || {
            let mut theirs = [0u8; 24];
            hosted.read_exact(&mut theirs).unwrap();
            hosted.write_all(&header(0, 7)).unwrap();
            let _ = read_frame(&mut hosted);
            let mut other = 0u32.to_be_bytes().to_vec();
            other.extend(field("mGBA", NICK_LEN));
            other.extend(field("0.10", NICK_LEN));
            hosted.write_all(&frame(CMD_INFO, &other)).unwrap();
            std::thread::sleep(Duration::from_millis(200));
        });
        assert!(matches!(join(joiner, "Guest", BOUND), Err(NetplayFail::Core(c)) if c == "mGBA"));
    }

    #[test]
    fn a_full_room_and_a_room_with_a_password_are_refused() {
        for (salt, magic) in [(0, MAGIC_FULL), (0x1234, MAGIC)] {
            let (mut hosted, joiner) = pair();
            std::thread::spawn(move || {
                let mut theirs = [0u8; 24];
                hosted.read_exact(&mut theirs).unwrap();
                let mut h = header(salt, 7);
                h[0..4].copy_from_slice(&magic.to_be_bytes());
                hosted.write_all(&h).unwrap();
                std::thread::sleep(Duration::from_millis(200));
            });
            match join(joiner, "Guest", BOUND) {
                Err(NetplayFail::Full) => assert_eq!(magic, MAGIC_FULL),
                Err(NetplayFail::Password) => assert_eq!(salt, 0x1234),
                other => panic!("let in a room it should not be: {:?}", other.err()),
            }
        }
    }

    /// The lobby pokes a host to see it is there: it gets the header, and the host waits on.
    #[test]
    fn a_host_answers_the_lobbys_poke_and_waits_on() {
        let (hosted, mut poker) = pair();
        let hosted = std::thread::spawn(move || host(hosted, "Host", 0, BOUND));
        poker.write_all(&MAGIC_POKE.to_be_bytes()).unwrap();
        let mut back = [0u8; 24];
        poker.read_exact(&mut back).unwrap();
        assert_eq!(&back[0..4], b"RANP");
        assert!(matches!(hosted.join().unwrap(), Err(NetplayFail::Probe)));
    }

    #[test]
    fn a_host_that_never_answers_times_out() {
        let (_hosted, joiner) = pair();
        assert!(matches!(
            join(joiner, "Guest", Duration::from_millis(200)),
            Err(NetplayFail::TimedOut)
        ));
    }

    /// A relay laid out by hand: a session asked for and given, its ping answered, and a player it
    /// brings claimed and handed on, to say who they are as if they had come directly.
    #[test]
    fn a_relay_host_takes_its_session_and_the_player_the_relay_brings() {
        let relay = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = relay.local_addr().unwrap().port();
        let session = [7u8; 12];
        let link_id = [9u8; 12];
        let fake = std::thread::spawn(move || {
            let (mut control, _) = relay.accept().unwrap();
            let mut ask = [0u8; 16];
            control.read_exact(&mut ask).unwrap();
            assert_eq!(
                ask,
                relay_id(RELAY_SESSION, &[0; 12]),
                "a session is asked for with no id"
            );
            control
                .write_all(&relay_id(RELAY_SESSION, &session))
                .unwrap();
            control.write_all(&RELAY_PING.to_be_bytes()).unwrap();
            let mut pong = [0u8; 4];
            control.read_exact(&mut pong).unwrap();
            assert_eq!(
                u32::from_be_bytes(pong),
                RELAY_PING,
                "the ping was not answered"
            );
            // a player comes: the host opens a connection for them and asks their address
            control.write_all(&relay_id(RELAY_LINK, &link_id)).unwrap();
            let (mut theirs, _) = relay.accept().unwrap();
            let mut addr = [0u8; 16];
            control.read_exact(&mut addr).unwrap();
            assert_eq!(addr, relay_id(RELAY_ADDR, &link_id));
            let mut answer = relay_id(RELAY_ADDR, &link_id).to_vec();
            answer.extend([0u8; 16]);
            control.write_all(&answer).unwrap();
            let mut claim = [0u8; 16];
            theirs.read_exact(&mut claim).unwrap();
            assert_eq!(
                claim,
                relay_id(RELAY_LINK, &link_id),
                "the link was not claimed"
            );
            // the player, through the relay, joins as they would directly
            let mut joined =
                join(theirs, "Guest", BOUND).expect("the join through the relay failed");
            joined.link.send(0, b"through the relay");
            (control, joined)
        });
        let hosting = RelayHost::open("127.0.0.1", port, BOUND).expect("no session");
        assert_eq!(hosting.session, session);
        let player = hosting.next_player(BOUND).expect("no player came");
        let mut hosted = host(player, "Host", 0, BOUND).expect("the host failed");
        let (_control, _joined) = fake.join().unwrap();
        assert_eq!(recv(&mut hosted.link), b"through the relay");
    }

    #[test]
    fn a_joiner_gives_the_relay_its_session_before_its_header() {
        let relay = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = relay.local_addr().unwrap().port();
        let fake = std::thread::spawn(move || {
            let (mut stream, _) = relay.accept().unwrap();
            let mut first = [0u8; 16];
            stream.read_exact(&mut first).unwrap();
            assert_eq!(first, relay_id(RELAY_SESSION, &[3; 12]));
            host(stream, "Host", 0, BOUND).map(|h| h.peer)
        });
        let joined = join_relay("127.0.0.1", port, [3; 12], "Guest", "", BOUND).expect("no join");
        assert_eq!(joined.peer, "Host");
        assert_eq!(fake.join().unwrap().expect("the host failed"), "Guest");
    }

    #[test]
    fn a_relay_that_gives_no_session_is_refused() {
        let relay = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = relay.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut control, _) = relay.accept().unwrap();
            let mut ask = [0u8; 16];
            control.read_exact(&mut ask).unwrap();
            control
                .write_all(&relay_id(RELAY_SESSION, &[0; 12]))
                .unwrap();
            std::thread::sleep(Duration::from_millis(200));
        });
        assert!(matches!(
            RelayHost::open("127.0.0.1", port, BOUND),
            Err(NetplayFail::Protocol(_))
        ));
    }

    #[test]
    fn a_rooms_password_is_answered_as_retroarch_hashes_it() {
        // sha256("1234ABCD" + "pikachu"), worked out apart
        assert_eq!(
            password_hash(0x1234_abcd, "pikachu"),
            "390671ab18dab021229bd9c97907b14cce238dcf81cbbbbab1c7bc4a3e202521"
        );
    }

    /// A room with a password lets in the one who gives it, turns away the one who gives another,
    /// and one who gives none is told it has one.
    #[test]
    fn a_room_with_a_password_lets_in_only_who_gives_it() {
        for (given, ok) in [("pikachu", true), ("eevee", false), ("", false)] {
            let (hosted, joiner) = pair();
            let hosting =
                std::thread::spawn(move || host_with(hosted, "Host", "pikachu", 0, BOUND));
            let joined = join_with(joiner, "Guest", given, BOUND);
            let hosted = hosting.join().unwrap();
            match (ok, joined) {
                (true, Ok(j)) => {
                    assert_eq!(j.peer, "Host");
                    assert!(hosted.is_ok(), "the host turned the right password away");
                }
                (false, Err(NetplayFail::WrongPassword)) => assert!(!given.is_empty()),
                (false, Err(NetplayFail::Password)) => assert!(given.is_empty()),
                (_, other) => panic!("{given:?}: {:?}", other.err()),
            }
        }
    }
}
