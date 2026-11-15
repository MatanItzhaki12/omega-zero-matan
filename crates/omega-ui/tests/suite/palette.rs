use std::collections::HashSet;

use omega_store::{colour_index, BACKDROP_COLOURS, COLOUR_COUNT};
use omega_ui::kit::palette::{palette, rgba, Palette};
use omega_ui::BACKDROP_PALETTE;

/// What assets/ui/foundation/palette.js gives for each backdrop colour, in palette.js's order:
/// --oz-bg, then the ramp darkest first. To make it again, run this from the repository's root
/// and paste what it prints:
///
/// ```text
/// node -e '
/// globalThis.window = globalThis;
/// globalThis.CustomEvent = class {};
/// globalThis.OmegaBackdrop = { paint() {} };
/// globalThis.document = { documentElement: { style: { setProperty() {} }, dataset: {} }, dispatchEvent() {} };
/// eval(require("fs").readFileSync("assets/ui/foundation/palette.js", "utf8"));
/// for (const name of OmegaPalette.COLOURS) {
///   const hex = Object.values(OmegaPalette.palette(name)).map(c => `"${c}"`);
///   console.log(`("${name}", [${hex.join(", ")}]),`);
/// }'
/// ```
const PALETTE_JS: [(&str, [&str; 13]); 14] = [
    (
        "night",
        [
            "#050508", "#181818", "#494949", "#383838", "#595959", "#636363", "#7d7d7d", "#828282",
            "#9d9d9d", "#b7b7b7", "#c4c4c4", "#cfcfcf", "#e6e6e6",
        ],
    ),
    (
        "slate",
        [
            "#22252c", "#16181a", "#454a4f", "#35383c", "#56595e", "#60646a", "#797e85", "#7d828a",
            "#9a9ca6", "#b2b7c1", "#c0c5d0", "#cbcfd7", "#e5e6ea",
        ],
    ),
    (
        "navy",
        [
            "#101b36", "#04192e", "#1d4b78", "#1b3a59", "#385a86", "#416499", "#547ec0", "#4d84ca",
            "#8397ea", "#91b6ff", "#a9c4ff", "#b9ceff", "#dfe5ff",
        ],
    ),
    (
        "teal",
        [
            "#0b2b2e", "#001d1b", "#005551", "#0f403d", "#266463", "#227172", "#2b8d90", "#239492",
            "#47adbf", "#57cbd3", "#53dbe6", "#8cdfe8", "#c5eef8",
        ],
    ),
    (
        "green",
        [
            "#102c16", "#0e1d04", "#34531c", "#2a3f1a", "#416436", "#42713d", "#538e4f", "#5a9348",
            "#53b37b", "#7bce82", "#7bdf8c", "#9de3aa", "#c7f2d8",
        ],
    ),
    (
        "plum",
        [
            "#291431", "#1c122b", "#523c72", "#3e2f55", "#644d7d", "#73548c", "#9269af", "#946dbc",
            "#c182c9", "#d59df3", "#e5aaff", "#ebbafc", "#fbd9fb",
        ],
    ),
    (
        "crimson",
        [
            "#300f16", "#290d19", "#6e324c", "#52293a", "#7d4558", "#8f4b5f", "#b45f77", "#bc6080",
            "#df7b82", "#fe91a8", "#ffa9b7", "#ffb9c2", "#ffdcdc",
        ],
    ),
    (
        "amber",
        [
            "#302208", "#261300", "#6a3d01", "#4f300d", "#765020", "#845a1a", "#a57221", "#b0731b",
            "#be9732", "#e6ab47", "#f8b83f", "#f2c87d", "#f5e5bb",
        ],
    ),
    (
        "trainer",
        [
            "#171719", "#22060b", "#5e202c", "#451b22", "#693136", "#793538", "#984447", "#a0444d",
            "#b95d49", "#d66c68", "#ea726a", "#ffbbb2", "#ffddd4",
        ],
    ),
    (
        "ruby",
        [
            "#25171c", "#21060e", "#5c2034", "#441a27", "#68303d", "#783441", "#974351", "#9e4359",
            "#ba5a56", "#d56b75", "#e9707a", "#ffbaba", "#ffddd8",
        ],
    ),
    (
        "sapphire",
        [
            "#181c28", "#001124", "#013a64", "#092c49", "#21466f", "#274e80", "#3462a1", "#2a68a9",
            "#5e76c5", "#5f90e1", "#6799f7", "#b7cfff", "#dde5ff",
        ],
    ),
    (
        "emerald",
        [
            "#17231f", "#041501", "#1c4310", "#173210", "#275027", "#235b2d", "#2c733b", "#327734",
            "#179163", "#46a766", "#3db66e", "#95e4b1", "#c4f3dc",
        ],
    ),
    (
        "firered",
        [
            "#281c1a", "#220607", "#5f2122", "#461b1b", "#6a322d", "#79362d", "#984639", "#a0453f",
            "#b7603a", "#d56f57", "#e87557", "#ffbda8", "#ffdecf",
        ],
    ),
    (
        "leafgreen",
        [
            "#1d251b", "#0e1300", "#313e00", "#252f03", "#3a4c17", "#3c5718", "#4b6f22", "#537212",
            "#498e48", "#6da248", "#6fb04c", "#addf9c", "#cef1d0",
        ],
    ),
];

