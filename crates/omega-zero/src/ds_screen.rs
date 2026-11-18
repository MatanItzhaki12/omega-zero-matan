//! A DS game's screens on the emulator thread: [`omega_ds::Ds`] fed from the core, the functions
//! its layouts watch handed to the core, and the picture it lays out published in place of the
//! core's frame of both screens.

use std::path::Path;

use omega_ds::{Ds, Layout};
use omega_retro::ds::DsHooks;
use omega_retro::{ButtonMask, RetroCore};

/// Each DS game's screen layout, where it was changed from Settings' default.
pub const LAYOUT_FILE: &str = "Config/ds_layout.ini";

/// The layout the DS game `stem` was set to show in, else `fallback`.
pub fn layout_for(root: &Path, stem: &str, fallback: Layout) -> Layout {
    omega_store::ini::value(root, LAYOUT_FILE, stem)
        .as_deref()
        .and_then(Layout::parse)
        .unwrap_or(fallback)
}

pub fn write_layout(root: &Path, stem: &str, layout: Layout) -> std::io::Result<()> {
    omega_store::ini::write(root, LAYOUT_FILE, stem, layout.as_str())
}

/// A DS game's screens, while its core runs.
pub struct DsScreens {
    ds: Ds,
    hooks: Option<DsHooks>,
    /// The DS's main memory, as the core answers for it: good while the game is loaded.
    main_ram: (*const u8, usize),
}

// SAFETY: made and used only on the emulator thread that runs the core it reads, whose main
// memory `main_ram` points into; only the empty slot it starts in crosses to that thread
unsafe impl Send for DsScreens {}

impl DsScreens {
    /// The screens of the game at `rom`, which `core` has just loaded, laid out `layout`.
    pub fn start(core: &mut dyn RetroCore, rom: &Path, layout: Layout) -> DsScreens {
        let code = omega_store::nds::info(rom)
            .map(|i| i.code)
            .unwrap_or_default();
        let mut ds = Ds::new(rom, &code, layout);
        let hooks = core.ds_hooks();
        let main_ram = core.memory().map_or((std::ptr::null(), 0), |m| {
            (m.system.0.cast_const(), m.system.1)
        });
        match (hooks, ds.watch()) {
            (Some(h), Some((bitmap, hits, base, end))) => {
                // SAFETY: on the thread that runs the core, which has the game loaded; the
                // bitmaps are the watch's, kept where they are until it is replaced on drop
                unsafe { h.watch(Some((bitmap, hits, base, end))) };
                crate::log!("ds: {code}, its layouts watched by the core");
            }
            (None, Some(_)) => crate::log!("ds: {code} has layouts, but the core cannot watch"),
            _ => crate::log!("ds: {code}, no layouts of its own"),
        }
        DsScreens {
            ds,
            hooks,
            main_ram,
        }
    }

    pub fn set_layout(&mut self, layout: Layout) {
        self.ds.set_layout(layout);
    }

    pub fn swap(&mut self) {
        self.ds.swap();
    }

    /// After a state is loaded or the game reset: what ran before is forgotten.
    pub fn forget(&mut self) {
        self.ds.forget();
    }

    /// The buttons the game is handed for the next frame, the touch screen touched on `core`
    /// as the cursor or a layout's button says.
    pub fn input(&mut self, core: &mut dyn RetroCore, held: ButtonMask) -> ButtonMask {
        let (keys, touch) = self
            .ds
            .before_frame(held.0, core.video_xrgb8888(), core.video_size());
        core.set_pointer(touch);
        ButtonMask(keys)
    }

    /// Counts a frame `core` ran.
    pub fn ran(&mut self) {
        let ram = match self.main_ram {
            (p, len) if !p.is_null() => {
                // SAFETY: the core's main memory, which stays while the game is loaded and is
                // only read here, on the thread that runs it, between its frames
                unsafe { std::slice::from_raw_parts(p, len) }
            }
            _ => &[][..],
        };
        self.ds.ran(ram);
    }

    /// The picture for the frame `core` drew last, laid out into `out`, and its size: none for a
    /// frame that is not the DS's two screens (the mock's, with no core).
    pub fn view(&mut self, core: &dyn RetroCore, out: &mut Vec<u8>) -> Option<(u32, u32)> {
        let layers = match &self.hooks {
            // SAFETY: on the core's thread, between its frames, read before the next
            Some(h) => unsafe { [h.layers(0), h.layers(1)] },
            None => [&[][..], &[][..]],
        };
        self.ds
            .view(core.video_xrgb8888(), core.video_size(), layers, out)
    }
}

impl Drop for DsScreens {
    fn drop(&mut self) {
        if let Some(h) = &self.hooks {
            // SAFETY: on the core's thread; the core no longer marks the bitmaps going with it
            unsafe { h.watch(None) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_games_layout_is_kept_by_its_name_and_falls_back_to_the_default() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(
            layout_for(d.path(), "Platinum", Layout::OneScreen),
            Layout::OneScreen
        );
        write_layout(d.path(), "Platinum", Layout::Stacked).unwrap();
        assert_eq!(
            layout_for(d.path(), "Platinum", Layout::OneScreen),
            Layout::Stacked
        );
        assert_eq!(layout_for(d.path(), "White", Layout::Focus), Layout::Focus);
    }
}
