//! Bluetooth sockets of the kernel's own, for the headphones' channels: L2CAP for AirPods,
//! RFCOMM for Galaxy Buds. Each blocks until it connects or is refused.

use std::fs::File;
use std::io;
use std::time::Duration;

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

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::os::fd::{AsRawFd, FromRawFd};

    const AF_BLUETOOTH: i32 = 31;
    const SOCK_STREAM: i32 = 1;
    const SOCK_SEQPACKET: i32 = 5;
    const BTPROTO_L2CAP: i32 = 0;
    const BTPROTO_RFCOMM: i32 = 3;
    const SOCK_CLOEXEC: i32 = 0o2000000;
    const SOL_SOCKET: i32 = 1;
    const SO_RCVTIMEO: i32 = 20;

    /// The kernel's sockaddr_l2: the family, the PSM little-endian, the address, the channel
    /// and the address type (0, classic Bluetooth).
    #[repr(C)]
    struct SockaddrL2 {
        family: u16,
        psm: u16,
        bdaddr: [u8; 6],
        cid: u16,
        bdaddr_type: u8,
    }

    /// The kernel's sockaddr_rc: the family, the address and the channel.
    #[repr(C)]
    struct SockaddrRc {
        family: u16,
        bdaddr: [u8; 6],
        channel: u8,
    }

    #[repr(C)]
    struct Timeval {
        sec: i64,
        usec: i64,
    }

    extern "C" {
        fn socket(domain: i32, kind: i32, protocol: i32) -> i32;
        fn connect(fd: i32, addr: *const u8, len: u32) -> i32;
        fn close(fd: i32) -> i32;
        fn setsockopt(fd: i32, level: i32, name: i32, value: *const u8, len: u32) -> i32;
    }

    fn open<T>(kind: i32, proto: i32, addr: &T) -> io::Result<File> {
        // SAFETY: a socket of the kernel's own, closed again where connect fails, and handed to
        // a File, which closes it, where it succeeds; connect reads addr for its size
        unsafe {
            let fd = socket(AF_BLUETOOTH, kind | SOCK_CLOEXEC, proto);
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            let len = std::mem::size_of::<T>() as u32;
            if connect(fd, (addr as *const T).cast(), len) != 0 {
                let e = io::Error::last_os_error();
                close(fd);
                return Err(e);
            }
            Ok(File::from_raw_fd(fd))
        }
    }

    pub fn l2cap(mac: &str, psm: u16) -> io::Result<File> {
        let addr = SockaddrL2 {
            family: AF_BLUETOOTH as u16,
            psm: psm.to_le(),
            bdaddr: bdaddr(mac).ok_or(io::ErrorKind::InvalidInput)?,
            cid: 0,
            bdaddr_type: 0,
        };
        open(SOCK_SEQPACKET, BTPROTO_L2CAP, &addr)
    }

    pub fn rfcomm(mac: &str, channel: u8) -> io::Result<File> {
        let addr = SockaddrRc {
            family: AF_BLUETOOTH as u16,
            bdaddr: bdaddr(mac).ok_or(io::ErrorKind::InvalidInput)?,
            channel,
        };
        open(SOCK_STREAM, BTPROTO_RFCOMM, &addr)
    }

    pub fn read_timeout(sock: &File, timeout: Option<Duration>) {
        let t = timeout.unwrap_or_default();
        let tv = Timeval {
            sec: t.as_secs() as i64,
            usec: i64::from(t.subsec_micros()),
        };
        // SAFETY: setsockopt reads tv for its size; a refusal leaves reads as they were
        unsafe {
            setsockopt(
                sock.as_raw_fd(),
                SOL_SOCKET,
                SO_RCVTIMEO,
                (&tv as *const Timeval).cast(),
                std::mem::size_of::<Timeval>() as u32,
            );
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::{l2cap, read_timeout, rfcomm};

#[cfg(not(target_os = "linux"))]
pub fn l2cap(_mac: &str, _psm: u16) -> io::Result<File> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(not(target_os = "linux"))]
pub fn rfcomm(_mac: &str, _channel: u8) -> io::Result<File> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(not(target_os = "linux"))]
pub fn read_timeout(_sock: &File, _timeout: Option<Duration>) {}

#[cfg(test)]
mod tests {
    #[test]
    fn an_address_goes_to_the_kernel_last_byte_first() {
        assert_eq!(
            super::bdaddr("AA:BB:CC:DD:EE:FF"),
            Some([0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa])
        );
        assert_eq!(super::bdaddr("nope"), None);
    }
}
