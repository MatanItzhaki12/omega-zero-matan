use std::path::PathBuf;
use std::time::Duration;

use omega_input::{Action, Gestures, Millis, RawEvent};
use omega_retro::Rumble;
use omega_store::Platform;
use omega_ui::FfState;

use crate::app::{App, Phase};
use crate::audio::{open_sink, AudioSink, Music, Ring, Sfx, GBA_HZ};
use crate::core::{open, Setup};
use crate::emu::{CoreState, EmuHandle, Speed};
use crate::frames::FrameRef;
use crate::input::Pad;
use crate::persist;

/// How many frames a core runs before the card stops noting it as opening: two seconds, well
/// past the first frame, where a core that cannot take its options gives up.
const SETTLED_FRAMES: u64 = 120;

pub struct Session {
    root: PathBuf,
    app: App,
    emu: Option<EmuHandle>,
    sink: Box<dyn AudioSink>,
    gestures: Gestures,
    pad: Pad,
    rewinding: bool,
    fast: bool,
    motor: u16,
    reloading: bool,
    driven: bool,
    /// The system and stem of the game the core was last opened for.
    running: Option<(Platform, String)>,
    /// The card notes the core as opening, until it has run a moment.
    starting: bool,
    /// A core being opened on a thread of its own, for the game by this stem: opening it and
    /// reading its resume state off the card take long enough, for a PSP game, to stall the
    /// cart going in.
    opening: Option<(String, std::sync::mpsc::Receiver<Opening>)>,
    /// The shelf's tune, played while no game is.
    music: Music,
}

/// What a core opened off the main thread brings back: the core, and what was read off the card
/// for its game.
struct Opening {
    opened: crate::core::Opened,
    resume: Option<Vec<u8>>,
    sav: Option<Vec<u8>>,
    disc: Option<(usize, String)>,
    platform: Platform,
    rom: PathBuf,
    player: Option<u8>,
}

impl Drop for Session {
    /// A session put away, as at power-off, leaves no game noted as opening: only one that went
    /// down with its core, and so never got here, does.
    fn drop(&mut self) {
        if self.starting {
            persist::clear_starting(&self.root);
        }
    }
}

impl Session {
    pub fn boot(root: PathBuf) -> Self {
        let mut sink: Box<dyn AudioSink> = open_sink();
        if let Err(e) = sink.open(GBA_HZ) {
            crate::log!("audio: {e}");
            // a computer with no sound device plays into a ring no one hears, as OMEGA_SILENT
            // has it, rather than into none at all
            #[cfg(feature = "host")]
            {
                let mut silent = crate::audio::StubSink::new();
                if silent.open(GBA_HZ).is_ok() {
                    sink = Box::new(silent);
                }
            }
        }
        Session {
            app: App::boot(&root),
            root,
            emu: None,
            sink,
            gestures: Gestures::new(),
            pad: Pad::default(),
            rewinding: false,
            fast: false,
            motor: 0,
            reloading: false,
            driven: false,
            running: None,
            opening: None,
            starting: false,
            music: Music::new(),
        }
    }

    pub fn play_sfx(&mut self, sfx: Sfx) {
        let ring = self.sink.ring();
        let rate = ring.sample_rate();
        if rate == 0 {
            return;
        }
        let mut samples = sfx.render(rate);
        crate::audio::volume::apply(&mut samples, self.app.sample_volume());
        ring.mix(&samples);
    }

    /// Keeps the shelf's tune a little ahead of the speaker while it may play: on the shelf and
    /// in its menus, Menu Music on, no game open or opening. It fades out as a cart goes in,
    /// and is drawn only while no core feeds the ring.
    fn play_music(&mut self) {
        let wanted = self.app.state.menu_music
            && self.emu.is_none()
            && self.opening.is_none()
            && !self.app.powering_off()
            && !matches!(
                self.app.phase(),
                Phase::Inserting { .. }
                    | Phase::Playing { .. }
                    | Phase::Ejecting { .. }
                    | Phase::Doze { .. }
            );
        self.music.set_playing(wanted);
        if self.emu.is_some() || self.music.silent() {
            return;
        }
        let ring = self.sink.ring();
        let rate = ring.sample_rate();
        if rate == 0 {
            return;
        }
        // past half the ring, where a ring that ran dry starts again
        let lead = ring.capacity_frames() * 3 / 4;
        let short = lead.saturating_sub(ring.queued_frames());
        if short > 0 {
            ring.push(&self.music.render(rate, short, self.app.sample_volume()));
        }
    }

