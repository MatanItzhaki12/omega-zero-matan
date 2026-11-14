#![cfg(any(target_os = "macos", target_os = "linux"))]

//! Settings drawn by the whole frontend on a headless GL canvas: the modal and its rows where
//! the kit puts them, at 2x on the art grid, in the palette of the backdrop showing round it.

use crate::common;

use std::collections::VecDeque;

use common::{clocked, tmp_root_with_carts};
use omega_gfx::{out_h, out_w, Compositor, HeadlessSurface};
use omega_input::{Btn, InputSource, Millis, RawEvent};
use omega_power::SimPlatform;
use omega_store::{read_app_state, write_app_state, BackdropStyle};
use omega_ui::kit::palette::{palette, Palette};
use omega_ui::{
    settings_frame, settings_row_face, settings_row_rects, settings_title_face, top_in, RowShown,
    SettingsRow, UndoFace,
};
use omega_zero::frontend::Frontend;

const NIGHT: u8 = 0;
const AMBER: u8 = 7;

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

fn tap(f: &mut Frontend, input: &mut Script, btn: Btn) {
    input.0.push_back(vec![RawEvent::Down(btn)]);
    f.advance(input);
    input.0.push_back(vec![RawEvent::Up(btn)]);
    f.advance(input);
}

/// The screen's pixel at (`x`, `y`), from the top left.
fn at(px: &[u8], x: u32, y: u32) -> [u8; 3] {
    let o = ((y * out_w() + x) * 4) as usize;
    [px[o], px[o + 1], px[o + 2]]
}

/// The screen's art pixel at (`x`, `y`): the 2x2 block a texel covers, which must be one colour.
fn art(px: &[u8], x: u32, y: u32) -> [u8; 3] {
    let c = at(px, 2 * x, 2 * y);
    for (dx, dy) in [(1, 0), (0, 1), (1, 1)] {
        assert_eq!(
            at(px, 2 * x + dx, 2 * y + dy),
            c,
            "art pixel ({x}, {y}) is off the grid"
        );
    }
    c
}

/// The frame once the screen has slid into place, written to SCRATCH_PNG_DIR if it is set.
fn composed(f: &mut Frontend, c: &mut Compositor, name: &str) -> Vec<u8> {
    f.app_mut().finish_slide();
    f.compose(c);
    let mut px = c.read_frame();
    for p in px.chunks_exact_mut(4) {
        p[3] = 0xff;
    }
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let path = format!("{dir}/settings-{name}-{}x{}.png", out_w(), out_h());
        let file = std::fs::File::create(&path).expect("create png");
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), out_w(), out_h());
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .expect("png header")
            .write_image_data(&px)
            .expect("png data");
        println!("wrote {path}");
    }
    px
}

/// Sets the card's background to the plain colour `colour`.
fn solid(root: &std::path::Path, colour: u8) {
    let mut s = read_app_state(root);
    s.backdrop = Some(BackdropStyle::Solid);
    s.backdrop_colour = colour;
    write_app_state(root, &s).expect("write omega-zero.state");
}

/// A frontend on a card with the plain background `colour`, Settings opened with START.
fn settings_over(colour: u8, c: &mut Compositor) -> (tempfile::TempDir, Frontend, Script) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    solid(d.path(), colour);
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(c);
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::Menu);
    assert!(f.app().settings_row().is_some(), "MENU opened no Settings");
    (d, f, input)
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

/// The modal as the kit's faces make it for `shown`, at art resolution, where `settings_rect` puts it.
fn expected(shown: &[RowShown], pal: &Palette) -> UndoFace {
    let rows: Vec<SettingsRow> = shown.iter().map(|s| s.row).collect();
    let frame = settings_frame(top_in(&rows), pal);
    let [mx, my, ..] = omega_ui::settings_rect(omega_ui::top_in(
        &shown.iter().map(|s| s.row).collect::<Vec<_>>(),
    ));
    let mut modal = UndoFace {
        rgba: frame.face.rgba.clone(),
        w: frame.face.w,
        h: frame.face.h,
    };
    if let Some(g) = shown.iter().find(|s| s.lit).map(|s| s.row.group()) {
        let [tx, ty, ..] = frame.titles[g.index()];
        over(&mut modal, &settings_title_face(g, pal), tx - mx, ty - my);
    }
    for (row, [x, y, w, _]) in settings_row_rects(&rows, frame.wells) {
        let s = shown.iter().find(|s| s.row == row).expect("a row shown");
        over(&mut modal, &settings_row_face(w, s, pal), x - mx, y - my);
    }
    modal
}

