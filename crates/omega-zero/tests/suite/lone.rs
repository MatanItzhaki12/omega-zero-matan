//! The lone panels, CONTROLS, THEME, DATE & TIME, LABELS, POWER and the screens after its
//! choice, RESTART and POWER OFF, against flow.html: each panel's face is held to a screenshot
//! of flow.html in Chrome (lone/ref/, made by lone/make-refs.sh from assets/ui/flow.html), pixel
//! for pixel, in Night and in Amber. The screenshots are the whole 640x480 screen; a panel is in
//! the middle of its page, which is the screen above the bottom bar.
//!
//! Set KIT_OUT to a folder to have each face written there as a PNG.

use crate::common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};

use omega_input::{Action, Btn};
use omega_store::{colour_index, write_app_state, AppState, BackdropStyle, Font};
use omega_ui::kit::layout::{screen_px, BAR_U};
use omega_ui::kit::palette::{palette, Palette};
use omega_ui::kit::pixel::text_width;
use omega_ui::kit::row::panel_height;
use omega_ui::{out_h, out_w, ClockPicker, Draw, PowerChoice, SettingsRow, TexId, UndoFace};
use omega_zero::app::App;
use omega_zero::background_screen::{Backdrop, BackgroundScreen};
use omega_zero::label_screen::{Labels, LabelsScreen};
use omega_zero::label_sync::{Account, LabelJob, LabelJobs, LabelNews, SyncFail, Tally, Who};
use omega_zero::list_view::LegendHint;
use omega_zero::nav::{offsets, progress, Dir, End, SLIDE_MS};
use omega_zero::screens::background::background_face;
use omega_zero::screens::clock::clock_face;
use omega_zero::screens::controls::{
    column_xs, content_height, controls_face, line_y, ControlsView, COLUMNS, WIDTH,
};
use omega_zero::screens::labels::{labels_face, labels_hints, NOTE_LINES};
use omega_zero::screens::lone::{fitted, Lone, NOTE_X};
use omega_zero::screens::power::{going_face, going_words, power_menu_face, NOTE_ALONE};

/// The time flow.html's Date & Time shows: 2026-10-10 16:00 UTC.
pub const FLOW_DATE: i64 = 1_791_648_000;

/// The page a 640x480 screen's menus sit in, in art pixels: all of it above the bar's `BAR_U`.
const PAGE: [u32; 2] = [320, 240 - BAR_U];

pub fn colours() -> [(u8, &'static str); 2] {
    [(0, "night"), (amber(), "amber")]
}

pub fn amber() -> u8 {
    colour_index("amber").expect("amber is a colour")
}

/// A ScreenScraper that is there, and answers nothing.
#[derive(Default)]
pub struct Present;

impl LabelJobs for Present {
    fn present(&self) -> bool {
        true
    }

    fn ask(&mut self, _job: LabelJob) {}

    fn news(&mut self) -> Vec<LabelNews> {
        Vec::new()
    }
}

/// A screenshot of flow.html: the whole screen, three bytes a pixel.
struct Shot {
    rgb: Vec<u8>,
    w: u32,
}

impl Shot {
    fn at(&self, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * self.w + x) * 3) as usize;
        [self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]]
    }
}

fn reference(name: &str) -> Shot {
    let path = format!(
        "{}/tests/suite/lone/ref/{name}.png",
        env!("CARGO_MANIFEST_DIR")
    );
    let file = std::fs::File::open(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().expect("a PNG");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("its pixels");
    let channels = info.color_type.samples();
    let rgb = buf[..info.buffer_size()]
        .chunks(channels)
        .flat_map(|px| [px[0], px[1], px[2]])
        .collect();
    Shot { rgb, w: info.width }
}

pub fn write(path: &str, face: &UndoFace) {
    let f = std::fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), face.w, face.h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .unwrap()
        .write_image_data(&face.rgba)
        .unwrap();
}

