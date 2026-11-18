use omega_store::Platform;

#[test]
fn every_platform_has_a_directory() {
    let names: Vec<&str> = Platform::ALL.iter().map(|p| p.dir_name()).collect();
    assert_eq!(names, vec!["GBA", "GB", "GBC", "SNES", "PS1", "PSP", "NDS"]);
}

#[test]
fn every_platform_is_a_shelf_of_its_own() {
    assert_eq!(
        Platform::ALL,
        [
            Platform::Gba,
            Platform::Gb,
            Platform::Gbc,
            Platform::Snes,
            Platform::Ps1,
            Platform::Psp,
            Platform::Nds
        ]
    );
    for (i, p) in Platform::ALL.iter().enumerate() {
        assert!(
            !Platform::ALL[..i].contains(p),
            "{p:?} is named twice in the ring, so two shelves would hold one platform"
        );
    }
}

#[test]
fn each_platform_takes_only_its_own_extensions() {
    assert!(Platform::Gba.accepts(std::path::Path::new("Metroid Fusion.gba")));
    assert!(!Platform::Gba.accepts(std::path::Path::new("Tetris.gb")));
    assert!(Platform::Gb.accepts(std::path::Path::new("Tetris.gb")));
    assert!(Platform::Gb.accepts(std::path::Path::new("Tetris.gbc")));
    assert!(!Platform::Gb.accepts(std::path::Path::new("Metroid Fusion.gba")));
    assert!(Platform::Gba.accepts(std::path::Path::new("Shrek.GBA")));
}

/// A PSP game is a UMD's image, compressed or not, or an eboot; it rewinds nothing, and held L2
/// is its analog stick.
#[test]
fn the_psp_takes_its_images_and_holds_l2_for_the_stick() {
    for name in ["Daxter.iso", "Lumines.CSO", "Patapon.chd", "Echochrome.pbp"] {
        assert!(Platform::Psp.accepts(std::path::Path::new(name)), "{name}");
    }
    assert!(!Platform::Psp.accepts(std::path::Path::new("Crash.cue")));
    assert_eq!(Platform::Psp.picture(), (480, 272));
    assert!(!Platform::Psp.rewinds());
    // PPSSPP paces a game on a thread of its own, so a PSP game has no fast-forward
    assert!(!Platform::Psp.fast_forwards());
    assert!(Platform::Ps1.fast_forwards() && Platform::Gba.fast_forwards());
    assert!(Platform::Psp.stick_on_l2());
    assert!(!Platform::Ps1.stick_on_l2());
    assert_eq!(Platform::Psp.name(), "PlayStation Portable");
}

/// A DS game is its cartridge's dump; it rewinds and fast-forwards nothing, held L2 is a cursor
/// on its touch screen, and it has two screens to show on one.
#[test]
fn the_ds_takes_its_dumps_and_holds_l2_for_the_touch_screen() {
    assert!(Platform::Nds.accepts(std::path::Path::new("Pokemon Platinum.nds")));
    assert!(Platform::Nds.accepts(std::path::Path::new("Pokemon White 2.NDS")));
    assert!(!Platform::Nds.accepts(std::path::Path::new("Metroid Fusion.gba")));
    assert_eq!(Platform::Nds.picture(), (256, 192));
    assert!(!Platform::Nds.rewinds());
    assert!(!Platform::Nds.fast_forwards());
    assert!(Platform::Nds.touch_on_l2() && Platform::Nds.two_screens());
    assert!(!Platform::Nds.stick_on_l2() && !Platform::Nds.game_owns_l2_r2());
    assert!(!Platform::Psp.touch_on_l2() && !Platform::Gba.two_screens());
    assert_eq!(Platform::Nds.name(), "Nintendo DS");
    assert_eq!(Platform::Nds.dir_name(), "NDS");
}