    pub fn audio_queued(&self) -> usize {
        self.sink.ring().queued_frames()
    }

    pub fn audio_ring(&self) -> std::sync::Arc<Ring> {
        self.sink.ring()
    }

    pub fn rumble(&mut self, strength: u16) {
        if strength == self.motor {
            return;
        }
        self.motor = strength;
        self.app.set_rumble(strength);
    }

    pub fn core_rumble(&self) -> Option<&Rumble> {
        self.emu.as_ref().map(EmuHandle::rumble)
    }

    pub fn app(&self) -> &App {
        &self.app
    }

    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    pub fn emu(&self) -> Option<&EmuHandle> {
        self.emu.as_ref()
    }

    pub fn set_driven(&mut self, driven: bool) {
        self.driven = driven;
        if let Some(emu) = &self.emu {
            emu.set_driven(driven);
        }
    }

    pub fn step_emulator(&self, present: Duration, timeout: Duration) -> bool {
        self.emu.as_ref().is_some_and(|emu| {
            emu.tick(present);
            emu.wait_frame(timeout)
        })
    }

    pub fn frame(&self) -> Option<FrameRef> {
        self.emu.as_ref().and_then(|e| e.latest_frame())
    }

    /// The last frame a core that draws on the GPU drew as a shared texture, taken.
    pub fn take_gpu_frame(&self) -> Option<omega_gfx::GpuFrame> {
        self.emu.as_ref().and_then(|e| e.take_gpu_frame())
    }

    pub fn core_settling(&self) -> bool {
        self.emu
            .as_ref()
            .is_some_and(|e| e.state() == CoreState::Loading)
    }

    pub fn has_core(&self) -> bool {
        self.emu.is_some()
    }

    pub fn emulator_locked(&self) -> bool {
        self.emu.as_ref().is_some_and(EmuHandle::locked)
    }

    pub fn frame_ready(&self) -> bool {
        self.emu.as_ref().is_some_and(EmuHandle::frame_ready)
    }

    pub fn frames_published(&self) -> u64 {
        self.emu.as_ref().map_or(0, EmuHandle::published_count)
    }

    pub fn observed_speed(&self) -> Option<Speed> {
        self.emu.as_ref().map(EmuHandle::observed_speed)
    }

    pub fn frames_taken(&self) -> u64 {
        self.emu.as_ref().map_or(0, EmuHandle::frames_taken)
    }

    pub fn game_visible(&self) -> bool {
        self.app.game_visible()
    }

    pub fn feed(&mut self, events: impl IntoIterator<Item = RawEvent>, now: Millis) {
        let keys = self.app.keys();
        self.pad.set_keys(keys);
        let mut actions = self.gestures.set_keys(keys);
        for ev in events {
            actions.extend(self.gestures.feed(ev, now));
        }
        actions.extend(self.gestures.tick(now));
        for action in actions {
            self.act(action);
        }
        self.sync_pad();
    }

    fn sync_pad(&mut self) {
        // the stick changed in the game's menu
        if let Some(stick) = self.app.take_stick() {
            self.pad.set_stick(stick);
        }
        for btn in self.app.taken_buttons() {
            self.pad.apply(Action::GbaUp(*btn));
        }
        if let Some(emu) = &self.emu {
            emu.set_input(self.pad.mask());
            let (x, y) = self.pad.analog();
            emu.set_analog(x, y);
        }
    }

    fn bridge_link(&mut self, f: impl FnOnce(&mut App)) {
        let had_link = self.app.link_active();
        f(&mut self.app);
        if had_link && !self.app.link_active() {
            if let Some(emu) = &self.emu {
                emu.end_link();
            }
        }
    }

    fn act(&mut self, action: Action) {
        if trace() {
            crate::log!("{action:?} in {:?}", self.app.phase());
        }
        match action {
            Action::RewindStart => self.rewinding = true,
            Action::RewindStop => self.rewinding = false,
            Action::FfStart => self.fast = true,
            Action::FfStop => self.fast = false,
            _ => {}
        }
        let menu = self.overlaid();
        self.bridge_link(|app| app.apply(action));
        if matches!(
            action,
            Action::VolumeUp | Action::VolumeDown | Action::MuteToggle
        ) {
            if let Some(emu) = &self.emu {
                emu.set_volume(self.app.sample_volume());
            }
        }
        if menu || self.overlaid() {
            self.pad.clear();
        } else if self.app.takes_from_the_game(action) {
            if let Action::GbaDown(btn) | Action::GbaUp(btn) = action {
                self.pad.apply(Action::GbaUp(btn));
            }
        } else {
            self.pad.apply(action);
        }
        self.sync_rumble();
    }

