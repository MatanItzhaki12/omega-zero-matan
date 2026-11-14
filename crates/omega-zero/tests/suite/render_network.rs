#![cfg(any(target_os = "macos", target_os = "linux"))]

//! The Network modal and its rows' panels as the frontend draws them, held pixel for pixel to the
//! kit's faces put where flow.html puts them, over a plain backdrop, and Network before, during
//! and after X's scan to flow.html's own screenshots of it (omega-ui's tests/suite/network/ref).
//! Set SCRATCH_PNG_DIR to a folder to have each frame written there.

use crate::common;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use common::{clocked, tmp_root_with_carts};
use omega_gfx::{out_h, out_w, Compositor, HeadlessSurface};
use omega_input::{Btn, InputSource, Millis, RawEvent};
use omega_power::SimPlatform;
use omega_store::{read_app_state, write_app_state, BackdropStyle};
use omega_ui::kit::layout::modal_rect;
use omega_ui::kit::list_row::{list_row_face, ListRow};
use omega_ui::kit::palette::{palette, Palette};
use omega_ui::kit::row::{row_face, row_rect, row_width, Row, ROW_H};
use omega_ui::kit::text::text_face;
use omega_ui::network::{
    info_chrome, info_rect, network_chrome, network_panels, network_title, NOTE_IN, NOTE_TOP,
};
use omega_ui::{SettingsRow, UndoFace};
use omega_zero::frontend::Frontend;
use omega_zero::wifi::{Saved, Seen, Status, WifiJob, WifiJobs, WifiNews};
use tempfile::TempDir;

const NIGHT: u8 = 0;
const AMBER: u8 = 7;

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

#[derive(Default)]
struct RadioState {
    news: Vec<WifiNews>,
    /// What a scan hears, or None to leave every scan unanswered.
    heard: Option<Vec<Seen>>,
}

/// A radio that comes up when asked, says what the test tells it, and answers a scan with the
/// networks it hears.
#[derive(Clone, Default)]
struct Radio(Arc<Mutex<RadioState>>);

impl Radio {
    fn say(&self, news: WifiNews) {
        self.0.lock().unwrap().news.push(news);
    }

    fn hears(&self, heard: Option<Vec<Seen>>) {
        self.0.lock().unwrap().heard = heard;
    }
}

impl WifiJobs for Radio {
    fn present(&self) -> bool {
        true
    }

    fn ask(&mut self, job: WifiJob) {
        let mut radio = self.0.lock().unwrap();
        match job {
            WifiJob::Up => radio.news.push(WifiNews::Started(true)),
            WifiJob::Scan => {
                if let Some(seen) = radio.heard.clone() {
                    radio.news.push(WifiNews::Seen(seen));
                }
            }
            _ => {}
        }
    }

    fn news(&mut self) -> Vec<WifiNews> {
        std::mem::take(&mut self.0.lock().unwrap().news)
    }
}

/// How the handheld is connected when a test opens Network.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Online {
    /// Wi-Fi off.
    Off,
    /// On, flow.html's three networks in reach of a scan, none joined.
    Unjoined,
    /// On and joined to Home, as flow.html has it.
    Joined,
}

struct Rig {
    _d: TempDir,
    f: Frontend,
    input: Script,
    radio: Radio,
}

