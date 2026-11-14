//! The kit's faces against the components they port: each is held to a screenshot of the HTML
//! kit in Chrome (kit/ref/, made by kit/make-refs.sh from kit/kit.html), pixel for pixel.
//!
//! The screenshots are drawn in palette.js's RAMP as written, which `RAMP` below is too, so a
//! face and its screenshot share their colours whatever the palette makes of a backdrop colour.
//! Set KIT_OUT to a folder to have each face written there as a PNG beside its reference.
//!
//! Text: kit.html turns macOS's font smoothing off (`-webkit-font-smoothing: antialiased`),
//! which otherwise rings every DOM glyph with a faint halo a canvas word never gets, and every
//! component draws with `text-rendering: geometricPrecision`, which flow.html's notes and
//! columns inherit from their panel. So set, Pixel Code at 9 px lands on whole pixels, fully
//! inked or clear, and Chrome's ink is exactly the full face's as fontdue rasterizes it, a
//! pixel inked where its coverage is at least half: the web font is the same outlines cut to
//! Latin-1, so é and × match too. flow.html's ◀ ▶ ▲ ▼, which the cut face has not, are
//! flow/art.js's sprites, copied from the full face and the same pixels as fontdue's.

use omega_ui::kit::action::{
    action_face, action_pair, action_width, ActionFace, Font, Side, BAR, RISE, TALL,
};
use omega_ui::kit::bar::{
    action_swap, bar_layout, cap_width, hint_width, hints_face, hints_width, BarLayout, Hint, Swap,
    KEY_H, SINK,
};
use omega_ui::kit::list_row::{list_row_face, DeviceKind, ListRow, DEVICE_W};
use omega_ui::kit::modal::{modal_face, modal_layout};
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::{panel_face, tab_width, title_face, PanelFace, TAB_W};
use omega_ui::kit::pixel::{
    big_word, flip, keyed, letters, letters_over, mirror, quad, ACCENT, ADVANCE, BAR_ROWS, CAP,
};
use omega_ui::kit::row::{panel_height, row_face, row_rect, row_width, Row, ROW_H};
use omega_ui::kit::text::text_face;
use omega_ui::{Draw, TexId, UndoFace};

pub(crate) const RAMP: Palette = Palette {
    bg: [0x30, 0x22, 0x08],
    dark: [0x2a, 0x0f, 0x04],
    edge: [0x6a, 0x3d, 0x01],
    well: [0x4f, 0x30, 0x0d],
    plate: [0x76, 0x50, 0x20],
    tab: [0x84, 0x5a, 0x1a],
    band: [0xa5, 0x72, 0x21],
    stripe: [0xb0, 0x73, 0x1b],
    dim: [0xbe, 0x97, 0x32],
    frame: [0xe6, 0xab, 0x47],
    frame_hi: [0xf8, 0xb8, 0x3f],
    label: [0xf2, 0xc8, 0x7d],
    cream: [0xf5, 0xe5, 0xbb],
};

