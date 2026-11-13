//! The cart going into the handheld: from where the shelf left it to the middle of the screen,
//! a little rise, then a pull down out of the bottom of the screen; coming out, the same played
//! backwards, a refused cart carrying its alert.

use omega_store::{Cart, Platform};
use omega_ui::kit::palette::palette;
use omega_ui::{
    alert_face, foot_y, label_colour, label_text, model_box, model_of, model_scale,
    model_shelf_box, out_h, out_w, shelf_box, shelf_scale, stage_y, CartDive, CartModel, Draw,
    MeshId, Pose, Shelf, SnesShell, TexId, ALERT_H, ALERT_W, SPRITE_PX, TOP_BAR_H,
};

/// What the app hands the dive to draw with: the cart's face, its model, and the alert.
const FACE: TexId = TexId::from_raw(3);
const MESH: MeshId = MeshId::from_raw(1);
const ALERT: TexId = TexId::from_raw(77);

/// How far into its way in the cart has come to the middle of the row, square and level: the
/// dive's SETTLE.
const SETTLE: f32 = 0.35;

/// Just short of all the way in: the last moment the cart is drawn.
const LAST: f32 = 1.0 - 1e-4;

/// A cart turned and floating the way the shelf holds the one in the middle while it sways.
const SWAYING: Pose = Pose {
    yaw: 0.17,
    lift: 3.0,
};

fn cart() -> Cart {
    Cart {
        platform: Platform::Gba,
        stem: "Emerald".into(),
        rom: "Games/GBA/Emerald.gba".into(),
        label: None,
        code: String::new(),
        shell: None,
        title: "POKEMON EMER".into(),
    }
}

fn pak() -> Cart {
    Cart {
        platform: Platform::Gb,
        stem: "Tetris".into(),
        rom: "Games/GB/Tetris.gb".into(),
        label: None,
        code: String::new(),
        shell: None,
        title: "TETRIS".into(),
    }
}

/// A SNES cart named `stem`, its shell told by its tags.
fn snes(stem: &str) -> Cart {
    Cart {
        platform: Platform::Snes,
        stem: stem.into(),
        rom: format!("Games/SNES/{stem}.sfc").into(),
        label: None,
        code: String::new(),
        shell: None,
        title: String::new(),
    }
}

fn disc() -> Cart {
    Cart {
        platform: Platform::Ps1,
        stem: "Vagrant Story".into(),
        rom: "Games/PS1/Vagrant Story.cue".into(),
        label: None,
        code: String::new(),
        shell: None,
        title: String::new(),
    }
}