impl Rig {
    fn new(colour: u8, online: Online) -> Rig {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        clocked(d.path());
        let mut s = read_app_state(d.path());
        s.backdrop = Some(BackdropStyle::Solid);
        s.backdrop_colour = colour;
        s.wifi = online != Online::Off;
        write_app_state(d.path(), &s).expect("write omega-zero.state");
        // root's password as flow.html shows it: six digits, as omega-zero makes one up
        std::fs::write(d.path().join("baseos.conf"), "ssh_password=482913\n")
            .expect("write baseos.conf");
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        let radio = Radio::default();
        f.app_mut().set_wifi_jobs(Box::new(radio.clone()));
        if online != Online::Off {
            let seen = [
                ("Home", -48, true),
                ("Café Wi-Fi", -70, false),
                ("Upstairs 5G", -80, true),
            ];
            radio.hears(Some(
                seen.iter()
                    .map(|&(ssid, rssi, secured)| Seen {
                        ssid: ssid.as_bytes().to_vec(),
                        rssi,
                        secured,
                    })
                    .collect(),
            ));
        }
        if online == Online::Joined {
            radio.say(WifiNews::Saved(vec![Saved {
                id: 0,
                ssid: b"Home".to_vec(),
                current: true,
                refused: false,
            }]));
            radio.say(WifiNews::Status(Status {
                enabled: true,
                state: "COMPLETED".into(),
                ssid: Some(b"Home".to_vec()),
                ip: Some("192.168.1.20".into()),
                rssi: Some(-48),
                mac: Some("e4:5f:01:2a:9c:33".into()),
                hostname: "rg35xxsp".into(),
                ntp: true,
                ntp_synced: true,
            }));
        }
        Rig {
            _d: d,
            f,
            input: Script(VecDeque::new()),
            radio,
        }
    }

    fn tap(&mut self, btn: Btn) {
        self.input.0.push_back(vec![RawEvent::Down(btn)]);
        self.f.advance(&mut self.input);
        self.input.0.push_back(vec![RawEvent::Up(btn)]);
        self.f.advance(&mut self.input);
    }

    /// Settings, with the cursor on `row`.
    fn settings_at(&mut self, row: SettingsRow) {
        if self.f.app().settings_row().is_none() {
            self.tap(Btn::Menu);
        }
        for _ in 0..SettingsRow::ALL.len() {
            if self.f.app().settings_row() == Some(row) {
                return;
            }
            self.tap(Btn::Down);
        }
        panic!("Settings has no {row:?} row");
    }

    /// Network, as it opens: nothing scanned.
    fn open_network(&mut self) {
        self.settings_at(SettingsRow::Network);
        self.tap(Btn::A);
    }

    /// Network, and X's scan answered.
    fn open_network_scanned(&mut self) {
        self.open_network();
        self.tap(Btn::X);
    }

    /// The panel of NETWORK's row `row` (0 WI-FI, 1 NETWORK TIME, 2 SSH): down to it, then Y.
    fn open_info(&mut self, row: usize) {
        self.open_network();
        for _ in 0..row {
            self.tap(Btn::Down);
        }
        self.tap(Btn::Y);
    }

    /// The SSH panel with SSH switched off: on its row from ON to OFF, then Y.
    fn open_ssh_switched_off(&mut self) {
        self.open_network();
        self.tap(Btn::Down);
        self.tap(Btn::Down);
        self.tap(Btn::Right);
        self.tap(Btn::Y);
    }

    /// The frame once the screen has slid into place, written out as `name` when asked.
    fn frame(&mut self, c: &mut Compositor, name: &str) -> Vec<u8> {
        self.f.app_mut().finish_slide();
        self.f.compose(c);
        let px = c.read_frame();
        if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
            let path = format!("{dir}/{name}-{}x{}.png", out_w(), out_h());
            let file = std::fs::File::create(&path).expect("create png");
            let mut e = png::Encoder::new(std::io::BufWriter::new(file), out_w(), out_h());
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header()
                .expect("png header")
                .write_image_data(&px)
                .expect("png data");
        }
        px
    }
}

fn at(px: &[u8], x: u32, y: u32) -> [u8; 3] {
    let o = ((y * out_w() + x) * 4) as usize;
    [px[o], px[o + 1], px[o + 2]]
}

/// `top` laid over `under` with its top left at (`x`, `y`), wherever it is not clear.
fn lay(under: &mut UndoFace, top: &UndoFace, x: u32, y: u32) {
    for ty in 0..top.h {
        for tx in 0..top.w {
            let from = ((ty * top.w + tx) * 4) as usize;
            if top.rgba[from + 3] == 0 {
                continue;
            }
            let to = (((y + ty) * under.w + x + tx) * 4) as usize;
            under.rgba[to..to + 4].copy_from_slice(&top.rgba[from..from + 4]);
        }
    }
}

