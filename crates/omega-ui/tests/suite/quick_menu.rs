//! Settings (settings_menu.rs).

use omega_gfx::{out_h, out_w};
use omega_store::{parse_stamp, ScreenFilter, FF_SPEEDS};
use omega_ui::kit::layout::modal_rect;
use omega_ui::kit::modal::{modal_face, CUT};
use omega_ui::kit::palette::{palette, Palette};
use omega_ui::kit::row::ROW_H;
use omega_ui::{
    date_time_text, settings_frame, settings_panels, settings_row_face, settings_row_rects,
    settings_row_width, settings_title_face, ClockPicker, QuickValue, RowShown, SettingsBaked,
    SettingsGroup, SettingsRow, UndoFace,
};

const NIGHT: u8 = 0;
const AMBER: u8 = 7;

/// `face`'s pixel at (`x`, `y`): its colour, or None where it is clear.
fn px(face: &UndoFace, x: u32, y: u32) -> Option<[u8; 3]> {
    let i = ((y * face.w + x) * 4) as usize;
    (face.rgba[i + 3] != 0).then(|| [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]])
}

/// The rows as flow.html's Settings shows them, with `lit` under the cursor.
fn flow_rows(lit: SettingsRow) -> Vec<RowShown> {
    let value = |row| match row {
        SettingsRow::Update => Some("V2.7.3"),
        SettingsRow::DateTime => Some("OCT 10 16:00"),
        SettingsRow::Network => Some("HOME"),
        SettingsRow::Bluetooth => Some("8BITDO SN30 PRO"),
        _ => None,
    };
    SettingsRow::ALL
        .into_iter()
        .map(|row| RowShown {
            row,
            value: value(row).map(str::to_string),
            lit: row == lit,
        })
        .collect()
}

/// `top` laid over `under` with its top left at (`x`, `y`), where it is not clear.
fn over(under: &mut UndoFace, top: &UndoFace, x: u32, y: u32) {
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

/// Settings' modal as the screen shows it, at art resolution and `modal_rect()`'s size, with
/// nothing under it: the frame, the lit panel's title and every row, each where `SettingsMenu`
/// draws it.
fn composed(shown: &[RowShown], pal: &Palette) -> UndoFace {
    let rows: Vec<SettingsRow> = shown.iter().map(|s| s.row).collect();
    let frame = settings_frame(omega_ui::top_in(&rows), pal);
    let [mx, my] = frame.at;
    let mut modal = UndoFace {
        rgba: frame.face.rgba.clone(),
        w: frame.face.w,
        h: frame.face.h,
    };
    let lit = shown.iter().find(|s| s.lit).map(|s| s.row.group());
    if let Some(g) = lit {
        let [tx, ty, ..] = frame.titles[g.index()];
        over(&mut modal, &settings_title_face(g, pal), tx - mx, ty - my);
    }
    for (row, [x, y, w, _]) in settings_row_rects(&rows, frame.wells) {
        let s = shown.iter().find(|s| s.row == row).expect("a row shown");
        over(&mut modal, &settings_row_face(w, s, pal), x - mx, y - my);
    }
    modal
}

fn read_png(path: &str) -> UndoFace {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().expect("a PNG");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("its pixels");
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type.samples() {
        4 => buf,
        _ => buf
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
    };
    UndoFace {
        rgba,
        w: info.width,
        h: info.height,
    }
}

fn write(path: &str, face: &UndoFace) {
    let f = std::fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), face.w, face.h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .unwrap()
        .write_image_data(&face.rgba)
        .unwrap();
}