fn reference(name: &str) -> UndoFace {
    let path = format!(
        "{}/tests/suite/kit/ref/{name}.png",
        env!("CARGO_MANIFEST_DIR")
    );
    let file = std::fs::File::open(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().expect("a PNG");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("its pixels");
    buf.truncate(info.buffer_size());
    // Chrome writes a screenshot with nothing clear in it, such as the link port's strip, as RGB
    if info.color_type == png::ColorType::Rgb {
        buf = buf
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect();
    }
    assert_eq!(
        buf.len(),
        (info.width * info.height * 4) as usize,
        "{name} is not RGBA"
    );
    UndoFace {
        rgba: buf,
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

/// `face` is `name`'s screenshot exactly: the same size, clear where it is clear, and every
/// other pixel the same opaque colour.
pub(crate) fn matches(face: &UndoFace, name: &str) {
    if let Ok(dir) = std::env::var("KIT_OUT") {
        write(&format!("{dir}/{name}.png"), face);
    }
    let shot = reference(name);
    assert_eq!((face.w, face.h), (shot.w, shot.h), "{name}'s size");
    let pixel = |f: &UndoFace, i: usize| match f.rgba[i * 4 + 3] {
        0 => [0; 4],
        _ => [
            f.rgba[i * 4],
            f.rgba[i * 4 + 1],
            f.rgba[i * 4 + 2],
            f.rgba[i * 4 + 3],
        ],
    };
    let off: Vec<_> = (0..(face.w * face.h) as usize)
        .filter(|&i| pixel(face, i) != pixel(&shot, i))
        .map(|i| {
            (
                i as u32 % face.w,
                i as u32 / face.w,
                pixel(face, i),
                pixel(&shot, i),
            )
        })
        .collect();
    assert!(
        off.is_empty(),
        "{name}: {} pixels differ from Chrome's, first (x, y, ours, Chrome's) {:?}",
        off.len(),
        &off[..off.len().min(8)]
    );
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

#[test]
fn the_modal_is_chromes() {
    matches(&modal_face(200, 120, &RAMP), "modal");
}

/// A modal holding BUTTON SETTING, active, over ATTACK MODE, sharing its height by `weights`.
fn modal_with_panels(w: u32, h: u32, weights: &[u32]) -> UndoFace {
    let mut face = modal_face(w, h, &RAMP);
    let titles = [("BUTTON SETTING", true), ("ATTACK MODE", false)];
    for ([x, y, pw, ph], (title, active)) in modal_layout(w, h, weights).into_iter().zip(titles) {
        over(
            &mut face,
            &panel_face(pw, ph, title, active, &RAMP).face,
            x,
            y,
        );
    }
    face
}

#[test]
fn panels_in_a_modal_are_chromes_even_off_a_whole_pixel() {
    matches(&modal_with_panels(200, 121, &[1, 1]), "modal-panels");
    matches(&modal_with_panels(200, 120, &[4, 3]), "modal-panels-4-3");
    matches(&modal_with_panels(200, 120, &[1, 1]), "modal-panels-1-1");
}

#[test]
fn panels_share_a_modal_and_one_stripe() {
    assert_eq!(
        modal_layout(200, 121, &[1, 1]),
        vec![[4, 2, 192, 58], [4, 59, 192, 58]]
    );
    // 57.5 pixels each: the edge between them rounds down the page
    assert_eq!(
        modal_layout(200, 120, &[1, 1]),
        vec![[4, 2, 192, 58], [4, 59, 192, 57]]
    );
    assert_eq!(
        modal_layout(200, 120, &[4, 3]),
        vec![[4, 2, 192, 66], [4, 67, 192, 49]]
    );
    assert_eq!(modal_layout(200, 120, &[1]), vec![[4, 2, 192, 114]]);
}

#[test]
fn panels_are_chromes() {
    for (title, name) in [("BUTTON SETTING", "button-setting"), ("CONFIGS", "configs")] {
        matches(
            &panel_face(160, 60, title, true, &RAMP).face,
            &format!("panel-{name}-active"),
        );
        matches(
            &panel_face(160, 60, title, false, &RAMP).face,
            &format!("panel-{name}"),
        );
    }
}

#[test]
fn a_panels_well_and_title_are_where_its_css_puts_them() {
    let panel = panel_face(160, 60, "CONFIGS", true, &RAMP);
    assert_eq!(panel.well, [6, 18, 148, 37]);
    assert_eq!(panel.title, [6, 2, 42, 11]);
}

#[test]
fn the_active_title_over_a_dim_one_is_the_active_panel() {
    let mut dim = panel_face(160, 60, "BUTTON SETTING", false, &RAMP);
    let [x, y, w, h] = dim.title;
    let light = title_face("BUTTON SETTING", true, &RAMP);
    assert_eq!((light.w, light.h), (w, h));
    over(&mut dim.face, &light, x, y);
    matches(&dim.face, "panel-button-setting-active");
}

#[test]
fn a_tab_grows_to_a_long_title_and_no_narrower() {
    assert_eq!(tab_width("BUTTON SETTING"), TAB_W);
    let long = "A TITLE FAR TOO LONG FOR ITS TAB";
    assert_eq!(tab_width(long), 1 + 4 + long.len() as u32 * 6 + 4);
}

#[test]
fn actions_are_chromes() {
    let cases = [
        (Side::Left, "CONTROLS", "SELECT", "action-left-controls"),
        (Side::Right, "SETTINGS", "START", "action-right-settings"),
        (Side::Left, "RETURN", "B", "action-left-return"),
        (Side::Right, "CONFIRM", "A", "action-right-confirm"),
        (Side::Left, "", "", "action-left-empty"),
        (Side::Right, "", "", "action-right-empty"),
    ];
    for (side, word, pill, name) in cases {
        let action = action_face(side, word, pill, Font::Bold, &RAMP);
        assert_eq!(action.rise, RISE);
        matches(&action.face, name);
    }
}

/// A bar's two actions made together in bold, as omega-bar gives them their room: `left` and
/// `right` as (word, pill).
fn pair(left: (&str, &str), right: (&str, &str)) -> [ActionFace; 2] {
    action_pair(left, right, Font::Bold, &RAMP)
}

/// The same in the thin font.
fn thin_pair(left: (&str, &str), right: (&str, &str)) -> [ActionFace; 2] {
    action_pair(left, right, Font::Thin, &RAMP)
}

/// An action given the room of the one beside it is as wide as that one, its word and its hint
/// in the middle of their boxes, as action-left.js lays it out with `room`.
#[test]
fn paired_actions_are_chromes() {
    let [controls, settings] = pair(("CONTROLS", "SELECT"), ("SETTINGS", "START"));
    matches(&controls.face, "action-left-controls");
    matches(&settings.face, "action-right-settings-paired");
    let [back, confirm] = pair(("RETURN", "B"), ("CONFIRM", "A"));
    matches(&back.face, "action-left-return-paired");
    matches(&confirm.face, "action-right-confirm");
}

/// In the thin font each pair is one width too, its words in Pixel Code's own strokes.
#[test]
fn thin_actions_are_chromes() {
    let [controls, settings] = thin_pair(("CONTROLS", "SELECT"), ("SETTINGS", "START"));
    matches(&controls.face, "action-left-controls-thin");
    matches(&settings.face, "action-right-settings-thin");
    let [back, confirm] = thin_pair(("RETURN", "B"), ("CONFIRM", "A"));
    matches(&back.face, "action-left-return-thin");
    matches(&confirm.face, "action-right-confirm-thin");
}

#[test]
fn actions_are_as_wide_as_their_text() {
    let width = |word, pill| action_width(word, pill, Font::Bold);
    assert_eq!(width("SETTINGS", "START"), 109);
    assert_eq!(width("CONTROLS", "SELECT"), 115);
    assert_eq!(width("RETURN", "B"), 90);
    assert_eq!(width("CONFIRM", "A"), 97);
    assert_eq!(width("", ""), 75, "room is kept for text to come");
    // thin, a pixel a letter narrower
    assert_eq!(action_width("SETTINGS", "START", Font::Thin), 101);
    assert_eq!(action_width("", "", Font::Thin), 75);
    let face = action_face(Side::Left, "SETTINGS", "START", Font::Bold, &RAMP).face;
    assert_eq!((face.w, face.h), (109, TALL));
    assert_eq!(TALL, BAR + RISE);
    assert_eq!(TALL, 19);
}

/// A bar's two actions are one width, the wider one's: the shelf's as wide as CONTROLS SELECT,
/// a menu's as CONFIRM A, and the first-boot clock's empty one as CONFIRM A beside it.
#[test]
fn a_bars_two_actions_are_one_width() {
    let widths = |[left, right]: [ActionFace; 2]| {
        assert_eq!((left.face.h, right.face.h), (TALL, TALL));
        [left.face.w, right.face.w]
    };
    let shelf = pair(("CONTROLS", "SELECT"), ("SETTINGS", "START"));
    assert_eq!(widths(shelf), [115, 115]);
    assert_eq!(widths(pair(("RETURN", "B"), ("CONFIRM", "A"))), [97, 97]);
    assert_eq!(widths(pair(("", ""), ("CONFIRM", "A"))), [97, 97]);
    assert_eq!(widths(pair(("", ""), ("", ""))), [75, 75]);
    let shelf = thin_pair(("CONTROLS", "SELECT"), ("SETTINGS", "START"));
    assert_eq!(widths(shelf), [107, 107]);
    assert_eq!(
        widths(thin_pair(("RETURN", "B"), ("CONFIRM", "A"))),
        [90, 90]
    );
    assert_eq!(widths(thin_pair(("", ""), ("CONFIRM", "A"))), [90, 90]);
}

#[test]
fn a_big_word_is_the_capitals_with_their_bars_doubled_and_a_shadow() {
    let word = big_word("SETTINGS", Font::Bold);
    let ink = letters("SETTINGS", ADVANCE + 1);
    assert_eq!(word.len(), CAP + BAR_ROWS.len() + 1);
    assert_eq!(word.len(), 11);
    assert!(word.iter().all(|row| row.len() == 8 * 7));
    // the capitals, letters' rows 2 to 8 with three of them twice, on rows 0 to 9, and the
    // shadow a row under each
    let inked = |rows: &[String], key: char| -> Vec<usize> {
        (0..rows.len()).filter(|&y| rows[y].contains(key)).collect()
    };
    assert_eq!(inked(&word, 'L'), (0..10).collect::<Vec<_>>());
    assert_eq!(inked(&word, 'w'), (1..=10).collect::<Vec<_>>());
    // each row its capitals' row made bold: the letters, and again a pixel to their right
    let capital = [0, 0, 1, 2, 3, 3, 4, 5, 6, 6];
    for (y, &cap) in capital.iter().enumerate() {
        let row = ink[ACCENT + cap].as_bytes();
        let at = |x: usize| row.get(x) == Some(&b'#');
        let bold: String = (0..row.len())
            .map(|x| {
                if at(x) || (x > 0 && at(x - 1)) {
                    'L'
                } else {
                    '.'
                }
            })
            .collect();
        assert_eq!(word[y].replace('w', "."), bold, "row {y}");
    }
}

#[test]
fn a_thin_big_word_is_the_capitals_as_they_are() {
    let word = big_word("SETTINGS", Font::Thin);
    let ink = letters("SETTINGS", ADVANCE);
    assert_eq!(word.len(), 11);
    assert!(word.iter().all(|row| row.len() == 8 * 6));
    let capital = [0, 0, 1, 2, 3, 3, 4, 5, 6, 6];
    for (y, &cap) in capital.iter().enumerate() {
        let row = ink[ACCENT + cap].replace('#', "L");
        assert_eq!(word[y].replace('w', "."), row, "row {y}");
    }
    // the same shadow, a pixel down and right of the ink
    assert!(word[10].contains('w') && !word[10].contains('L'));
}

#[test]
fn capitals_fill_letters_rows_two_to_eight() {
    let rows = letters("BUTTON SETTING 0123456789", ADVANCE);
    let inked: Vec<usize> = (0..rows.len()).filter(|&y| rows[y].contains('#')).collect();
    assert_eq!(inked, (ACCENT..ACCENT + CAP).collect::<Vec<_>>());
    assert_eq!(rows[0].len(), 25 * ADVANCE);
    assert_eq!(letters("", ADVANCE), vec![".".to_string(); 11]);
}

#[test]
fn a_colon_between_figures_stands_a_pixel_higher_as_the_browser_draws_it() {
    // the rows the colon's dots are on, the third letter in each
    let dots = |text: &str| -> Vec<usize> {
        let rows = letters(text, ADVANCE);
        (0..rows.len())
            .filter(|&y| rows[y][2 * ADVANCE..3 * ADVANCE].contains('#'))
            .collect()
    };
    let plain = dots("ab:cd");
    let raised = dots("16:00");
    assert_eq!(plain.len(), 2, "two dots, a pixel each");
    assert_eq!(raised, plain.iter().map(|y| y - 1).collect::<Vec<_>>());
    assert_eq!(dots("UT: 0"), raised, "a space apart still counts");
    assert_eq!(dots("16:ab"), plain, "a figure on one side only does not");
}

/// Pixel Code's capital accents stand a row over the line, which `letters` leaves out and
/// `letters_over` keeps; a row's value, which nothing cuts, draws it on the band's frame line as
/// the browser does, and its name, which `overflow: hidden` cuts, does not.
#[test]
fn a_capitals_accent_rises_a_row_over_the_line_in_a_rows_value() {
    let over = letters_over("\u{c1}", ADVANCE, 1);
    assert!(over[0].contains('#'), "the Á's accent has no top row");
    assert_eq!(over[1..], letters("\u{c1}", ADVANCE)[..], "the line moved");

    let top = |face: &UndoFace| (0..face.w).any(|x| face.rgba[(x * 4 + 3) as usize] != 0);
    let value = row_face(60, &row("A", Some("\u{c1}"), ""), &RAMP);
    let name = row_face(60, &row("\u{c1}", Some("A"), ""), &RAMP);
    assert!(top(&value), "the value's accent is cut");
    assert!(!top(&name), "the name's accent is not cut");
}

#[test]
fn mirror_makes_a_sprite_from_its_quarter_and_flip_turns_it() {
    assert_eq!(mirror(&[".s", "sA"]), [".s.", "sAs", ".s."]);
    assert_eq!(
        mirror(&["..k", ".kH", "kHs"]),
        ["..k..", ".kHk.", "kHsHk", ".kHk.", "..k.."]
    );
    assert_eq!(flip(&["AAAA....", "Abb"]), ["....AAAA", "bbA"]);
    assert_eq!(flip(&flip(&["k.e", "Hs"])), ["k.e", "Hs"]);
}

#[test]
fn keyed_paints_each_key_and_leaves_dots_clear() {
    let face = keyed(&["kA", ".c"], &RAMP);
    assert_eq!((face.w, face.h), (2, 2));
    assert_eq!(&face.rgba[0..4], &[0x2a, 0x0f, 0x04, 255]);
    assert_eq!(&face.rgba[4..8], &[0xe6, 0xab, 0x47, 255]);
    assert_eq!(face.rgba[11], 0, "a dot is clear");
    assert_eq!(&face.rgba[12..16], &[0xf5, 0xe5, 0xbb, 255]);
}

#[test]
fn a_face_is_drawn_two_screen_pixels_to_a_texel() {
    let tex = TexId::from_raw(1);
    assert_eq!(
        quad(tex, 115, 19, 10.0, 20.0),
        Draw::Tex {
            x: 10.0,
            y: 20.0,
            w: 230.0,
            h: 38.0,
            tex,
            alpha: 1.0
        }
    );
}

/// A row as kit.html builds one from its query: `label`, `value`, and the flags that are set.
fn row<'a>(label: &'a str, value: Option<&'a str>, flags: &str) -> Row<'a> {
    let has = |flag| flags.split(' ').any(|f| f == flag);
    Row {
        label,
        value,
        lit: has("lit"),
        carets: has("carets"),
        opens: has("opens"),
        dim: has("dim"),
        caret_room: false,
    }
}

#[test]
fn rows_are_chromes() {
    let cases = [
        ("row-plain", row("FAST FORWARD", Some("6X"), "carets")),
        (
            "row-lit-carets",
            row("FAST FORWARD", Some("6X"), "carets lit"),
        ),
        ("row-lit", row("RUMBLE", Some("ON"), "lit")),
        ("row-opens", row("LABELS", None, "opens")),
        ("row-opens-lit", row("NETWORK", Some("HOME"), "opens lit")),
        (
            "row-carets-opens-lit",
            row("SHADER", Some("SHARP"), "carets opens lit"),
        ),
        ("row-dim", row("WI-FI", Some("OFF"), "carets dim")),
        ("row-dim-lit", row("WI-FI", Some("OFF"), "carets dim lit")),
        ("row-dim-opens", row("SSH", None, "opens dim")),
        ("row-value", row("IP", Some("192.168.1.20"), "")),
        ("row-case", row("Host", Some("rg35xxsp.local"), "")),
        ("row-name", row("FETCH LABELS", None, "")),
    ];
    for (name, row) in cases {
        let face = row_face(150, &row, &RAMP);
        assert_eq!((face.w, face.h), (150, ROW_H));
        matches(&face, name);
    }
}

#[test]
fn a_row_too_narrow_cuts_its_label_as_chrome_does() {
    let cut = row("FAST FORWARD SOUND", Some("OFF"), "lit");
    matches(&row_face(60, &cut, &RAMP), "row-cut");
    let carets = row("FAST FORWARD", Some("6X"), "carets lit");
    matches(&row_face(50, &carets, &RAMP), "row-cut-carets");
    // no room at all: the label is gone and the value runs on past the band's inside
    let squeezed = row("RUMBLE", Some("OFF"), "");
    matches(&row_face(30, &squeezed, &RAMP), "row-squeezed");
}

/// A network's row as kit.html builds one: `label`, `value`, `signal`, and the flags set.
fn network<'a>(
    label: &'a str,
    value: Option<&'a str>,
    signal: Option<u8>,
    flags: &str,
) -> ListRow<'a> {
    let has = |flag| flags.split(' ').any(|f| f == flag);
    ListRow {
        label,
        value,
        signal,
        lock: has("lock"),
        device: None,
        lit: has("lit"),
        dim: has("dim"),
    }
}

#[test]
fn network_rows_are_chromes() {
    let cases = [
        (
            "list-home-lit",
            network("Home", Some("CONNECTED"), Some(3), "lock lit"),
        ),
        ("list-cafe", network("Café Wi-Fi", None, Some(2), "")),
        ("list-cafe-lit", network("Café Wi-Fi", None, Some(2), "lit")),
        (
            "list-upstairs",
            network("Upstairs 5G", None, Some(1), "lock"),
        ),
        ("list-attic", network("Attic", None, Some(0), "lock")),
        (
            "list-saved",
            network("Office", Some("SAVED"), Some(1), "lock"),
        ),
        ("list-dim", network("Home", None, Some(3), "lock dim")),
        (
            "list-dim-lit",
            network("Home", Some("CONNECTED"), Some(3), "lock dim lit"),
        ),
        ("list-bare", network("Hidden", None, None, "")),
    ];
    for (name, row) in cases {
        let face = list_row_face(150, &row, &RAMP);
        assert_eq!((face.w, face.h), (150, ROW_H));
        matches(&face, name);
    }
    let long = network("A very long network name", None, Some(2), "lock lit");
    matches(&list_row_face(100, &long, &RAMP), "list-cut");
}

#[test]
fn a_signal_past_three_is_three() {
    let full = network("Home", None, Some(3), "lit");
    let past = network("Home", None, Some(9), "lit");
    let face = |row| list_row_face(150, row, &RAMP).rgba;
    assert!(face(&past) == face(&full));
}

/// A device's row as kit.html builds one: `label`, `value`, its kind, and the flags set.
fn device<'a>(
    label: &'a str,
    value: Option<&'a str>,
    kind: DeviceKind,
    flags: &str,
) -> ListRow<'a> {
    let has = |flag| flags.split(' ').any(|f| f == flag);
    ListRow {
        label,
        value,
        signal: None,
        lock: false,
        device: Some(kind),
        lit: has("lit"),
        dim: has("dim"),
    }
}

