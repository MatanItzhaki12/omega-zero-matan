#![cfg(any(target_os = "macos", target_os = "linux"))]
//! The lone panels as the frontend draws them: each is its face (held to flow.html in lone.rs)
//! at two screen pixels to an art pixel, where flow.html centres it on this canvas, over the
//! backdrop, and the screen holding POWER slides in as it powers off; and a new colour on the
//! Background screen re-tints its panel in the next frame.
//!
//! Set SCRATCH_PNG_DIR to a folder to have each frame written there as a PNG.

use crate::common;
use crate::lone::{amber, colours, Present, FLOW_DATE};

use std::collections::VecDeque;

use common::tmp_root_with_carts;
use omega_gfx::{out_h, out_w, Compositor, HeadlessSurface};
use omega_input::{Action, Btn, InputSource, Millis, RawEvent};
use omega_power::SimPlatform;
use omega_store::{read_app_state, write_app_state, BackdropStyle};
use omega_ui::kit::layout::centred;
use omega_ui::kit::palette::palette;
use omega_ui::{PowerChoice, SettingsRow, UndoFace, SPRITE_PX};
use omega_zero::app::Phase;
use omega_zero::frontend::Frontend;
use omega_zero::screens::background::background_face;
use omega_zero::screens::clock::clock_face;
use omega_zero::screens::controls::{controls_face, ControlsView};
use omega_zero::screens::labels::labels_face;
use omega_zero::screens::power::going_face;
use tempfile::TempDir;

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Screen {
    Controls,
    Background,
    DateTime,
    Labels,
    /// POWER held: it powers off at once, as on Slot, the screen saying so slid in.
    PoweringOff,
}

const SCREENS: [Screen; 5] = [
    Screen::Controls,
    Screen::Background,
    Screen::DateTime,
    Screen::Labels,
    Screen::PoweringOff,
];

struct Rig {
    _d: TempDir,
    f: Frontend,
    input: Script,
}

impl Rig {
    /// A card whose clock is set and whose background is a `colour` grid, booted at flow.html's
    /// date with a ScreenScraper there, with `screen` open; or, `clock_set` false, at first boot.
    fn boot(c: &mut Compositor, colour: u8, clock_set: bool) -> Rig {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        let mut s = read_app_state(d.path());
        s.clock_set = clock_set;
        s.backdrop = Some(BackdropStyle::Grid);
        s.backdrop_colour = colour;
        write_app_state(d.path(), &s).expect("write omega-zero.state");
        let platform = SimPlatform::at(d.path().to_path_buf()).stopped_at(FLOW_DATE);
        let mut f = Frontend::boot(Box::new(platform));
        f.app_mut().set_label_jobs(Box::new(Present));
        f.upload_faces(c);
        Rig {
            _d: d,
            f,
            input: Script(VecDeque::new()),
        }
    }

    fn on(c: &mut Compositor, screen: Screen, colour: u8) -> Rig {
        let mut r = Rig::boot(c, colour, true);
        let row = match screen {
            Screen::Controls => {
                r.tap(Btn::Select);
                assert!(matches!(r.f.app().phase(), Phase::Controls));
                return r;
            }
            Screen::Background | Screen::Labels => {
                r.tap(Btn::Menu);
                r.f.app_mut().finish_slide();
                let row = match screen {
                    Screen::Background => omega_zero::options::OptionRow::Theme,
                    _ => omega_zero::options::OptionRow::Labels,
                };
                common::to_shelf_row(r.f.app_mut(), row);
                r.tap(Btn::A);
                return r;
            }
            Screen::DateTime => SettingsRow::DateTime,
            // POWER held on the shelf: it powers off at once
            Screen::PoweringOff => {
                r.f.app_mut().apply(Action::PowerHold);
                return r;
            }
        };
        r.tap(Btn::Menu);
        for _ in 0..row.index() {
            r.tap(Btn::Down);
        }
        r.tap(Btn::A);
        r
    }

    fn tap(&mut self, btn: Btn) {
        self.input.0.push_back(vec![RawEvent::Down(btn)]);
        self.f.advance(&mut self.input);
        self.input.0.push_back(vec![RawEvent::Up(btn)]);
        self.f.advance(&mut self.input);
    }