#[test]
fn general_is_as_tall_as_its_rows_and_configs_takes_the_rest() {
    // flow.html's modal at 640x480: 288 by 200
    let [general, configs] = settings_panels(288, 200, 5);
    assert_eq!(general, [4, 2, 280, 88], "five rows and a panel's chrome");
    assert_eq!(
        configs,
        [4, 89, 280, 107],
        "it shares GENERAL's last row, its stripe"
    );
    assert_eq!(
        configs[1] + configs[3],
        200 - CUT,
        "CONFIGS stops at the frame"
    );
    for size in [(328, 200), (328, 320)] {
        let [g, c] = settings_panels(size.0, size.1, 5);
        assert_eq!(g[3], 88, "{size:?}: GENERAL grew with the modal");
        assert_eq!(c[1] + c[3], size.1 - CUT, "{size:?}");
    }
}

/// The rows a handheld lists, with Bluetooth or without.
fn listed(bluetooth: bool) -> Vec<SettingsRow> {
    SettingsRow::ALL
        .into_iter()
        .filter(|r| bluetooth || *r != SettingsRow::Bluetooth)
        .collect()
}

#[test]
fn every_row_stands_in_its_own_panels_well() {
    for rows in [listed(true), listed(false)] {
        let frame = settings_frame(omega_ui::top_in(&rows), palette(NIGHT));
        for (row, [x, y, w, h]) in settings_row_rects(&rows, frame.wells) {
            let [wx, wy, ww, wh] = frame.wells[row.group().index()];
            assert_eq!((x, w, h), (wx - 1, ww + 2, ROW_H), "{row:?}");
            assert!(y >= wy && y + h <= wy + wh, "{row:?} is out of its well");
        }
    }
    let configs = |rows: &[SettingsRow]| {
        rows.iter()
            .filter(|r| r.group() == SettingsGroup::Configs)
            .count() as u32
    };
    // CONFIGS has room for every row under GENERAL holding all of its own
    let room = omega_ui::room_below(omega_ui::top_in(&listed(true)));
    assert!(
        configs(&listed(true)) <= room,
        "CONFIGS has no room for Saves"
    );
    // as tall as its rows and no taller, in the middle of the room between the bars
    let [_, y, _, h] = omega_ui::settings_rect(omega_ui::top_in(&listed(true)));
    let [_, my, _, mh] = modal_rect();
    assert!(
        y >= my && y + h <= my + mh,
        "the modal left the room between the bars"
    );
    assert!(
        (y - my).abs_diff(my + mh - (y + h)) <= 1,
        "the modal is not in the middle: {} above, {} below",
        y - my,
        my + mh - (y + h)
    );
    let frame = settings_frame(omega_ui::top_in(&listed(true)), palette(NIGHT));
    let rects = settings_row_rects(&listed(true), frame.wells);
    let at = |row: SettingsRow| rects.iter().find(|(r, _)| *r == row).unwrap().1;
    assert_eq!(
        at(SettingsRow::DateTime)[1],
        frame.wells[0][1],
        "GENERAL's first row is not at its well's top"
    );
    assert_eq!(
        at(SettingsRow::Display)[1],
        frame.wells[1][1],
        "CONFIGS' first row is not at its well's top"
    );
    assert_eq!(
        at(SettingsRow::About)[1] + ROW_H,
        frame.wells[0][1] + frame.wells[0][3],
        "GENERAL does not hug its rows"
    );
    let left = frame.wells[1][1] + frame.wells[1][3] - (at(SettingsRow::Backup)[1] + ROW_H);
    assert!(
        left < ROW_H,
        "CONFIGS holds {left} art pixels under Saves, room for another row"
    );
}

#[test]
fn without_a_radio_the_rows_after_network_close_up_and_general_with_them() {
    // with no Wi-Fi there is no Update to fetch over it either
    let rows: Vec<SettingsRow> = SettingsRow::ALL
        .into_iter()
        .filter(|r| !matches!(r, SettingsRow::Network | SettingsRow::Update))
        .collect();
    let frame = settings_frame(omega_ui::top_in(&rows), palette(NIGHT));
    let all = settings_frame(omega_ui::top_in(&SettingsRow::ALL), palette(NIGHT));
    assert_eq!(
        frame.wells[0][3] + 2 * ROW_H,
        all.wells[0][3],
        "GENERAL did not hug the three rows left"
    );
    let rects = settings_row_rects(&rows, frame.wells);
    let y = |row: SettingsRow| rects.iter().find(|(r, _)| *r == row).unwrap().1[1];
    assert_eq!(y(SettingsRow::Bluetooth), y(SettingsRow::DateTime) + ROW_H);
    assert_eq!(y(SettingsRow::About), y(SettingsRow::DateTime) + 2 * ROW_H);
    assert!(rects.iter().all(|(r, _)| *r != SettingsRow::Network));
}