/// A cart of every shape that dives: the GBA cart, the Game Boy pak, the SNES cart in the
/// American shell and the Super Famicom's, and the PlayStation disc.
fn both() -> [(&'static str, Cart); 5] {
    [
        ("the GBA cart", cart()),
        ("the Game Boy pak", pak()),
        ("the American SNES cart", snes("Super Metroid (USA)")),
        ("the Super Famicom cart", snes("Rockman X (Japan)")),
        ("the PlayStation disc", disc()),
    ]
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct Quad {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Quad {
    fn middle(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

fn quad(d: &Draw) -> Quad {
    match *d {
        Draw::Rect { x, y, w, h, .. }
        | Draw::Tex { x, y, w, h, .. }
        | Draw::Turned { x, y, w, h, .. }
        | Draw::Spun { x, y, w, h, .. }
        | Draw::Model { x, y, w, h, .. } => Quad { x, y, w, h },
        Draw::Game | Draw::Shot { .. } => Quad {
            x: 0.0,
            y: 0.0,
            w: out_w() as f32,
            h: out_h() as f32,
        },
    }
}

/// The middle of the screen.
fn mid() -> f32 {
    out_w() as f32 / 2.0
}

/// `v` is on a whole art pixel.
fn on_grid(v: f32) -> bool {
    let px = SPRITE_PX as f32;
    ((v / px).round() * px - v).abs() < 0.01
}

/// Where a shelf holding `c` alone stands it, and how big, as the app hands them to the dive.
fn on_shelf(c: &Cart) -> (f32, f32) {
    Shelf::new(vec![c.clone()]).selected_at()
}

/// `c` going in from a shelf that holds it alone, `seat` of the way: its face drawn flat and
/// square, no alert, the screen behind it still dark.
fn going_in(c: &Cart, seat: f32) -> CartDive<'_> {
    let (rest, scale) = on_shelf(c);
    CartDive {
        cart: c,
        face: Some(FACE),
        mesh: None,
        stack: None,
        rest,
        scale,
        pose: Pose::REST,
        seat,
        alert: None,
        dim: 0.0,
        screen: 0.0,
        game: false,
        disc: None,
    }
}

fn drawn(dive: &CartDive) -> Vec<Draw> {
    let mut out = Vec::new();
    dive.draw(&mut out);
    out
}

/// The cart's face in `out`, where it is and how far it is turned, if it is drawn at all.
fn face_in(out: &[Draw]) -> Option<(Quad, f32)> {
    out.iter().find_map(|d| match *d {
        Draw::Tex { tex, .. } if tex == FACE => Some((quad(d), 0.0)),
        Draw::Spun {
            tex, yaw, depth, ..
        } if tex == FACE && depth == 0.0 => Some((quad(d), yaw)),
        Draw::Model { tex, yaw, .. } if tex == FACE => Some((quad(d), yaw)),
        _ => None,
    })
}

/// Where `c` is drawn `seat` of the way in.
fn cart_at(c: &Cart, seat: f32) -> Quad {
    face_in(&drawn(&going_in(c, seat)))
        .unwrap_or_else(|| panic!("no cart drawn {seat} of the way in"))
        .0
}

/// The top of `c` at `steps + 1` moments evenly along its way in, the last just short of all the
/// way, with how far in each is.
fn tops(c: &Cart, steps: usize) -> Vec<(f32, f32)> {
    (0..=steps)
        .map(|i| (i as f32 / steps as f32).min(LAST))
        .map(|seat| (seat, cart_at(c, seat).y))
        .collect()
}

/// The full-screen black the shelf goes dark under, at `alpha`.
fn dark(alpha: f32) -> Draw {
    Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: out_w() as f32,
        h: out_h() as f32,
        colour: [0.0, 0.0, 0.0, alpha],
    }
}

#[test]
fn an_unseated_cart_stands_where_the_shelf_left_it() {
    for (name, c) in both() {
        let mut shelf = Shelf::new(vec![c.clone()]);
        shelf.set_faces(vec![FACE]);
        let mut row = Vec::new();
        shelf.draw_row(None, 0.0, 0.0, 1.0, &mut row);
        let (on_shelf, _) = face_in(&row).expect("the shelf did not draw the cart");
        let leaving = cart_at(&c, 0.0);
        assert_eq!(
            on_shelf, leaving,
            "{name} jumps from {on_shelf:?} on the shelf to {leaving:?} as it starts in"
        );
    }
}

#[test]
fn the_cart_rises_a_little_before_it_goes_down() {
    for (name, c) in both() {
        let ys = tops(&c, 200);
        let start = ys[0].1;
        let (peak, &(at, top)) = ys
            .iter()
            .enumerate()
            .min_by(|a, b| a.1 .1.total_cmp(&b.1 .1))
            .expect("no way in");
        let rise = start - top;
        let h = model_shelf_box(model_of(&c)).1;
        assert!(
            rise > 2.0 && rise < h / 10.0,
            "{name} rises {rise} px off the {start} it started at: not the little lift of a \
             hand letting go"
        );
        assert!(
            at > 0.0 && at <= 0.25,
            "{name} is at its highest {at} of the way in, not on its way out of the hand"
        );
        for pair in ys[..=peak].windows(2) {
            assert!(
                pair[1].1 <= pair[0].1 + 0.001,
                "{name} goes down to {} at {} on its way up",
                pair[1].1,
                pair[1].0
            );
        }
        for pair in ys[peak..].windows(2) {
            assert!(
                pair[1].1 >= pair[0].1 - 0.001,
                "{name} comes back up to {} at {} on its way down",
                pair[1].1,
                pair[1].0
            );
        }
    }
}

#[test]
fn the_cart_is_pulled_down_faster_and_faster() {
    for (name, c) in both() {
        let ys: Vec<(f32, f32)> = (5..=19)
            .map(|i| i as f32 * 0.05)
            .map(|seat| (seat, cart_at(&c, seat).y))
            .collect();
        let drops: Vec<(f32, f32)> = ys
            .windows(2)
            .map(|pair| (pair[1].0, pair[1].1 - pair[0].1))
            .collect();
        // more than a rounding error more, so a steady fall does not pass for a gathering one
        for pair in drops.windows(2) {
            assert!(
                pair[1].1 > pair[0].1 + 0.01,
                "{name} drops {} px in the twentieth to {} after {} in the one before: it is \
                 not gathering speed",
                pair[1].1,
                pair[1].0,
                pair[0].1
            );
        }
    }
}

#[test]
fn all_the_way_in_the_cart_is_out_of_the_bottom_of_the_screen() {
    for (name, c) in both() {
        let last = cart_at(&c, LAST);
        assert!(
            last.y >= out_h() as f32 - 1.0,
            "{name}'s top is at {} just before it is all the way in: it vanishes {} px short \
             of the bottom of the screen",
            last.y,
            out_h() as f32 - last.y
        );
        for seat in [1.0, 1.5] {
            let out = drawn(&CartDive {
                alert: Some((ALERT, 1.0)),
                dim: 0.5,
                screen: 0.5,
                game: true,
                ..going_in(&c, seat)
            });
            assert_eq!(
                out,
                [dark(0.5), Draw::Game],
                "at seat {seat} something of {name} is still drawn"
            );
        }
    }
}

/// Coming out, the app runs the seat back from 1 to 0: the cart comes up out of the bottom of the
/// screen fast, slows to a stop a little over its place, and comes down onto it.
#[test]
fn coming_out_is_the_way_in_played_backwards() {
    for (name, c) in both() {
        let steps = 200;
        let ys: Vec<Option<f32>> = (0..=steps)
            .map(|i| 1.0 - i as f32 / steps as f32)
            .map(|seat| face_in(&drawn(&going_in(&c, seat))).map(|(q, _)| q.y))
            .collect();
        assert_eq!(ys[0], None, "{name} is drawn before it has started out");
        let ys: Vec<f32> = ys[1..]
            .iter()
            .map(|y| y.unwrap_or_else(|| panic!("{name} went missing on its way out")))
            .collect();
        assert!(
            ys[0] > out_h() as f32 * 0.9,
            "{name} first shows with its top at {}, not out of the bottom of the screen",
            ys[0]
        );
        let home = cart_at(&c, 0.0).y;
        assert_eq!(
            ys[ys.len() - 1],
            home,
            "{name} does not come back to where the shelf left it"
        );
        let peak = (0..ys.len())
            .min_by(|&a, &b| ys[a].total_cmp(&ys[b]))
            .expect("no way out");
        assert!(
            ys[peak] < home,
            "{name} comes straight down onto its place, never a little over it"
        );
        for (i, pair) in ys[..=peak].windows(2).enumerate() {
            assert!(
                pair[1] <= pair[0] + 0.001,
                "{name} drops back to {} at step {i} on its way up out of the handheld",
                pair[1]
            );
        }
        for pair in ys[peak..].windows(2) {
            assert!(
                pair[1] >= pair[0] - 0.001,
                "{name} goes back up to {} as it comes down onto its place",
                pair[1]
            );
        }
    }
}

#[test]
fn a_cart_keeps_its_shelf_size_all_the_way_in() {
    for (name, c) in both() {
        let (w, h) = model_box(model_of(&c));
        let (sw, sh) = model_shelf_box(model_of(&c));
        let share = model_scale(model_of(&c));
        assert_eq!(
            (sw, sh),
            (w as f32 * share, h as f32 * share),
            "{name} stands on the shelf at another size"
        );
        for step in 0..=400 {
            let seat = (step as f32 / 400.0).min(LAST);
            let q = cart_at(&c, seat);
            assert!(
                (q.w - sw).abs() < 0.01 && (q.h - sh).abs() < 0.01,
                "{name} is {}x{} at seat {seat} rather than the {sw}x{sh} it stands on the \
                 shelf at",
                q.w,
                q.h
            );
        }
    }
    assert_eq!(shelf_box(Platform::Gba), (360.0, 212.0), "the GBA cart");
    assert_eq!(shelf_scale(Platform::Gb), 0.8, "the Game Boy pak");
    assert_eq!(
        model_shelf_box(CartModel::Snes(SnesShell::Ntsc)),
        (376.0, 242.0),
        "the American SNES cart"
    );
    assert_eq!(
        model_shelf_box(CartModel::Snes(SnesShell::Famicom)),
        (360.0, 238.0),
        "the Super Famicom cart"
    );
}

#[test]
fn the_cart_comes_to_the_middle_of_the_screen_square_and_level_as_it_settles() {
    for (name, c) in both() {
        let at = |seat: f32| {
            face_in(&drawn(&CartDive {
                mesh: Some(MESH),
                pose: SWAYING,
                ..going_in(&c, seat)
            }))
            .unwrap_or_else(|| panic!("no {name} drawn at seat {seat}"))
        };
        let (start, yaw) = at(0.0);
        assert_eq!(yaw, SWAYING.yaw, "{name} left the row turned another way");
        let sh = model_shelf_box(model_of(&c)).1;
        assert!(
            (start.y + start.h - (foot_y(sh) - SWAYING.lift)).abs() < 0.01,
            "{name} left the row at {}, not floating where the row had it",
            start.y
        );
        let mut last = yaw;
        for step in 1..=20 {
            let seat = SETTLE * step as f32 / 20.0;
            let (_, yaw) = at(seat);
            assert!(
                (0.0..=last).contains(&yaw),
                "{name} turned to {yaw} at seat {seat} from {last}: it is not squaring up"
            );
            last = yaw;
        }
        for step in 0..=20 {
            let seat = (SETTLE + (1.0 - SETTLE) * step as f32 / 20.0).min(LAST);
            let (q, yaw) = at(seat);
            assert_eq!(yaw, 0.0, "{name} is still turned at seat {seat}");
            let (x, _) = q.middle();
            assert!(
                on_grid(x) && (x - mid()).abs() <= SPRITE_PX as f32 / 2.0,
                "{name}'s middle is at {x} at seat {seat}, not in the middle of the screen at \
                 {} on the art grid",
                mid()
            );
            let level = cart_at(&c, seat);
            assert!(
                (q.y - level.y).abs() < 0.01,
                "{name} is at {} at seat {seat} against {} for a cart that never floated: it \
                 is still floating",
                q.y,
                level.y
            );
        }
    }
}

#[test]
fn a_cart_the_row_had_not_finished_moving_slides_across_as_it_goes_in() {
    for (name, c) in both() {
        let mut shelf = Shelf::new(vec![c.clone(), c.clone(), c.clone()]);
        shelf.right();
        shelf.update(1.0 / 60.0);
        shelf.update(1.0 / 60.0);
        let (rest, scale) = shelf.selected_at();
        let (sw, sh) = model_shelf_box(model_of(&c));
        assert!(
            (rest - (mid() - sw / 2.0)).abs() > 100.0 && scale < 0.95 * model_scale(model_of(&c)),
            "the row settled before the press: this proves nothing at {rest} and {scale}"
        );
        let at = |seat: f32| {
            face_in(&drawn(&CartDive {
                rest,
                scale,
                ..going_in(&c, seat)
            }))
            .unwrap_or_else(|| panic!("no {name} drawn at seat {seat}"))
            .0
        };
        let start = at(0.0);
        let own = model_box(model_of(&c)).0 as f32;
        assert!(
            (start.x - rest).abs() < 0.01 && (start.w - own * scale).abs() < 0.01,
            "{name} starts at {start:?} rather than in the quad the row had it in"
        );
        assert!(
            (start.y + start.h - foot_y(sh)).abs() < 0.01,
            "{name} starts with its foot at {} rather than on the row's floor",
            start.y + start.h
        );
        let settled = at(SETTLE);
        assert!(
            (settled.middle().0 - mid()).abs() <= 1.0 && (settled.w - sw).abs() < 0.01,
            "{name} settles at {settled:?} rather than at its shelf size in the middle"
        );
        let off = |q: Quad| (q.middle().0 - mid()).abs();
        let mut last = start;
        for step in 1..=20 {
            let q = at((step as f32 / 20.0).min(LAST));
            assert!(
                off(q) <= off(last) + 0.01,
                "{name} went back to {} from {}",
                q.x,
                last.x
            );
            assert!(
                (q.w - sw).abs() <= (last.w - sw).abs() + 0.01,
                "{name} moved away from its shelf size, to {} from {}",
                q.w,
                last.w
            );
            last = q;
        }
    }
}

#[test]
fn the_rise_never_takes_a_cart_into_the_bar_at_the_top() {
    let ceiling = stage_y() + TOP_BAR_H;
    for (name, c) in both() {
        for (seat, top) in tops(&c, 200) {
            assert!(
                top >= ceiling,
                "{name}'s top is at {top} at seat {seat}, under the bar at {ceiling}"
            );
        }
    }
}

/// However the cart is drawn, as its model, its face spun flat, or a bare label colour while its
/// face is still on its way, it goes in the same way.
#[test]
fn a_cart_dives_the_same_way_however_it_is_drawn() {
    for (name, c) in both() {
        let [r, g, b] = label_colour(&label_text(&c)).map(|v| v as f32 / 255.0);
        for step in 0..=20 {
            let seat = (step as f32 / 20.0).min(LAST);
            let flat = cart_at(&c, seat);
            let model = face_in(&drawn(&CartDive {
                mesh: Some(MESH),
                ..going_in(&c, seat)
            }))
            .map(|(q, _)| q);
            assert_eq!(
                model,
                Some(flat),
                "{name}'s model and its face part at seat {seat}"
            );
            let bare = drawn(&CartDive {
                face: None,
                ..going_in(&c, seat)
            });
            assert_eq!(
                bare,
                [Draw::Rect {
                    x: flat.x,
                    y: flat.y,
                    w: flat.w,
                    h: flat.h,
                    colour: [r, g, b, 1.0],
                }],
                "{name} without its face is not its label colour where its face would be, at \
                 seat {seat}"
            );
        }
    }
}

/// The slot under the shelf is gone: all a dive draws is the dark over the shelf, the game's
/// screen coming on, the cart and its alert.
#[test]
fn nothing_but_the_dark_the_game_the_cart_and_its_alert_is_drawn() {
    #[derive(Copy, Clone, PartialEq, Debug)]
    enum Part {
        Dark,
        Game,
        Cart,
        Alert,
    }
    for (name, c) in both() {
        for step in 0..=20 {
            let seat = step as f32 / 20.0;
            let out = drawn(&CartDive {
                mesh: Some(MESH),
                pose: SWAYING,
                alert: Some((ALERT, 0.8)),
                dim: 0.6,
                screen: 0.5,
                game: true,
                ..going_in(&c, seat)
            });
            let parts: Vec<Part> = out
                .iter()
                .map(|d| match *d {
                    _ if *d == dark(0.6) => Part::Dark,
                    Draw::Game => Part::Game,
                    Draw::Model { tex, mesh, .. } if tex == FACE && mesh == MESH => Part::Cart,
                    Draw::Tex { tex, alpha, .. } if tex == ALERT && alpha == 0.8 => Part::Alert,
                    ref other => panic!("at seat {seat} {name}'s dive draws {other:?}"),
                })
                .collect();
            let want: &[Part] = match seat < 1.0 {
                true => &[Part::Dark, Part::Game, Part::Cart, Part::Alert],
                false => &[Part::Dark, Part::Game],
            };
            assert_eq!(
                parts, want,
                "at seat {seat} {name}'s dive is not the dark, the game's screen behind the \
                 cart, and the alert over it"
            );
        }
    }
}

#[test]
fn a_dark_screen_lists_no_game_layer() {
    let c = cart();
    let games = |game: bool, screen: f32| {
        drawn(&CartDive {
            game,
            screen,
            ..going_in(&c, 0.5)
        })
        .iter()
        .filter(|d| matches!(d, Draw::Game))
        .count()
    };
    assert_eq!(
        games(true, 0.0),
        0,
        "the picture is drawn with the screen off"
    );
    assert_eq!(
        games(false, 1.0),
        0,
        "a picture is drawn with no game to show"
    );
    assert_eq!(
        games(true, 0.01),
        1,
        "the picture is not drawn as the screen comes on"
    );
}

#[test]
fn dim_draws_a_black_rect_over_the_whole_screen_at_its_alpha() {
    let c = cart();
    let first = |dim: f32, seat: f32| {
        drawn(&CartDive {
            dim,
            ..going_in(&c, seat)
        })
        .first()
        .copied()
        .expect("nothing drawn")
    };
    for seat in [0.0, 0.5, 1.0] {
        for (dim, alpha) in [(0.25, 0.25), (0.5, 0.5), (1.0, 1.0), (1.5, 1.0)] {
            assert_eq!(
                first(dim, seat),
                dark(alpha),
                "at seat {seat} a dim of {dim} is not black over the whole screen at {alpha}"
            );
        }
    }
    assert!(
        drawn(&going_in(&c, 0.5))
            .iter()
            .all(|d| quad(d).w < out_w() as f32),
        "the screen goes dark with no dim"
    );
}

#[test]
fn the_alert_fits_on_the_cart_face() {
    let (w, h) = ((ALERT_W * SPRITE_PX) as f32, (ALERT_H * SPRITE_PX) as f32);
    for (name, c) in both() {
        let (cw, ch) = model_shelf_box(model_of(&c));
        assert!(w < cw && h < ch, "a {w}x{h} alert on {name} at {cw}x{ch}");
        assert!(
            h * 6.0 > ch,
            "a {h} px alert on {name} {ch} px tall is a speck"
        );
    }
}

/// The alert is a sprite of the kit's: an art pixel to a texel, every one of them solid or
/// clear, in the palette's bright frame colour ruled in its dark one, whatever the colour.
#[test]
fn the_alert_is_a_sprite_in_the_palettes_bright_frame_colour() {
    for name in ["night", "amber", "teal"] {
        let pal = palette(omega_store::colour_index(name).expect("a backdrop colour"));
        let face = alert_face(pal);
        assert_eq!((face.w, face.h), (ALERT_W, ALERT_H), "{name}");
        let mut inks: Vec<[u8; 3]> = Vec::new();
        for p in face.rgba.chunks(4) {
            assert!(p[3] == 0 || p[3] == 255, "{name}: a soft pixel");
            if p[3] == 255 && !inks.contains(&[p[0], p[1], p[2]]) {
                inks.push([p[0], p[1], p[2]]);
            }
        }
        inks.sort_unstable();
        let mut want = vec![pal.frame_hi, pal.dark];
        want.sort_unstable();
        assert_eq!(
            inks, want,
            "{name}: the alert is not in the frame's colours"
        );
    }
}

/// The alert lands on whole art pixels at two screen pixels to one of its own, in the middle of
/// the cart to the nearest art pixel, wherever the cart coming out under it is.
#[test]
fn the_alert_is_drawn_on_the_art_grid_in_the_middle_of_the_cart_as_it_comes_out() {
    let (w, h) = ((ALERT_W * SPRITE_PX) as f32, (ALERT_H * SPRITE_PX) as f32);
    for (name, c) in both() {
        for step in 1..=20 {
            let seat = 1.0 - step as f32 / 20.0;
            let out = drawn(&CartDive {
                pose: SWAYING,
                alert: Some((ALERT, 1.0)),
                ..going_in(&c, seat)
            });
            let alert = out
                .iter()
                .find_map(|d| match *d {
                    Draw::Tex { tex, .. } if tex == ALERT => Some(quad(d)),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no alert on {name} at seat {seat}"));
            assert_eq!(
                (alert.w, alert.h),
                (w, h),
                "{name}'s alert is scaled at seat {seat}"
            );
            assert!(
                [alert.x, alert.y].into_iter().all(on_grid),
                "at seat {seat} {name}'s alert is at {alert:?}, off the art grid"
            );
            let (cart, _) = face_in(&out).expect("no cart under the alert");
            let ((ax, ay), (cx, cy)) = (alert.middle(), cart.middle());
            assert!(
                (ax - cx).abs() <= SPRITE_PX as f32 / 2.0 + 0.01
                    && (ay - cy).abs() <= SPRITE_PX as f32 / 2.0 + 0.01,
                "at seat {seat} {name}'s alert is in the middle at {ax}, {ay} against the cart's \
                 {cx}, {cy}"
            );
        }
    }
}