const KINDS: [DeviceKind; 4] = [
    DeviceKind::Audio,
    DeviceKind::Gamepad,
    DeviceKind::Keyboard,
    DeviceKind::Other,
];

#[test]
fn device_rows_are_chromes() {
    let rows = [
        (
            "list-gamepad",
            "8BitDo SN30 Pro",
            Some("CONNECTED"),
            DeviceKind::Gamepad,
        ),
        (
            "list-audio",
            "WH-1000XM4",
            Some("PAIRED"),
            DeviceKind::Audio,
        ),
        ("list-keyboard", "Keychron K3", None, DeviceKind::Keyboard),
        ("list-other", "4C:4C:6B:1A:80:D3", None, DeviceKind::Other),
    ];
    for (name, label, value, kind) in rows {
        for (flags, suffix) in [("", ""), ("lit", "-lit")] {
            let face = list_row_face(150, &device(label, value, kind, flags), &RAMP);
            assert_eq!((face.w, face.h), (150, ROW_H));
            matches(&face, &format!("{name}{suffix}"));
        }
    }
    let dim = device("Keychron K3", Some("PAIRING"), DeviceKind::Keyboard, "dim");
    matches(&list_row_face(150, &dim, &RAMP), "list-device-dim");
    let long = device(
        "8BitDo SN30 Pro",
        Some("CONNECTING"),
        DeviceKind::Gamepad,
        "lit",
    );
    matches(&list_row_face(100, &long, &RAMP), "list-device-cut");
}

