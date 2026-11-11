//! A NAS on the local network, reached over Samba (SMB 2 and 3) as a Windows share is: finding
//! the ones on the network, signing in to one, its shares and folders, and writing a file into
//! one of them. Save Backup is built on it.
//!
//! The Samba client is `smb2`'s, pure Rust and asynchronous: each call here runs it to the end on
//! a runtime of its own on the calling thread, so the callers are the backup's and the settings'
//! own threads, never the frame's.
//!
//! Finding a NAS asks every address of this handheld's network whether it answers on Samba's
//! port, 445, and asks those that do for their NetBIOS name, as Windows' network list does. One
//! that does not answer to its name is shown by its address.

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::time::Duration;

/// Samba's port.
pub const SMB_PORT: u16 = 445;
const NETBIOS_PORT: u16 = 137;

/// How long a NAS is given to answer: to a connection, and to each request after.
const CONNECT: Duration = Duration::from_secs(5);
const RESPONSE: Duration = Duration::from_secs(20);

/// Where a NAS is, and how to sign in to it: an empty user is a guest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Login {
    pub host: String,
    pub user: String,
    pub password: String,
}

/// A NAS found on the network: its name, when it gave one, and its address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub name: Option<String>,
    pub addr: Ipv4Addr,
}

impl Found {
    /// What a list shows it as: its name, else its address.
    pub fn shown(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.addr.to_string())
    }
}

/// Why the NAS could not be used, as the settings say it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NasFail {
    /// Nothing answered at the address: off, or on another network.
    Unreachable,
    /// The user or password was refused.
    Login,
    /// The share or folder is not there, or this user may not write to it.
    Denied,
    Other(String),
}

impl NasFail {
    pub fn line(&self) -> &'static str {
        match self {
            NasFail::Unreachable => "The NAS did not answer",
            NasFail::Login => "The NAS refused the user or password",
            NasFail::Denied => "The NAS refused the folder",
            NasFail::Other(_) => "The NAS could not be used",
        }
    }
}

impl std::fmt::Display for NasFail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NasFail::Other(e) => write!(f, "{e}"),
            other => f.write_str(other.line()),
        }
    }
}

fn fail(e: smb2::Error) -> NasFail {
    use smb2::ErrorKind;
    // what the NAS said, in full, for the log: the settings say it in a few words
    crate::log!("nas: {e}");
    match e.kind() {
        ErrorKind::AuthRequired | ErrorKind::SigningRequired => NasFail::Login,
        ErrorKind::AccessDenied | ErrorKind::NotFound => NasFail::Denied,
        ErrorKind::ConnectionLost | ErrorKind::TimedOut => NasFail::Unreachable,
        _ => {
            let text = e.to_string();
            let lower = text.to_lowercase();
            if lower.contains("logon") || lower.contains("password") || lower.contains("guest") {
                NasFail::Login
            } else if lower.contains("refused") || lower.contains("unreachable") {
                NasFail::Unreachable
            } else {
                NasFail::Other(text)
            }
        }
    }
}

/// The host and port `host` names: Samba's port when it gives none.
fn address(host: &str) -> String {
    let host = host.trim();
    match host.rsplit_once(':') {
        Some((_, port)) if port.parse::<u16>().is_ok() && !host.starts_with('[') => {
            host.to_string()
        }
        _ => format!("{host}:{SMB_PORT}"),
    }
}

/// A runtime to run one call to the end on this thread.
fn runtime() -> Result<tokio::runtime::Runtime, NasFail> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| NasFail::Other(e.to_string()))
}

/// A NAS signed in to, until it is dropped.
pub struct Nas {
    rt: tokio::runtime::Runtime,
    client: smb2::SmbClient,
    /// The share last opened, kept for the next file into it.
    share: Option<(String, smb2::Tree)>,
}

impl Nas {
    /// Signs in to the NAS `login` names.
    pub fn open(login: &Login) -> Result<Nas, NasFail> {
        let rt = runtime()?;
        let config = smb2::ClientConfig {
            addr: address(&login.host),
            timeout: CONNECT,
            username: login.user.clone(),
            password: login.password.clone(),
            response_timeout: Some(RESPONSE),
            ..Default::default()
        };
        let client = rt
            .block_on(smb2::SmbClient::connect(config))
            .map_err(fail)?;
        Ok(Nas {
            rt,
            client,
            share: None,
        })
    }

    /// The shares a backup can go to: the disk ones, without the hidden ones Windows keeps.
    pub fn shares(&mut self) -> Result<Vec<String>, NasFail> {
        let mut shares: Vec<String> = self
            .rt
            .block_on(self.client.list_shares())
            .map_err(fail)?
            .into_iter()
            .map(|s| s.name)
            .collect();
        shares.sort_by_key(|s| s.to_lowercase());
        Ok(shares)
    }

