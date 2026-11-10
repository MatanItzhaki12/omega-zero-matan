//! Where a slow piece of work's time went, step by step, written to the log when it was slow.

use std::time::{Duration, Instant};

pub struct Stopwatch {
    began: Instant,
    at: Instant,
    steps: Vec<(&'static str, Duration)>,
}

impl Default for Stopwatch {
    fn default() -> Self {
        Self::new()
    }
}

impl Stopwatch {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            began: now,
            at: now,
            steps: Vec::new(),
        }
    }

    /// The time since the step before, as `name`'s.
    pub fn step(&mut self, name: &'static str) {
        let now = Instant::now();
        self.steps.push((name, now - self.at));
        self.at = now;
    }

    /// Logs each step's time, as `what`'s, if it all took longer than `over`.
    pub fn log_over(&self, what: &str, over: Duration) {
        if let Some(report) = self.report_over(what, over) {
            eprintln!("omega-zero: {report}");
        }
    }

    /// What `log_over` logs: each step of a millisecond or more, none if it all took `over` or
    /// less.
    pub fn report_over(&self, what: &str, over: Duration) -> Option<String> {
        if self.began.elapsed() <= over {
            return None;
        }
        let parts: Vec<String> = self
            .steps
            .iter()
            .filter(|(_, d)| *d >= Duration::from_millis(1))
            .map(|(name, d)| format!("{name} {:.0}", d.as_secs_f64() * 1e3))
            .collect();
        Some(format!("slow {what}: {}", parts.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slow_piece_of_work_says_which_step_took_its_time() {
        let mut sw = Stopwatch::new();
        sw.step("quick");
        std::thread::sleep(Duration::from_millis(30));
        sw.step("slow");
        let report = sw
            .report_over("update", Duration::from_millis(20))
            .expect("the slow work was not told");
        assert!(report.starts_with("slow update: slow "), "{report}");
        assert!(!report.contains("quick"), "{report}");
        assert_eq!(sw.report_over("update", Duration::from_secs(10)), None);
    }
}
