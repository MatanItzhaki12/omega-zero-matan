use crate::common;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use omega_input::{Action, Btn, Millis};
use omega_store::{read_app_state, write_app_state, AppState};
use omega_ui::SettingsRow;
use omega_zero::app::{App, Phase};
use omega_zero::screens::info::{InfoScreen, InfoView};
use omega_zero::screens::ssh::SshScreen;
use omega_zero::ssh::{SshServer, Trouble, SETTLE_MS};
use omega_zero::wifi::{Status, WifiJob, WifiJobs, WifiNews};
use omega_zero::wifi_screen::{Sel, SettingLine, Watch};
use tempfile::TempDir;

/// A radio that is on, with whatever network the test says joined.
#[derive(Clone, Default)]
struct Radio(Arc<Mutex<Vec<WifiNews>>>);

impl WifiJobs for Radio {
    fn present(&self) -> bool {
        true
    }

    fn ask(&mut self, job: WifiJob) {
        if job == WifiJob::Up {
            self.0.lock().unwrap().push(WifiNews::Started(true));
        }
    }

    fn news(&mut self) -> Vec<WifiNews> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

/// What the app asked of the SSH server.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Asked {
    Start,
    Stop,
    Password(String),
}

#[derive(Default)]
struct ServerState {
    asked: Vec<Asked>,
    /// The baseos.conf BaseOS would give root its password from; none, as on the desktop,
    /// unless the test gives one.
    conf: Option<PathBuf>,
    /// The running system will not take a password.
    refuses: bool,
}

/// An SSH server that only says what it was asked to do.
#[derive(Clone, Default)]
struct Server(Arc<Mutex<ServerState>>);

impl Server {
    fn with_conf(conf: PathBuf) -> Server {
        let server = Server::default();
        server.0.lock().unwrap().conf = Some(conf);
        server
    }

    fn asked(&self) -> Vec<Asked> {
        self.0.lock().unwrap().asked.clone()
    }

    fn forget_asked(&self) {
        self.0.lock().unwrap().asked.clear();
    }
}

impl SshServer for Server {
    fn present(&self) -> bool {
        true
    }

    fn start(&mut self) {
        self.0.lock().unwrap().asked.push(Asked::Start);
    }

    fn stop(&mut self) {
        self.0.lock().unwrap().asked.push(Asked::Stop);
    }

    fn baseos_conf(&self, _root: &Path) -> Option<PathBuf> {
        self.0.lock().unwrap().conf.clone()
    }

    fn set_root_password(&mut self, password: &str) -> io::Result<()> {
        let mut state = self.0.lock().unwrap();
        state.asked.push(Asked::Password(password.to_string()));
        if state.refuses {
            return Err(io::Error::other("refused"));
        }
        Ok(())
    }
}

struct Rig {
    d: TempDir,
    a: App,
    radio: Radio,
    server: Server,
    now: Millis,
}

impl Rig {
    fn new() -> Rig {
        Rig::with(AppState::default(), |_| Server::default())
    }

    /// The app booted with `state` on the card, and the server `server` makes for the card.
    fn with(state: AppState, server: impl FnOnce(&Path) -> Server) -> Rig {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        write_app_state(
            d.path(),
            &AppState {
                clock_set: true,
                wifi: true,
                ..state
            },
        )
        .unwrap();
        let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
        let radio = Radio::default();
        a.set_wifi_jobs(Box::new(radio.clone()));
        let server = server(d.path());
        a.set_ssh_server(Box::new(server.clone()));
        let mut r = Rig {
            d,
            a,
            radio,
            server,
            now: 1_000,
        };
        r.tick(16);
        r
    }

    fn press(&mut self, btn: Btn) {
        self.a.apply(Action::GbaDown(btn));
        self.a.apply(Action::GbaUp(btn));
    }

    fn tick(&mut self, ms: Millis) {
        self.now += ms;
        self.a.tick_ms(self.now);
    }

