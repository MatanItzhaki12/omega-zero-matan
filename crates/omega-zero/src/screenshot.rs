//! SELECT+X in a game keeps its screen as a picture: the frame the game last drew, at the
//! game's own size, as a PNG in `Screenshots/<system>/` named after the game and the time. The
//! picture is taken and written on a thread of its own, so the game does not wait on the card.

use std::path::PathBuf;

use omega_store::format_stamp;
use omega_ui::Toast;

use crate::app::App;

impl App {
    pub(crate) fn take_screenshot(&mut self) {
        let (Some(root), Some(cart)) = (self.root(), self.seated_cart()) else {
            return;
        };
        let Some(snapshot) = self.snapshot.as_ref().and_then(|s| s.detached()) else {
            return;
        };
        let path = screenshot_path(
            root.join("Screenshots").join(cart.platform.dir_name()),
            &cart.stem,
            &format_stamp(self.wall_secs()),
        );
        let spawned = std::thread::Builder::new()
            .name("omega-screenshot".into())
            .spawn(move || {
                let Some(png) = snapshot.thumb() else {
                    crate::log!("screenshot: the core gave no picture");
                    return;
                };
                let written = path
                    .parent()
                    .map_or(Ok(()), std::fs::create_dir_all)
                    .and_then(|()| omega_store::atomic_write(&path, &png));
                if let Err(e) = written {
                    crate::log!("screenshot: {}: {e}", path.display());
                }
            });
        if spawned.is_ok() {
            self.hud.toast(Toast::Screenshot, self.now());
        }
    }
}

/// Where a screenshot of `stem` taken at `stamp` goes in `dir`.
fn screenshot_path(dir: PathBuf, stem: &str, stamp: &str) -> PathBuf {
    dir.join(format!("{stem} {stamp}.png"))
}

#[cfg(test)]
mod tests {
    use super::screenshot_path;
    use std::path::PathBuf;

    #[test]
    fn a_screenshot_is_named_after_its_game_and_time() {
        let at = screenshot_path(
            PathBuf::from("Screenshots/GBA"),
            "Pokemon - Emerald Version (USA, Europe)",
            "2026-11-11_09-30-00",
        );
        assert_eq!(
            at,
            PathBuf::from(
                "Screenshots/GBA/Pokemon - Emerald Version (USA, Europe) 2026-11-11_09-30-00.png"
            )
        );
    }
}
