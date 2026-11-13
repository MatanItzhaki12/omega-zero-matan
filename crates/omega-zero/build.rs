use std::fmt::Write as _;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8(out.stdout).ok()?.trim().to_string())
}

fn main() {
    let hash = git(&["rev-parse", "--short", "HEAD"])
        .or_else(|| {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
            let root = root.canonicalize().ok()?;
            git(&[
                "config",
                "--global",
                "--add",
                "safe.directory",
                root.to_str()?,
            ])?;
            git(&["rev-parse", "--short", "HEAD"])
        })
        .unwrap_or_else(|| "unknown".into());
    // A worktree's HEAD lives in the main checkout's .git/worktrees, not under its own .git,
    // which there is a file. Watching a path that does not exist reruns this script, and so
    // rebuilds the crate and relinks every test, on every build.
    match git(&["rev-parse", "--absolute-git-dir"]) {
        Some(dir) => {
            println!("cargo:rerun-if-changed={dir}/HEAD");
            println!("cargo:rerun-if-changed={dir}/logs/HEAD");
        }
        None => println!("cargo:rerun-if-changed=build.rs"),
    }
    let dirty = git(&["status", "--porcelain"]).is_some_and(|s| !s.is_empty());
    let date = Command::new("date")
        .args(["-u", "+%Y-%m-%d"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map_or_else(|| "unknown".into(), |s| s.trim().to_string());

    println!("cargo:rustc-env=OMEGA_GIT_HASH={hash}");
    println!("cargo:rustc-env=OMEGA_GIT_DIRTY={}", u8::from(dirty));
    println!("cargo:rustc-env=OMEGA_BUILD_DATE={date}");

    screenscraper_key();
}

/// The same mask `label_sync::unmask` takes off.
fn mask(text: &str) -> String {
    text.bytes()
        .enumerate()
        .fold(String::new(), |mut out, (i, b)| {
            let _ = write!(
                out,
                "{:02x}",
                b ^ (0x5a_u8.wrapping_add((i as u8).wrapping_mul(31)))
            );
            out
        })
}

/// ScreenScraper's key for omega-zero, from the environment of whoever builds it, masked so it is
/// not lying in the binary as plain text.
fn screenscraper_key() {
    println!("cargo:rerun-if-env-changed=OMEGA_SS_DEVID");
    println!("cargo:rerun-if-env-changed=OMEGA_SS_DEVPASSWORD");
    // a space or line break pasted in with the key is not part of it
    let id: String = std::env::var("OMEGA_SS_DEVID")
        .unwrap_or_default()
        .split_whitespace()
        .collect();
    let pw: String = std::env::var("OMEGA_SS_DEVPASSWORD")
        .unwrap_or_default()
        .split_whitespace()
        .collect();
    if !id.is_empty() && !pw.is_empty() {
        println!("cargo:rustc-env=OMEGA_SS_KEY={}:{}", mask(&id), mask(&pw));
    }
}
