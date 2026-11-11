//! The games' clocks in the handheld's own time. BaseOS keeps the system clock in UTC and sets
//! no zone, so a core asking the time of day, as gpSP and mGBA do for a cart with a clock on it
//! (Pokémon Ruby, Sapphire and Emerald, Boktai), would be told UTC's. omega-zero sets `TZ` to
//! the offset Date & Time was given: at boot, before any core is open, and again when the
//! offset is changed, which is only ever done on the shelf.

/// `offset_min`, minutes east of UTC, as a POSIX `TZ`, which counts hours west: UTC+3 is
/// `UTC-3`, India's UTC+5:30 is `UTC-5:30`.
pub fn posix_tz(offset_min: i16) -> String {
    let west = -i32::from(offset_min);
    let sign = if west < 0 { "-" } else { "" };
    let (h, m) = (west.abs() / 60, west.abs() % 60);
    if m == 0 {
        format!("UTC{sign}{h}")
    } else {
        format!("UTC{sign}{h}:{m:02}")
    }
}

/// Points the process's clock at `offset_min` east of UTC.
pub fn follow(offset_min: i16) {
    let tz = posix_tz(offset_min);
    if std::env::var("TZ").ok().as_deref() != Some(tz.as_str()) {
        // set while no core runs: at boot, and from Date & Time on the shelf
        std::env::set_var("TZ", tz);
    }
}

#[cfg(test)]
mod tests {
    use super::posix_tz;

    #[test]
    fn east_of_utc_is_written_west_as_posix_counts() {
        assert_eq!(posix_tz(0), "UTC0");
        assert_eq!(posix_tz(120), "UTC-2");
        assert_eq!(posix_tz(-300), "UTC5");
        assert_eq!(posix_tz(330), "UTC-5:30");
        assert_eq!(posix_tz(-570), "UTC9:30");
        assert_eq!(posix_tz(840), "UTC-14");
    }
}
