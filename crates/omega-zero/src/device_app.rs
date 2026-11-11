use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use omega_gfx::{
    canvas, set_canvas, Canvas, Compositor, FbdevSurface, Rotation, Surface, CANVAS_ENV,
};
use omega_power::{baseos_release, trace_first_frame, DevicePlatform};
use omega_ui::UndoFace;
use omega_zero::frontend::Frontend;
use omega_zero::input::DeviceInput;

const CARD: &str = "/mnt/sdcard";

const MIN_FRAME: Duration = Duration::from_millis(12);

const STEP_TIMEOUT: Duration = Duration::from_millis(12);

const MARGIN: Duration = Duration::from_micros(1500);

struct Pacer {
    last_return: Option<Instant>,
    period: Duration,
    work: Duration,
    works: [Duration; WORK_WINDOW],
    next: usize,
}

const WORK_WINDOW: usize = 32;

impl Pacer {
    fn new() -> Self {
        Pacer {
            last_return: None,
            period: Duration::from_micros(16_760),
            work: Duration::from_millis(4),
            works: [Duration::from_millis(4); WORK_WINDOW],
            next: 0,
        }
    }

    fn wait(&self) {
        let Some(last) = self.last_return else {
            return;
        };
        let delay = self.period.saturating_sub(self.work + MARGIN);
        if let Some(left) = (last + delay).checked_duration_since(Instant::now()) {
            std::thread::sleep(left);
        }
    }

    fn swapped(&mut self, work: Duration, blocked: bool) -> bool {
        let now = Instant::now();
        let mut missed = false;
        if let Some(last) = self.last_return {
            let seen = now - last;
            missed = seen > self.period.mul_f64(1.5);
            if blocked && seen > Duration::from_millis(12) && seen < Duration::from_millis(22) {
                self.period = self.period.mul_f64(0.95) + seen.mul_f64(0.05);
            }
        }
        self.works[self.next] = work;
        self.next = (self.next + 1) % WORK_WINDOW;
        self.work = self.works.iter().copied().max().unwrap_or(work);
        self.last_return = Some(now);
        missed
    }
}

/// Picks the canvas for this panel, held the way BaseOS says it stands, before anything is
/// drawn. OMEGA_CANVAS and OMEGA_ROTATION (degrees counter-clockwise) override what is found.
fn choose_canvas(panel: (u32, u32)) -> Rotation {
    let release = baseos_release();
    let rotation = std::env::var("OMEGA_ROTATION")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .or(release.as_ref().map(|r| r.rotation_ccw))
        .map(Rotation::from_ccw_degrees)
        .unwrap_or_default();
    let upright = rotation.upright(panel);
    if std::env::var_os(CANVAS_ENV).is_none() {
        if let Err(drawn) = set_canvas(Canvas::for_panel(upright.0, upright.1)) {
            omega_zero::log!("the canvas was already {drawn:?} before the panel was read");
        }
    }
    let c = canvas();
    omega_zero::log!(
        "{} panel {}x{} turned {rotation:?}, canvas {}x{}",
        release
            .as_ref()
            .and_then(|r| r.model.clone().or(r.target.clone()))
            .unwrap_or_else(|| "unknown".into()),
        panel.0,
        panel.1,
        c.w,
        c.h
    );
    rotation
}

/// The boot logo being read off the card.
type Reading = std::thread::JoinHandle<Option<UndoFace>>;

/// Reads the logo the bootloader left lit on the panel, on a thread of its own while the shelf
/// is made, when the canvas is the panel the right way up: drawn over the first frames, it is
/// then where and as big as the bootloader drew it. Turned or scaled, it could not be.
fn read_boot_logo(root: &Path, panel: (u32, u32), rotation: Rotation) -> Option<Reading> {
    let c = canvas();
    if rotation != Rotation::R0 || (c.w, c.h) != panel {
        return None;
    }
    let system = root.join("System");
    std::thread::Builder::new()
        .name("omega-logo".into())
        .spawn(move || omega_zero::bootlogo::shown(&system, panel))
        .ok()
}