    fn join(&mut self, hostname: &str) {
        self.radio
            .0
            .lock()
            .unwrap()
            .push(WifiNews::Status(status(hostname)));
        self.tick(16);
        assert!(self.a.wifi().status().joined(), "Wi-Fi did not join");
    }

    fn open_network(&mut self) {
        self.a.apply(Action::QuickMenu);
        assert!(self.a.settings_row().is_some(), "START opened no Settings");
        while self.a.settings_row() != Some(SettingsRow::Network) {
            self.press(Btn::Down);
            assert_ne!(
                self.a.settings_row(),
                Some(SettingsRow::Display),
                "the bar went round without reaching Network"
            );
        }
        self.press(Btn::A);
        assert!(matches!(self.a.phase(), Phase::Network(_)), "no Network");
    }

    /// Network, with the cursor on SSH's row.
    fn open_ssh_row(&mut self) {
        self.open_network();
        for _ in 0..4 {
            if self.on_ssh_row() {
                return;
            }
            self.press(Btn::Down);
        }
        panic!("Network has no SSH row: {:?}", self.a.phase());
    }

    fn on_ssh_row(&self) -> bool {
        matches!(self.a.phase(), Phase::Network(s) if s.sel == Sel::Ssh)
    }

    /// The SSH row as NETWORK shows it.
    fn ssh_row(&mut self) -> SettingLine {
        let view = self.a.network_view().expect("Network is not showing");
        view.settings
            .into_iter()
            .find(|line| line.label == "SSH")
            .expect("NETWORK has no SSH row")
    }

    fn ssh_value(&mut self) -> &'static str {
        self.ssh_row().value.expect("the SSH row shows no value")
    }

    /// The SSH panel, by way of Y on the row, which leaves SSH as it was.
    fn open_ssh(&mut self) {
        self.open_ssh_row();
        let on = self.a.ssh_on();
        self.press(Btn::Y);
        assert_eq!(self.a.ssh_on(), on, "Y switched SSH");
        assert!(
            matches!(self.a.phase(), Phase::Info(InfoScreen::Ssh(_))),
            "Y did not open SSH: {:?}",
            self.a.phase()
        );
    }

    /// The open panel's lines and note.
    fn panel(&self) -> InfoView {
        self.a.info_view().expect("the SSH panel is not showing")
    }

    fn saved_ssh(&self) -> bool {
        read_app_state(self.d.path()).ssh
    }
}

fn status(hostname: &str) -> Status {
    Status {
        enabled: true,
        state: "COMPLETED".into(),
        ssid: Some(b"Home".to_vec()),
        ip: Some("192.168.1.20".into()),
        rssi: Some(-50),
        mac: Some("e4:5f:01:2a:9c:33".into()),
        hostname: hostname.into(),
        ntp: true,
        ntp_synced: true,
    }
}

fn off() -> AppState {
    AppState {
        ssh: false,
        ..AppState::default()
    }
}

#[test]
fn the_ssh_row_is_on_or_off_and_left_and_right_switch_it() {
    let mut r = Rig::new();
    r.open_ssh_row();
    assert_eq!(
        r.ssh_row(),
        SettingLine {
            label: "SSH",
            value: Some("ON"),
            carets: true,
            opens: false,
        }
    );
    let mut steps = Vec::new();
    for btn in [Btn::Right, Btn::Right, Btn::Left, Btn::Left] {
        r.press(btn);
        steps.push((r.ssh_value(), r.a.ssh_on()));
    }
    assert_eq!(
        steps,
        [("OFF", false), ("ON", true), ("OFF", false), ("ON", true)],
    );
    assert_eq!(
        r.server.asked(),
        [Asked::Stop, Asked::Start, Asked::Stop, Asked::Start],
    );
    assert!(r.on_ssh_row(), "the cursor left the row");
}

#[test]
fn on_or_off_is_kept_on_the_card() {
    let mut r = Rig::new();
    assert!(r.saved_ssh(), "SSH is not on to begin with");
    r.open_ssh_row();
    r.press(Btn::Right);
    assert!(!r.saved_ssh(), "OFF was not kept");
    r.press(Btn::Y);
    r.press(Btn::B);
    assert!(!r.saved_ssh(), "the panel changed what was kept");
    r.press(Btn::Right);
    assert!(r.saved_ssh(), "ON was not kept");
}

