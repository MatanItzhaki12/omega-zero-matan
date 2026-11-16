use omega_zero::build_info::Build;

fn sample() -> Build {
    Build {
        version: "0.1.0".into(),
        hash: "9e11a10",
        dirty: false,
        date: "2026-08-12",
    }
}

#[test]
fn the_serial_is_upper_case_hex() {
    assert_eq!(sample().serial(), "9E11A10");
    assert!(sample().serial().chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn a_dirty_build_is_marked_after_its_hash() {
    let dirty = Build {
        dirty: true,
        ..sample()
    };
    assert_eq!(sample().shown(), "9E11A10");
    assert_eq!(dirty.shown(), "9E11A10-DIRTY");
}

#[test]
fn a_build_with_no_git_still_has_a_serial() {
    let unknown = Build {
        hash: "unknown",
        date: "unknown",
        ..sample()
    };
    assert_eq!(unknown.serial(), "UNKNOWN");
    assert_eq!(unknown.shown(), "UNKNOWN");
}

#[test]
fn the_current_build_is_populated() {
    let b = Build::current();
    assert!(!b.version.is_empty());
    assert!(!b.hash.is_empty());
    assert!(!b.date.is_empty());
}
