//! The PlayStation disc: its model, rim and hole; its face, cut to the circle and the hole; a
//! game on several discs as a stack, each disc its own model and face, drawn back to front on
//! the shelf and into the handheld.

use omega_gfx::{MeshVertex, FOCAL};
use omega_store::{scan, Cart};
use omega_ui::{
    cart_face, cart_mesh, cut_label, foot_y, generated_face, label_colour, label_text, label_uncut,
    model_shelf_box, out_w, rest_y, stage_y, wears_label, CartDive, CartFace, CartMeshes,
    CartModel, DiscStack, Draw, MeshId, Pose, Shelf, StackDive, StackMeshes, TexId, BAR_H, DISC_PX,
    MOCKUP_H, STACK_DIVE, TOP_BAR_H,
};
use tempfile::TempDir;

/// The selected cart sways this far either way and floats this many pixels up and down.
const SWAY_DEG: i32 = 20;
const BOB: f32 = 4.0;

/// A card holding `games`, each a name and how many discs it is on: a lone disc is a cue
/// sheet, a game on several an .m3u playlist whose discs sit in a folder of its own.
fn card(games: &[(&str, usize)]) -> TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    let dir = d.path().join("Games/PS1");
    std::fs::create_dir_all(&dir).unwrap();
    for (stem, discs) in games {
        if *discs == 1 {
            std::fs::write(dir.join(format!("{stem}.cue")), "").unwrap();
            continue;
        }
        std::fs::create_dir_all(dir.join(stem)).unwrap();
        let mut list = String::new();
        for n in 1..=*discs {
            let disc = format!("{stem} (Disc {n})");
            std::fs::write(dir.join(stem).join(format!("{disc}.cue")), "").unwrap();
            list.push_str(&format!("{stem}/{disc}.cue\n"));
        }
        std::fs::write(dir.join(format!("{stem}.m3u")), list).unwrap();
    }
    d
}

fn carts(d: &TempDir) -> Vec<Cart> {
    scan(d.path()).expect("scan the card")
}

/// A label for `name` under Labels/PS1, all one colour.
fn label(d: &TempDir, name: &str, rgb: [u8; 3]) {
    let dir = d.path().join("Labels/PS1");
    std::fs::create_dir_all(&dir).unwrap();
    let f = std::fs::File::create(dir.join(format!("{name}.png"))).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), 64, 64);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .unwrap()
        .write_image_data(&rgb.repeat(64 * 64))
        .unwrap();
}

fn pixel(face: &CartFace, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * face.w + x) * 4) as usize;
    [
        face.rgba[i],
        face.rgba[i + 1],
        face.rgba[i + 2],
        face.rgba[i + 3],
    ]
}

/// How far the art pixel holding face pixel `x, y` of a disc lies from the disc's middle, in
/// art pixels.
fn art_radius(x: u32, y: u32) -> f32 {
    let r = DISC_PX as f32 / 4.0;
    let (ax, ay) = ((x / 2) as f32 + 0.5 - r, (y / 2) as f32 + 0.5 - r);
    (ax * ax + ay * ay).sqrt()
}

/// A point in a disc's print, clear of its words, as `x, y` across one disc's face.
const PRINT_AT: (u32, u32) = (DISC_PX / 2 + 90, DISC_PX / 2);

/// The walls of a mesh: every quad standing upright.
fn walls(mesh: &[MeshVertex]) -> Vec<&[MeshVertex]> {
    mesh.chunks_exact(6)
        .filter(|quad| quad[0].normal[2] == 0.0)
        .collect()
}

/// How far the middle of a wall's front edge is from the middle of the disc, and which way
/// from the middle it faces: positive out, negative in.
fn wall_place(quad: &[MeshVertex]) -> (f32, f32) {
    let (a, b) = (quad[0].xyz, quad[1].xyz);
    let (mx, my) = ((a[0] + b[0]) / 2.0 - 0.5, (a[1] + b[1]) / 2.0 - 0.5);
    let r = (mx * mx + my * my).sqrt();
    let [nx, ny, _] = quad[0].normal;
    (r, (nx * mx + ny * my) / r)
}