#[test]
fn every_device_icon_stands_on_the_capitals_in_the_middle_of_its_room() {
    for kind in KINDS {
        let icon = kind.icon();
        let w = icon[0].len() as u32;
        assert!(icon.iter().all(|row| row.len() as u32 == w), "{kind:?}");
        assert!(w <= DEVICE_W, "{kind:?} is wider than its room");
        assert_eq!((DEVICE_W - w) % 2, 0, "{kind:?} is off its room's middle");
        assert!(icon[0].contains('#') && icon[6].contains('#'), "{kind:?}");
    }
}

/// A device's row draws neither a padlock nor bars, and keeps no room for them: what a network's
/// `signal` and `lock` say changes nothing on it.
#[test]
fn a_devices_row_has_its_icon_and_nothing_of_a_networks() {
    let row = device("Keychron K3", Some("PAIRED"), DeviceKind::Keyboard, "lit");
    let networkish = ListRow {
        signal: Some(3),
        lock: true,
        ..row
    };
    let face = |row| list_row_face(150, row, &RAMP).rgba;
    assert!(face(&networkish) == face(&row));
}

/// Down a list of devices the values end on one column and the icons share a middle one,
/// whatever their kinds.
#[test]
fn a_list_of_devices_lines_up_its_values_and_icons() {
    let columns = |face: &omega_ui::UndoFace, colour: [u8; 3]| -> Vec<u32> {
        (0..face.w)
            .filter(|&x| {
                (0..face.h).any(|y| {
                    let i = ((y * face.w + x) * 4) as usize;
                    face.rgba[i + 3] != 0 && face.rgba[i..i + 3] == colour
                })
            })
            .collect()
    };
    let mut ends = Vec::new();
    for kind in KINDS {
        let face = list_row_face(150, &device("Pad", Some("PAIRED"), kind, ""), &RAMP);
        let value = columns(&face, RAMP.dim);
        let ink = columns(&face, RAMP.label);
        let icon: Vec<u32> = ink.into_iter().filter(|&x| x > 100).collect();
        let (first, last) = (icon[0], icon[icon.len() - 1]);
        ends.push((value[value.len() - 1], first + last));
    }
    assert!(ends.iter().all(|e| *e == ends[0]), "{ends:?}");
}