#[test]
fn the_frame_is_the_modal_with_both_titles_dim() {
    let pal = palette(NIGHT);
    let frame = settings_frame(5, pal);
    let [_, _, mw, mh] = omega_ui::settings_rect(5);
    assert_eq!((frame.face.w, frame.face.h), (mw, mh));
    // the frame's corners and edges are the modal's own
    let modal = modal_face(mw, mh, pal);
    for (x, y) in [
        (0, 0),
        (3, 0),
        (0, 3),
        (1, 1),
        (mw - 1, mh - 1),
        (mw / 2, 0),
        (0, mh / 2),
    ] {
        assert_eq!(px(&frame.face, x, y), px(&modal, x, y), "({x}, {y})");
    }
    assert_eq!(px(&frame.face, 3, 0), Some(pal.dark), "the outline");
    assert_eq!(px(&frame.face, 1, 3), Some(pal.frame_hi), "the bright line");
    // both titles are dim, in the very pixels the lit ones ink
    let [mx, my] = frame.at;
    for g in SettingsGroup::ALL {
        let [tx, ty, tw, th] = frame.titles[g.index()];
        let lit = settings_title_face(g, pal);
        assert_eq!((lit.w, lit.h), (tw, th), "{g:?}'s title");
        let mut inks = 0;
        for y in 0..th {
            for x in 0..tw {
                if px(&lit, x, y).is_some() {
                    inks += 1;
                    let at = px(&frame.face, tx - mx + x, ty - my + y);
                    assert_eq!(at, Some(pal.dim), "{g:?} at ({x}, {y})");
                }
            }
        }
        assert!(inks > 20, "{g:?}'s title has {inks} pixels of ink");
        assert!(
            (0..th).all(|y| (0..tw).all(|x| px(&lit, x, y).is_none_or(|c| c == pal.cream))),
            "{g:?}'s lit title is not cream"
        );
    }
}

#[test]
fn the_lit_row_is_a_band_and_the_others_are_clear_round_their_words() {
    let pal = palette(NIGHT);
    let w = settings_row_width(268);
    let shown = |row, lit| RowShown {
        row,
        value: Some("6X".into()),
        lit,
    };
    let lit = settings_row_face(w, &shown(SettingsRow::Update, true), pal);
    assert_eq!((lit.w, lit.h), (w, ROW_H));
    // the band: a frame line with its corner pixels left out, round the tab colour
    assert_eq!(px(&lit, 0, 0), None, "the band's corner");
    assert_eq!(px(&lit, 1, 0), Some(pal.frame), "the band's top line");
    assert_eq!(px(&lit, 0, 1), Some(pal.frame), "the band's left line");
    assert_eq!(
        px(&lit, w - 1, ROW_H - 2),
        Some(pal.frame),
        "the band's right line"
    );
    assert_eq!(px(&lit, 2, 2), Some(pal.tab), "the band's fill");
    assert_eq!(px(&lit, w / 2, ROW_H - 2), Some(pal.tab), "the band's fill");
    let plain = settings_row_face(w, &shown(SettingsRow::Update, false), pal);
    for (x, y) in [(1, 0), (0, 1), (2, 2), (w / 2, ROW_H - 2)] {
        assert_eq!(
            px(&plain, x, y),
            None,
            "an unlit row has a band at ({x}, {y})"
        );
    }
    let colours = |f: &UndoFace| {
        let mut c: Vec<[u8; 3]> = (0..f.h)
            .flat_map(|y| (0..f.w).filter_map(move |x| px(f, x, y)))
            .collect();
        c.sort_unstable();
        c.dedup();
        c
    };
    assert_eq!(
        colours(&plain),
        vec![pal.dim, pal.label],
        "unlit: label and dim value"
    );
    let mut want = vec![pal.tab, pal.frame, pal.frame_hi, pal.cream];
    want.sort_unstable();
    assert_eq!(colours(&lit), want, "lit: band, cream label, bright value");
}