#[test]
fn the_rim_faces_out_of_the_disc_and_the_holes_walls_face_into_the_hole() {
    let mesh = cart_mesh(CartModel::Disc);
    let walls = walls(&mesh);
    let (rim, hole): (Vec<&[MeshVertex]>, Vec<&[MeshVertex]>) =
        walls.iter().partition(|q| wall_place(q).0 > 0.25);
    assert!(
        (20..=28).contains(&rim.len()),
        "the rim is cut {} times, not the 20 to 28 of a low-poly disc",
        rim.len()
    );
    assert!(hole.len() >= 8, "the hole is cut {} times", hole.len());
    for quad in &rim {
        let (r, out) = wall_place(quad);
        assert!(
            out > 0.99,
            "the rim's wall {r} out faces {out} out of the disc, not straight out"
        );
    }
    for quad in &hole {
        let (r, out) = wall_place(quad);
        assert!(
            out < -0.99,
            "the hole's wall {r} out faces {out} out of the disc, not into the hole"
        );
        assert!(
            r < 0.08,
            "the hole's wall stands {r} out, not round the hole"
        );
    }
    let thick = mesh.iter().map(|v| v.xyz[2]).fold(0.0, f32::max);
    assert!(thick > 0.0, "the disc has no thickness");
    for quad in walls {
        assert!(
            quad.iter().all(|v| v.xyz[2] == 0.0 || v.xyz[2] == thick),
            "a wall does not run from the face back to the disc's thickness"
        );
    }
}

/// A wall takes its colour from one point of the face: the rim from the clear edge, the hole's
/// from the lip round it, both solid enough and light enough to read against the black print.
#[test]
fn the_walls_take_the_clear_plastic_of_the_edge_and_the_lip() {
    let d = card(&[("Vagrant Story (USA)", 1)]);
    let face = cart_face(&carts(&d)[0]);
    let print = pixel(&face, PRINT_AT.0, PRINT_AT.1);
    let luma = |p: [u8; 4]| p[0] as u32 + p[1] as u32 + p[2] as u32;
    for quad in walls(&cart_mesh(CartModel::Disc)) {
        let uv = quad[0].uv;
        assert!(
            quad.iter().all(|v| v.uv == uv),
            "a wall takes its colour from more than one point of the face"
        );
        let p = pixel(
            &face,
            (uv[0] * face.w as f32) as u32,
            (uv[1] * face.h as f32) as u32,
        );
        let (r, _) = wall_place(quad);
        assert!(
            p[3] >= 0xc0 && luma(p) > luma(print) + 200,
            "the wall {r} out takes {p:?} from the face, against the print's {print:?}"
        );
    }
}

#[test]
fn the_face_is_cut_to_the_circle_and_the_hole_with_a_clear_hub() {
    let d = card(&[("Vagrant Story (USA)", 1)]);
    let face = cart_face(&carts(&d)[0]);
    assert_eq!((face.w, face.h), (DISC_PX, DISC_PX));
    for y in 0..face.h {
        for x in 0..face.w {
            let r = art_radius(x, y);
            let a = pixel(&face, x, y)[3];
            let on = r < DISC_PX as f32 / 4.0 && r >= 9.0;
            assert_eq!(
                a > 0,
                on,
                "the face at ({x}, {y}), {r} art pixels out, has alpha {a}"
            );
        }
    }
    let middle = DISC_PX / 2;
    let hub = pixel(&face, middle + 26, middle)[3];
    assert!(
        hub > 0 && hub < 0x80,
        "the hub round the hole is {hub} opaque, not clear"
    );
    assert_eq!(
        pixel(&face, PRINT_AT.0, PRINT_AT.1)[3],
        255,
        "the print is not solid"
    );
}

/// The face is pixel art on the 2-pixel grid: the circle, the hole and every word land on whole
/// art pixels.
#[test]
fn a_generated_face_is_drawn_in_whole_art_pixels() {
    for (stem, discs) in [("Vagrant Story (USA)", 1), ("Final Fantasy VII (USA)", 3)] {
        let d = card(&[(stem, discs)]);
        let face = cart_face(&carts(&d)[0]);
        for y in (0..face.h).step_by(2) {
            for x in (0..face.w).step_by(2) {
                let cell =
                    [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(i, j)| pixel(&face, x + i, y + j));
                assert!(
                    cell.iter().all(|p| *p == cell[0]),
                    "{stem}: the art pixel at ({x}, {y}) is not one colour: {cell:?}"
                );
            }
        }
    }
}

