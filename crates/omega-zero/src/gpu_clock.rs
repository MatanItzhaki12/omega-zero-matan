//! The GPU's clock while a PSP game runs. The H700's Mali is left by BaseOS on devfreq's
//! `simple_ondemand`, which sits at its lowest step, 420 MHz, and looks only every 100 ms at
//! whether it was kept busy enough to step up. PPSSPP draws the PSP's frames on it, so a game
//! loading under the spinning disc, or a scene that suddenly asks more, waits on that step a
//! few frames at a time. While a PSP game goes in and plays, the clock's floor is raised to its
//! top step, 648 MHz; back on the shelf, or in any other game, it is let down again.

/// The GPU's devfreq floor, in Hz; 0 lets it go as low as it likes.
const MIN_FREQ: &str = "/sys/class/devfreq/gpu/min_freq";
/// The steps it offers, lowest first.
const STEPS: &str = "/sys/class/devfreq/gpu/available_frequencies";

/// Holds the GPU's clock up while it is asked to, writing only when that changes.
#[derive(Debug, Default)]
pub struct GpuClock {
    held: Option<bool>,
}

impl GpuClock {
    pub fn hold(&mut self, high: bool) {
        if self.held == Some(high) {
            return;
        }
        self.held = Some(high);
        if cfg!(feature = "device") {
            let floor = if high { top_step() } else { Some(0) };
            if let Some(hz) = floor {
                if let Err(e) = std::fs::write(MIN_FREQ, hz.to_string()) {
                    crate::log!("gpu clock: {MIN_FREQ}: {e}");
                }
            }
        }
    }
}

fn top_step() -> Option<u64> {
    top_of(&std::fs::read_to_string(STEPS).ok()?)
}

/// The highest of the steps `list` gives, as devfreq prints them.
fn top_of(list: &str) -> Option<u64> {
    list.split_whitespace().filter_map(|s| s.parse().ok()).max()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_top_step_is_the_highest_listed() {
        let steps = "420000000 456000000 504000000 552000000 600000000 648000000 \n";
        assert_eq!(top_of(steps), Some(648_000_000));
        assert_eq!(top_of(""), None);
    }

    #[test]
    fn a_hold_writes_only_when_it_changes() {
        let mut g = GpuClock::default();
        g.hold(true);
        assert_eq!(g.held, Some(true));
        g.hold(true);
        g.hold(false);
        assert_eq!(g.held, Some(false));
    }
}