/// The frame shows `art` at 2x with its top left at art pixel `origin`, the backdrop wherever it
/// is clear: every screen pixel of it, none off.
fn shows(px: &[u8], art: &UndoFace, [ox, oy]: [u32; 2], name: &str) {
    let backdrop = at(px, 1, 1);
    let mut off = Vec::new();
    for y in 0..art.h {
        for x in 0..art.w {
            let i = ((y * art.w + x) * 4) as usize;
            let want = match art.rgba[i + 3] {
                0 => backdrop,
                _ => [art.rgba[i], art.rgba[i + 1], art.rgba[i + 2]],
            };
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (sx, sy) = (2 * (ox + x) + dx, 2 * (oy + y) + dy);
                let got = at(px, sx, sy);
                if got != want {
                    off.push((sx, sy, got, want));
                }
            }
        }
    }
    assert!(
        off.is_empty(),
        "{name}: {} screen pixels off, first (x, y, drawn, kit) {:?}",
        off.len(),
        &off[..off.len().min(6)]
    );
}

/// A row of NETWORK's: each has a value Left and Right change.
fn setting<'a>(label: &'a str, value: &'a str, lit: bool) -> Row<'a> {
    Row {
        label,
        value: Some(value),
        lit,
        carets: true,
        opens: false,
        dim: false,
        caret_room: false,
    }
}

/// The Network modal as flow.html draws it, NETWORK's `settings` rows and `networks` in the
/// list, the cursor on the first setting.
fn network_art(settings: &[Row], networks: &[ListRow], notes: &[&str], pal: &Palette) -> UndoFace {
    let chrome = network_chrome(settings.len(), pal);
    let mut art = chrome.face;
    let [tx, ty, _, _] = chrome.titles[0];
    lay(&mut art, &network_title(0, pal), tx, ty);
    for (i, row) in settings.iter().enumerate() {
        let [x, y, w, _] = row_rect(chrome.wells[0], i as u32);
        lay(&mut art, &row_face(w, row, pal), x, y);
    }
    for (i, row) in networks.iter().enumerate() {
        let [x, y, w, _] = row_rect(chrome.wells[1], i as u32);
        lay(&mut art, &list_row_face(w, row, pal), x, y);
    }
    let [x, y, _, _] = chrome.wells[1];
    let y = y + networks.len() as u32 * ROW_H + NOTE_TOP;
    for (i, note) in notes.iter().enumerate() {
        lay(
            &mut art,
            &text_face(note, pal.dim),
            x + NOTE_IN,
            y + 10 * i as u32,
        );
    }
    art
}

/// flow.html's three networks, Home joined or not.
fn flows_networks(joined: bool) -> [ListRow<'static>; 3] {
    let network = |label, value, signal, lock| ListRow {
        label,
        value,
        signal: Some(signal),
        lock,
        device: None,
        lit: false,
        dim: false,
    };
    [
        network("Home", joined.then_some("CONNECTED"), 3, true),
        network("Café Wi-Fi", None, 2, false),
        network("Upstairs 5G", None, 1, true),
    ]
}

/// Home as the list shows it before a scan: joined, with no signal or padlock, which only a
/// scan hears.
fn home_unscanned() -> ListRow<'static> {
    ListRow {
        signal: None,
        lock: false,
        ..flows_networks(true)[0]
    }
}