/// The bar `w` art pixels wide as omega-bar draws it with these actions, one width, and hints:
/// the hints between the actions and the actions at the ends, on a face `TALL` high whose row
/// `RISE` is the bar's top, clear everywhere else.
pub(crate) fn bar(w: u32, left: (&str, &str), right: (&str, &str), hints: &[Hint]) -> UndoFace {
    bar_in(Font::Bold, w, left, right, hints)
}

/// The same bar, its actions' words in `font`.
fn bar_in(font: Font, w: u32, left: (&str, &str), right: (&str, &str), hints: &[Hint]) -> UndoFace {
    let [left, right] = action_pair(left, right, font, &RAMP);
    let layout = bar_layout(w, &left, &right);
    let mut face = omega_ui::kit::pixel::blank(w, TALL);
    let [x, y, hw, _] = layout.hints;
    over(&mut face, &hints_face(hw, hints, &RAMP), x, y + RISE);
    for (action, [ax, ay]) in [(&left, layout.left), (&right, layout.right)] {
        over(
            &mut face,
            &action.face,
            ax as u32,
            (ay + RISE as i32) as u32,
        );
    }
    face
}

#[test]
fn the_bar_is_chromes_at_640_and_720() {
    let shelf = (("CONTROLS", "SELECT"), ("SETTINGS", "START"));
    let menu = (("RETURN", "B"), ("CONFIRM", "A"));
    let forget = [Hint {
        key: "Y",
        word: "FORGET",
    }];
    let two = [
        forget[0],
        Hint {
            key: "X",
            word: "RENAME",
        },
    ];
    for w in [320, 360] {
        matches(&bar(w, shelf.0, shelf.1, &[]), &format!("bar-shelf-{w}"));
        matches(&bar(w, menu.0, menu.1, &[]), &format!("bar-menu-{w}"));
        matches(&bar(w, menu.0, menu.1, &forget), &format!("bar-forget-{w}"));
        matches(&bar(w, menu.0, menu.1, &two), &format!("bar-two-{w}"));
    }
    let fetch = [Hint {
        key: "START",
        word: "FETCH",
    }];
    matches(&bar(320, menu.0, menu.1, &fetch), "bar-fetch-320");
    // the first-boot clock's: its empty action as wide as CONFIRM
    matches(&bar(320, ("", ""), menu.1, &[]), "bar-confirm-320");
}