    pub fn update(&mut self, dt: f32) {
        let mut sw = crate::stopwatch::Stopwatch::new();
        self.bridge_link(|app| app.update(dt));
        sw.step("app");
        if let Some(emu) = &self.emu {
            emu.set_volume(self.app.sample_volume());
        }
        if let Some((client_id, transport)) = self.app.take_link_transport() {
            match &self.emu {
                Some(emu) => match self.app.link_player() {
                    Some(player) => emu.begin_cable(player, transport),
                    None => emu.begin_link(client_id, transport),
                },
                None => crate::log!("link: a transport arrived with no core to run it"),
            }
        }
        if let Some(on) = self.app.take_colour_correction() {
            if let Some((key, value)) = crate::core::colour_option(self.app.core(), on) {
                if let Some(emu) = &self.emu {
                    emu.set_option(key, value);
                }
            }
        }
        if let (Some(emu), Some((platform, _))) = (&self.emu, &self.running) {
            emu.set_turbo(platform.turbo_xy() && self.app.turbo_on());
            // gpSP only: mGBA plays as soon on its own, and has no time for a frame twice
            emu.set_run_ahead(
                self.app.run_ahead_on() && self.app.core() == omega_store::Core::Gpsp,
            );
        }
        if self.app.take_core_options() {
            if let Some(emu) = &self.emu {
                for (key, value) in self.app.core_extras() {
                    emu.set_option(key, value);
                }
            }
        }
        if let Some(disc) = self.app.take_disc_swap() {
            if let Some(emu) = &self.emu {
                emu.set_disc(disc);
            }
        }
        if let Some((stem, serial)) = self.app.take_link_reload() {
            self.reload_for_link(&stem, serial);
        }
        if self.app.link_active() {
            if self.emu.as_ref().is_some_and(EmuHandle::bios_mismatch) {
                self.bridge_link(|app| app.bios_mismatch());
            } else if self.emu.as_ref().is_some_and(EmuHandle::peer_ended) {
                self.bridge_link(|app| app.peer_ended());
            } else if self.emu.as_ref().is_some_and(EmuHandle::link_lost) {
                self.app.peer_lost();
            }
        }
        if let Some(sfx) = self.app.take_sfx() {
            self.play_sfx(sfx);
        }
        self.play_music();
        sw.step("options");
        self.sync_core();
        sw.step("core");
        self.settle_start();
        sw.step("settle");
        self.sync_reload();
        self.keep_swapped_disc();
        if !self.has_core() {
            for action in self.gestures.drop_ff_latch() {
                self.act(action);
            }
        }
        self.app
            .set_game_ready(self.emu.as_ref().is_some_and(EmuHandle::has_published));
        self.sync_speed();
        self.sync_rewind_hud();
        self.sync_ff_hud();
        self.sync_rumble();
        self.sync_pad();
        sw.step("rest");
        sw.log_over("update", std::time::Duration::from_millis(30));
    }

    fn sync_rumble(&mut self) {
        let want = match &self.emu {
            Some(emu) if self.playing() && self.app.rumble_enabled() => emu.rumble().strength(),
            _ => 0,
        };
        self.rumble(want);
    }

    fn sync_ff_hud(&mut self) {
        let ff = match (self.actually_fast_forwarding(), self.gestures.ff_latched()) {
            (false, _) => FfState::Off,
            (true, false) => FfState::Held,
            (true, true) => FfState::Latched,
        };
        self.app.set_ff(ff);
    }

    fn sync_rewind_hud(&mut self) {
        let fill = self
            .actually_rewinding()
            .then(|| self.emu.as_ref().map(EmuHandle::rewind_fill))
            .flatten();
        match fill {
            Some(fill) => self.app.show_rewind(fill),
            None => self.app.hide_rewind(),
        }
    }

    fn actually_rewinding(&self) -> bool {
        self.rewinding && self.playing() && self.app.may_rewind()
    }

    fn actually_fast_forwarding(&self) -> bool {
        self.fast && self.playing() && self.app.may_fast_forward()
    }

    fn inserting(&self) -> bool {
        matches!(self.app.phase(), Phase::Inserting { .. })
    }

    fn showing_polaroids(&self) -> bool {
        matches!(self.app.phase(), Phase::Polaroids { .. })
    }