#[test]
fn the_rows_left_and_right_change_step_between_arrows_and_the_rows_that_open_end_in_one() {
    let carets: Vec<SettingsRow> = SettingsRow::ALL
        .into_iter()
        .filter(|r| r.carets())
        .collect();
    // Update's arrows switch between the releases and the test builds; the rest open panels
    assert_eq!(carets, [SettingsRow::Update]);
    let pal = palette(NIGHT);
    let w = settings_row_width(268);
    let face = |row, lit| {
        settings_row_face(
            w,
            &RowShown {
                row,
                value: None,
                lit,
            },
            pal,
        )
    };
    // a › stands at the right end of every row that opens, lit or not, inside the band's
    // line and its room
    for row in SettingsRow::ALL.into_iter().filter(|r| r.opens()) {
        for lit in [false, true] {
            let f = face(row, lit);
            let value = if lit { pal.frame_hi } else { pal.dim };
            let last = (0..f.w)
                .filter(|&x| (0..f.h).any(|y| px(&f, x, y) == Some(value)))
                .max();
            assert_eq!(last, Some(w - 1 - 5), "{row:?}'s › (lit {lit})");
        }
    }
}

/// flow.html's Settings, screenshotted by settings/make-refs.sh: the modal at one pixel to an
/// art pixel, at each canvas, in Night with the cursor on Fast Forward and in Amber with it on
/// Date & Time.
const FLOW_REFS: [(&str, u8, SettingsRow); 5] = [
    ("640x480-night-0", NIGHT, SettingsRow::Display),
    ("720x480-night-0", NIGHT, SettingsRow::Display),
    ("720x720-night-0", NIGHT, SettingsRow::Display),
    ("640x480-amber-5", AMBER, SettingsRow::DateTime),
    ("720x480-amber-5", AMBER, SettingsRow::DateTime),
];

#[test]
#[ignore = "flow.html's screenshots hold the CONFIGS rows from before GAMEPLAY and SHELF"]
fn settings_is_flows_pixel_for_pixel() {
    let canvas = format!("{}x{}", out_w(), out_h());
    for (name, colour, lit) in FLOW_REFS.into_iter().filter(|r| r.0.starts_with(&canvas)) {
        let ours = composed(&flow_rows(lit), palette(colour));
        if let Ok(dir) = std::env::var("SETTINGS_OUT") {
            write(&format!("{dir}/settings-{name}.png"), &ours);
        }
        let path = format!(
            "{}/tests/suite/settings/ref/settings-{name}.png",
            env!("CARGO_MANIFEST_DIR")
        );
        let shot = read_png(&path);
        assert_eq!((ours.w, ours.h), (shot.w, shot.h), "{name}'s size");
        let off: Vec<_> = (0..ours.h)
            .flat_map(|y| (0..ours.w).map(move |x| (x, y)))
            .filter_map(|(x, y)| {
                let c = px(&ours, x, y)?;
                let want = px(&shot, x, y).expect("an opaque screenshot");
                (c != want).then_some((x, y, c, want))
            })
            .collect();
        assert!(
            off.is_empty(),
            "{name}: {} art pixels differ from flow.html's, first (x, y, ours, Chrome's) {:?}",
            off.len(),
            &off[..off.len().min(8)]
        );
    }
}

