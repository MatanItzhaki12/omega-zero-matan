use crate::common;

use std::sync::{Arc, Mutex};

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use omega_input::{Action, Btn, Millis};
use omega_store::{write_app_state, AppState, Platform};
use omega_ui::TexId;
use omega_zero::app::{App, Phase};
use omega_zero::label_screen::{Sel, SyncState};
use omega_zero::label_sync::{
    Account, LabelJob, LabelJobs, LabelNews, Quota, SyncFail, Tally, Who, ACCOUNT_FILE,
};
use omega_zero::list_view::{LegendHint, RowValue};
use omega_zero::wifi::{Status, WifiJob, WifiJobs, WifiNews};
use tempfile::TempDir;

#[derive(Default)]
struct ScraperState {
    absent: bool,
    syncs: Vec<(Account, Vec<String>)>,
    stops: usize,
    news: Vec<LabelNews>,
}

/// A ScreenScraper that answers only what the test says, and remembers what it was asked.
#[derive(Clone, Default)]
struct Scraper(Arc<Mutex<ScraperState>>);

impl Scraper {
    fn say(&self, news: LabelNews) {
        self.0.lock().unwrap().news.push(news);
    }

    fn syncs(&self) -> Vec<(Account, Vec<String>)> {
        self.0.lock().unwrap().syncs.clone()
    }
}

impl LabelJobs for Scraper {
    fn present(&self) -> bool {
        !self.0.lock().unwrap().absent
    }

    fn ask(&mut self, job: LabelJob) {
        let mut s = self.0.lock().unwrap();
        match job {
            LabelJob::Sync { account, wants } => s
                .syncs
                .push((account, wants.into_iter().map(|w| w.stem).collect())),
            LabelJob::Stop => s.stops += 1,
        }
    }

    fn news(&mut self) -> Vec<LabelNews> {
        std::mem::take(&mut self.0.lock().unwrap().news)
    }
}

/// A radio that is on and has whatever network the test says joined.
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

struct Rig {
    d: TempDir,
    a: App,
    scraper: Scraper,
    radio: Radio,
    now: Millis,
}

impl Rig {
    fn with(account: Option<Account>, labelled: &[&str]) -> Rig {
        Rig::on(
            tmp_root_with_carts(&["Emerald", "Fusion", "Zero"]),
            account,
            labelled,
        )
    }

    /// The rig on a card the test has filled itself, its GBA carts `labelled` already.
    fn on(d: TempDir, account: Option<Account>, labelled: &[&str]) -> Rig {
        write_app_state(
            d.path(),
            &AppState {
                clock_set: true,
                wifi: true,
                ..AppState::default()
            },
        )
        .unwrap();
        if let Some(a) = &account {
            a.save(d.path()).unwrap();
        }
        for stem in labelled {
            let label = d.path().join(format!("Labels/GBA/{stem}.png"));
            std::fs::create_dir_all(label.parent().unwrap()).unwrap();
            std::fs::write(label, b"png").unwrap();
        }
        let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
        let (scraper, radio) = (Scraper::default(), Radio::default());
        a.set_wifi_jobs(Box::new(radio.clone()));
        a.set_label_jobs(Box::new(scraper.clone()));
        let mut r = Rig {
            d,
            a,
            scraper,
            radio,
            now: 1_000,
        };
        r.tick(16);
        r
    }

    fn signed_in() -> Rig {
        Rig::with(Some(account()), &[])
    }

    fn press(&mut self, btn: Btn) {
        self.a.apply(Action::GbaDown(btn));
        self.a.apply(Action::GbaUp(btn));
    }

    fn tick(&mut self, ms: Millis) {
        self.now += ms;
        self.a.tick_ms(self.now);
    }

    fn join_wifi(&mut self) {
        self.radio.0.lock().unwrap().push(WifiNews::Status(Status {
            enabled: true,
            state: "COMPLETED".into(),
            ssid: Some(b"Home".to_vec()),
            ip: Some("192.168.1.20".into()),
            rssi: Some(-50),
            mac: None,
            hostname: "rg35xxsp".into(),
            ntp: true,
            ntp_synced: true,
        }));
        self.tick(16);
        assert!(self.a.wifi().status().joined(), "Wi-Fi did not join");
    }

    fn open_labels(&mut self) {
        self.a.apply(Action::QuickMenu);
        assert!(self.a.settings_row().is_some(), "START opened no Settings");
        common::to_shelf_row(&mut self.a, omega_zero::options::OptionRow::Labels);
        self.press(Btn::A);
        assert!(
            matches!(self.a.phase(), Phase::Labels(_)),
            "A on Labels did not open it"
        );
    }

    fn sel(&self) -> Sel {
        match self.a.phase() {
            Phase::Labels(s) => s.sel,
            other => panic!("not on the Labels screen: {other:?}"),
        }
    }

    fn go(&mut self, sel: Sel) {
        if self.sel() != sel {
            self.press(Btn::Down);
        }
        assert_eq!(self.sel(), sel);
    }

    fn status(&self) -> Vec<String> {
        self.a.labels_view().expect("the list is showing").status
    }

