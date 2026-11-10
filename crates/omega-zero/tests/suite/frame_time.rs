use crate::common;

use std::time::Duration;

use common::{clocked, tmp_root_with_carts};
use omega_input::{InputSource, Millis, RawEvent};
use omega_power::SimPlatform;
use omega_zero::frontend::Frontend;

struct Idle;

impl InputSource for Idle {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        Vec::new()
    }
}

#[test]
fn a_frame_steps_by_the_time_between_the_last_two_the_panel_showed() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.advance(&mut Idle);
    f.shown();
    std::thread::sleep(Duration::from_millis(60));
    f.shown();
    let before = f.app().now();
    // begun at once after the frame was shown: it still steps by the time between the two
    f.advance(&mut Idle);
    let stepped = f.app().now() - before;
    assert!(
        (55..=250).contains(&stepped),
        "the frame stepped {stepped} ms rather than the 60 between the two shown"
    );
}