#[test]
fn a_sync_bakes_the_frame_once_and_a_row_only_when_it_changes() {
    let mut baked = SettingsBaked::default();
    let rows = flow_rows(SettingsRow::Display);
    let first = baked.rebake(NIGHT, &rows);
    assert!(first.frame, "the first sync baked no frame");
    assert_eq!(first.rows, rows, "the first sync left rows unbaked");

    let again = baked.rebake(NIGHT, &rows);
    assert!(
        !again.frame && again.rows.is_empty() && again.gone.is_empty(),
        "{again:?}"
    );

    // a value changes: that row alone
    let mut faster = rows.clone();
    faster[0].value = Some("4X".into());
    let plan = baked.rebake(NIGHT, &faster);
    assert!(!plan.frame);
    assert_eq!(plan.rows, vec![faster[0].clone()]);

    // the clock's minute: Date & Time alone
    let mut later = faster.clone();
    let date = later.iter_mut().find(|r| r.row == SettingsRow::DateTime);
    date.expect("Date & Time is shown").value = Some("OCT 10 16:01".into());
    let plan = baked.rebake(NIGHT, &later);
    let rebaked: Vec<SettingsRow> = plan.rows.iter().map(|s| s.row).collect();
    assert_eq!(rebaked, [SettingsRow::DateTime]);

    // the cursor moves: the row it leaves and the row it lands on
    let mut moved = later.clone();
    for s in &mut moved {
        s.lit = s.row == SettingsRow::Gameplay;
    }
    let plan = baked.rebake(NIGHT, &moved);
    let rebaked: Vec<SettingsRow> = plan.rows.iter().map(|s| s.row).collect();
    assert_eq!(rebaked, [SettingsRow::Display, SettingsRow::Gameplay]);
    assert!(!plan.frame, "moving the cursor baked the frame");

    // a new palette: everything
    let plan = baked.rebake(AMBER, &moved);
    assert!(plan.frame);
    assert_eq!(plan.rows.len(), moved.len());

    // the radio goes: GENERAL closes up round the rows left, so the frame and they are baked
    let radioless: Vec<RowShown> = moved
        .iter()
        .filter(|s| s.row != SettingsRow::Network)
        .cloned()
        .collect();
    let plan = baked.rebake(AMBER, &radioless);
    assert!(plan.frame, "{plan:?}");
    assert_eq!(plan.rows.len(), radioless.len());
}

#[test]
fn the_palette_tints_every_face() {
    let rows = flow_rows(SettingsRow::Display);
    for colour in [NIGHT, AMBER] {
        let pal = palette(colour);
        let frame = settings_frame(5, pal);
        assert_eq!(px(&frame.face, 3, 0), Some(pal.dark), "colour {colour}");
        assert_eq!(px(&frame.face, 3, 40), Some(pal.frame), "colour {colour}");
        let lit = rows.iter().find(|r| r.lit).unwrap();
        let lit = settings_row_face(settings_row_width(268), lit, pal);
        assert_eq!(px(&lit, 2, 2), Some(pal.tab), "colour {colour}");
    }
    assert_ne!(palette(NIGHT).frame, palette(AMBER).frame);
    let amber = palette(AMBER).frame;
    assert!(amber[0] > amber[2], "amber's frame is not warm: {amber:?}");
}

#[test]
fn the_fast_forward_row_offers_the_four_ceilings_the_card_can_hold() {
    assert_eq!(FF_SPEEDS, [0, 2, 3, 4, 6]);
    assert_eq!(
        FF_SPEEDS.map(QuickValue::speed),
        [
            Some(QuickValue::Off),
            Some(QuickValue::Speed2),
            Some(QuickValue::Speed3),
            Some(QuickValue::Speed4),
            Some(QuickValue::Speed6),
        ]
    );
    for other in [1, 5, 7, 9, 16, 28, 255] {
        assert_eq!(
            QuickValue::speed(other),
            None,
            "{other}x is not a value the row has"
        );
    }
}