#[test]
fn a_game_on_several_discs_has_a_face_for_each_disc_side_by_side() {
    let d = card(&[("Final Fantasy VIII (USA)", 4), ("Vagrant Story (USA)", 1)]);
    let carts = carts(&d);
    let ink = [0xf4, 0xf1, 0xea];
    for (cart, discs) in carts.iter().zip([4u32, 1]) {
        let face = cart_face(cart);
        assert_eq!(
            (face.w, face.h),
            (DISC_PX * discs, DISC_PX),
            "{}: not {discs} faces side by side",
            cart.stem
        );
        let accent = label_colour(&label_text(cart));
        for disc in 0..discs {
            let at = disc * DISC_PX;
            let title = (at..at + DISC_PX)
                .flat_map(|x| (20..90).map(move |y| (x, y)))
                .any(|(x, y)| pixel(&face, x, y)[..3] == ink);
            assert!(title, "{} disc {}: no title", cart.stem, disc + 1);
            let number = (at + 80..at + DISC_PX - 80)
                .flat_map(|x| (200..240).map(move |y| (x, y)))
                .any(|(x, y)| pixel(&face, x, y)[..3] == accent);
            assert_eq!(
                number,
                discs > 1,
                "{} disc {}: its number is printed {number} under the hole",
                cart.stem,
                disc + 1
            );
        }
    }
}

#[test]
fn each_disc_wears_its_own_label_and_disc_one_falls_back_to_the_games() {
    let d = card(&[("Starfall Saga (USA)", 4)]);
    label(&d, "Starfall Saga (USA)", [0xe0, 0x30, 0x30]);
    label(&d, "Starfall Saga (USA) (Disc 2)", [0x30, 0xe0, 0x30]);
    label(&d, "Starfall Saga (USA) (Disc 3)", [0x30, 0x30, 0xe0]);
    let face = cart_face(&carts(&d)[0]);
    let print = |disc: u32| pixel(&face, disc * DISC_PX + PRINT_AT.0, PRINT_AT.1);
    assert_eq!(
        print(0),
        [0xe0, 0x30, 0x30, 255],
        "disc 1 does not wear the game's label"
    );
    assert_eq!(
        print(1),
        [0x30, 0xe0, 0x30, 255],
        "disc 2 does not wear its own label"
    );
    assert_eq!(
        print(2),
        [0x30, 0x30, 0xe0, 255],
        "disc 3 does not wear its own label"
    );
    let plain = print(3);
    assert!(
        plain[..3].iter().all(|c| *c < 0x60),
        "disc 4 has no label of its own but is not a black disc: {plain:?}"
    );
    // its own label wins over the game's
    label(&d, "Starfall Saga (USA) (Disc 1)", [0xe0, 0xe0, 0x30]);
    let face = cart_face(&carts(&d)[0]);
    assert_eq!(
        pixel(&face, PRINT_AT.0, PRINT_AT.1),
        [0xe0, 0xe0, 0x30, 255]
    );
}

/// Each disc's label is cut once like a cart's, so a boot draws a stack from its discs' cuts.
#[test]
fn each_discs_label_is_cut_and_the_stack_drawn_from_the_cuts_is_the_one_from_its_scans() {
    let d = card(&[("Starfall Saga (USA)", 3)]);
    label(&d, "Starfall Saga (USA)", [0xe0, 0x30, 0x30]);
    label(&d, "Starfall Saga (USA) (Disc 2)", [0x30, 0xe0, 0x30]);
    let cart = &carts(&d)[0];
    let from_scans = cart_face(cart);
    assert!(label_uncut(cart), "the discs' scans are not cut yet");
    assert!(
        generated_face(cart).rgba != from_scans.rgba,
        "the generated stack wears the scans"
    );

    assert!(cut_label(cart));
    for name in ["Starfall Saga (USA)", "Starfall Saga (USA) (Disc 2)"] {
        let cut = d.path().join(format!("Labels/PS1/.cut/{name}.png"));
        assert!(cut.is_file(), "{name} was not cut");
    }
    assert!(!label_uncut(cart), "a disc's scan is still uncut");
    assert!(
        cart_face(cart).rgba == from_scans.rgba,
        "the stack drawn from the cuts is not the one drawn from the scans"
    );
}