/// A palette's colours in palette.js's order, each by the --oz-* property it is in the kit.
fn tokens(p: &Palette) -> [(&'static str, [u8; 3]); 13] {
    [
        ("--oz-bg", p.bg),
        ("--oz-dark", p.dark),
        ("--oz-edge", p.edge),
        ("--oz-well", p.well),
        ("--oz-plate", p.plate),
        ("--oz-tab", p.tab),
        ("--oz-band", p.band),
        ("--oz-stripe", p.stripe),
        ("--oz-dim", p.dim),
        ("--oz-frame", p.frame),
        ("--oz-frame-hi", p.frame_hi),
        ("--oz-label", p.label),
        ("--oz-cream", p.cream),
    ]
}

fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn named(name: &str) -> &'static Palette {
    palette(colour_index(name).expect("a backdrop colour"))
}

#[test]
fn every_colour_is_the_one_palette_js_gives() {
    let mut wrong = Vec::new();
    for (colour, (name, want)) in PALETTE_JS.iter().enumerate() {
        assert_eq!(BACKDROP_COLOURS[colour], *name, "the table is out of order");
        for ((token, got), want) in tokens(palette(colour as u8)).into_iter().zip(want) {
            if hex(got) != *want {
                wrong.push(format!("{name}'s {token} is {}, not {want}", hex(got)));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn each_palette_sits_on_the_backdrop_the_shelf_is_drawn_on() {
    for (colour, rgb) in BACKDROP_PALETTE.into_iter().enumerate() {
        let name = BACKDROP_COLOURS[colour];
        assert_eq!(palette(colour as u8).bg, rgb, "{name}");
    }
}

#[test]
fn amber_s_darkest_brown_leans_toward_red_only_as_far_as_the_others_may() {
    // The game's own is #2a0f04, further from the ramp's hue than 15 degrees.
    assert_eq!(hex(named("amber").dark), "#261300");
}

#[test]
fn night_turns_the_ramp_to_plain_greys() {
    for (token, [r, g, b]) in tokens(named("night")).into_iter().skip(1) {
        assert!(r == g && g == b, "Night's {token} is {}", hex([r, g, b]));
    }
}

#[test]
fn a_colour_past_the_last_is_night() {
    for colour in [COLOUR_COUNT, 60, 200, u8::MAX] {
        assert_eq!(palette(colour), named("night"), "{colour}");
    }
}

#[test]
fn every_sprite_key_is_its_colour_and_nothing_else_is_one() {
    // Plum's steps all differ, so a key that stood for the wrong one would show.
    let p = named("plum");
    let keys = [
        ('k', p.dark),
        ('e', p.edge),
        ('w', p.well),
        ('p', p.plate),
        ('t', p.tab),
        ('b', p.band),
        ('s', p.stripe),
        ('d', p.dim),
        ('A', p.frame),
        ('H', p.frame_hi),
        ('L', p.label),
        ('c', p.cream),
    ];
    let steps: HashSet<_> = keys.iter().map(|(_, colour)| colour).collect();
    assert_eq!(steps.len(), keys.len(), "plum repeats a step");
    for (k, colour) in keys {
        assert_eq!(p.key(k), Some(colour), "{k}");
    }
    for k in ['.', ' ', 'D', 'K', 'E', 'a', 'h', 'l', 'C', 'x'] {
        assert_eq!(p.key(k), None, "{k:?}");
    }
}

#[test]
fn rgba_is_the_colour_out_of_one_with_its_alpha() {
    assert_eq!(rgba([255, 0, 51], 0.5), [1.0, 0.0, 0.2, 0.5]);
}