/// Network as it opens, joined to Home: the list holds Home alone, CONNECTED, and the note says
/// to press X, as flow.html's #scan=none.
#[test]
fn network_before_a_scan_holds_the_network_joined_and_says_to_press_x() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    for (colour, name) in [(NIGHT, "night"), (AMBER, "amber")] {
        let mut r = Rig::new(colour, Online::Joined);
        r.f.upload_faces(&mut c);
        r.open_network();
        let px = r.frame(&mut c, &format!("network-unscanned-{name}"));
        let settings = [
            setting("WI-FI", "ON", true),
            setting("NETWORK TIME", "ON", false),
            setting("SSH", "ON", false),
        ];
        let art = network_art(
            &settings,
            &[home_unscanned()],
            &["IP 192.168.1.20", "Press X to look for networks."],
            palette(colour),
        );
        let [x, y, _, _] = modal_rect();
        shows(&px, &art, [x, y], &format!("network unscanned, {name}"));
    }
}

/// X's scan under way: Home still alone, and the note says it is looking, as flow.html's
/// #scan=running.
#[test]
fn network_while_it_scans_says_it_is_looking() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let mut r = Rig::new(NIGHT, Online::Joined);
    r.radio.hears(None);
    r.f.upload_faces(&mut c);
    r.open_network();
    r.tap(Btn::X);
    let px = r.frame(&mut c, "network-scanning-night");
    let settings = [
        setting("WI-FI", "ON", true),
        setting("NETWORK TIME", "ON", false),
        setting("SSH", "ON", false),
    ];
    let art = network_art(
        &settings,
        &[home_unscanned()],
        &["Looking for networks…"],
        palette(NIGHT),
    );
    let [x, y, _, _] = modal_rect();
    shows(&px, &art, [x, y], "network scanning");
}

#[test]
fn the_modal_sits_where_flow_puts_it() {
    if (out_w(), out_h()) == (640, 480) {
        assert_eq!(modal_rect(), [16, 14, 288, 206]);
        assert_eq!(network_panels(3), [[4, 2, 280, 62], [4, 63, 280, 139]]);
        assert_eq!(network_panels(1), [[4, 2, 280, 36], [4, 37, 280, 165]]);
        assert_eq!(info_rect(5), [60, 60, 200, 104], "flow's SSH panel");
        assert_eq!(info_rect(0), [60, 91, 200, 42], "flow's SSH panel offline");
    }
    let [_, _, _, h] = modal_rect();
    for settings in [1, 3] {
        let [first, list] = network_panels(settings);
        assert_eq!(first[3], 23 + 13 * settings as u32, "NETWORK hugs its rows");
        assert_eq!(
            list[1],
            first[1] + first[3] - 1,
            "the panels share a stripe"
        );
        assert_eq!(list[1] + list[3], h - 4, "the list runs down to the frame");
    }
}

#[test]
fn network_joined_draws_flows_modal() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    for (colour, name) in [(NIGHT, "night"), (AMBER, "amber")] {
        let mut r = Rig::new(colour, Online::Joined);
        r.f.upload_faces(&mut c);
        r.open_network_scanned();
        let px = r.frame(&mut c, &format!("network-joined-{name}"));
        let pal = palette(colour);
        let settings = [
            setting("WI-FI", "ON", true),
            setting("NETWORK TIME", "ON", false),
            setting("SSH", "ON", false),
        ];
        let art = network_art(&settings, &flows_networks(true), &["IP 192.168.1.20"], pal);
        let [x, y, _, _] = modal_rect();
        shows(&px, &art, [x, y], &format!("network joined, {name}"));
    }
}

#[test]
fn the_ssh_row_switches_off_and_on_with_its_carets() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let mut r = Rig::new(NIGHT, Online::Joined);
    r.f.upload_faces(&mut c);
    r.open_network_scanned();
    r.tap(Btn::Down);
    r.tap(Btn::Down);
    let pal = palette(NIGHT);
    let [x, y, _, _] = modal_rect();
    for value in ["OFF", "ON"] {
        r.tap(Btn::Right);
        let px = r.frame(&mut c, &format!("network-ssh-{}", value.to_lowercase()));
        let settings = [
            setting("WI-FI", "ON", false),
            setting("NETWORK TIME", "ON", false),
            setting("SSH", value, true),
        ];
        let art = network_art(&settings, &flows_networks(true), &["IP 192.168.1.20"], pal);
        shows(&px, &art, [x, y], &format!("SSH on {value}"));
    }
}