/// How many rows GENERAL holds on `f`'s Settings.
fn top(f: &Frontend) -> u32 {
    omega_ui::top_in(&f.app().settings_rows())
}

/// Every pixel the modal's faces ink is on the screen, at 2x where the kit puts it.
fn holds_the_modal(px: &[u8], f: &Frontend, name: &str) {
    holds_the_modal_in(px, f, name, None);
}

/// As `holds_the_modal`, only within `within` (art pixels on the screen), where a toast may
/// stand over the rest.
fn holds_the_modal_in(px: &[u8], f: &Frontend, name: &str, within: Option<[u32; 4]>) {
    let pal = f.app().palette();
    let want = expected(&f.app().settings_shown(), pal);
    let [mx, my, ..] = omega_ui::settings_rect(top(f));
    let [wx, wy, ww, wh] = within.unwrap_or([mx, my, want.w, want.h]);
    let mut off = Vec::new();
    for y in wy - my..wy - my + wh {
        for x in wx - mx..wx - mx + ww {
            let i = ((y * want.w + x) * 4) as usize;
            if want.rgba[i + 3] == 0 {
                continue;
            }
            let c = [want.rgba[i], want.rgba[i + 1], want.rgba[i + 2]];
            let got = art(px, mx + x, my + y);
            if got != c {
                off.push((x, y, got, c));
            }
        }
    }
    assert!(
        off.is_empty(),
        "{name}: {} art pixels of the modal are off, first (x, y, drawn, face) {:?}",
        off.len(),
        &off[..off.len().min(8)]
    );
}

/// Where the lit row's band is, in art pixels: the row `row` of what is shown.
fn row_rect(f: &Frontend, row: SettingsRow) -> [u32; 4] {
    let rows = f.app().settings_rows();
    let frame = settings_frame(top_in(&rows), f.app().palette());
    settings_row_rects(&rows, frame.wells)
        .into_iter()
        .find(|(r, _)| *r == row)
        .expect("a row shown")
        .1
}

#[test]
fn settings_draws_the_modal_and_the_lit_rows_band_over_the_backdrop() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let (_d, mut f, _) = settings_over(NIGHT, &mut c);
    let px = composed(&mut f, &mut c, "night");
    let pal = palette(NIGHT);
    holds_the_modal(&px, &f, "night");

    // the frame: its dark outline, its bright line, the frame colour the panels sit on
    let [mx, my, mw, mh] = omega_ui::settings_rect(top(&f));
    assert_eq!(art(&px, mx + 3, my), pal.dark, "the frame's top outline");
    assert_eq!(art(&px, mx, my + 3), pal.dark, "the frame's left outline");
    assert_eq!(
        art(&px, mx + 1, my + 3),
        pal.frame_hi,
        "the frame's bright line"
    );
    assert_eq!(art(&px, mx + 3, my + mh / 2), pal.frame, "the frame's side");
    assert_eq!(
        art(&px, mx + mw - 4, my + mh / 2),
        pal.frame,
        "the frame's other side"
    );

    // the lit row's band, Date & Time's as Settings opens: a frame line with its corners left
    // out, round the tab colour
    let [x, y, w, h] = row_rect(&f, SettingsRow::DateTime);
    assert_eq!(art(&px, x + 1, y), pal.frame, "the band's top line");
    assert_eq!(art(&px, x, y + 1), pal.frame, "the band's left line");
    assert_eq!(
        art(&px, x + w - 1, y + h - 2),
        pal.frame,
        "the band's right line"
    );
    assert_eq!(art(&px, x + 2, y + 2), pal.tab, "the band");
    assert_eq!(art(&px, x + w / 2, y + h - 2), pal.tab, "the band");
    assert_eq!(art(&px, x, y), pal.well, "the band's corner is the well's");
    let [x, y, w, h] = row_rect(&f, SettingsRow::Gameplay);
    for (dx, dy) in [(1, 0), (2, 2), (w / 2, h - 2)] {
        assert_eq!(
            art(&px, x + dx, y + dy),
            pal.well,
            "an unlit row has a band"
        );
    }

    // round the modal, the backdrop and nothing over it
    // on a 480-line screen the modal reaches the top, and nothing stands over it
    let above = (my > 0).then(|| (mx + mw / 2, my - 1));
    for (x, y) in [(0, 0), (mx - 1, my + mh / 2), (mx + mw, my)]
        .into_iter()
        .chain(above)
    {
        assert_eq!(art(&px, x, y), pal.bg, "the backdrop at art ({x}, {y})");
    }
    assert_eq!(art(&px, mx + mw / 2, my + mh), pal.bg, "under the modal");
    assert_eq!(art(&px, mx, my), pal.bg, "the frame's clear corner");
}