/// `face`'s rows `rows` are those of flow.html's panel in the screenshot `name`, where the
/// panel is `flow_h` art pixels tall, every pixel the face draws the same colour; the face is
/// clear only at its corners, where the backdrop shows through.
fn matches_flow(face: &UndoFace, name: &str, flow_h: u32, rows: std::ops::Range<u32>) {
    if let Ok(dir) = std::env::var("KIT_OUT") {
        write(&format!("{dir}/{name}.png"), face);
    }
    let shot = reference(name);
    let (x0, y0) = ((PAGE[0] - face.w) / 2, (PAGE[1] - flow_h) / 2);
    let mut off = Vec::new();
    for y in rows {
        for x in 0..face.w {
            let i = ((y * face.w + x) * 4) as usize;
            if face.rgba[i + 3] == 0 {
                continue;
            }
            let ours = [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]];
            let chrome = shot.at(2 * (x0 + x), 2 * (y0 + y));
            if ours != chrome {
                off.push((x, y, ours, chrome));
            }
        }
    }
    assert!(
        off.is_empty(),
        "{name}: {} pixels differ from Chrome's, first (x, y, ours, Chrome's) {:?}",
        off.len(),
        &off[..off.len().min(8)]
    );
}

fn background_view(colour: u8) -> omega_zero::list_view::ListView {
    BackgroundScreen::default().view(
        Backdrop {
            style: BackdropStyle::Grid,
            moving: true,
            colour,
        },
        Font::Bold,
    )
}

#[test]
fn background_is_flows() {
    for (colour, name) in colours() {
        let face = background_face(&background_view(colour), palette(colour));
        assert_eq!((face.w, face.h), (200, 75), "{name}");
        matches_flow(&face, &format!("background-{name}"), face.h, 0..face.h);
    }
}

#[test]
fn date_and_time_is_flows() {
    for (colour, name) in colours() {
        let face = clock_face(&ClockPicker::from_secs(FLOW_DATE), palette(colour));
        assert_eq!((face.w, face.h), (240, 49), "{name}");
        matches_flow(&face, &format!("datetime-{name}"), face.h, 0..face.h);
    }
}

/// The POWER menu as holding POWER opens it, its cursor on RESTART.
#[test]
fn the_power_menu_is_flows() {
    for (colour, name) in colours() {
        let face = power_menu_face(PowerChoice::Restart, palette(colour));
        assert_eq!((face.w, face.h), (200, panel_height(2 * 13)), "{name}");
        matches_flow(&face, &format!("power-{name}"), face.h, 0..face.h);
    }
}

/// The screens after its choice: RESTART saying "Restarting…" and POWER OFF "Turning off…",
/// each note on its own in the dim colour, as SSH's.
#[test]
fn the_screens_after_the_power_menus_choice_are_flows() {
    for (choice, shot) in [
        (PowerChoice::Restart, "restarting"),
        (PowerChoice::PowerOff, "poweroff"),
    ] {
        for (colour, name) in colours() {
            let face = going_face(choice, palette(colour));
            assert_eq!((face.w, face.h), (200, panel_height(NOTE_ALONE)), "{name}");
            matches_flow(&face, &format!("{shot}-{name}"), face.h, 0..face.h);
        }
    }
    assert_eq!(
        going_words(PowerChoice::Restart),
        ("RESTART", "Restarting…")
    );
    assert_eq!(
        going_words(PowerChoice::PowerOff),
        ("POWER OFF", "Turning off…")
    );
}

/// Moving the cursor to POWER OFF lights its row and puts out RESTART's, and nothing else.
#[test]
fn the_power_menus_cursor_lights_its_row() {
    let pal = palette(0);
    let (restart, off) = (
        power_menu_face(PowerChoice::Restart, pal),
        power_menu_face(PowerChoice::PowerOff, pal),
    );
    assert_eq!((restart.w, restart.h), (off.w, off.h));
    let row_of = |i: usize| (i / 4) as u32 / restart.w;
    let changed: Vec<u32> = restart
        .rgba
        .chunks(4)
        .zip(off.rgba.chunks(4))
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| row_of(4 * i))
        .collect();
    assert!(!changed.is_empty(), "the cursor did not move");
    // the well starts 18 down: RESTART's row is 18 to 30, POWER OFF's 31 to 43
    assert!(
        changed.iter().all(|y| (18..44).contains(y)),
        "the cursor changed more than the two rows: {:?}",
        changed.iter().min()..changed.iter().max()
    );
}

fn labels(account: Option<Account>) -> Labels {
    Labels::new(Box::new(Present), account)
}