#[test]
fn network_with_wifi_off_holds_only_wifi_and_says_so() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    for (colour, name) in [(NIGHT, "night"), (AMBER, "amber")] {
        let mut r = Rig::new(colour, Online::Off);
        r.f.upload_faces(&mut c);
        r.open_network();
        let px = r.frame(&mut c, &format!("network-off-{name}"));
        let pal = palette(colour);
        let art = network_art(
            &[setting("WI-FI", "OFF", true)],
            &[],
            &["Wi-Fi is off."],
            pal,
        );
        let [x, y, _, _] = modal_rect();
        shows(&px, &art, [x, y], &format!("network off, {name}"));
    }
}

#[test]
fn the_lit_network_is_banded_and_its_panel_active() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let mut r = Rig::new(AMBER, Online::Unjoined);
    r.f.upload_faces(&mut c);
    r.open_network_scanned();
    for _ in 0..4 {
        r.tap(Btn::Down);
    }
    let px = r.frame(&mut c, "network-cafe-lit-amber");
    let pal = palette(AMBER);

    let chrome = network_chrome(3, pal);
    let [mx, my, _, _] = modal_rect();
    // the band round the second network: its frame line along the top, the tab colour inside
    let [x, y, w, _] = row_rect(chrome.wells[1], 1);
    let (left, top) = (mx + x, my + y);
    for bx in [left + 1, left + w / 2, left + w - 2] {
        assert_eq!(
            at(&px, 2 * bx, 2 * top),
            pal.frame,
            "the band's top at x {bx}"
        );
        assert_eq!(
            at(&px, 2 * bx, 2 * (top + 1)),
            pal.tab,
            "inside the band at x {bx}"
        );
    }
    assert_ne!(
        at(&px, 2 * left, 2 * top),
        pal.frame,
        "the band's corner is not cut"
    );
    // the rows either side are not lit
    for row in [0, 2] {
        let [x, y, w, _] = row_rect(chrome.wells[1], row);
        assert_ne!(
            at(&px, 2 * (mx + x + w / 2), 2 * (my + y)),
            pal.frame,
            "row {row}"
        );
    }
    // AVAILABLE NETWORKS' title is lit and NETWORK's dim: each title's first column of ink
    let inked = |[tx, ty, _, th]: [u32; 4], colour: [u8; 3]| {
        (0..th).any(|dy| (0..6).any(|dx| at(&px, 2 * (mx + tx + dx), 2 * (my + ty + dy)) == colour))
    };
    assert!(
        inked(chrome.titles[1], pal.cream),
        "AVAILABLE NETWORKS is not lit"
    );
    assert!(inked(chrome.titles[0], pal.dim), "NETWORK is not dim");
    assert!(!inked(chrome.titles[0], pal.cream), "NETWORK is still lit");

    let settings = [
        setting("WI-FI", "ON", false),
        setting("NETWORK TIME", "OFF", false),
        setting("SSH", "ON", false),
    ];
    let mut networks = flows_networks(false);
    networks[1].lit = true;
    let mut art = network_art(
        &settings,
        &networks,
        &["Not connected. Pick a network to join."],
        pal,
    );
    // network_art lights NETWORK's title; here it is the list's
    let dim = omega_ui::kit::panel::title_face("NETWORK", false, pal);
    let [tx, ty, _, _] = chrome.titles[0];
    lay(&mut art, &dim, tx, ty);
    let [tx, ty, _, _] = chrome.titles[1];
    lay(&mut art, &network_title(1, pal), tx, ty);
    shows(&px, &art, [mx, my], "Café Wi-Fi lit");
}

