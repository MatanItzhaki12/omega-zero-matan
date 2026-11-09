//! The Bluetooth LE beacon AirPods send out, as OpenPods reads it
//! (github.com/adolfintel/openpods, GPL-3.0): Apple's manufacturer data, 27 bytes starting
//! 07 19, carries the model and each bud's and the case's battery to the nearest ten, and
//! which are charging. It needs no connection, so it says the battery where the AirPods'
//! own channel says none. The handheld looks for it with a short LE scan through
//! bluetoothctl, and takes the strongest beacon of AirPods in reach, as OpenPods does.

use super::Battery;

/// Apple's company id, which its manufacturer data is keyed by.
const APPLE: &str = "0x004c";
const LENGTH: usize = 27;
/// A beacon weaker than this is someone else's AirPods.
const MIN_RSSI: i16 = -70;

/// What a beacon says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Beacon {
    pub model: &'static str,
    pub left: Option<Battery>,
    pub right: Option<Battery>,
    pub case: Option<Battery>,
}

/// What Apple's manufacturer data `data` says, if it is AirPods' beacon. OpenPods reads it as
/// hex, a character a nibble: the 7th to 10th the model, the 11th whether the buds are
/// flipped, the 13th and 14th the left and right buds' battery (0 to 10, 15 for away), the
/// 15th which are charging, the 16th the case's battery.
pub fn decode(data: &[u8]) -> Option<Beacon> {
    if data.len() != LENGTH || data[0] != 0x07 || data[1] != 0x19 {
        return None;
    }
    let hex: Vec<u8> = data.iter().flat_map(|b| [b >> 4, b & 0x0f]).collect();
    let flip = hex[10] & 0x02 == 0;
    let level = |n: u8| (n <= 10).then_some(n * 10);
    let charge = hex[14];
    let (left_n, right_n) = if flip {
        (hex[12], hex[13])
    } else {
        (hex[13], hex[12])
    };
    let (left_bit, right_bit) = if flip { (0b10, 0b01) } else { (0b01, 0b10) };
    let model = match (hex[6], hex[7], hex[8], hex[9]) {
        (0x0, 0x2, 0x2, 0x0) => "AirPods",
        (0x0, 0xf, 0x2, 0x0) => "AirPods 2",
        (0x1, 0x3, 0x2, 0x0) => "AirPods 3",
        (0x0, 0xe, 0x2, 0x0) => "AirPods Pro",
        (0x1, 0x4, 0x2, 0x0) | (0x2, 0x4, 0x2, 0x0) => "AirPods Pro 2",
        (0x2, 0x7, 0x2, 0x0) => "AirPods Pro 3",
        (_, 0xa, _, _) => "AirPods Max",
        _ => "AirPods",
    };
    Some(Beacon {
        model,
        left: level(left_n).map(|level| Battery {
            level,
            charging: charge & left_bit != 0,
        }),
        right: level(right_n).map(|level| Battery {
            level,
            charging: charge & right_bit != 0,
        }),
        case: level(hex[15]).map(|level| Battery {
            level,
            charging: charge & 0b100 != 0,
        }),
    })
}

/// Apple's manufacturer data in what `bluetoothctl info` printed, the bytes after its
/// `ManufacturerData Value:` line as hex pairs, before each line's text.
pub fn manufacturer_data(info: &str) -> Option<Vec<u8>> {
    let mut lines = info.lines();
    lines
        .find(|l| l.contains("ManufacturerData Key:") && l.to_ascii_lowercase().contains(APPLE))?;
    lines.find(|l| l.contains("ManufacturerData Value:"))?;
    let mut out = Vec::new();
    for line in lines {
        let pairs: Vec<u8> = line
            .split_whitespace()
            .map_while(|t| {
                (t.len() == 2)
                    .then(|| u8::from_str_radix(t, 16).ok())
                    .flatten()
            })
            .collect();
        if pairs.is_empty() {
            break;
        }
        out.extend(pairs);
        if out.len() >= LENGTH {
            break;
        }
    }
    out.truncate(LENGTH);
    Some(out)
}