    fn overlaid(&self) -> bool {
        self.showing_polaroids() || self.held()
    }

    fn playing(&self) -> bool {
        matches!(self.app.phase(), Phase::Playing { .. }) && !self.held()
    }

    fn held(&self) -> bool {
        self.app.power_shown() || self.app.game_menu_open() || self.app.shutting_down()
    }

    fn dozing(&self) -> bool {
        matches!(self.app.phase(), Phase::Doze { .. })
    }

    fn ejecting(&self) -> bool {
        matches!(self.app.phase(), Phase::Ejecting { .. })
    }

    fn sync_speed(&self) {
        if let Some(emu) = &self.emu {
            emu.set_fast_steps(u32::from(self.app.ff_speed()));
            emu.set_ff_sound(self.app.ff_sound());
            emu.set_speed(
                if self.inserting()
                    || self.ejecting()
                    || self.showing_polaroids()
                    || self.dozing()
                    || (self.held() && !self.app.link_active())
                {
                    Speed::Paused
                } else if self.actually_fast_forwarding() {
                    Speed::Fast
                } else {
                    Speed::Normal
                },
            );
            emu.set_rewinding(self.actually_rewinding());
        }
    }

    fn no_core() -> bool {
        std::env::var_os("OMEGA_NO_CORE").is_some_and(|v| v != "0")
    }

