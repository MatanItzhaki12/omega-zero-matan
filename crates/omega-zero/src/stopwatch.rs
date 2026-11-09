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
        if self.began.elapsed() <= over {
            return;
        }
        let parts: Vec<String> = self
            .steps
            .iter()
            .filter(|(_, d)| *d >= Duration::from_millis(1))
            .map(|(name, d)| format!("{name} {:.0}", d.as_secs_f64() * 1e3))
            .collect();
        eprintln!("omega-zero: slow {what}: {}", parts.join(", "));
    }
}