    fn legend(&self) -> Vec<LegendHint> {
        self.a.labels_view().expect("the list is showing").legend
    }

    fn typed(&self) -> Option<(String, String)> {
        self.a
            .typing()
            .map(|t| (t.prompt.clone(), t.keys.text.clone()))
    }
}

fn account() -> Account {
    Account {
        user: "middlebaws".into(),
        password: "secret".into(),
    }
}

#[test]
fn labels_opens_from_shelf_and_b_comes_back_to_its_row() {
    let mut r = Rig::signed_in();
    r.open_labels();
    assert_eq!(r.sel(), Sel::Account);
    r.press(Btn::B);
    assert_eq!(
        common::shelf_row(&r.a),
        Some(omega_zero::options::OptionRow::Labels),
        "B did not go back to SHELF's Labels row: {:?}",
        r.a.phase()
    );
}

#[test]
fn signing_in_asks_for_a_user_name_then_a_password_and_keeps_both() {
    let mut r = Rig::with(None, &[]);
    r.open_labels();
    let view = r.a.labels_view().unwrap();
    assert_eq!(view.rows[0].value, Some(RowValue::Said("Sign in")));
    assert_eq!(view.legend, [LegendHint::Back, LegendHint::SignIn]);
    r.press(Btn::A);
    r.press(Btn::A);
    r.press(Btn::A);
    assert_eq!(
        r.typed(),
        Some(("ScreenScraper user name".into(), "qq".into()))
    );
    r.press(Btn::Start);
    assert_eq!(
        r.typed(),
        Some(("ScreenScraper password for qq".into(), String::new())),
        "the password field kept the user name"
    );
    r.press(Btn::Start);
    assert!(r.typed().is_some(), "an empty password was taken");
    r.press(Btn::A);
    r.press(Btn::Start);
    assert_eq!(r.typed(), None);
    let saved = Account {
        user: "qq".into(),
        password: "q".into(),
    };
    assert_eq!(Account::load(r.d.path()), Some(saved.clone()));
    assert_eq!(r.a.labels().account(), Some(&saved));
    assert_eq!(r.sel(), Sel::Sync, "the cursor did not move on to fetching");
    let view = r.a.labels_view().unwrap();
    assert_eq!(view.rows[0].value, Some(RowValue::Name("qq".into())));
}

#[test]
fn b_on_an_empty_field_puts_the_keyboard_away_without_signing_in() {
    let mut r = Rig::with(None, &[]);
    r.open_labels();
    r.press(Btn::A);
    r.press(Btn::B);
    assert_eq!(r.typed(), None);
    assert_eq!(r.a.labels().account(), None);
    assert!(!r.d.path().join(ACCOUNT_FILE).exists());
}

#[test]
fn y_signs_out_and_forgets_the_account() {
    let mut r = Rig::signed_in();
    r.open_labels();
    assert!(r.legend().contains(&LegendHint::SignOut));
    r.press(Btn::Y);
    assert_eq!(r.a.labels().account(), None);
    assert!(!r.d.path().join(ACCOUNT_FILE).exists());
}

#[test]
fn fetching_waits_for_a_network_and_then_looks_up_only_the_carts_without_labels() {
    let mut r = Rig::with(Some(account()), &["Fusion"]);
    r.open_labels();
    r.go(Sel::Sync);
    assert!(!r.legend().contains(&LegendHint::Start));
    assert_eq!(r.status(), ["Join a Wi-Fi network to fetch labels."]);
    r.press(Btn::A);
    assert!(r.scraper.syncs().is_empty(), "it looked up offline");
    r.join_wifi();
    assert!(r.legend().contains(&LegendHint::Start));
    r.press(Btn::A);
    assert_eq!(
        r.scraper.syncs(),
        [(account(), vec!["Emerald".to_string(), "Zero".to_string()])]
    );
    assert_eq!(
        r.a.labels().state(),
        &SyncState::Working {
            done: 0,
            total: 2,
            title: "Emerald".into()
        }
    );
}

#[test]
fn the_screen_follows_the_sync_and_a_stops_it() {
    let mut r = Rig::signed_in();
    r.join_wifi();
    r.open_labels();
    r.go(Sel::Sync);
    r.press(Btn::A);
    r.scraper.say(LabelNews::Quota(Quota {
        left: 19_953,
        per_day: 20_000,
    }));
    r.scraper.say(LabelNews::Working {
        done: 1,
        total: 3,
        title: "Fusion".into(),
    });
    r.tick(16);
    let view = r.a.labels_view().unwrap();
    assert_eq!(view.rows[1].value, Some(RowValue::Name("2 / 3".into())));
    assert_eq!(view.status[0], "Looking up 2 of 3: Fusion");
    assert_eq!(view.legend, [LegendHint::Back, LegendHint::Stop]);
    r.press(Btn::A);
    assert_eq!(r.scraper.0.lock().unwrap().stops, 1);
    r.scraper.say(LabelNews::Finished(Tally {
        saved: 1,
        bare: 1,
        unknown: 1,
    }));
    r.tick(16);
    assert_eq!(
        r.status(),
        [
            "Done: 1 new label.",
            "1 has no scan, 1 is not on ScreenScraper."
        ]
    );
}

