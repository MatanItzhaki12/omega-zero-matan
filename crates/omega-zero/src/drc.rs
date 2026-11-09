const MAX_CORRECTION: f64 = 0.005;

pub fn drc_ratio(queued_frames: usize, target_frames: usize) -> f64 {
    if target_frames == 0 {
        return 1.0;
    }
    let error = target_frames as f64 - queued_frames as f64;
    let ratio = 1.0 + MAX_CORRECTION * error / target_frames as f64;
    ratio.clamp(1.0 - MAX_CORRECTION, 1.0 + MAX_CORRECTION)
}

/// How full the ring is kept: 2.4 video frames' worth (40 ms), which is how far the game's
/// sound runs behind it, and the slack a frame late by that much still has. A half of the
/// ring, as it was, put 66 ms between a game and its sound.
pub fn drc_target(capacity_frames: usize) -> usize {
    capacity_frames * 3 / 10
}
