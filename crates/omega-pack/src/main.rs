//! Writes a release's System update: `omega-pack <card tree> <version> <out dir> [tree before...]`
//! leaves `system-manifest.txt`, `system-files.bin` and `system-patches.bin` in the out dir,
//! with patches from each tree before given.

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [new, version, out, before @ ..] = &args[..] else {
        eprintln!("usage: omega-pack <card tree> <version> <out dir> [tree before...]");
        std::process::exit(2);
    };
    let before: Vec<PathBuf> = before.iter().map(PathBuf::from).collect();
    let (m, files, patches) = match omega_pack::build(new.as_ref(), version, &before) {
        Ok(built) => built,
        Err(e) => {
            eprintln!("omega-pack: {e}");
            std::process::exit(1);
        }
    };
    let out = PathBuf::from(out);
    let write = |name: &str, bytes: &[u8]| {
        if let Err(e) =
            std::fs::create_dir_all(&out).and_then(|()| std::fs::write(out.join(name), bytes))
        {
            eprintln!("omega-pack: {name}: {e}");
            std::process::exit(1);
        }
    };
    write(omega_pack::MANIFEST_ASSET, m.text().as_bytes());
    write(omega_pack::FILES_ASSET, &files);
    write(omega_pack::PATCHES_ASSET, &patches);
    println!(
        "omega-pack: {} files ({} bytes), {} patches ({} bytes)",
        m.files.len(),
        files.len(),
        m.patches.len(),
        patches.len()
    );
}