#[test]
fn a_on_on_or_off_switches_ssh_as_it_does_wifi() {
    let mut r = Rig::new();
    r.open_ssh_row();
    r.press(Btn::A);
    assert_eq!((r.ssh_value(), r.a.ssh_on()), ("OFF", false));
    r.press(Btn::A);
    assert_eq!((r.ssh_value(), r.a.ssh_on()), ("ON", true));
    assert_eq!(r.server.asked(), [Asked::Stop, Asked::Start]);
    assert!(r.on_ssh_row(), "A on ON or OFF opened {:?}", r.a.phase());
}

#[test]
fn y_opens_the_login_and_b_comes_back_to_the_row_as_it_was() {
    for (state, shown) in [(AppState::default(), "ON"), (off(), "OFF")] {
        let mut r = Rig::with(state, |_| Server::default());
        r.open_ssh_row();
        assert_eq!(r.ssh_value(), shown, "the row opened on another value");
        r.server.forget_asked();
        r.press(Btn::Y);
        assert!(
            matches!(r.a.phase(), Phase::Info(InfoScreen::Ssh(_))),
            "{:?}",
            r.a.phase()
        );
        r.press(Btn::B);
        assert!(
            r.on_ssh_row(),
            "B did not go back to the SSH row: {:?}",
            r.a.phase()
        );
        assert_eq!(r.ssh_value(), shown);
        assert!(r.server.asked().is_empty(), "the panel switched SSH");
        r.press(Btn::B);
        assert_eq!(r.a.settings_row(), Some(SettingsRow::Network));
    }
}

#[test]
fn no_value_of_the_row_leads_anywhere() {
    let mut r = Rig::new();
    r.open_ssh_row();
    for btn in [Btn::Right, Btn::A, Btn::Left, Btn::A] {
        r.press(btn);
        assert!(r.on_ssh_row(), "{btn:?} left the row: {:?}", r.a.phase());
        assert!(
            ["ON", "OFF"].contains(&r.ssh_value()),
            "{:?}",
            r.ssh_value()
        );
    }
}

#[test]
fn ssh_left_off_is_stopped_as_omega_zero_starts_and_again_while_baseos_starts_late() {
    let mut r = Rig::with(off(), |_| Server::default());
    assert!(!r.a.ssh_on());
    assert_eq!(r.server.asked().first(), Some(&Asked::Stop), "not stopped");
    while r.now < SETTLE_MS {
        r.tick(500);
    }
    let asked = r.server.asked();
    assert!(
        asked.len() > 2 && asked.iter().all(|a| *a == Asked::Stop),
        "{asked:?}"
    );
    r.server.forget_asked();
    r.tick(60_000);
    r.tick(60_000);
    assert!(
        r.server.asked().is_empty(),
        "still stopping it long after starting: {:?}",
        r.server.asked()
    );
}

#[test]
fn switching_ssh_on_ends_the_stopping_at_once() {
    let mut r = Rig::with(off(), |_| Server::default());
    r.open_ssh_row();
    r.press(Btn::Right);
    assert!(r.a.ssh_on());
    r.server.forget_asked();
    while r.now < SETTLE_MS {
        r.tick(500);
    }
    assert!(r.server.asked().is_empty(), "{:?}", r.server.asked());
}

#[test]
fn ssh_left_on_is_left_as_it_is_as_omega_zero_starts() {
    let mut r = Rig::new();
    for _ in 0..10 {
        r.tick(5_000);
    }
    assert!(r.server.asked().is_empty(), "{:?}", r.server.asked());
}

#[test]
fn the_top_bar_shows_ssh_while_it_is_on_and_a_network_is_joined() {
    let mut r = Rig::new();
    assert!(!r.a.links().ssh, "lit with no network joined");
    r.join("rg35xxsp");
    assert!(r.a.links().ssh, "not lit once joined");
    r.open_ssh_row();
    r.press(Btn::Right);
    assert!(!r.a.links().ssh, "lit with SSH off");
    r.press(Btn::Right);
    assert!(r.a.links().ssh, "not lit once on again");
}