#[test]
fn labels_is_flows() {
    for (colour, name) in colours() {
        let mut view = LabelsScreen::default().view(&labels(None), false, true);
        // Holo Sheen, the last row, came after flow.html: the rest is flow's
        view.rows.pop();
        let face = labels_face(&view, palette(colour));
        assert_eq!((face.w, face.h), (280, 75), "{name}");
        matches_flow(&face, &format!("labels-{name}"), face.h, 0..face.h);
    }
}

/// The Controls panel's lines are its own, trimmed to fit flow.html's grid, and the GBA's has
/// two more than flow.html's; its title, its headings and the lines both say are flow.html's
/// pixel for pixel.
#[test]
fn the_controls_panel_is_flows_where_it_says_what_flow_says() {
    let view = ControlsView::new(omega_store::Platform::Gba);
    for (colour, name) in colours() {
        let face = controls_face(&view, palette(colour));
        let flow_h = panel_height(13 + 11 * 11);
        let well_y = 18;
        // the header, the headings, and the first two lines of each column: A PLAY and HOLD A
        // CLEAN START, L2 REWIND and R2 FAST FORWARD
        matches_flow(
            &face,
            &format!("controls-{name}"),
            flow_h,
            0..well_y + line_y(2),
        );
    }
}

/// At 640x480 the page is 320 by 225 art pixels: every system's panel fits it, and every line
/// fits its column, its buttons in their room and what they do in theirs.
#[test]
fn the_controls_panel_fits_at_640() {
    for platform in omega_store::Platform::ALL {
        let view = ControlsView::new(platform);
        let face = controls_face(&view, palette(0));
        assert_eq!(face.w, WIDTH);
        assert!(face.w <= PAGE[0], "{} wide", face.w);
        assert!(
            face.h <= PAGE[1],
            "{platform:?}'s is {} tall, more than the page",
            face.h
        );
        assert_eq!(face.h, panel_height(content_height(&view)));
        let well_w = WIDTH - 12;
        let [_, second] = column_xs();
        assert_eq!(
            second + COLUMNS[1].width(),
            well_w,
            "the columns fill the well"
        );
        for (group, column) in view.groups.iter().zip(COLUMNS) {
            assert_eq!(group.name, group.name.to_uppercase());
            for (keys, does) in &group.lines {
                assert!(
                    text_width(keys) <= 6 * column.keys,
                    "{platform:?} {} {keys} is wider than its {} letters",
                    group.name,
                    column.keys
                );
                assert!(
                    text_width(does) <= 6 * column.does,
                    "{platform:?} {} {does} is wider than its {} letters",
                    group.name,
                    column.does
                );
                assert_eq!(*keys, keys.to_uppercase(), "drawn as written");
                assert_eq!(*does, does.to_uppercase(), "drawn as written");
            }
        }
    }
}

/// The panel's notes keep their room, so the panel holds still as the sync goes on.
#[test]
fn the_labels_panel_holds_still_whatever_it_says() {
    let pal = palette(0);
    let screen = LabelsScreen::default();
    let one = screen.view(&labels(Some(account())), false, true);
    assert_eq!(one.status.len(), 1, "{:?}", one.status);
    let two = screen.view(&labels(None), false, true);
    assert_eq!(two.status.len(), NOTE_LINES);
    assert_eq!(labels_face(&one, pal).h, labels_face(&two, pal).h);
}

fn account() -> Account {
    Account {
        user: "Zero".into(),
        password: "pw".into(),
    }
}