#[test]
fn a_refusal_says_what_to_do() {
    let mut r = Rig::signed_in();
    r.join_wifi();
    r.open_labels();
    r.go(Sel::Sync);
    r.press(Btn::A);
    r.scraper
        .say(LabelNews::Failed(SyncFail::Refused(Who::Player)));
    r.tick(16);
    assert_eq!(
        r.status()[0],
        "ScreenScraper refused that name or password."
    );
    assert!(!r.a.labels().working());
}

#[test]
fn a_label_that_arrives_goes_on_its_cart_and_its_face_is_drawn_again() {
    let mut r = Rig::signed_in();
    let faces: Vec<TexId> = (0..3).map(TexId::from_raw).collect();
    r.a.set_faces(faces);
    r.join_wifi();
    r.open_labels();
    r.go(Sel::Sync);
    r.press(Btn::A);
    let label = r.d.path().join("Labels/GBA/Fusion.png");
    std::fs::write(&label, b"png").unwrap();
    r.scraper.say(LabelNews::Saved {
        platform: Platform::Gba,
        stem: "Fusion".into(),
    });
    r.tick(16);
    let fusion = r.a.carts().find(|c| c.stem == "Fusion").unwrap();
    assert_eq!(fusion.label.as_deref(), Some(label.as_path()));
    let stale = r.a.take_stale_faces();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].0.stem, "Fusion");
    assert_eq!(stale[0].1, TexId::from_raw(1), "not Fusion's own face");
    assert!(
        r.a.take_stale_faces().is_empty(),
        "a face was redrawn twice"
    );
}

#[test]
fn a_build_without_screenscrapers_key_says_so() {
    let mut r = Rig::signed_in();
    r.scraper.0.lock().unwrap().absent = true;
    r.open_labels();
    assert_eq!(r.status()[0], "This build has no ScreenScraper key.");
}

#[test]
fn a_playstation_game_is_looked_up_disc_by_disc() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion", "Zero"]);
    let ps1 = d.path().join("Games/PS1");
    std::fs::create_dir_all(&ps1).unwrap();
    for n in 1..=2 {
        std::fs::write(
            ps1.join(format!("Parasite Eve (USA) (Disc {n}).cue")),
            b"FILE \"x.bin\" BINARY\n",
        )
        .unwrap();
    }
    let mut r = Rig::on(d, Some(account()), &["Emerald", "Fusion", "Zero"]);
    r.join_wifi();
    r.open_labels();
    r.go(Sel::Sync);
    r.press(Btn::A);
    let game = "Parasite Eve (USA)".to_string();
    assert_eq!(
        r.scraper.syncs(),
        [(account(), vec![game.clone(), game.clone()])],
        "each disc is not looked up on its own"
    );
    assert_eq!(
        r.a.labels().state(),
        &SyncState::Working {
            done: 0,
            total: 2,
            title: "Parasite Eve, disc 1".into()
        }
    );

    // the second disc's label arrives first: the game has none of its own yet
    let labels = r.d.path().join("Labels/PS1");
    std::fs::create_dir_all(&labels).unwrap();
    std::fs::write(labels.join("Parasite Eve (USA) (Disc 2).png"), b"png").unwrap();
    r.scraper.say(LabelNews::Saved {
        platform: Platform::Ps1,
        stem: game.clone(),
    });
    r.tick(16);
    let label = |r: &Rig| r.a.carts().find(|c| c.stem == game).unwrap().label.clone();
    assert_eq!(label(&r), None, "the game took a label that is not there");

    std::fs::write(labels.join("Parasite Eve (USA) (Disc 1).png"), b"png").unwrap();
    std::fs::write(labels.join("Parasite Eve (USA).png"), b"png").unwrap();
    r.scraper.say(LabelNews::Saved {
        platform: Platform::Ps1,
        stem: game.clone(),
    });
    r.tick(16);
    assert_eq!(label(&r), Some(labels.join("Parasite Eve (USA).png")));
}

/// Holo Sheen, under Fetch Labels, is on until A, left or right turns it off, and it stays as it
/// was set: the next boot reads it back.
#[test]
fn holo_sheen_turns_off_and_on_and_is_kept() {
    let mut r = Rig::with(None, &[]);
    r.open_labels();
    r.go(Sel::Sync);
    r.go(Sel::Holo);
    let shown = |r: &Rig| {
        let view = r.a.labels_view().expect("the list is showing");
        view.rows[2].value.clone()
    };
    assert_eq!(
        shown(&r),
        Some(RowValue::Setting(true)),
        "the sheen is not on at first"
    );
    r.press(Btn::A);
    assert_eq!(shown(&r), Some(RowValue::Setting(false)));
    assert!(
        !omega_store::read_app_state(r.d.path()).holo,
        "it was not kept off"
    );
    r.press(Btn::Right);
    assert_eq!(shown(&r), Some(RowValue::Setting(true)));
    assert!(omega_store::read_app_state(r.d.path()).holo);
    r.press(Btn::Down);
    assert_eq!(r.sel(), Sel::Account, "the rows did not wrap");
}
