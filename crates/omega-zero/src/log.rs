//! omega-zero's log: one line to stderr for each thing worth knowing, which the launch script
//! keeps as `omega-zero.log` on the card. Every line is stamped with the seconds since
//! omega-zero started, so a stall or a slow start can be read off the gaps between lines, and
//! starts with `omega-zero:` as the lines the cores print beside it do not.
//!
//! ```
//! omega_zero::log!("audio: {}", "no device");
//! // [   12.345] omega-zero: audio: no device
//! ```

use std::sync::LazyLock;
use std::time::Instant;

static STARTED: LazyLock<Instant> = LazyLock::new(Instant::now);

/// Starts the log's clock: called first thing, so the stamps count from the start.
pub fn start() {
    LazyLock::force(&STARTED);
}

/// The stamp a line written now begins with.
pub fn stamp() -> String {
    format!("[{:9.3}]", STARTED.elapsed().as_secs_f64())
}

/// Writes one line to the log: `format!`'s arguments, stamped and prefixed.
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        ::std::eprintln!("{} omega-zero: {}", $crate::log::stamp(), ::std::format_args!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_stamp_is_seconds_with_their_thousandths() {
        let s = super::stamp();
        assert!(s.starts_with('[') && s.ends_with(']'), "{s}");
        let secs: f64 = s[1..s.len() - 1].trim().parse().expect("not a number");
        assert!(secs >= 0.0);
        assert_eq!(s.len(), 11, "{s} is not the width every line's is");
    }
}