    /// The frame once the screen has slid into place.
    fn frame(&mut self, c: &mut Compositor, name: &str) -> Vec<u8> {
        self.f.app_mut().finish_slide();
        self.f.compose(c);
        let px = c.read_frame();
        if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
            let path = format!("{dir}/{name}-{}x{}.png", out_w(), out_h());
            let face = UndoFace {
                rgba: px.clone(),
                w: out_w(),
                h: out_h(),
            };
            crate::lone::write(&path, &face);
        }
        px
    }

    /// The panel `screen` shows now, as its face.
    fn face(&self, screen: Screen, colour: u8) -> UndoFace {
        let (app, pal) = (self.f.app(), palette(colour));
        match screen {
            Screen::Controls => controls_face(&ControlsView::new(omega_store::Platform::Gba), pal),
            Screen::Background => background_face(&app.background_view().expect("open"), pal),
            Screen::DateTime => clock_face(app.picker().expect("open"), pal),
            Screen::Labels => labels_face(&app.labels_view().expect("open"), pal),
            Screen::PoweringOff => going_face(PowerChoice::PowerOff, pal),
        }
    }
}

/// A compositor, and the surface it draws on, which has to outlive it.
fn compositor() -> Option<(HeadlessSurface, Compositor)> {
    let surface = HeadlessSurface::new().ok()?;
    let c = Compositor::new(&surface).ok()?;
    Some((surface, c))
}

/// `face` is in the frame `px` where flow.html centres it, at two screen pixels to each of its
/// art pixels; its clear corners show whatever is under it.
fn drawn_as(px: &[u8], face: &UndoFace, name: &str) {
    let [x0, y0, w, h] = centred(face.w, face.h);
    assert!(
        (x0 + w) * SPRITE_PX <= out_w() && (y0 + h) * SPRITE_PX <= out_h(),
        "{name}: the panel runs off the screen"
    );
    let mut off = Vec::new();
    for y in 0..face.h {
        for x in 0..face.w {
            let i = ((y * face.w + x) * 4) as usize;
            if face.rgba[i + 3] == 0 {
                continue;
            }
            let want = [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (sx, sy) = (SPRITE_PX * (x0 + x) + dx, SPRITE_PX * (y0 + y) + dy);
                let o = ((sy * out_w() + sx) * 4) as usize;
                let got = [px[o], px[o + 1], px[o + 2]];
                if got != want {
                    off.push((sx, sy, got, want));
                }
            }
        }
    }
    assert!(
        off.is_empty(),
        "{name}: {} screen pixels are not the panel's, first (x, y, drawn, face) {:?}",
        off.len(),
        &off[..off.len().min(6)]
    );
}

#[test]
fn every_lone_panel_is_drawn_where_flow_centres_it_in_night_and_amber() {
    let Some((_surface, mut c)) = compositor() else {
        return;
    };
    for screen in SCREENS {
        for (colour, name) in colours() {
            let mut r = Rig::on(&mut c, screen, colour);
            let name = format!("{screen:?}-{name}").to_lowercase();
            let px = r.frame(&mut c, &name);
            drawn_as(&px, &r.face(screen, colour), &name);
        }
    }
}

#[test]
fn a_new_colour_re_tints_the_background_panel_in_the_next_frame() {
    let Some((_surface, mut c)) = compositor() else {
        return;
    };
    let mut r = Rig::on(&mut c, Screen::Background, 0);
    let night = r.frame(&mut c, "background-night");
    r.tap(Btn::Down);
    r.tap(Btn::Down);
    r.tap(Btn::Left);
    assert_eq!(
        r.f.app().palette_colour(),
        amber(),
        "Left from Night is Amber"
    );
    let px = r.frame(&mut c, "background-retinted");
    let face = r.face(Screen::Background, amber());
    drawn_as(&px, &face, "re-tinted");
    assert_ne!(night, px, "the frame did not change");
}

#[test]
fn the_clock_underline_follows_the_field_on_the_screen() {
    let Some((_surface, mut c)) = compositor() else {
        return;
    };
    let mut r = Rig::on(&mut c, Screen::DateTime, 0);
    for i in 0..6 {
        let px = r.frame(&mut c, &format!("datetime-field-{i}"));
        drawn_as(&px, &r.face(Screen::DateTime, 0), &format!("field {i}"));
        r.tap(Btn::Right);
    }
    r.tap(Btn::Up);
    let px = r.frame(&mut c, "datetime-changed");
    drawn_as(&px, &r.face(Screen::DateTime, 0), "a field changed");
}

/// At first boot the clock is the same panel, with no slide to it.
#[test]
fn the_first_boot_clock_is_the_date_and_time_panel() {
    let Some((_surface, mut c)) = compositor() else {
        return;
    };
    let mut r = Rig::boot(&mut c, amber(), false);
    assert!(matches!(
        r.f.app().phase(),
        Phase::SetClock {
            from_menu: false,
            ..
        }
    ));
    let px = r.frame(&mut c, "datetime-first-boot");
    drawn_as(&px, &r.face(Screen::DateTime, amber()), "first boot");
}