#[test]
fn the_palette_tints_settings_with_the_backdrop() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    // the sim has no radios to hide: GENERAL holds all its rows
    let [mx, my, _, mh] = omega_ui::settings_rect(omega_ui::top_in(&SettingsRow::ALL));
    let mut sides = Vec::new();
    for (colour, name) in [(NIGHT, "night"), (AMBER, "amber")] {
        let (_d, mut f, _) = settings_over(colour, &mut c);
        let px = composed(&mut f, &mut c, name);
        let pal = palette(colour);
        holds_the_modal(&px, &f, name);
        assert_eq!(art(&px, 0, 0), pal.bg, "{name}'s backdrop");
        assert_eq!(art(&px, mx + 3, my + mh / 2), pal.frame, "{name}'s frame");
        let [x, y, ..] = row_rect(&f, SettingsRow::DateTime);
        assert_eq!(art(&px, x + 2, y + 2), pal.tab, "{name}'s band");
        sides.push(art(&px, mx + 3, my + mh / 2));
    }
    assert_ne!(sides[0], sides[1], "Night and Amber draw the same frame");
    let [r, g, b] = sides[1];
    assert!(r > g && g > b, "Amber's frame is not amber: {:?}", sides[1]);
}

#[test]
fn the_cursor_in_general_lights_its_title_and_its_row() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let (_d, mut f, mut input) = settings_over(AMBER, &mut c);
    while f.app().settings_row() != Some(SettingsRow::DateTime) {
        tap(&mut f, &mut input, Btn::Down);
    }
    let px = composed(&mut f, &mut c, "general");
    holds_the_modal(&px, &f, "general");
    let pal = palette(AMBER);
    let rows = f.app().settings_rows();
    let frame = settings_frame(top_in(&rows), pal);
    // a title's first column of ink, cream where the cursor is and dim where it is not
    let ink = |[x, y, w, h]: [u32; 4]| {
        (0..w)
            .flat_map(|dx| (0..h).map(move |dy| (x + dx, y + dy)))
            .map(|(x, y)| art(&px, x, y))
            .filter(|c| *c == pal.cream || *c == pal.dim)
            .collect::<Vec<_>>()
    };
    let (general, configs) = (ink(frame.titles[0]), ink(frame.titles[1]));
    assert!(
        !configs.is_empty() && configs.iter().all(|c| *c == pal.dim),
        "CONFIGS is lit"
    );
    assert!(
        !general.is_empty() && general.iter().all(|c| *c == pal.cream),
        "GENERAL is dim"
    );
    let [x, y, ..] = row_rect(&f, SettingsRow::DateTime);
    assert_eq!(art(&px, x + 2, y + 2), pal.tab, "Date & Time is not lit");
    let [x, y, ..] = row_rect(&f, SettingsRow::Display);
    assert_eq!(art(&px, x + 2, y + 2), pal.well, "Display is still lit");
}

#[test]
fn a_new_value_is_on_the_screen_the_frame_after() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let (_d, mut f, mut input) = settings_over(NIGHT, &mut c);
    // the cursor opens on Date & Time; down to Update, whose arrows switch between the
    // releases and the test builds
    while f.app().settings_row() != Some(SettingsRow::Update) {
        tap(&mut f, &mut input, Btn::Down);
    }
    assert_eq!(f.app().settings_row(), Some(SettingsRow::Update));
    let px = composed(&mut f, &mut c, "update-before");
    holds_the_modal(&px, &f, "before");
    let was = f.app().settings_text(SettingsRow::Update);
    tap(&mut f, &mut input, Btn::Right);
    assert_ne!(
        f.app().settings_text(SettingsRow::Update),
        was,
        "Right changed nothing"
    );
    // the switch is said in a toast over the top of the screen: the row itself is what changed
    let px = composed(&mut f, &mut c, "update-after");
    let row = row_rect(&f, SettingsRow::Update);
    holds_the_modal_in(&px, &f, "after", Some(row));
}