    /// Runs the game on the core opened for it, once its thread has opened it.
    fn take_opened_core(&mut self) {
        let Some((stem, rx)) = &self.opening else {
            return;
        };
        match rx.try_recv() {
            Ok(opening) => {
                let stem = stem.clone();
                self.opening = None;
                self.finish_core(&stem, opening);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.opening = None,
        }
    }

    fn sync_core(&mut self) {
        if Self::no_core() {
            return;
        }
        self.take_opened_core();
        let stem = match self.app.phase() {
            Phase::Shelf => {
                self.emu = None;
                self.opening = None;
                return;
            }
            Phase::Inserting { cart, .. } => cart.clone(),
            _ => return,
        };
        if self.opening.as_ref().is_some_and(|(s, _)| *s != stem) {
            self.opening = None;
        }
        if self.emu.is_none() {
            if self.opening.is_none() {
                let (_, serial) = self.app.link_mode(&stem);
                self.spawn_core(&stem, serial);
            }
            return;
        }
        match self.emu.as_ref().map(EmuHandle::state) {
            Some(CoreState::Loading) => {}
            Some(CoreState::Ready) => self.app.on_core_ready(),
            Some(CoreState::Failed) | None => {
                self.emu = None;
                self.app.on_core_failed();
            }
        }
    }

    fn spawn_core(&mut self, stem: &str, serial: &'static str) {
        let Some((rom, platform)) = self
            .app
            .seated_cart()
            .filter(|c| c.stem == stem)
            .map(|c| (c.rom.clone(), c.platform))
        else {
            return;
        };
        let core = omega_store::core_for_platform(&self.root, stem, platform);
        self.app.set_core(core);
        self.app.set_platform(platform);
        let fallback = match platform {
            Platform::Psp => self.app.psp_screen(),
            _ => crate::video_mode::VideoMode::default(),
        };
        self.app
            .set_video_mode(crate::video_mode::video_mode_or(&self.root, stem, fallback));
        // the D-pad as the stick, for a game that wants one the handheld has none of
        self.pad
            .set_stick(crate::input::stick_for(&self.root, stem, platform));
        self.app.set_link_loaded(serial);
        // hardcore picks up from no state, as it loads none
        let resume_wanted = !self.app.starting_clean() && !self.app.hardcore();
        let player = self.app.link_player();
        let setup = match platform {
            Platform::Snes | Platform::Ps1 | Platform::Psp => Setup::Console,
            Platform::Gba | Platform::Gb | Platform::Gbc => Setup::Handheld {
                serial,
                colour: self.app.colour_correction(),
                link: player,
            },
        };
        // the core and what its game needs off the card, opened on a thread of its own so the
        // cart goes on going in meanwhile
        self.starting = true;
        let (tx, rx) = std::sync::mpsc::channel();
        let (root, theirs) = (self.root.clone(), stem.to_string());
        let wants_resume = resume_wanted;
        // a save still being written, as the game put away last is: waited for there, so the
        // resume read is the one just written
        let writing = self.app.take_pending_save(platform, stem);
        let spawned = std::thread::Builder::new()
            .name("omega-core-open".into())
            .spawn(move || {
                // the cart going in is drawn meanwhile: it comes first
                crate::spawn::stand_back();
                // noted before the core opens, so a core that takes omega-zero down as it opens
                // is known; here rather than on the main thread, as the note is synced to the
                // card, which a PSP game's image is being read off as it starts, and the wait
                // stalled the cart going in
                persist::mark_starting(&root, platform, &theirs);
                if let Some(writing) = writing {
                    let _ = writing.join();
                }
                let resume = wants_resume
                    .then(|| persist::read_resume(&root, platform, core, &theirs))
                    .flatten();
                let opened = open(&root, core, setup);
                // a game put away on its second disc comes back on it, for its resume state is
                // of that disc; a game started afresh starts from the first
                let disc = resume
                    .is_some()
                    .then(|| persist::read_disc(&root, platform, &theirs))
                    .flatten();
                let sav = persist::read_sav(&root, platform, &theirs);
                let _ = tx.send(Opening {
                    opened,
                    resume,
                    sav,
                    disc,
                    platform,
                    rom,
                    player,
                });
            });
        if spawned.is_ok() {
            self.opening = Some((stem.to_string(), rx));
        }
    }

    /// Runs the game by `stem` on the core opened for it.
    fn finish_core(&mut self, stem: &str, opening: Opening) {
        let Opening {
            opened,
            resume,
            sav,
            disc,
            platform,
            rom,
            player,
        } = opening;
        self.app.set_named_core(opened.named);
        let mut game = opened.core;
        if let Some((index, path)) = disc {
            game.set_initial_disc(index, &path);
            self.app.set_disc_in(stem, index);
        }
        let ring = self.sink.ring();
        let rules = crate::emu::Rules {
            turbo: platform.turbo_xy() && self.app.turbo_on(),
            // rewind keeps its snapshots whatever the setting, which L2 itself follows, so
            // switching it on in a game rewinds at once
            rewinds: platform.rewinds(),
            cheevos: self.app.achievements_console(),
        };
        self.app.achievements_game_starts();
        let emu =
            EmuHandle::spawn_with_rules(platform, rules, game, rom, ring, sav, resume, player);
        // frame blending and the Game Boy's palette, as Settings has them
        self.app.take_core_options();
        for (key, value) in self.app.core_extras() {
            emu.set_option(key, value);
        }
        emu.set_volume(self.app.sample_volume());
        emu.set_driven(self.driven);
        self.app.set_snapshot(Box::new(emu.snapshot()));
        self.emu = Some(emu);
        self.running = Some((platform, stem.to_string()));
    }

    /// Takes the note of the core opening off the card once it has run a moment, or once there
    /// is no core: it opened, or failed to, without taking omega-zero down.
    fn settle_start(&mut self) {
        // a core still opening on its own thread has not settled
        let settled = self.opening.is_none()
            && self
                .emu
                .as_ref()
                .is_none_or(|emu| emu.published_count() >= SETTLED_FRAMES);
        if self.starting && settled {
            persist::clear_starting(&self.root);
            self.starting = false;
        }
    }

    /// Keeps the disc a swap put in, so that the game comes back on it.
    fn keep_swapped_disc(&mut self) {
        let (Some(emu), Some((platform, stem))) = (&self.emu, &self.running) else {
            return;
        };
        if let Some((index, path)) = emu.take_swapped_disc() {
            if let Err(e) = persist::write_disc(&self.root, *platform, stem, index, &path) {
                crate::log!("disc: could not keep disc {index} of {stem}: {e}");
            }
        }
    }

    fn reload_for_link(&mut self, stem: &str, serial: &'static str) {
        crate::log!("link: loading {stem} again with gpsp_serial={serial}");
        if self.emu.is_some() {
            self.app.flush_resume();
        }
        self.emu = None;
        self.spawn_core(stem, serial);
        self.reloading = true;
    }

    fn sync_reload(&mut self) {
        if !self.reloading {
            return;
        }
        if self.opening.is_some() {
            return;
        }
        match self.emu.as_ref().map(EmuHandle::state) {
            Some(CoreState::Loading) => {}
            Some(CoreState::Ready) => {
                self.reloading = false;
                self.app.link_reload_done();
            }
            Some(CoreState::Failed) | None => {
                self.reloading = false;
                self.emu = None;
                self.app.link_reload_failed();
                if let Some((stem, serial)) = self.app.take_link_reload() {
                    self.reload_for_link(&stem, serial);
                }
            }
        }
    }
}

pub(crate) fn trace() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("OMEGA_TRACE").is_some())
}