/// Every note Labels says fits its panel whole; a cart's title, which can be any length, is
/// cut short with an ellipsis rather than running off the panel.
#[test]
fn every_labels_note_fits_and_a_long_title_is_cut_short() {
    let room = 280 - 12 - NOTE_X;
    let mut lines = Vec::new();
    lines.extend(labels(None).status_lines(true));
    lines.extend(labels(Some(account())).status_lines(false));
    lines.extend(labels(Some(account())).status_lines(true));
    lines.extend(
        Labels::new(Box::new(omega_zero::label_sync::NoKey::default()), None).status_lines(true),
    );
    for fail in [
        SyncFail::Refused(Who::Player),
        SyncFail::Refused(Who::Program),
        SyncFail::Spent,
        SyncFail::Down,
        SyncFail::Offline,
    ] {
        lines.extend(stood(LabelNews::Failed(fail)).status_lines(true));
    }
    for tally in [
        Tally::default(),
        Tally {
            saved: 1,
            bare: 1,
            unknown: 1,
        },
        Tally {
            saved: 12,
            bare: 3,
            unknown: 0,
        },
        Tally {
            saved: 2,
            bare: 0,
            unknown: 9,
        },
    ] {
        lines.extend(stood(LabelNews::Finished(tally)).status_lines(true));
    }
    for line in &lines {
        assert_eq!(&fitted(line, room), line, "{line:?} is cut short");
    }
    let long = "Looking up 2 of 3: The Legend of Zelda: The Minish Cap";
    let cut = fitted(long, room);
    assert!(cut.ends_with('…'), "{cut:?}");
    assert!(text_width(&cut) <= room, "{cut:?} runs off the panel");
}

/// A ScreenScraper that has said `news` and nothing since.
struct Told(Vec<LabelNews>);

impl LabelJobs for Told {
    fn present(&self) -> bool {
        true
    }

    fn ask(&mut self, _job: LabelJob) {}

    fn news(&mut self) -> Vec<LabelNews> {
        std::mem::take(&mut self.0)
    }
}

/// Labels, signed in, once ScreenScraper has said `news`.
fn stood(news: LabelNews) -> Labels {
    let mut l = Labels::new(Box::new(Told(vec![news])), Some(account()));
    l.tick();
    l
}

/// Labels' keys beyond B and A go to the bottom bar: Y signs out on the account's row, START
/// fetches from either row once there is a network, and stops a fetch going.
#[test]
fn labels_hints_say_what_y_and_start_do() {
    let words = |l: &Labels, screen: &LabelsScreen, online| {
        labels_hints(&screen.view(l, online, true).legend)
            .iter()
            .map(|h| format!("{} {}", h.key, h.word))
            .collect::<Vec<_>>()
    };
    let mut screen = LabelsScreen::default();
    assert!(words(&labels(None), &screen, true).is_empty());
    let mut signed = labels(Some(account()));
    assert_eq!(words(&signed, &screen, false), ["Y SIGN OUT"]);
    assert_eq!(words(&signed, &screen, true), ["Y SIGN OUT", "START FETCH"]);
    screen.input(&mut signed, Action::GbaDown(Btn::Down), None);
    assert_eq!(words(&signed, &screen, true), ["START FETCH"]);
    assert_eq!(
        words(
            &stood(LabelNews::Working {
                done: 1,
                total: 3,
                title: "Fusion".into(),
            }),
            &screen,
            true
        ),
        ["START STOP"]
    );
}

#[test]
fn start_fetches_from_either_row() {
    use omega_zero::label_screen::Outcome;
    let mut screen = LabelsScreen::default();
    let mut signed = labels(Some(account()));
    let start = Action::GbaDown(Btn::Start);
    assert_eq!(screen.input(&mut signed, start, None), Outcome::Sync);
    screen.input(&mut signed, Action::GbaDown(Btn::Down), None);
    assert_eq!(screen.input(&mut signed, start, None), Outcome::Sync);
    let mut out = labels(None);
    assert_eq!(screen.input(&mut out, start, None), Outcome::Stay);
}

/// A row a style has no use for stays on the panel, faint, so the panel is as tall for every
/// style: four rows, BACKGROUND, MOTION, COLOUR and FONT, under the title THEME.
#[test]
fn the_theme_panel_keeps_its_four_rows() {
    let view = BackgroundScreen::default().view(
        Backdrop {
            style: BackdropStyle::Solid,
            moving: true,
            colour: 0,
        },
        Font::Thin,
    );
    assert_eq!(view.rows.len(), 4);
    assert!(view.rows[1].dim, "Solid does not move");
    assert!(!view.rows[3].dim, "the font is faint");
    assert_eq!(background_face(&view, palette(0)).h, panel_height(4 * 13));
    assert_eq!(omega_zero::screens::background::TITLE, "THEME");
    assert!(view.status.is_empty(), "a note under the rows");
    assert_eq!(view.legend, [LegendHint::Back, LegendHint::Change]);
}

