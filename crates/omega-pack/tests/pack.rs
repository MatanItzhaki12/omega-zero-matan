//! A release's System update, from the trees on either side of it to the files the handheld
//! ends up with.

use std::path::Path;

use omega_pack::{build, gone, plan, rebuild, sha256, Manifest, Step};

fn put(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

/// A program of some size, much the same from one version to the next.
fn program(version: u8) -> Vec<u8> {
    let mut v: Vec<u8> = (0..200_000u32).map(|i| (i * 7 + i / 13) as u8).collect();
    v[1000] = version;
    v[150_000] = version.wrapping_mul(3);
    v
}

#[test]
fn sha256_matches_the_standard_vectors_in_pieces_or_whole() {
    assert_eq!(
        sha256::hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    assert_eq!(sha256::hex(b"abc"), abc);
    let mut s = sha256::Sha256::default();
    s.update(b"a");
    s.update(b"bc");
    assert_eq!(s.hex(), abc);
    let long = vec![b'a'; 1_000_000];
    let mut s = sha256::Sha256::default();
    for piece in long.chunks(777) {
        s.update(piece);
    }
    assert_eq!(
        s.hex(),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
}

#[test]
fn the_manifest_reads_back_as_it_was_written() {
    let old = tempfile::tempdir().unwrap();
    let new = tempfile::tempdir().unwrap();
    put(old.path(), "System/omega-zero", &program(1));
    put(new.path(), "System/omega-zero", &program(2));
    put(new.path(), "System/PPSSPP/flash0/font name.pgf", b"a font");
    let (m, _, _) = build(new.path(), "2.6.1", &[old.path().to_path_buf()]).unwrap();
    assert_eq!(Manifest::parse(&m.text()), Some(m.clone()));
    assert_eq!(m.files.len(), 2);
    assert_eq!(m.patches.len(), 1, "no patch from the program before");
}

#[test]
fn a_handheld_fetches_a_patch_for_what_it_has_and_whole_files_for_the_rest() {
    let old = tempfile::tempdir().unwrap();
    let new = tempfile::tempdir().unwrap();
    put(old.path(), "System/omega-zero", &program(1));
    put(old.path(), "System/wifi.sh", b"echo wifi");
    put(old.path(), "System/old.sh", b"echo gone");
    put(new.path(), "System/omega-zero", &program(2));
    put(new.path(), "System/wifi.sh", b"echo wifi");
    put(new.path(), "System/bluealsa", b"a new program");
    let (m, files, patches) = build(new.path(), "2.6.1", &[old.path().to_path_buf()]).unwrap();

    let have = |rel: &str| {
        std::fs::read(old.path().join(rel))
            .ok()
            .map(|b| sha256::hex(&b))
    };
    let steps = plan(&m, have);
    assert_eq!(
        steps.len(),
        2,
        "the script that did not change was fetched: {steps:?}"
    );
    for step in &steps {
        let rel = &step.file().path;
        let (data, old_bytes) = match step {
            Step::Whole(_) => (&files, None),
            Step::Patch(..) => (&patches, std::fs::read(old.path().join(rel)).ok()),
        };
        let at = match step {
            Step::Whole(f) => f.offset,
            Step::Patch(_, p) => p.offset,
        } as usize;
        let piece = &data[at..at + step.bytes() as usize];
        let made = rebuild(step, piece, old_bytes.as_deref()).unwrap();
        assert_eq!(made, std::fs::read(new.path().join(rel)).unwrap(), "{rel}");
    }
    let program_step = steps
        .iter()
        .find(|s| s.file().path == "System/omega-zero")
        .unwrap();
    assert!(
        matches!(program_step, Step::Patch(..)),
        "the program came whole"
    );
    assert!(
        program_step.bytes() * 20 < program(2).len() as u64,
        "the patch is not much smaller than the program: {}",
        program_step.bytes()
    );

    let installed: Vec<String> = ["System/omega-zero", "System/wifi.sh", "System/old.sh"]
        .map(String::from)
        .to_vec();
    assert_eq!(gone(&installed, &m), ["System/old.sh"]);
}

#[test]
fn a_piece_that_is_not_the_file_named_is_refused() {
    let new = tempfile::tempdir().unwrap();
    put(new.path(), "System/wifi.sh", b"echo wifi");
    let (m, files, _) = build(new.path(), "2.6.1", &[]).unwrap();
    let steps = plan(&m, |_| None);
    let mut piece = files.clone();
    let last = piece.len() - 1;
    piece[last] ^= 1;
    assert!(rebuild(&steps[0], &piece, None).is_err());
}

#[test]
fn a_manifest_naming_a_path_outside_system_is_refused() {
    let sha = "a".repeat(64);
    for path in ["System/../etc/passwd", "Saves/x", "/System/x", "System//x"] {
        let text = format!("omega-system 1\nversion 1\nfile {sha} 1 - 0 1 {path}\n");
        assert_eq!(Manifest::parse(&text), None, "{path}");
    }
}