#[test]
fn joined_it_says_how_to_log_in() {
    let mut r = Rig::new();
    r.join("rg35xxsp");
    r.open_ssh();
    assert_eq!(
        r.panel(),
        InfoView {
            title: "SSH",
            lines: vec![
                ("IP", "192.168.1.20".to_string()),
                ("HOST", "rg35xxsp.local".to_string()),
                ("USER", "root".to_string()),
                ("PASSWORD", "root".to_string()),
                ("PORT", "22".to_string()),
            ],
            note: "SFTP uses the same login",
        }
    );
}

#[test]
fn offline_it_asks_for_a_network_first() {
    let mut r = Rig::new();
    r.open_ssh();
    let view = r.panel();
    assert!(view.lines.is_empty(), "{view:?}");
    assert_eq!(view.note, "Join a Wi-Fi network first.");
}

#[test]
fn with_ssh_off_it_asks_for_it_on_first() {
    let mut r = Rig::with(off(), |_| Server::default());
    r.join("rg35xxsp");
    r.open_ssh();
    let view = r.panel();
    assert!(view.lines.is_empty(), "{view:?}");
    assert_eq!(view.note, "Turn SSH on first.");
}

#[test]
fn a_handheld_with_no_name_is_reached_by_its_address() {
    let screen = SshScreen {
        password: "root".into(),
        on: true,
        trouble: None,
    };
    let view = screen.view(&status(""));
    assert_eq!(view.lines[1], ("HOST", "192.168.1.20".to_string()));
}

#[test]
fn the_password_is_read_from_the_cards_baseos_conf_when_the_screen_opens() {
    let mut r = Rig::new();
    std::fs::write(
        r.d.path().join("baseos.conf"),
        "# baseos.conf\nhostname=rg35xxsp\nssh_password=hunter2\n",
    )
    .unwrap();
    r.join("rg35xxsp");
    r.open_ssh();
    let Phase::Info(InfoScreen::Ssh(screen)) = r.a.phase() else {
        panic!("not on the SSH panel");
    };
    assert_eq!(screen.password, "hunter2");
    let view = r.panel();
    assert_eq!(view.lines[3], ("PASSWORD", "hunter2".to_string()));
}

/// BaseOS's own baseos.conf, as it ships: every setting commented out.
pub const SHIPPED: &str = "\
# BaseOS settings. Edit this file on TF1, then restart normally to apply.
# Missing values use the device model ID (e.g. rg34xxsp) and enable mDNS.
# Hostname: 1-63 ASCII letters, digits or hyphens; no leading/trailing hyphen.
# hostname=MyHandheld
# Publish <hostname>.local on Wi-Fi: true or false.
# mdns=true

# Headphone exit-pop workaround on supported kernels: true or false.
# Default off. Keeps analogue output buffers powered while idle, which may
# increase idle drain; buffers are disabled before deep sleep. Speaker fix
# remains enabled regardless. Restart to apply.
# headphone_pop_fix=false

# SSH password (plain text). See the settings reference for details:
# https://github.com/pvaibhav/BaseOS#settings
# ssh_password=your-password
";

#[test]
fn a_card_with_no_password_gets_one_on_the_login_at_once() {
    let mut r = Rig::with(AppState::default(), |root| {
        let conf = root.join("baseos.conf");
        std::fs::write(&conf, SHIPPED).unwrap();
        Server::with_conf(conf)
    });
    let conf = std::fs::read_to_string(r.d.path().join("baseos.conf")).unwrap();
    let made = conf
        .lines()
        .find_map(|line| line.strip_prefix("ssh_password="))
        .expect("no password written")
        .to_string();
    assert!(
        made.len() == 6 && made.bytes().all(|b| b.is_ascii_digit()),
        "{made:?}"
    );
    assert_eq!(
        r.server.asked(),
        [Asked::Password(made.clone())],
        "root was not given it"
    );
    r.join("rg35xxsp");
    r.open_ssh();
    let view = r.panel();
    assert_eq!(view.lines[2], ("USER", "root".to_string()));
    assert_eq!(view.lines[3], ("PASSWORD", made));
    assert_eq!(view.note, "SFTP uses the same login");
}