#[test]
fn the_thin_bar_is_chromes() {
    let thin = |left, right, hints| bar_in(Font::Thin, 320, left, right, hints);
    let shelf = (("CONTROLS", "SELECT"), ("SETTINGS", "START"));
    matches(&thin(shelf.0, shelf.1, &[]), "bar-shelf-thin-320");
    let forget = [Hint {
        key: "Y",
        word: "FORGET",
    }];
    let menu = (("RETURN", "B"), ("CONFIRM", "A"));
    matches(&thin(menu.0, menu.1, &forget), "bar-menu-thin-320");
}

#[test]
fn the_hints_take_the_room_between_the_actions() {
    let [controls, settings] = pair(("CONTROLS", "SELECT"), ("SETTINGS", "START"));
    let [back, confirm] = pair(("RETURN", "B"), ("CONFIRM", "A"));
    let shelf = bar_layout(320, &controls, &settings);
    assert_eq!(shelf.hints, [115, 0, 90, BAR]);
    assert_eq!((shelf.left, shelf.right), ([0, -4], [205, -4]));
    assert_eq!(
        bar_layout(360, &controls, &settings).hints,
        [115, 0, 130, 15]
    );
    let menu = bar_layout(320, &back, &confirm);
    assert_eq!(menu.hints, [97, 0, 126, 15]);
    assert_eq!(menu.right, [223, -4]);
    assert_eq!(bar_layout(360, &back, &confirm).hints, [97, 0, 166, 15]);
}

/// Between the bar's two actions there is nothing but the hints, whatever is behind the bar
/// showing through: with none every pixel there is clear, and with Y FORGET and X RENAME the
/// only ones drawn are their caps' dark and their letters' cream, on the caps' rows and across
/// the span the hints are centred by.
#[test]
fn nothing_is_drawn_between_the_actions_but_the_hints() {
    let menu = (("RETURN", "B"), ("CONFIRM", "A"));
    let [left, right] = pair(menu.0, menu.1);
    let two = [
        Hint {
            key: "Y",
            word: "FORGET",
        },
        Hint {
            key: "X",
            word: "RENAME",
        },
    ];
    // the caps start a row above the capitals, which are on the bar's rows 5 to 11
    let caps = RISE + 4..RISE + 4 + KEY_H;
    for w in [320, 360] {
        let [x0, _, room, _] = bar_layout(w, &left, &right).hints;
        for hints in [&[][..], &two[..]] {
            let face = bar(w, menu.0, menu.1, hints);
            let from = x0 + (room - hints_width(hints)) / 2;
            let span = from..from + hints_width(hints);
            let mut drawn = 0;
            for y in 0..TALL {
                for x in x0..x0 + room {
                    let i = ((y * w + x) * 4) as usize;
                    let px = &face.rgba[i..i + 4];
                    if px[3] == 0 {
                        continue;
                    }
                    drawn += 1;
                    assert!(
                        span.contains(&x) && caps.contains(&y),
                        "{w}, {hints:?}: ({x}, {y}) is drawn, off the hints"
                    );
                    assert!(
                        [RAMP.dark, RAMP.cream].contains(&[px[0], px[1], px[2]]),
                        "{w}, {hints:?}: ({x}, {y}) is {px:?}, not a cap's or a letter's"
                    );
                }
            }
            assert_eq!(drawn > 0, !hints.is_empty(), "{w}, {hints:?}");
        }
    }
}

