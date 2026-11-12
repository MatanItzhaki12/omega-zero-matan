//! Bluetooth headphones' own volume: what AirPods' swipe and the like set, kept by bluetoothd
//! as the A2DP transport's Volume (0 to 127, AVRCP's absolute volume). The headphones play at
//! it themselves, so while sound goes to them it is the volume: the console's buttons set it,
//! and the headphones changing it shows on the console's gauge. A thread of its own reads and
//! sets it through `dbus-send`, once a second while headphones are connected, and at once for a
//! volume set.

use std::sync::atomic::{AtomicI16, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// The headphones whose volume is watched, by address.
static WATCHED: Mutex<Option<String>> = Mutex::new(None);
/// The volume last read, 0 to 100; -1 for none yet.
static SEEN: AtomicI16 = AtomicI16::new(-1);
/// The volume to set, 0 to 100; -1 for nothing to set.
static WANT: AtomicI16 = AtomicI16::new(-1);
/// The thread, woken as a volume is set rather than at its next look a moment later.
static THREAD: std::sync::OnceLock<std::thread::Thread> = std::sync::OnceLock::new();

const MAX: u16 = 127;

/// AVRCP's 0 to 127 as a percentage, and back.
pub fn percent(raw: u16) -> u8 {
    ((u32::from(raw.min(MAX)) * 100 + u32::from(MAX) / 2) / u32::from(MAX)) as u8
}

pub fn raw(percent: u8) -> u16 {
    ((u32::from(percent.min(100)) * u32::from(MAX) + 50) / 100) as u16
}

/// Watches the volume of the headphones at `mac`, or of none.
pub fn watch(mac: Option<&str>) {
    let changed = match WATCHED.lock() {
        Ok(mut w) if w.as_deref() != mac => {
            *w = mac.map(str::to_string);
            true
        }
        _ => false,
    };
    if changed {
        SEEN.store(-1, Ordering::Relaxed);
        WANT.store(-1, Ordering::Relaxed);
    }
    if mac.is_some() && THREAD.get().is_none() {
        if let Ok(t) = std::thread::Builder::new()
            .name("omega-bt-volume".into())
            .spawn(run)
        {
            let _ = THREAD.set(t.thread().clone());
        }
    }
}

/// The headphones' volume, 0 to 100, once read.
pub fn level() -> Option<u8> {
    u8::try_from(SEEN.load(Ordering::Relaxed)).ok()
}

/// Sets the headphones' volume, 0 to 100.
pub fn set(level: u8) {
    WANT.store(i16::from(level.min(100)), Ordering::Relaxed);
    if let Some(t) = THREAD.get() {
        t.unpark();
    }
}

fn dbus(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("dbus-send")
        .args(["--system", "--print-reply", "--dest=org.bluez"])
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The A2DP transport of the headphones at `mac`, as `bluetoothctl transport.list` names it.
fn transport(mac: &str) -> Option<String> {
    let out = std::process::Command::new("bluetoothctl")
        .arg("transport.list")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    transport_in(&String::from_utf8_lossy(&out.stdout), mac)
}

/// The transport of the device at `mac` in what `bluetoothctl transport.list` printed.
pub fn transport_in(list: &str, mac: &str) -> Option<String> {
    let dev = format!("dev_{}", mac.replace(':', "_"));
    list.split_whitespace()
        .find(|w| w.starts_with("/org/bluez/") && w.contains(&dev))
        .map(str::to_string)
}

/// The number in what `dbus-send --print-reply` printed for a uint16.
pub fn uint16_in(reply: &str) -> Option<u16> {
    reply
        .split_whitespace()
        .skip_while(|w| *w != "uint16")
        .nth(1)?
        .parse()
        .ok()
}

fn run() {
    crate::spawn::stand_back();
    let mut path: Option<(String, String)> = None;
    loop {
        // a look a second, sooner for a volume to set
        std::thread::park_timeout(Duration::from_millis(800));
        let Some(mac) = WATCHED.lock().ok().and_then(|w| w.clone()) else {
            path = None;
            continue;
        };
        if path.as_ref().is_none_or(|(m, _)| *m != mac) {
            path = transport(&mac).map(|p| (mac.clone(), p));
        }
        let Some((_, t)) = &path else {
            continue;
        };
        let want = WANT.swap(-1, Ordering::Relaxed);
        if let Ok(want) = u8::try_from(want) {
            let value = format!("variant:uint16:{}", raw(want));
            let set = dbus(&[
                t,
                "org.freedesktop.DBus.Properties.Set",
                "string:org.bluez.MediaTransport1",
                "string:Volume",
                &value,
            ]);
            if set.is_some() {
                SEEN.store(i16::from(want), Ordering::Relaxed);
            }
            continue;
        }
        let got = dbus(&[
            t,
            "org.freedesktop.DBus.Properties.Get",
            "string:org.bluez.MediaTransport1",
            "string:Volume",
        ]);
        match got.as_deref().and_then(uint16_in) {
            // a volume asked for while this was read makes it out of date: it is set first
            Some(_) if WANT.load(Ordering::Relaxed) >= 0 => {}
            Some(v) => SEEN.store(i16::from(percent(v)), Ordering::Relaxed),
            // the transport went, as it does when the headphones reconnect: found again
            None => path = None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avrcp_s_volume_goes_to_a_percentage_and_back() {
        assert_eq!(percent(127), 100);
        assert_eq!(percent(0), 0);
        assert_eq!(percent(94), 74);
        for p in 0..=100 {
            assert_eq!(percent(raw(p)), p, "{p}% did not come back");
        }
    }

    #[test]
    fn the_transport_and_its_volume_are_read_out_of_what_was_printed() {
        let list = "Transport /org/bluez/hci0/dev_74_77_86_47_EE_5D/sep1/fd1\n";
        assert_eq!(
            transport_in(list, "74:77:86:47:EE:5D").as_deref(),
            Some("/org/bluez/hci0/dev_74_77_86_47_EE_5D/sep1/fd1")
        );
        assert_eq!(transport_in(list, "11:22:33:44:55:66"), None);
        let reply = "method return time=1 sender=:1.1 -> destination=:1.6 serial=1\n   variant       uint16 110\n";
        assert_eq!(uint16_in(reply), Some(110));
    }
}