#[test]
fn a_password_that_could_not_be_saved_leaves_root_its_own_and_says_so() {
    let mut r = Rig::with(AppState::default(), |root| {
        Server::with_conf(root.join("no such folder/baseos.conf"))
    });
    assert!(
        r.server.asked().is_empty(),
        "root was given a password the card does not keep"
    );
    r.join("rg35xxsp");
    r.open_ssh();
    let view = r.panel();
    assert_eq!(view.lines[3], ("PASSWORD", "root".to_string()));
    assert_eq!(view.note, Trouble::NotSaved.note());
}

#[test]
fn a_password_the_system_would_not_take_says_it_needs_a_restart() {
    let mut r = Rig::with(AppState::default(), |root| {
        let conf = root.join("baseos.conf");
        std::fs::write(&conf, SHIPPED).unwrap();
        let server = Server::with_conf(conf);
        server.0.lock().unwrap().refuses = true;
        server
    });
    r.join("rg35xxsp");
    r.open_ssh();
    let view = r.panel();
    assert_ne!(view.lines[3].1, "root", "the card's new one is not shown");
    assert_eq!(view.note, Trouble::NotApplied.note());
}

#[test]
fn every_note_fits_the_panel() {
    let notes = [
        "SFTP uses the same login",
        "Join a Wi-Fi network first.",
        "Waiting for an address…",
        "Turn SSH on first.",
        Trouble::NotSaved.note(),
        Trouble::NotApplied.note(),
    ];
    let pal = omega_ui::kit::palette::palette(0);
    for lines in [0, 5] {
        let well = omega_ui::network::info_chrome("SSH", lines, pal).well;
        for note in notes {
            let face = omega_ui::kit::text::text_face(note, pal.dim);
            assert!(
                face.w + omega_ui::network::NOTE_IN * 2 <= well[2],
                "{note:?} is {} wide in a well {} wide",
                face.w,
                well[2]
            );
        }
    }
}

#[test]
fn how_closely_the_radio_is_watched_follows_the_screen() {
    let mut r = Rig::new();
    assert_eq!(r.a.wifi_watch(), Watch::Glance, "on the shelf");
    r.open_network();
    assert_eq!(r.a.wifi_watch(), Watch::Networks, "on Network");
    r.press(Btn::B);
    assert_eq!(r.a.wifi_watch(), Watch::Status, "on Settings");
    r.press(Btn::B);
    r.open_ssh();
    assert_eq!(r.a.wifi_watch(), Watch::Networks, "on SSH, under Network");
    r.press(Btn::B);
    r.press(Btn::B);
    r.a.finish_slide();
    common::to_shelf_row(&mut r.a, omega_zero::options::OptionRow::Labels);
    r.press(Btn::A);
    assert!(matches!(r.a.phase(), Phase::Labels(_)));
    assert_eq!(r.a.wifi_watch(), Watch::Glance, "on Labels");
    r.press(Btn::B);
    r.a.finish_slide();
    while common::shelf_row(&r.a) != Some(omega_zero::options::OptionRow::Theme) {
        r.press(Btn::Up);
    }
    r.press(Btn::A);
    assert!(matches!(r.a.phase(), Phase::Background(_)));
    assert_eq!(r.a.wifi_watch(), Watch::Idle, "on Background");
    r.press(Btn::B);
    r.press(Btn::B);
    r.press(Btn::B);
    assert!(matches!(r.a.phase(), Phase::Shelf));
    r.a.apply(Action::SelectTap);
    assert!(matches!(r.a.phase(), Phase::Controls));
    assert_eq!(r.a.wifi_watch(), Watch::Idle, "on Controls");
}