    fn tree(&mut self, share: &str) -> Result<&mut smb2::Tree, NasFail> {
        let open = matches!(&self.share, Some((name, _)) if name == share);
        if !open {
            let tree = self
                .rt
                .block_on(self.client.connect_share(share))
                .map_err(fail)?;
            self.share = Some((share.to_string(), tree));
        }
        Ok(&mut self.share.as_mut().expect("the share just opened").1)
    }

    /// The folders in `dir` of `share`, by name, hidden ones left out.
    pub fn folders(&mut self, share: &str, dir: &str) -> Result<Vec<String>, NasFail> {
        self.tree(share)?;
        let (_, tree) = self.share.as_mut().expect("the share");
        let entries = self
            .rt
            .block_on(self.client.list_directory(tree, dir))
            .map_err(fail)?;
        let mut folders: Vec<String> = entries
            .into_iter()
            .filter(|e| e.is_directory && !e.name.starts_with('.') && e.name != "..")
            .map(|e| e.name)
            .collect();
        folders.sort_by_key(|s| s.to_lowercase());
        Ok(folders)
    }

    /// What is in `dir` of `share`: each entry's name, whether it is a folder, and its size;
    /// hidden ones left out.
    pub fn entries(&mut self, share: &str, dir: &str) -> Result<Vec<(String, bool, u64)>, NasFail> {
        self.tree(share)?;
        let (_, tree) = self.share.as_mut().expect("the share");
        let entries = self
            .rt
            .block_on(self.client.list_directory(tree, dir))
            .map_err(fail)?;
        Ok(entries
            .into_iter()
            .filter(|e| !e.name.starts_with('.') && e.name != "..")
            .map(|e| (e.name, e.is_directory, e.size))
            .collect())
    }

    /// The file at `path` in `share`.
    pub fn get(&mut self, share: &str, path: &str) -> Result<Vec<u8>, NasFail> {
        self.tree(share)?;
        let (_, tree) = self.share.as_mut().expect("the share");
        self.rt
            .block_on(self.client.read_file(tree, path))
            .map_err(fail)
    }

    /// Makes folder `dir` of `share`, each folder on the way to it too. One already there is
    /// left as it is.
    pub fn make_folder(&mut self, share: &str, dir: &str) -> Result<(), NasFail> {
        self.tree(share)?;
        let mut path = String::new();
        for part in dir.split('/').filter(|p| !p.is_empty()) {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(part);
            let (_, tree) = self.share.as_mut().expect("the share");
            // one already there answers with an error, which the write after shows if it was
            // anything else
            let _ = self.rt.block_on(self.client.create_directory(tree, &path));
        }
        Ok(())
    }

    /// Writes `bytes` to `path` in `share`, in place of the file there: written beside it
    /// first and renamed over it, so a write cut short never leaves half a save.
    pub fn put(&mut self, share: &str, path: &str, bytes: &[u8]) -> Result<(), NasFail> {
        if let Some((dir, _)) = path.rsplit_once('/') {
            self.make_folder(share, dir)?;
        } else {
            self.tree(share)?;
        }
        let part = format!("{path}.part");
        let (_, tree) = self.share.as_mut().expect("the share");
        self.rt
            .block_on(self.client.write_file(tree, &part, bytes))
            .map_err(fail)?;
        let (_, tree) = self.share.as_mut().expect("the share");
        let replace = smb2::RenameOptions::new().replace_if_exists(true);
        self.rt
            .block_on(self.client.rename_with(tree, &part, path, replace))
            .map_err(fail)?;
        Ok(())
    }
}

/// This handheld's address on its network, and so the network's: none while it is on none.
pub fn local_address() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    // a datagram socket's connect sends nothing: it only picks the way out, and the address
    socket
        .connect((Ipv4Addr::new(192, 168, 255, 255), 9))
        .ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(v4) if !v4.is_unspecified() && !v4.is_loopback() => Some(v4),
        _ => None,
    }
}

/// The NASes on this handheld's network: every other address of its /24 that answers on
/// Samba's port within `wait`, each with its NetBIOS name where it gives one.
pub fn find(wait: Duration) -> io::Result<Vec<Found>> {
    let Some(me) = local_address() else {
        return Err(io::Error::new(io::ErrorKind::NotConnected, "on no network"));
    };
    let [a, b, c, _] = me.octets();
    let hosts: Vec<Ipv4Addr> = (1..=254u8)
        .map(|d| Ipv4Addr::new(a, b, c, d))
        .filter(|h| *h != me)
        .collect();
    Ok(answering(&hosts, wait)
        .into_iter()
        .map(|addr| Found {
            name: netbios_name(addr, Duration::from_millis(600)),
            addr,
        })
        .collect())
}

/// Which of `hosts` answer on Samba's port within `wait`, asked all at once, in their order.
pub fn answering(hosts: &[Ipv4Addr], wait: Duration) -> Vec<Ipv4Addr> {
    answering_on(hosts, SMB_PORT, wait)
}