/// What the panel is drawn in follows the colour picked: the same panel in another colour
/// differs, and is the new palette's.
#[test]
fn a_new_colour_re_tints_the_background_panel() {
    let (night, in_amber_pal) = (palette(0), palette(amber()));
    let in_night = background_face(&background_view(0), night);
    let in_amber = background_face(&background_view(amber()), in_amber_pal);
    assert_ne!(in_night.rgba, in_amber.rgba);
    let tab = |face: &UndoFace, pal: &Palette| {
        let i = ((5 * face.w + 4) * 4) as usize;
        assert_eq!(&face.rgba[i..i + 3], &pal.tab, "the title tab");
    };
    tab(&in_night, night);
    tab(&in_amber, in_amber_pal);
}

/// Each lone panel stands straight on the backdrop, nothing laid between them as a veil, and
/// slides in from the left with its screen, CONTROLS as the panels under Settings, while the
/// backdrop stays where it is.
#[test]
fn the_lone_panels_stand_on_the_backdrop_and_slide_with_their_screens() {
    let pattern = TexId::from_raw(700);
    let panel = |n| Lone {
        tex: TexId::from_raw(n),
        w: 200,
        h: 62,
    };
    // Settings' own row, or SHELF's
    let opened: [(
        Option<Result<SettingsRow, omega_zero::options::OptionRow>>,
        usize,
    ); 4] = [
        (None, 801),
        (Some(Err(omega_zero::options::OptionRow::Theme)), 802),
        (Some(Ok(SettingsRow::DateTime)), 803),
        (Some(Err(omega_zero::options::OptionRow::Labels)), 804),
    ];
    for (row, n) in opened {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        let state = AppState {
            clock_set: true,
            ..AppState::default()
        };
        write_app_state(d.path(), &state).unwrap();
        let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
        a.tick_ms(16);
        a.set_backdrop_face(pattern, out_w() + 40, BackdropStyle::Grid, 0);
        a.set_controls_panel(panel(801));
        a.set_background_panel(panel(802));
        a.set_clock_panel(panel(803));
        a.set_labels_panel(panel(804));
        match row {
            None => a.apply(Action::SelectTap),
            Some(Ok(row)) => {
                a.apply(Action::QuickMenu);
                a.finish_slide();
                while a.settings_row() != Some(row) {
                    press(&mut a, Btn::Down);
                }
                press(&mut a, Btn::A);
            }
            Some(Err(row)) => {
                a.apply(Action::QuickMenu);
                a.finish_slide();
                crate::common::to_shelf_row(&mut a, row);
                press(&mut a, Btn::A);
            }
        }
        let home = screen_px(panel(n).rect()[0]);
        let started = a.slide().expect("no slide").started;
        a.tick_ms(started + SLIDE_MS / 2);
        let p = progress(started, a.now());
        let dx = offsets(Dir::Deeper, End::Arriving, p, out_w() as f32).dx;
        let (_, x) = drawn_at(&frame(&a), TexId::from_raw(n)).expect("the panel is not drawn");
        assert_eq!(
            x,
            home + dx,
            "{row:?}: the panel does not slide with its screen"
        );

        a.finish_slide();
        let out = frame(&a);
        let (shown, x) = drawn_at(&out, TexId::from_raw(n)).expect("the panel at rest");
        assert_eq!(x, home, "{row:?}");
        let (under, _) = drawn_at(&out, pattern).expect("no backdrop");
        assert!(under < shown);
        let veiled = out[under + 1..shown].iter().any(
            |d| matches!(*d, Draw::Rect { w, h, .. } if w == out_w() as f32 && h == out_h() as f32),
        );
        assert!(
            !veiled,
            "{row:?}: a veil between the backdrop and the panel"
        );
    }
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn frame(a: &App) -> Vec<Draw> {
    let mut out = Vec::new();
    a.draw(&mut out);
    out
}

/// Where `tex` is first drawn across, and its place in the frame.
fn drawn_at(out: &[Draw], tex: TexId) -> Option<(usize, f32)> {
    out.iter().enumerate().find_map(|(i, d)| match *d {
        Draw::Tex { x, tex: t, .. } if t == tex => Some((i, x)),
        _ => None,
    })
}