/// A game with no label of its own still wears one when a disc has its own, and a game none of
/// whose discs has one wears its generated stack.
#[test]
fn a_game_wears_a_label_when_any_of_its_discs_has_one() {
    let d = card(&[("Starfall Saga (USA)", 2), ("Bare Bones (USA)", 2)]);
    label(&d, "Starfall Saga (USA) (Disc 2)", [0x30, 0xe0, 0x30]);
    let carts = carts(&d);
    let cart = |stem: &str| carts.iter().find(|c| c.stem == stem).unwrap();
    let starfall = cart("Starfall Saga (USA)");
    assert!(starfall.label.is_none(), "the game has no label of its own");
    assert!(wears_label(starfall), "its second disc has one");
    assert!(cart_face(starfall).rgba != generated_face(starfall).rgba);
    let bare = cart("Bare Bones (USA)");
    assert!(!wears_label(bare));
    assert!(cart_face(bare).rgba == generated_face(bare).rgba);
}

/// A game whose discs sit in a folder of their own, or whose playlist was made for it under
/// Config, still finds each disc's label on the card.
#[test]
fn each_disc_finds_its_label_wherever_its_game_is_kept() {
    let d = tempfile::tempdir().unwrap();
    let folder = d.path().join("Games/PS1/Chrono Cross (USA)");
    std::fs::create_dir_all(&folder).unwrap();
    let discs: Vec<_> = (1..=2)
        .map(|n| folder.join(format!("Chrono Cross (USA) (Disc {n}).cue")))
        .collect();
    let list: String = discs.iter().map(|p| format!("{}\n", p.display())).collect();
    let playlists = d.path().join("Config/playlists/PS1");
    std::fs::create_dir_all(&playlists).unwrap();
    std::fs::write(playlists.join("Chrono Cross (USA).m3u"), list).unwrap();
    label(&d, "Chrono Cross (USA) (Disc 2)", [0x30, 0xe0, 0x30]);
    for rom in [playlists.join("Chrono Cross (USA).m3u"), discs[1].clone()] {
        let cart = Cart {
            platform: omega_store::Platform::Ps1,
            stem: "Chrono Cross (USA)".into(),
            rom: rom.clone(),
            label: None,
            code: String::new(),
            shell: None,
            title: String::new(),
        };
        let face = cart_face(&cart);
        let last = face.w / DISC_PX - 1;
        assert_eq!(
            pixel(&face, last * DISC_PX + PRINT_AT.0, PRINT_AT.1),
            [0x30, 0xe0, 0x30, 255],
            "{}: disc 2's label is not found",
            rom.display()
        );
    }
}

/// Every mesh `StackMeshes::build` makes, with the id it was given.
fn built() -> (StackMeshes, Vec<(MeshId, Vec<MeshVertex>)>) {
    let mut made = Vec::new();
    let stacks = StackMeshes::build(|v| {
        let id = MeshId::from_raw(100 + made.len());
        made.push((id, v.to_vec()));
        id
    });
    (stacks, made)
}

fn vertices(made: &[(MeshId, Vec<MeshVertex>)], id: MeshId) -> &[MeshVertex] {
    &made
        .iter()
        .find(|(m, _)| *m == id)
        .expect("a mesh never built")
        .1
}