#[test]
fn the_rows_run_in_the_order_the_user_chose() {
    let labels = SettingsRow::ALL.map(SettingsRow::label);
    assert_eq!(
        labels,
        [
            "Date & Time",
            "Network",
            "Bluetooth",
            "Update",
            "About",
            "Display",
            "Gameplay",
            "Shelf",
            "RetroAchievements",
            "Saves"
        ]
    );
    let opens: Vec<SettingsRow> = SettingsRow::ALL.into_iter().filter(|r| r.opens()).collect();
    assert_eq!(
        opens,
        [
            SettingsRow::DateTime,
            SettingsRow::Network,
            SettingsRow::Bluetooth,
            SettingsRow::About,
            SettingsRow::Display,
            SettingsRow::Gameplay,
            SettingsRow::Shelf,
            SettingsRow::Achievements,
            SettingsRow::Backup
        ]
    );
}

#[test]
fn the_handheld_itself_comes_first_and_the_configs_after() {
    let groups = SettingsRow::ALL.map(SettingsRow::group);
    let general = groups
        .iter()
        .take_while(|g| **g == SettingsGroup::General)
        .count();
    assert_eq!(general, 5, "{groups:?}");
    assert!(
        groups[general..]
            .iter()
            .all(|g| *g == SettingsGroup::Configs),
        "{groups:?}"
    );
    assert_eq!(
        SettingsGroup::ALL.map(SettingsGroup::label),
        ["General", "Configs"]
    );
    assert_eq!(
        SettingsGroup::ALL.map(SettingsGroup::title),
        ["GENERAL", "CONFIGS"]
    );
}

#[test]
fn a_hidden_row_is_stepped_over_both_ways() {
    let rows: Vec<SettingsRow> = SettingsRow::ALL
        .into_iter()
        .filter(|r| *r != SettingsRow::Network)
        .collect();
    assert_eq!(SettingsRow::DateTime.down_in(&rows), SettingsRow::Bluetooth);
    assert_eq!(SettingsRow::Bluetooth.up_in(&rows), SettingsRow::DateTime);
    assert_eq!(SettingsRow::About.down_in(&rows), SettingsRow::Display);
    assert_eq!(SettingsRow::Backup.down_in(&rows), SettingsRow::DateTime);
    assert_eq!(SettingsRow::DateTime.down(), SettingsRow::Network);
}

#[test]
fn the_values_read_as_flow_prints_them() {
    assert_eq!(
        QuickValue::ALL.map(QuickValue::text),
        ["2X", "3X", "4X", "6X", "ON", "OFF", "LCD", "GRID"]
    );
    assert_eq!(QuickValue::filter(ScreenFilter::Lcd), QuickValue::Lcd);
    assert_eq!(QuickValue::filter(ScreenFilter::Grid), QuickValue::Grid);
    assert_eq!(QuickValue::filter(ScreenFilter::Off), QuickValue::Off);
    assert!(SettingsRow::Update.carets());
    assert!(!SettingsRow::Update.opens());
    assert!(SettingsRow::Display.opens() && !SettingsRow::Display.carets());
    assert_eq!(QuickValue::flag(true), QuickValue::On);
    assert_eq!(QuickValue::flag(false), QuickValue::Off);
}

#[test]
fn the_date_and_time_read_as_a_month_a_day_and_the_carousels_24_hour_clock() {
    let at = |stamp: &str| date_time_text(parse_stamp(stamp).expect("a stamp"));
    assert_eq!(at("2026-09-15_16-35-00"), "SEP 15 16:35");
    assert_eq!(at("2027-01-05_04-07-59"), "JAN 5 04:07");
    assert_eq!(
        at("2026-10-10_16-00-00"),
        "OCT 10 16:00",
        "flow.html's date"
    );
}

#[test]
fn a_picker_for_a_set_clock_starts_at_the_local_time_and_its_offset() {
    let utc = parse_stamp("2026-09-15_21-35-42").expect("a stamp");
    let p = ClockPicker::local(utc, -300);
    assert_eq!(p.offset_min(), -300);
    assert_eq!(
        p.secs(),
        utc - 42,
        "the picker does not show the minute it was opened in"
    );
    assert!(p.text().starts_with("2026-09-15 16:35"), "{}", p.text());
}