/// A row's panel titled `title` as flow.html draws it: `lines` down its well, `note` under them.
fn info_art(title: &str, lines: &[(&str, &str)], note: &str, pal: &Palette) -> UndoFace {
    let panel = info_chrome(title, lines.len(), pal);
    let mut art = panel.face;
    for (i, (label, value)) in lines.iter().enumerate() {
        let row = Row {
            label,
            value: Some(value),
            ..Row::default()
        };
        let [x, y, _, _] = row_rect(panel.well, i as u32);
        lay(
            &mut art,
            &row_face(row_width(panel.well[2]), &row, pal),
            x,
            y,
        );
    }
    let [x, y, _, _] = panel.well;
    let y = y + lines.len() as u32 * ROW_H + NOTE_TOP;
    lay(&mut art, &text_face(note, pal.dim), x + NOTE_IN, y);
    art
}

#[test]
fn ssh_draws_flows_lone_panel_joined_offline_and_switched_off() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let login = [
        ("IP", "192.168.1.20"),
        ("HOST", "rg35xxsp.local"),
        ("USER", "root"),
        ("PASSWORD", "482913"),
        ("PORT", "22"),
    ];
    for (colour, name) in [(NIGHT, "night"), (AMBER, "amber")] {
        let pal = palette(colour);
        for (shot, online, on, lines, note) in [
            (
                "ssh",
                Online::Joined,
                true,
                &login[..],
                "SFTP uses the same login",
            ),
            (
                "ssh-off",
                Online::Unjoined,
                true,
                &[][..],
                "Join a Wi-Fi network first.",
            ),
            (
                "ssh-switched-off",
                Online::Joined,
                false,
                &[][..],
                "Turn SSH on first.",
            ),
        ] {
            let mut r = Rig::new(colour, online);
            r.f.upload_faces(&mut c);
            if on {
                r.open_info(2);
            } else {
                r.open_ssh_switched_off();
            }
            assert_eq!(r.f.app().ssh_on(), on, "{shot}");
            let px = r.frame(&mut c, &format!("{shot}-{name}"));
            let art = info_art("SSH", lines, note, pal);
            let [px0, py0, w, h] = info_rect(lines.len());
            assert_eq!((art.w, art.h), (w, h));
            shows(&px, &art, [px0, py0], &format!("{shot}, {name}"));
        }
    }
}

#[test]
fn wifi_and_network_time_draw_flows_lone_panel() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let wifi = [
        ("NETWORK", "Home"),
        ("SIGNAL", "-48 dBm"),
        ("IP", "192.168.1.20"),
        ("HOST", "rg35xxsp.local"),
        ("MAC", "e4:5f:01:2a:9c:33"),
    ];
    for (colour, name) in [(NIGHT, "night"), (AMBER, "amber")] {
        let pal = palette(colour);
        for (shot, online, row, title) in [
            ("wifi", Online::Joined, 0, "WI-FI"),
            ("wifi-unjoined", Online::Unjoined, 0, "WI-FI"),
            ("time", Online::Joined, 1, "NETWORK TIME"),
            ("time-unjoined", Online::Unjoined, 1, "NETWORK TIME"),
        ] {
            let mut r = Rig::new(colour, online);
            r.f.upload_faces(&mut c);
            r.open_info(row);
            let px = r.frame(&mut c, &format!("{shot}-{name}"));
            let view = r.f.app().info_view().expect("no panel open");
            assert_eq!(view.title, title, "{shot}");
            if shot == "wifi" {
                let want: Vec<_> = wifi.iter().map(|&(l, v)| (l, v.to_string())).collect();
                assert_eq!(view.lines, want);
            }
            let lines: Vec<(&str, &str)> =
                view.lines.iter().map(|(l, v)| (*l, v.as_str())).collect();
            let art = info_art(title, &lines, view.note, pal);
            let [px0, py0, w, h] = info_rect(lines.len());
            assert_eq!((art.w, art.h), (w, h));
            shows(&px, &art, [px0, py0], &format!("{shot}, {name}"));
        }
    }
}

