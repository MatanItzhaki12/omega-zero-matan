use std::path::Path;
use std::time::{Duration, Instant};

use omega_gfx::{out_h, out_w, Compositor, HostSurface, Surface};
use omega_power::SimPlatform;
use omega_ui::UndoFace;
use omega_zero::bootlogo;
use omega_zero::frontend::Frontend;
use omega_zero::input::HostInput;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

const FRAME: Duration = Duration::from_micros(16_667);

struct HostApp {
    gfx: Option<(HostSurface, Compositor)>,
    frontend: Frontend,
    input: HostInput,
}

impl HostApp {
    fn new() -> Self {
        HostApp {
            gfx: None,
            frontend: Frontend::boot(Box::new(SimPlatform::new())),
            input: HostInput::new(),
        }
    }
}

impl ApplicationHandler for HostApp {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let built = HostSurface::new(events).and_then(|s| Compositor::new(&s).map(|c| (s, c)));
        let (surface, mut compositor) = match built {
            Ok(gfx) => gfx,
            Err(e) => {
                omega_zero::log!("{e}");
                events.exit();
                return;
            }
        };
        if let Err(e) = compositor.set_rotation(omega_gfx::host_rotation()) {
            omega_zero::log!("{e}");
        }
        self.frontend.upload_faces(&mut compositor);
        if let Some(logo) = boot_logo(self.frontend.app().root()) {
            self.frontend.show_boot_logo(&mut compositor, &logo);
        }
        self.frontend
            .start_opening(&omega_zero::opening::Facts::read());
        self.gfx = Some((surface, compositor));
    }

    fn window_event(&mut self, events: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        self.input.on_window_event(&event);
        let Some((surface, compositor)) = self.gfx.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => events.exit(),
            WindowEvent::Resized(size) => surface.resize(size),
            WindowEvent::RedrawRequested => {
                let began = Instant::now();
                self.frontend.render(compositor, surface.window_size());
                let swap = std::time::Instant::now();
                let swapped = surface.swap();
                omega_zero::latency::swapped(swap.elapsed().as_secs_f64() * 1000.0);
                if let Err(e) = swapped {
                    omega_zero::log!("{e}");
                    events.exit();
                    return;
                }
                surface.request_redraw();
                self.frontend.advance(&mut self.input);
                if self.frontend.restarting() {
                    self.frontend.restart();
                }
                if self.frontend.powering_off() {
                    self.frontend.poweroff();
                    events.exit();
                    return;
                }
                if let Some(left) = FRAME.checked_sub(began.elapsed()) {
                    std::thread::sleep(left);
                }
            }
            _ => {}
        }
    }
}

/// The boot logo for the canvas: the card's own, or failing that the one omega-zero ships, from
/// the repo this was built in, so a card made with `task sdcard` shows it too.
fn boot_logo(root: Option<&Path>) -> Option<UndoFace> {
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../card/System");
    root.map(|r| r.join("System"))
        .into_iter()
        .chain([shipped])
        .find_map(|system| bootlogo::shown(&system, (out_w(), out_h())))
}

pub fn run() {
    let events = match EventLoop::new() {
        Ok(e) => e,
        Err(e) => {
            omega_zero::log!("{e}");
            return;
        }
    };
    events.set_control_flow(ControlFlow::Poll);
    if let Err(e) = events.run_app(&mut HostApp::new()) {
        omega_zero::log!("{e}");
    }
}