pub fn run() {
    let root = std::env::var_os("OMEGA_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(CARD));
    // first of all, while the process is nearly empty: gpSP's code cache needs room beside it
    omega_zero::core::keep_gpsp_room(&root);
    let mut surface = match FbdevSurface::new() {
        Ok(s) => s,
        Err(e) => {
            omega_zero::log!("{e}");
            return;
        }
    };
    let rotation = choose_canvas(surface.window_size());
    let logo = read_boot_logo(&root, surface.window_size(), rotation);
    // a context for cores that draw on the GPU, lent to each game that wants one
    match omega_gfx::Surface::gpu_thread(&surface) {
        Ok(gpu) => omega_zero::gpu_core::provide(gpu),
        Err(e) => omega_zero::log!("gpu: {e}"),
    }
    let mut compositor = match Compositor::new(&surface) {
        Ok(c) => c,
        Err(e) => {
            omega_zero::log!("{e}");
            return;
        }
    };
    // EGL has the framebuffer open now; the scripts and daemons started from here on get none
    // of it
    omega_zero::spawn::keep_files_to_self();
    if let Err(e) = compositor.set_rotation(rotation) {
        omega_zero::log!("the panel cannot be turned {rotation:?}, drawing it unturned: {e}");
    }
    let platform = DevicePlatform::new(root.clone());
    omega_zero::log!("{}", platform.report());
    platform.trace_boot();
    let mut frontend = Frontend::boot(Box::new(platform));
    frontend.upload_faces(&mut compositor);
    if let Some(logo) = logo.and_then(|reading| reading.join().ok()).flatten() {
        frontend.show_boot_logo(&mut compositor, &logo);
    }
    frontend.start_opening(&omega_zero::opening::Facts::read());
    let mut input = DeviceInput::open(&root);
    let card = root.clone();
    let _ = std::thread::Builder::new()
        .name("omega-bootlogo".into())
        .spawn(move || omega_zero::bootlogo::refresh(&card));
    let mut drawn = false;
    // from here the small files on the card are written on its own thread, as the screen is
    // drawn; powering off and restarting wait for them
    omega_zero::card_writer::defer(true);
    frontend.drive_emulator();
    let mut pacer = Pacer::new();
    let (mut frames, mut missed, mut early, mut unanswered) = (0u32, 0u32, 0u32, 0u32);
    loop {
        pacer.wait();
        let began = Instant::now();
        frontend.advance(&mut input);
        let advanced = began.elapsed();
        if frontend.restarting() {
            frontend.restart();
        }
        if frontend.powering_off() {
            frontend.poweroff();
            return;
        }
        let locked = frontend.emulator_locked();
        // a tick the emulator did not answer in time is drawn with the frame before it
        let stepping = Instant::now();
        let answered = frontend.step_emulator(pacer.period, STEP_TIMEOUT);
        let stepped = stepping.elapsed();
        let rendering = Instant::now();
        unanswered = unanswered.saturating_add(u32::from(locked && !answered));
        frontend.render(&mut compositor, surface.window_size());
        let swap = Instant::now();
        let work = swap - began;
        if let Err(e) = surface.swap() {
            omega_zero::log!("{e}");
            return;
        }
        let swap_took = swap.elapsed();
        frontend.shown();
        // a frame a cart going in waited for: where its time went
        let total = began.elapsed();
        if total > Duration::from_millis(30)
            && matches!(
                frontend.app().phase(),
                omega_zero::app::Phase::Inserting { .. }
            )
        {
            let ms = |d: Duration| d.as_secs_f64() * 1e3;
            omega_zero::log!(
                "slow frame going in: {:.0} ms: update {:.0}, emulator {:.0}, draw {:.0}, swap {:.0}",
                ms(total),
                ms(advanced),
                ms(stepped),
                ms(swap - rendering),
                ms(swap_took)
            );
        }
        omega_zero::latency::swapped(swap_took.as_secs_f64() * 1000.0);
        let dropped = pacer.swapped(work, swap_took > Duration::from_millis(1));
        if omega_zero::latency::tracing() {
            frames += 1;
            missed += u32::from(dropped);
            early += u32::from(swap_took > pacer.period / 2);
            if frames == 600 {
                let (longest, sound, sound_max) = omega_zero::latency::window();
                omega_zero::log!(
                    "pace: 600 frames, {missed} missed a latch, {early} drawn early, {unanswered} ticks unanswered, work {:.1} ms, period {:.2} ms, core frame up to {longest:.1} ms, sound waits {sound:.0} ms (up to {sound_max:.1})",
                    pacer.work.as_secs_f64() * 1e3,
                    pacer.period.as_secs_f64() * 1e3
                );
                (frames, missed, early, unanswered) = (0, 0, 0, 0);
            }
        }
        if !drawn {
            drawn = true;
            trace_first_frame();
        }
        if let Some(left) = MIN_FRAME.checked_sub(began.elapsed()) {
            std::thread::sleep(left);
        }
    }
}