#[test]
fn a_new_backdrop_colour_re_tints_the_modal() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let mut r = Rig::new(NIGHT, Online::Joined);
    r.f.upload_faces(&mut c);
    r.open_network_scanned();
    let [mx, my, _, _] = modal_rect();
    // the frame colour, inside the modal's corner where no panel covers it
    let frame_at = |px: &[u8]| at(px, 2 * (mx + 3), 2 * (my + 3));
    let px = r.frame(&mut c, "network-tint-before");
    assert_eq!(frame_at(&px), palette(NIGHT).frame);

    // Settings > Background > Colour, one step back from Night round to Amber
    r.tap(Btn::B);
    common::to_shelf_row(r.f.app_mut(), omega_zero::options::OptionRow::Theme);
    r.tap(Btn::A);
    r.tap(Btn::Down);
    r.tap(Btn::Left);
    assert_eq!(
        r.f.app().palette_colour(),
        AMBER,
        "the colour did not change"
    );
    // back to SHELF, and to Settings
    r.tap(Btn::B);
    r.f.app_mut().finish_slide();
    r.tap(Btn::B);
    r.open_network_scanned();
    let px = r.frame(&mut c, "network-tint-after");
    assert_eq!(
        frame_at(&px),
        palette(AMBER).frame,
        "the modal kept Night's colours"
    );
    let settings = [
        setting("WI-FI", "ON", true),
        setting("NETWORK TIME", "ON", false),
        setting("SSH", "ON", false),
    ];
    let art = network_art(
        &settings,
        &flows_networks(true),
        &["IP 192.168.1.20"],
        palette(AMBER),
    );
    shows(&px, &art, [mx, my], "re-tinted to Amber");
}

/// flow.html's screenshot of Network in its #scan= state `scan` on this canvas, in Night
/// (omega-ui's tests/suite/network/ref): the modal alone, one pixel to an art pixel, clear
/// wherever `face` is, as its cut corners show the backdrop.
fn flow_shot(scan: &str, face: &UndoFace) -> UndoFace {
    let path = format!(
        "{}/../omega-ui/tests/suite/network/ref/flow-network-{scan}-{}x{}.png",
        env!("CARGO_MANIFEST_DIR"),
        out_w(),
        out_h()
    );
    let file = std::fs::File::open(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().expect("a PNG");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("its pixels");
    let channels = info.color_type.samples();
    assert_eq!((info.width, info.height), (face.w, face.h), "{path}");
    let rgba = buf[..info.buffer_size()]
        .chunks(channels)
        .zip(face.rgba.chunks(4))
        .flat_map(|(px, kit)| [px[0], px[1], px[2], kit[3]])
        .collect();
    UndoFace {
        rgba,
        w: info.width,
        h: info.height,
    }
}

/// Network as it opens, while X's scan runs and once it is done is flow.html's screenshot of it
/// in each #scan= state, to the screen pixel.
#[test]
fn the_modal_before_during_and_after_a_scan_is_flows_screenshot_of_it() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let settings = [
        setting("WI-FI", "ON", true),
        setting("NETWORK TIME", "ON", false),
        setting("SSH", "ON", false),
    ];
    let unscanned = [home_unscanned()];
    for (scan, networks, notes) in [
        (
            "none",
            &unscanned[..],
            &["IP 192.168.1.20", "Press X to look for networks."][..],
        ),
        ("running", &unscanned[..], &["Looking for networks…"][..]),
        ("done", &flows_networks(true)[..], &["IP 192.168.1.20"][..]),
    ] {
        let mut r = Rig::new(NIGHT, Online::Joined);
        if scan == "running" {
            r.radio.hears(None);
        }
        r.f.upload_faces(&mut c);
        r.open_network();
        if scan != "none" {
            r.tap(Btn::X);
        }
        let name = format!("flow-network-{scan}");
        let px = r.frame(&mut c, &name);
        let face = network_art(&settings, networks, notes, palette(NIGHT));
        let [x, y, _, _] = modal_rect();
        // flow.html's Network says no address: where this one does, its own panel is held
        let held = match scan {
            "running" => flow_shot(scan, &face),
            _ => face,
        };
        shows(&px, &held, [x, y], &name);
    }
}