#[test]
fn hints_are_as_wide_as_their_caps_and_words() {
    let forget = Hint {
        key: "Y",
        word: "FORGET",
    };
    let rename = Hint {
        key: "X",
        word: "RENAME",
    };
    assert_eq!(cap_width("Y"), 9);
    assert_eq!(cap_width("START"), 33);
    assert_eq!(hint_width(&forget), 9 + 3 + 36);
    // ink to ink: the last word's blank pixel is not ink, and 8 pixels between two
    assert_eq!(hints_width(&[forget]), 47);
    assert_eq!(hints_width(&[forget, rename]), 47 + 8 + 47);
    assert_eq!(hints_width(&[Hint { key: "Y", word: "" }]), 9);
    assert_eq!(hints_width(&[]), 0);
    let face = hints_face(126, &[forget], &RAMP);
    assert_eq!((face.w, face.h), (126, BAR));
}

#[test]
fn the_bar_stands_on_the_screens_bottom_and_its_actions_sink_on_their_own() {
    let [left, right] = pair(("RETURN", "B"), ("CONFIRM", "A"));
    let layout = bar_layout(320, &left, &right);
    let tex = TexId::from_raw(3);
    assert_eq!(BarLayout::top(480), 450.0);
    assert_eq!(
        layout.hints_quad(tex, 480),
        quad(tex, 126, BAR, 194.0, 450.0)
    );
    assert_eq!(
        layout.action_quad(Side::Left, tex, &left, 480, 0),
        quad(tex, 97, TALL, 0.0, 442.0)
    );
    assert_eq!(
        layout.action_quad(Side::Right, tex, &right, 720, SINK),
        quad(tex, 97, TALL, 446.0, 690.0 - 8.0 + 38.0)
    );
}

#[test]
fn a_change_of_actions_sinks_the_old_and_raises_the_new() {
    let swap = |new, sink| Swap { new, sink };
    assert_eq!(action_swap(0.0), swap(false, 0));
    // flow/player.js: even(p / 0.5 * 38) screen pixels, 9.5 art pixels rounding up
    assert_eq!(action_swap(0.25), swap(false, 10));
    assert_eq!(action_swap(0.49), swap(false, 19));
    assert_eq!(action_swap(0.5), swap(true, SINK));
    assert_eq!(action_swap(0.75), swap(true, 10));
    assert_eq!(action_swap(1.0), swap(true, 0));
    assert_eq!(action_swap(2.0), swap(true, 0));
    assert_eq!(SINK, TALL, "sunk that far an action is out of sight");
}

#[test]
fn lines_of_text_are_chromes() {
    let cases = [
        ("text-sftp", "SFTP uses the same login", RAMP.dim),
        ("text-offline", "Join a Wi-Fi network first.", RAMP.dim),
        ("text-bluetooth-off", "Bluetooth is off.", RAMP.dim),
        (
            "text-labels",
            "Sign in with your free ScreenScraper account\nto fetch the real labels of your carts.",
            RAMP.dim,
        ),
        ("text-heading", "IN GAME", RAMP.dim),
        ("text-menu-twice", "MENU ×2", RAMP.frame_hi),
        ("text-does", "LOAD/SAVE STATE", RAMP.cream),
    ];
    for (name, text, colour) in cases {
        matches(&text_face(text, colour), name);
    }
}

/// flow.html draws the Controls panel's ◀ ▶ ▲ ▼ as sprites copied from the full Pixel Code,
/// which the web font it is set in has no letter for; the face the crate embeds has them, pixel
/// for pixel the same.
#[test]
fn the_controls_panels_arrows_are_pixel_codes_own_as_flow_copies_them() {
    matches(
        &text_face("SELECT+▲▼", RAMP.frame_hi),
        "text-select-up-down",
    );
    matches(&text_face("◀▶", RAMP.frame_hi), "text-left-right");
}

/// The … a note ends with while something is under way is a sprite in flow.html too, copied from
/// the full face, three dots on the baseline row.
#[test]
fn a_notes_ellipsis_is_pixel_codes_own_as_flow_copies_it() {
    matches(&text_face("Looking for devices…", RAMP.dim), "text-looking");
}

/// What a well holds, laid over `panel`: rows from its top, then whatever is drawn after them.
fn hold(panel: &mut PanelFace, rows: &[Row]) {
    for (i, row) in rows.iter().enumerate() {
        let [x, y, w, _] = row_rect(panel.well, i as u32);
        over(&mut panel.face, &row_face(w, row, &RAMP), x, y);
    }
}

/// A note in a well, as flow.css's .note: 3 pixels in, `below` the rows above it.
fn note(panel: &mut PanelFace, text: &str, below: u32) {
    let [x, y, _, _] = panel.well;
    over(
        &mut panel.face,
        &text_face(text, RAMP.dim),
        x + 3,
        y + below,
    );
}

