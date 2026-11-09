//! Bluetooth, opened from Settings: switching it on and off, and the devices in reach. It is laid
//! out as Network is, a BLUETOOTH panel of one row over DEVICES, which takes the rest of the
//! modal, so its faces are Network's kind and drawn the same way.

use omega_input::Action;
use omega_ui::kit::bar::Hint;
use omega_ui::kit::layout::modal_rect;
use omega_ui::kit::modal::modal_face;
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::{panel_face, title_face};
use omega_ui::network::{network_panels, NetworkChrome, NetworkFaces};
use omega_ui::{Draw, SettingsRow, UndoFace};

use crate::app::{App, Phase};
use crate::bluetooth::BluetoothJobs;
use crate::bluetooth_screen::{Bluetooth, BluetoothScreen, BluetoothView, Outcome, Summary, Watch};
use crate::nav::Dir;
use crate::screens::bar::{FORGET, SCAN};

/// The modal's panels' titles, top first.
pub const BLUETOOTH_TITLE: &str = "BLUETOOTH";
pub const DEVICES_TITLE: &str = "DEVICES";
const TITLES: [&str; 2] = [BLUETOOTH_TITLE, DEVICES_TITLE];

/// The Bluetooth modal and its panels baked as one face in `pal`, their titles dim: BLUETOOTH as
/// tall as its one row, DEVICES a pixel over it, sharing its stripe, and down to the frame.
pub fn bluetooth_chrome(pal: &Palette) -> NetworkChrome {
    let [_, _, w, h] = modal_rect();
    let mut face = modal_face(w, h, pal);
    let mut wells = [[0; 4]; 2];
    let mut titles = [[0; 4]; 2];
    for (i, [x, y, pw, ph]) in network_panels(1).into_iter().enumerate() {
        let panel = panel_face(pw, ph, TITLES[i], false, pal);
        lay(&mut face, &panel.face, x, y);
        let [wx, wy, ww, wh] = panel.well;
        wells[i] = [x + wx, y + wy, ww, wh];
        let [tx, ty, tw, th] = panel.title;
        titles[i] = [x + tx, y + ty, tw, th];
    }
    NetworkChrome {
        face,
        wells,
        titles,
    }
}

/// Panel `panel`'s title (0 BLUETOOTH, 1 DEVICES) as the active panel shows it, for drawing
/// over the chrome's dim one.
pub fn bluetooth_title(panel: usize, pal: &Palette) -> UndoFace {
    title_face(TITLES[panel.min(1)], true, pal)
}

/// `top` laid over `under` with its top left at (`x`, `y`), wherever it is not clear.
fn lay(under: &mut UndoFace, top: &UndoFace, x: u32, y: u32) {
    for ty in 0..top.h.min(under.h.saturating_sub(y)) {
        for tx in 0..top.w.min(under.w.saturating_sub(x)) {
            let from = ((ty * top.w + tx) * 4) as usize;
            if top.rgba[from + 3] == 0 {
                continue;
            }
            let to = (((y + ty) * under.w + x + tx) * 4) as usize;
            under.rgba[to..to + 4].copy_from_slice(&top.rgba[from..from + 4]);
        }
    }
}

impl App {
    pub(crate) fn bluetooth_input(&mut self, action: Action) {
        let now = self.now();
        let Phase::Bluetooth(screen) = &mut self.phase else {
            return;
        };
        match screen.input(&mut self.bluetooth, action, now) {
            Outcome::Close => self.go(
                Phase::Settings {
                    row: SettingsRow::Bluetooth,
                },
                Dir::Back,
            ),
            Outcome::Headphones => self.go(
                Phase::Options(crate::options::OptionsScreen::new(
                    crate::options::OptionsPage::Headphones,
                )),
                Dir::Deeper,
            ),
            Outcome::Stay => {}
        }
        if self.state.bluetooth != self.bluetooth.enabled() {
            self.state.bluetooth = self.bluetooth.enabled();
            self.persist();
        }
    }

    /// How closely the radio is watched: the Bluetooth screen lists the devices and looks for
    /// more when X asks, and Settings names the one connected. Nothing else asks it anything: the
    /// top bar shows only whether Bluetooth is on.
    pub fn bluetooth_watch(&self) -> Watch {
        match self.phase {
            Phase::Bluetooth(_) => Watch::Devices,
            Phase::Settings { .. } => Watch::Status,
            _ => Watch::Idle,
        }
    }

    /// What the Bluetooth modal shows, while it is open.
    pub fn bluetooth_view(&mut self) -> Option<BluetoothView> {
        match &mut self.phase {
            Phase::Bluetooth(screen) => Some(screen.view(&self.bluetooth)),
            _ => None,
        }
    }

    /// Whether Y forgets the device under the cursor, for the bar to offer Y FORGET: the
    /// Bluetooth modal is open on a device the handheld has paired.
    pub fn bluetooth_forgets(&self) -> bool {
        match &self.phase {
            Phase::Bluetooth(screen) => screen.forgets(&self.bluetooth),
            _ => false,
        }
    }

    /// The keys the Bluetooth screen `screen` has beyond the bar's actions: X SCAN while
    /// Bluetooth is on, and beside it Y FORGET on a device the handheld has paired.
    pub(crate) fn bluetooth_hints(&self, screen: &BluetoothScreen) -> Vec<Hint<'static>> {
        let scan = screen.scans(&self.bluetooth).then_some(SCAN);
        let forget = screen.forgets(&self.bluetooth).then_some(FORGET);
        let phones = screen
            .headphones(&self.bluetooth)
            .then_some(crate::screens::bar::HEADPHONES);
        scan.into_iter().chain(forget).chain(phones).collect()
    }

    pub fn set_bluetooth_jobs(&mut self, jobs: Box<dyn BluetoothJobs>) {
        self.bluetooth = Bluetooth::new(jobs);
        self.bluetooth.start(self.state.bluetooth);
    }

    pub fn bluetooth(&self) -> &Bluetooth {
        &self.bluetooth
    }

    pub fn bluetooth_summary(&self) -> Summary {
        self.bluetooth.summary()
    }

    pub fn set_bluetooth_faces(&mut self, faces: NetworkFaces) {
        self.bluetooth_faces = Some(faces);
    }

    pub(crate) fn draw_bluetooth(&self, out: &mut Vec<Draw>) {
        if let Some(faces) = &self.bluetooth_faces {
            faces.draw(out);
        }
    }
}