/// The signal strength `bluetoothctl info` printed.
pub fn rssi(info: &str) -> Option<i16> {
    info.lines().find_map(|l| {
        let v = l.trim().strip_prefix("RSSI:")?.trim();
        // "-52" or "0xffffffcc (-52)"
        let v = v.rsplit('(').next()?.trim_end_matches(')');
        v.trim().parse().ok()
    })
}

/// The addresses a scan's output names, in order, once each.
pub fn addresses(scan: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in scan.lines() {
        let Some(at) = line.find("Device ") else {
            continue;
        };
        let mac: String = line[at + 7..].chars().take(17).collect();
        let ok = mac.len() == 17 && mac.split(':').count() == 6;
        if ok && !out.contains(&mac) {
            out.push(mac);
        }
    }
    out
}

fn run(args: &[&str]) -> String {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(args.join(" "))
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// Looks for AirPods' beacons for a few seconds and reads the strongest: none where none is
/// near enough.
pub fn look() -> Option<Beacon> {
    // bluetoothctl leaves at once on an empty stdin: the sleep holds it open for the scan
    let scan = run(&["sleep 7 | bluetoothctl --timeout 6 scan le"]);
    let mut best: Option<(i16, Beacon)> = None;
    for mac in addresses(&scan).into_iter().take(24) {
        let info = run(&["bluetoothctl info", &mac]);
        let Some(beacon) = manufacturer_data(&info).as_deref().and_then(decode) else {
            continue;
        };
        let strength = rssi(&info).unwrap_or(MIN_RSSI);
        if strength >= MIN_RSSI && best.as_ref().is_none_or(|(s, _)| strength > *s) {
            best = Some((strength, beacon));
        }
    }
    best.map(|(_, b)| b)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INFO: &str = "Device 5B:1A:2C:3D:4E:5F (random)
\tName: 5B-1A-2C-3D-4E-5F
\tRSSI: 0xffffffcc (-52)
\tManufacturerData Key: 0x004c (76)
\tManufacturerData Value:
  07 19 01 27 20 2b 99 8f 11 00 05 6a 4c 3d 2e 1f  ...' +....jL=..
  10 01 02 03 04 05 06 07 08 09 0a                 ...........
";

    #[test]
    fn a_beacon_is_read_out_of_bluetoothctl_s_info() {
        let data = manufacturer_data(INFO).unwrap();
        assert_eq!(data.len(), 27);
        assert_eq!(rssi(INFO), Some(-52));
        let b = decode(&data).unwrap();
        assert_eq!(b.model, "AirPods Pro 3");
        // nibble 10 is 0x2's bit set: not flipped, so 13th is left (9) and 12th right (9)
        assert_eq!(b.left.map(|b| b.level), Some(90));
        assert_eq!(b.right.map(|b| b.level), Some(90));
        // nibble 15 is 0xf: the case is away
        assert_eq!(b.case, None);
        // nibble 14 is 0x8: none charging
        assert!(!b.left.unwrap().charging);
    }

    #[test]
    fn only_airpods_beacons_are_read() {
        assert_eq!(decode(&[0x10; 27]), None);
        assert_eq!(decode(&[0x07, 0x19, 0x01]), None);
        assert_eq!(
            manufacturer_data("\tManufacturerData Key: 0x0075 (117)\n"),
            None
        );
    }

    #[test]
    fn a_scan_names_each_device_once() {
        let scan = "[NEW] Device 5B:1A:2C:3D:4E:5F 5B-1A-2C-3D-4E-5F\n\
                    [CHG] Device 5B:1A:2C:3D:4E:5F RSSI: -60\n\
                    [NEW] Device 74:77:86:47:EE:5D AirPods Pro\n";
        assert_eq!(addresses(scan), ["5B:1A:2C:3D:4E:5F", "74:77:86:47:EE:5D"]);
    }
}