#[test]
fn ssh_is_flows() {
    let login = [
        ("IP", "192.168.1.20"),
        ("HOST", "rg35xxsp.local"),
        ("USER", "root"),
        ("PASSWORD", "482913"),
        ("PORT", "22"),
    ];
    let rows: Vec<Row> = login
        .iter()
        .map(|&(label, value)| row(label, Some(value), ""))
        .collect();
    // the rows, then the note 4 below them on its line and 2 more under it
    let content = 5 * ROW_H + 4 + 10 + 2;
    let mut panel = panel_face(200, panel_height(content), "SSH", true, &RAMP);
    assert_eq!(panel.well[3], content);
    hold(&mut panel, &rows);
    note(&mut panel, "SFTP uses the same login", 5 * ROW_H + 4);
    matches(&panel.face, "flow-ssh");

    // offline, or with SSH switched off, the note alone: 4 above its line and 5 under it
    for (text, name) in [
        ("Join a Wi-Fi network first.", "flow-ssh-offline"),
        ("Turn SSH on first.", "flow-ssh-switched-off"),
    ] {
        let mut panel = panel_face(200, panel_height(4 + 10 + 5), "SSH", true, &RAMP);
        note(&mut panel, text, 4);
        matches(&panel.face, name);
    }
}

#[test]
fn labels_is_flows() {
    let rows = [
        row("SCREENSCRAPER", Some("SIGN IN"), "lit"),
        row("FETCH LABELS", None, ""),
    ];
    let content = 2 * ROW_H + 4 + 2 * 10 + 2;
    let mut panel = panel_face(280, panel_height(content), "LABELS", true, &RAMP);
    hold(&mut panel, &rows);
    let text =
        "Sign in with your free ScreenScraper account\nto fetch the real labels of your carts.";
    note(&mut panel, text, 2 * ROW_H + 4);
    matches(&panel.face, "flow-labels");
}

/// The Controls panel's two columns: a heading, then each button and what it does, a line and a
/// pixel apart; each column's buttons as many letters wide as `keys`, and 16 pixels between
/// the two columns.
#[test]
fn the_controls_panel_is_flows() {
    let shelf: [(&str, &str); 8] = [
        ("A", "PLAY"),
        ("HOLD A", "CLEAN START"),
        ("X", "CORE"),
        ("START", "SETTINGS"),
        ("SELECT", "CONTROLS"),
        ("◀▶", "BROWSE"),
        ("▲▼", "LETTER"),
        ("L1 R1", "SHELF"),
    ];
    let in_game: [(&str, &str); 11] = [
        ("L2", "REWIND"),
        ("R2", "FAST FORWARD"),
        ("MENU ×2", "SAVE STATES"),
        ("HOLD MENU", "EJECT"),
        ("SELECT+MENU", "LINK"),
        ("SELECT+▲▼", "BRIGHTNESS"),
        ("SELECT+◀▶", "BLUE LIGHT"),
        ("SELECT+L1/R1", "LOAD/SAVE STATE"),
        ("SELECT+Y", "COLOR"),
        ("VOL+ VOL-", "MUTE"),
        ("HOLD POWER", "POWER MENU"),
    ];
    // the heading's line and 2 below it, a pixel, then the lines; and a pixel under them all
    let content = 10 + 2 + 1 + 11 * 11 - 1 + 1;
    let mut panel = panel_face(304, panel_height(content), "CONTROLS", true, &RAMP);
    let [wx, wy, ww, _] = panel.well;
    assert_eq!(ww, 108 + 16 + 168, "the widest line sets the panel's width");
    /// A button and what it does.
    type Line<'a> = (&'a str, &'a str);
    let columns: [(u32, &str, u32, &[Line]); 2] =
        [(0, "SHELF", 6, &shelf), (108 + 16, "IN GAME", 12, &in_game)];
    for (x, heading, keys, lines) in columns {
        over(&mut panel.face, &text_face(heading, RAMP.dim), wx + x, wy);
        for (i, (key, does)) in lines.iter().enumerate() {
            let y = wy + 13 + 11 * i as u32;
            over(&mut panel.face, &text_face(key, RAMP.frame_hi), wx + x, y);
            let does_x = wx + x + 6 * keys + 6;
            over(&mut panel.face, &text_face(does, RAMP.cream), does_x, y);
        }
    }
    matches(&panel.face, "flow-controls");
}

#[test]
fn rows_stand_in_a_well_a_pixel_into_its_sides() {
    let panel = panel_face(160, panel_height(3 * ROW_H), "CONFIGS", true, &RAMP);
    assert_eq!(panel.well, [6, 18, 148, 39]);
    assert_eq!(row_width(148), 150);
    assert_eq!(row_rect(panel.well, 0), [5, 18, 150, ROW_H]);
    assert_eq!(row_rect(panel.well, 2), [5, 18 + 2 * ROW_H, 150, ROW_H]);
    assert_eq!(panel_height(0), 23);
}
/// A row kept clear of the carets' room draws its value where a row with carets draws its
/// own unlit, so the two line up in one panel.
#[test]
fn a_row_with_caret_room_lines_up_with_one_with_carets() {
    let with = |carets: bool, caret_room: bool| Row {
        label: "NAME",
        value: Some("HOST"),
        carets,
        caret_room,
        ..Row::default()
    };
    let carets = row_face(150, &with(true, false), &RAMP);
    let room = row_face(150, &with(false, true), &RAMP);
    let neither = row_face(150, &with(false, false), &RAMP);
    assert_eq!(
        room.rgba, carets.rgba,
        "the value moved off the carets' line"
    );
    assert_ne!(
        neither.rgba, carets.rgba,
        "a row with no room already lined up"
    );
}
