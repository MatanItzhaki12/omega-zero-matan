use omega_retro::LibretroCore;
use std::sync::MutexGuard;

use crate::CORE_LOCK;

fn lock() -> MutexGuard<'static, ()> {
    CORE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn dylib() -> std::path::PathBuf {
    omega_retro::host_core(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor"),
        "mgba",
    )
}

#[test]
fn an_unset_option_reads_back_as_absent() {
    let _g = lock();
    let Ok(core) = LibretroCore::open(&dylib()) else {
        eprintln!("no core available on this host, skipping");
        return;
    };
    assert_eq!(core.option("gpsp_serial"), None);
}

#[test]
fn a_set_option_reads_back() {
    let _g = lock();
    let Ok(mut core) = LibretroCore::open(&dylib()) else {
        eprintln!("no core available on this host, skipping");
        return;
    };
    core.set_option("gpsp_serial", "rfu");
    assert_eq!(core.option("gpsp_serial"), Some("rfu".to_string()));

    core.set_option("gpsp_serial", "mul_poke");
    assert_eq!(core.option("gpsp_serial"), Some("mul_poke".to_string()));
}