#[test]
fn each_disc_of_a_stack_stands_behind_the_last_with_a_gap_about_twice_a_disc() {
    let (stacks, made) = built();
    for discs in 2..=4 {
        let mut last: Option<(f32, f32)> = None;
        for disc in 0..discs {
            let mesh = vertices(&made, stacks.disc(disc, discs));
            let face: Vec<_> = mesh.iter().filter(|v| v.normal[2] == -1.0).collect();
            let front = face[0].xyz[2];
            let back = mesh.iter().map(|v| v.xyz[2]).fold(f32::MIN, f32::max);
            assert!(
                face.iter().all(|v| v.xyz[2] == front),
                "disc {disc} of {discs}: its face is not flat"
            );
            let (u0, u1) = (disc as f32 / discs as f32, (disc + 1) as f32 / discs as f32);
            assert!(
                mesh.iter()
                    .all(|v| v.uv[0] >= u0 - 1e-6 && v.uv[0] <= u1 + 1e-6),
                "disc {disc} of {discs} takes its face from beyond its own {u0}..{u1}"
            );
            if let Some((was_front, was_back)) = last {
                let (thick, gap) = (was_back - was_front, front - was_back);
                assert!(
                    gap > 1.5 * thick && gap < 2.5 * thick,
                    "disc {disc} of {discs} stands {gap} behind the last, a disc being {thick}"
                );
            }
            last = Some((front, back));
        }
        // in one model, its discs from the back to the front, each risen over the one before
        let whole = vertices(&made, stacks.whole(discs));
        let mut at = 0;
        let mut was_up = f32::INFINITY;
        for disc in (0..discs).rev() {
            let one = vertices(&made, stacks.disc(disc, discs));
            let risen = &whole[at..at + one.len()];
            at += one.len();
            let up = one[0].xyz[1] - risen[0].xyz[1];
            assert!(
                one.iter().zip(risen).all(|(a, b)| {
                    (a.xyz[1] - b.xyz[1] - up).abs() < 1e-6
                        && (a.xyz[0], a.xyz[2], a.normal, a.uv, a.shade)
                            == (b.xyz[0], b.xyz[2], b.normal, b.uv, b.shade)
                }),
                "disc {disc} of the stack of {discs} in one model is not the disc risen"
            );
            assert!(
                up < was_up && (disc > 0 || up == 0.0),
                "disc {disc} of the stack of {discs} in one model is risen {up}, after {was_up}"
            );
            was_up = up;
        }
        assert_eq!(
            at,
            whole.len(),
            "the stack of {discs} has more than its discs"
        );
    }
}