fn answering_on(hosts: &[Ipv4Addr], port: u16, wait: Duration) -> Vec<Ipv4Addr> {
    let asks: Vec<_> = hosts
        .iter()
        .map(|&host| {
            std::thread::spawn(move || {
                TcpStream::connect_timeout(&SocketAddr::from((host, port)), wait)
                    .is_ok()
                    .then_some(host)
            })
        })
        .collect();
    asks.into_iter()
        .filter_map(|ask| ask.join().ok().flatten())
        .collect()
}

/// A NetBIOS node status question: who is at the address, "*" in NetBIOS' first-level encoding.
fn node_status_query() -> Vec<u8> {
    let mut q = vec![
        0x4f, 0x5a, // the transaction
        0x00, 0x00, // a query
        0x00, 0x01, // one question
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0x20, // the name's length, encoded
    ];
    let mut name = [b' '; 16];
    name[0] = b'*';
    name[15] = 0;
    for byte in name {
        q.push(b'A' + (byte >> 4));
        q.push(b'A' + (byte & 0x0f));
    }
    q.extend([0x00, 0x00, 0x21, 0x00, 0x01]); // the end of the name, NBSTAT, IN
    q
}

/// The machine's name in a node status answer: the first unique one of a file server or
/// workstation, without its padding.
pub fn parse_node_status(answer: &[u8]) -> Option<String> {
    // the header, the name asked about (34 bytes and its end), type, class, TTL and length
    let at = 12 + 34 + 2 + 2 + 4 + 2;
    let count = *answer.get(at)? as usize;
    let names = answer.get(at + 1..at + 1 + count * 18)?;
    names
        .chunks_exact(18)
        .find_map(|entry| {
            let suffix = entry[15];
            let group = entry[16] & 0x80 != 0;
            (!group && (suffix == 0x20 || suffix == 0x00)).then(|| {
                String::from_utf8_lossy(&entry[..15])
                    .trim_end_matches([' ', '\0'])
                    .to_string()
            })
        })
        .filter(|n| !n.is_empty())
}

/// The NetBIOS name of the machine at `addr`, when it answers within `wait`.
pub fn netbios_name(addr: Ipv4Addr, wait: Duration) -> Option<String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.set_read_timeout(Some(wait)).ok()?;
    socket
        .send_to(&node_status_query(), (addr, NETBIOS_PORT))
        .ok()?;
    let mut buf = [0u8; 1024];
    let (n, _) = socket.recv_from(&mut buf).ok()?;
    parse_node_status(&buf[..n])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_without_a_port_is_given_sambas() {
        assert_eq!(address("192.168.1.5"), "192.168.1.5:445");
        assert_eq!(address(" nas.local "), "nas.local:445");
        assert_eq!(address("10.0.0.2:1445"), "10.0.0.2:1445");
    }

    #[test]
    fn the_node_status_question_asks_for_star() {
        let q = node_status_query();
        assert_eq!(q.len(), 50);
        // '*' is 0x2a: C and K; a space, 0x20, is C and A
        assert_eq!(&q[13..17], b"CKCA");
        assert_eq!(&q[q.len() - 4..], &[0x00, 0x21, 0x00, 0x01]);
    }

    fn answer(names: &[(&str, u8, bool)]) -> Vec<u8> {
        let mut a = vec![0u8; 12];
        a.push(0x20);
        a.extend([b'A'; 32]);
        a.push(0);
        a.extend([0x00, 0x21, 0x00, 0x01, 0, 0, 0, 0, 0, 0]);
        a.push(names.len() as u8);
        for (name, suffix, group) in names {
            let mut padded = format!("{name:<15}").into_bytes();
            padded.truncate(15);
            a.extend(padded);
            a.push(*suffix);
            a.extend([if *group { 0x80 } else { 0x00 }, 0x04]);
        }
        a
    }

    #[test]
    fn a_node_status_answer_gives_the_machines_own_name() {
        let a = answer(&[
            ("WORKGROUP", 0x00, true),
            ("DISKSTATION", 0x20, false),
            ("DISKSTATION", 0x00, false),
        ]);
        assert_eq!(parse_node_status(&a).as_deref(), Some("DISKSTATION"));
        assert_eq!(
            parse_node_status(&answer(&[("WORKGROUP", 0x00, true)])),
            None
        );
        assert_eq!(parse_node_status(&[0; 20]), None);
    }

    #[test]
    fn a_host_answering_on_the_port_is_found() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let found = answering_on(
            &[Ipv4Addr::LOCALHOST, Ipv4Addr::new(127, 0, 0, 2)],
            port,
            Duration::from_millis(300),
        );
        assert!(found.contains(&Ipv4Addr::LOCALHOST), "{found:?}");
    }
}
