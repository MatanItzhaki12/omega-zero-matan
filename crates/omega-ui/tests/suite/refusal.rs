use omega_ui::Refusal;

fn peak(r: &Refusal, from: u64, to: u64) -> f32 {
    (from..to).map(|t| r.offset(t).abs()).fold(0.0, f32::max)
}

#[test]
fn the_shake_decays_and_ends() {
    let r = Refusal::started(1_000);
    let early = peak(&r, 1_000, 1_100);
    let late = peak(&r, 1_200, 1_300);
    assert!(early > 0.0, "nothing moved");
    assert!(
        late < early,
        "the shake is not decaying: {early} then {late}"
    );
    assert_eq!(r.offset(1_400), 0.0, "the shake never ends");
    assert!(!r.active(1_400), "the refusal outlives its own shake");
}

/// The shake moves what it shakes a whole art pixel at a time, never one screen pixel or part
/// of one, so it stays on the grid as it swings.
#[test]
fn the_shake_moves_a_whole_art_pixel_at_a_time() {
    let r = Refusal::started(0);
    let mut seen = Vec::new();
    for t in 0..400 {
        let x = r.offset(t);
        assert_eq!(
            x.rem_euclid(2.0),
            0.0,
            "{x} px at {t} ms is off the art grid"
        );
        if !seen.contains(&(x as i32)) {
            seen.push(x as i32);
        }
    }
    seen.sort_unstable();
    assert_eq!(
        seen,
        [-6, -4, -2, 0, 2, 4, 6],
        "the shake swings by other steps"
    );
}

#[test]
fn the_shake_crosses_where_it_started() {
    let r = Refusal::started(0);
    let left = (0..300).map(|t| r.offset(t)).fold(f32::INFINITY, f32::min);
    let right = (0..300)
        .map(|t| r.offset(t))
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(left < 0.0 && right > 0.0, "{left} to {right}");
}