/// Each disc behind the first shows over the one in front of it, its face drawn where it stands
/// in the stack drawn as one model. (The back edges of its walls, a disc's thickness further
/// off, may land a pixel apart.)
#[test]
fn each_disc_of_a_stack_shows_over_the_one_in_front() {
    let (stacks, made) = built();
    let stack = DiscStack {
        tex: TexId::from_raw(5),
        discs: 4,
        meshes: stacks,
    };
    let rect = (100.0, 120.0, 288.0, 288.0);
    let mut out = Vec::new();
    omega_ui::push_stack(&mut out, rect, &stack, [1.0; 4], 0.0, 0.0);
    let drawn = models_of(&out, stack.tex);
    let ys: Vec<f32> = drawn.iter().map(|m| m.1[1]).collect();
    assert!(
        ys.windows(2).all(|w| w[0] < w[1]) && ys[3] == rect.1,
        "the discs do not rise from the front disc to the back one: {ys:?}"
    );
    let whole = vertices(&made, stack.whole());
    let mut at = 0;
    for (mesh, [x, y, w, h], ..) in drawn {
        let one = vertices(&made, mesh);
        let faces = one
            .iter()
            .zip(&whole[at..])
            .filter(|(v, _)| v.normal[2] == -1.0);
        for (v, risen) in faces {
            let (a, b) = (projected(v, (x, y, w, h), 0.0), projected(risen, rect, 0.0));
            assert!(
                (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3,
                "a disc drawn on its own lands at {a:?}, and in the stack's one model at {b:?}"
            );
        }
        at += one.len();
    }
}

/// Where a vertex of a model drawn at `rect`, spun `yaw`, lands on the canvas: the vertex
/// shader's sums.
fn projected(v: &MeshVertex, (x, y, w, h): (f32, f32, f32, f32), yaw: f32) -> (f32, f32) {
    let (lx, ly) = (v.xyz[0] * w - w / 2.0, v.xyz[1] * h - h / 2.0);
    let depth = v.xyz[2] * w;
    let (sin, cos) = yaw.sin_cos();
    let across = lx * cos - depth * sin;
    let back = lx * sin + depth * cos;
    let k = (FOCAL + back) / FOCAL;
    (x + w / 2.0 + across / k, y + h / 2.0 + ly / k)
}

/// The tallest stack, swaying and floating as the shelf holds the disc in the middle, stays
/// between the bars and clear of the discs either side of it.
#[test]
fn the_tallest_stack_sways_clear_of_both_bars_and_its_neighbours() {
    let (stacks, made) = built();
    let stack = DiscStack {
        tex: TexId::from_raw(5),
        discs: 4,
        meshes: stacks,
    };
    let (w, h) = model_shelf_box(CartModel::Disc);
    let x = out_w() as f32 / 2.0 - w / 2.0;
    let (ceiling, floor) = (stage_y() + TOP_BAR_H, stage_y() + MOCKUP_H - BAR_H);
    let neighbour = out_w() as f32 / 2.0 + w - 0.78 * w / 2.0;
    let (top, foot) = (rest_y(h), foot_y(h));
    assert!(
        (top + foot - (ceiling + floor)).abs() < 0.01,
        "the disc stands at {top}..{foot}, not centred between {ceiling} and {floor}"
    );
    for deg in -SWAY_DEG..=SWAY_DEG {
        let yaw = (deg as f32).to_radians();
        for lift in [-BOB, BOB] {
            let mut out = Vec::new();
            omega_ui::push_stack(&mut out, (x, top - lift, w, h), &stack, [1.0; 4], 0.0, yaw);
            let rims = models_of(&out, stack.tex)
                .into_iter()
                .flat_map(|(mesh, rect, ..)| {
                    walls(vertices(&made, mesh))
                        .into_iter()
                        .flatten()
                        .map(move |v| (*v, rect))
                        .collect::<Vec<_>>()
                });
            for (v, [rx, ry, rw, rh]) in rims {
                let (px, py) = projected(&v, (rx, ry, rw, rh), yaw);
                assert!(
                    py >= ceiling && py <= floor,
                    "turned {deg} degrees, the stack reaches {py}, past the bars at {ceiling} \
                     and {floor}"
                );
                assert!(
                    (out_w() as f32 - px) < neighbour - 2.0 && px < neighbour - 2.0,
                    "turned {deg} degrees, the stack reaches {px}, into the disc beside it at \
                     {neighbour}"
                );
            }
        }
    }
}

fn faced(d: &TempDir) -> Shelf {
    let mut s = Shelf::new(carts(d));
    let n = s.carts.len();
    s.set_faces((0..n).map(|i| TexId::from_raw(40 + i)).collect());
    let mut next = 0;
    s.set_meshes(CartMeshes::upload(|_| {
        next += 1;
        MeshId::from_raw(next)
    }));
    s
}

fn row(s: &Shelf) -> Vec<Draw> {
    let mut out = Vec::new();
    s.draw_row(None, 0.0, 0.0, 1.0, &mut out);
    out
}

/// The models drawn with `tex`, in the order they are drawn: their meshes, rects, colours and
/// how far they are spun.
fn models_of(out: &[Draw], tex: TexId) -> Vec<(MeshId, [f32; 4], [f32; 4], f32)> {
    out.iter()
        .filter_map(|d| match *d {
            Draw::Model {
                x,
                y,
                w,
                h,
                mesh,
                tex: t,
                colour,
                yaw,
                ..
            } if t == tex => Some((mesh, [x, y, w, h], colour, yaw)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_stack_is_drawn_from_its_back_disc_to_its_front_one_each_its_own_model() {
    let d = card(&[
        ("Final Fantasy VII (USA)", 3),
        ("Metal Gear Solid (USA)", 2),
        ("Vagrant Story (USA)", 1),
    ]);
    let mut s = faced(&d);
    for _ in 0..200 {
        s.update(1.0 / 60.0);
    }
    for (i, discs) in [3, 2, 1].into_iter().enumerate() {
        s.select(i);
        for _ in 0..80 {
            s.update(1.0 / 60.0);
        }
        let stem = s.carts[i].stem.clone();
        let stack: DiscStack = s.stack_for(&stem).expect("a disc stands as no stack");
        assert_eq!(stack.discs, discs, "{stem}");
        let out = row(&s);
        let drawn = models_of(&out, stack.tex);
        let meshes: Vec<MeshId> = drawn.iter().map(|m| m.0).collect();
        let back_to_front: Vec<MeshId> = (0..discs).rev().map(|k| stack.mesh(k)).collect();
        assert_eq!(meshes, back_to_front, "{stem} is not drawn back to front");
        let (_, rect, colour, yaw) = drawn[discs - 1];
        let held = |m: &(MeshId, [f32; 4], [f32; 4], f32)| {
            (m.1[0], m.1[2], m.1[3], m.2, m.3) == (rect[0], rect[2], rect[3], colour, yaw)
                && m.1[1] <= rect[1]
        };
        assert!(
            drawn.iter().all(held),
            "{stem}'s discs are not held as one stack"
        );
        assert_eq!(colour, [1.0; 4], "{stem} in the middle is dimmed");
        assert!(yaw != 0.0 && yaw == s.pose().yaw, "{stem} does not sway");
        assert_eq!(
            s.mesh_for(&stem),
            Some(stack.whole()),
            "{stem} hands out another model than its whole stack"
        );
        // its neighbours are dimmed, discs and all
        for side in [(i + 1) % 3, (i + 2) % 3] {
            let tex = TexId::from_raw(40 + side);
            let side_models = models_of(&out, tex);
            assert!(
                !side_models.is_empty(),
                "the disc beside {stem} is not drawn"
            );
            assert!(
                side_models.iter().all(|m| m.2[0] < 1.0 && m.3 == 0.0),
                "the disc beside {stem} is not dimmed and still"
            );
        }
    }
}

#[test]
fn a_cart_stands_as_no_stack() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Games/GBA")).unwrap();
    std::fs::write(d.path().join("Games/GBA/Zelda.gba"), [0u8; 8]).unwrap();
    let s = faced(&d);
    assert_eq!(s.stack_for("Zelda"), None);
    assert!(s.mesh_for("Zelda").is_some());
}

/// The stack goes into the handheld the way [`STACK_DIVE`] says: whole, every disc from the
/// rect a lone disc would be at; or disc 1 alone, the rest left at the top of the rise.
#[test]
fn a_stack_goes_in_back_to_front_from_where_a_lone_disc_would() {
    let d = card(&[("Final Fantasy IX (USA)", 4), ("Vagrant Story (USA)", 1)]);
    let s = faced(&d);
    let stem = s.carts[0].stem.clone();
    let stack = s.stack_for(&stem).unwrap();
    let lone = &s.carts[1];
    let (rest, scale) = s.selected_at();
    let dive = |cart: &Cart, stack: Option<DiscStack>, seat: f32| {
        let mut out = Vec::new();
        CartDive {
            cart,
            face: Some(stack.map_or(TexId::from_raw(9), |s| s.tex)),
            mesh: Some(MeshId::from_raw(9)),
            stack,
            rest,
            scale,
            pose: Pose {
                yaw: 0.2,
                lift: 2.0,
            },
            seat,
            alert: None,
            dim: seat,
            screen: 0.0,
            game: false,
            disc: None,
        }
        .draw(&mut out);
        out
    };
    for step in 0..20 {
        let seat = step as f32 / 20.0;
        let single = models_of(&dive(lone, None, seat), TexId::from_raw(9));
        let (_, rect, _, yaw) = single[0];
        let drawn = models_of(&dive(&s.carts[0], Some(stack), seat), stack.tex);
        let meshes: Vec<MeshId> = drawn.iter().map(|m| m.0).collect();
        let back_to_front: Vec<MeshId> = (0..4).rev().map(|k| stack.mesh(k)).collect();
        assert_eq!(
            meshes, back_to_front,
            "the stack goes in out of order at {seat}"
        );
        let front = drawn[3];
        assert_eq!(
            (front.1, front.3),
            (rect, yaw),
            "disc 1 goes in another way than a lone disc at {seat}"
        );
        for back in &drawn[..3] {
            match STACK_DIVE {
                StackDive::Whole => assert!(
                    (back.1[0], back.1[2], back.1[3], back.2)
                        == (rect[0], rect[2], rect[3], [1.0; 4])
                        && back.1[1] < rect[1],
                    "the stack comes apart at {seat}"
                ),
                StackDive::FrontDisc => {
                    assert!(back.1[1] <= rect[1], "a back disc goes in at {seat}");
                    assert!(
                        back.2[0] <= 1.0 - seat + 1e-6,
                        "a back disc stays lit at {seat}"
                    );
                }
            }
        }
    }
}
