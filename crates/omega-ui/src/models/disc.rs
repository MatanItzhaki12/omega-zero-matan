//! The PlayStation disc, and the stack a game on several discs stands as.
//!
//! A disc is a thin low-poly ring: its rim cut in a couple of dozen flat walls, a hole through
//! its middle walled the same way facing into the hole, and its face one texture whose alpha
//! cuts the circle and the hole. A game on several discs stands as all of them one behind the
//! other, disc 1 in front, a gap a little wider than a disc between each, the way books stand
//! on a shelf with a little air between them.
//!
//! Every disc of a game has its own face, laid side by side in one texture, disc 1 on the left:
//! a stack is uploaded and drawn again when a label arrives exactly as a cart is. Each disc is
//! drawn with its own model, which takes its face from its own part of that texture and stands
//! in its own place behind the disc in front.

use std::f32::consts::{PI, TAU};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use omega_gfx::{Draw, MeshId, MeshVertex, TexId, FOCAL};
use omega_store::{ps1, Cart, Platform};

use crate::cart::{label_colour, label_text, CartFace, Inside, Spec};
use crate::label_cut::label_art;
use crate::silhouette::Detail;
use crate::spin::posed_cart;
use crate::sprite::SPRITE_PX;
use crate::text;

/// A disc is this many art pixels across, each two of the screen's.
const ACROSS: u32 = 144;

/// A disc's face in pixels, square, the size it is drawn at on the shelf.
pub const DISC_PX: u32 = ACROSS * SPRITE_PX;

/// How thick a disc is, as a share of its width. A real one is 1.2 mm on 120, too thin to show
/// at all as it sways; this is about four times that, a rim of two pixels at the end of the
/// sway once the perspective has taken its share.
pub const THICK: f32 = 0.045;

/// The air between one disc of a stack and the next, from the back of one to the face of the
/// one behind, as a share of a disc's width: twice as much as a disc is thick, so the discs of
/// a stack stand clearly apart.
pub const GAP: f32 = 0.09;

/// From the face of one disc of a stack to the face of the one behind it.
pub const PITCH: f32 = THICK + GAP;

/// How far up each disc of a stack stands from the one in front of it, for each unit it stands
/// behind it: a stack seen from a little above, the way the records in a crate show their tops
/// over the one in front. Straight behind, the perspective would hide every disc behind the
/// first.
const RISE: f32 = 0.45;

/// How big a disc stands on the shelf, as a share of its own size.
pub const SHELF_SCALE: f32 = 1.0;

/// The most discs of one game the shelf draws in its stack.
pub const MAX_STACK: usize = 4;

/// How much less light each disc of a stack gets than the one in front of it, in the shadow of
/// the discs before it.
const STACK_SHADE: f32 = 0.1;

/// How a game on several discs goes into the handheld.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum StackDive {
    /// The whole stack goes in, the way a cart does.
    Whole,
    /// Disc 1 goes in on its own, and the discs behind it stay where the stack came to, going
    /// dark with the shelf.
    FrontDisc,
}

/// How a stack goes in.
pub const STACK_DIVE: StackDive = StackDive::Whole;

/// How fast the discs on the shelf turn about their own middles while it stands still, in
/// turns a second, the way a disc turns in its drive: not at all.
pub const SPIN: f32 = 0.0;

/// How many discs `cart` stands as on the shelf: one, or each disc of a game on several, up to
/// [`MAX_STACK`].
pub fn stack_of(cart: &Cart) -> usize {
    match cart.platform {
        Platform::Ps1 => ps1::discs(&cart.rom).len().clamp(1, MAX_STACK),
        _ => 1,
    }
}

// The rings of a disc, in art pixels out from its middle, as a CD is made: the hole 15 mm
// across, the clear hub out to 33 mm with the ring discs stack on, the mirror band out to
// 46 mm, then the print out to the edge at 120 mm, which is clear again.

/// Its radius.
const R: f32 = ACROSS as f32 / 2.0;
/// The hole.
const HOLE: f32 = 9.0;
/// The hole's lip, an art pixel wide, the colour the hole's walls take.
const LIP: f32 = 10.0;
/// The ring discs stack on, an art pixel wide.
const STACKING: f32 = 15.0;
/// Where the clear hub ends.
const HUB: f32 = 20.0;
/// Where the mirror band ends and the print starts.
const MIRROR: f32 = 27.0;
/// A band in the game's own colour just in from the edge, on a face with no label.
const ACCENT: (f32, f32) = (65.0, 67.0);
/// Where the clear edge starts, the colour the rim's walls take.
const EDGE: f32 = 70.0;

/// What a disc is made of at some distance from its middle.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Ring {
    /// Off the disc: outside it, or in the hole.
    Off,
    Lip,
    Hub,
    Stacking,
    Mirror,
    Print,
    Accent,
    Edge,
}

fn ring(d: f32) -> Ring {
    match d {
        d if !(HOLE..R).contains(&d) => Ring::Off,
        d if d < LIP => Ring::Lip,
        d if (STACKING..STACKING + 1.0).contains(&d) => Ring::Stacking,
        d if d < HUB => Ring::Hub,
        d if d < MIRROR => Ring::Mirror,
        d if d >= EDGE => Ring::Edge,
        d if (ACCENT.0..ACCENT.1).contains(&d) => Ring::Accent,
        _ => Ring::Print,
    }
}

/// How far the middle of the art pixel at `ax, ay` is from the middle of the disc, in art
/// pixels.
fn from_middle(ax: u32, ay: u32) -> f32 {
    let (dx, dy) = (ax as f32 + 0.5 - R, ay as f32 + 0.5 - R);
    (dx * dx + dy * dy).sqrt()
}

// A disc with no label is a black PlayStation disc. Its clear parts are see-through: the hub
// shows what is behind it, a little greyed, its lip and stacking ring a little more.
const PRINT: [u8; 4] = [0x1a, 0x19, 0x21, 0xff];
const PRINT_LIT: [u8; 4] = [0x3a, 0x38, 0x4c, 0xff];
const MIRROR_DARK: [u8; 4] = [0x74, 0x77, 0x84, 0xff];
const MIRROR_LIT: [u8; 4] = [0xc4, 0xc8, 0xd4, 0xff];
const HUB_CLEAR: [u8; 4] = [0xd8, 0xdc, 0xe6, 0x48];
const STACKING_CLEAR: [u8; 4] = [0xe4, 0xe8, 0xf0, 0x90];
const LIP_CLEAR: [u8; 4] = [0xc8, 0xcc, 0xd6, 0xd0];
const EDGE_CLEAR: [u8; 4] = [0x9c, 0xa0, 0xae, 0xff];
const INK: [u8; 3] = [0xf4, 0xf1, 0xea];

/// Whether the art pixel at `ax, ay` catches the light: two wedges across from each other, top
/// left and bottom right, whole in their middles and dithered a pixel in two at their edges.
fn sheen(ax: u32, ay: u32) -> bool {
    let (dx, dy) = (ax as f32 + 0.5 - R, ay as f32 + 0.5 - R);
    let off = (dy.atan2(dx) - PI / 4.0).rem_euclid(PI);
    let off = off.min(PI - off).to_degrees();
    off < 9.0 || (off < 18.0 && (ax + ay).is_multiple_of(2))
}

/// The colour of a disc with no label at the art pixel `ax, ay`, `accent` its band.
fn plain(ax: u32, ay: u32, accent: [u8; 3]) -> [u8; 4] {
    let lit = sheen(ax, ay);
    match ring(from_middle(ax, ay)) {
        Ring::Off => [0; 4],
        Ring::Lip => LIP_CLEAR,
        Ring::Hub => HUB_CLEAR,
        Ring::Stacking => STACKING_CLEAR,
        Ring::Mirror if lit => MIRROR_LIT,
        Ring::Mirror => MIRROR_DARK,
        Ring::Print if lit => PRINT_LIT,
        Ring::Print => PRINT,
        Ring::Accent => [accent[0], accent[1], accent[2], 0xff],
        Ring::Edge => EDGE_CLEAR,
    }
}

/// Where a face with no label prints its title, above the hole, and its disc's number, below
/// it: `x, y, w, h` in the face's pixels, each on the art grid and inside the print.
const TITLE_BOX: (u32, u32, u32, u32) = (64, 22, 160, 66);
const NUMBER_BOX: (u32, u32, u32, u32) = (84, 202, 120, 36);

/// One disc's face, `DISC_PX` square: disc `disc` of `discs` of the game called `title`, with
/// the label `art` printed on it, if it has one, laid at the disc's size. The art covers the
/// disc from the mirror band out; the clear hub, its hole and the clear edge are the disc's own.
fn disc_face(title: &str, disc: usize, discs: usize, art: Option<&[u8]>) -> Vec<u8> {
    let accent = label_colour(title);
    let mut rgba = Vec::with_capacity((DISC_PX * DISC_PX * 4) as usize);
    for y in 0..DISC_PX {
        for x in 0..DISC_PX {
            let (ax, ay) = (x / SPRITE_PX, y / SPRITE_PX);
            let mut c = plain(ax, ay, accent);
            let printed = matches!(
                ring(from_middle(ax, ay)),
                Ring::Mirror | Ring::Print | Ring::Accent
            );
            if let (Some(art), true) = (art, printed) {
                let i = ((y * DISC_PX + x) * 4) as usize;
                c = over(&art[i..i + 4], c);
            }
            rgba.extend_from_slice(&c);
        }
    }
    if art.is_none() {
        print(&mut rgba, title, TITLE_BOX, 3, INK);
        if discs > 1 {
            print(
                &mut rgba,
                &format!("DISC {}", disc + 1),
                NUMBER_BOX,
                1,
                accent,
            );
        }
    }
    rgba
}

/// `top`, a label's pixel, laid over the disc's own colour `under`.
fn over(top: &[u8], under: [u8; 4]) -> [u8; 4] {
    let a = top[3] as u32;
    let mut out = under;
    for c in 0..3 {
        out[c] = ((top[c] as u32 * a + under[c] as u32 * (255 - a) + 127) / 255) as u8;
    }
    out[3] = 0xff;
    out
}

/// `words` printed in Pixel Code at one art pixel to one of its own, centred in `bx, by, bw,
/// bh` of a disc's face on up to `lines` lines, in `ink`. Its strokes are moved onto the art
/// grid, so each lands on whole art pixels.
fn print(
    rgba: &mut [u8],
    words: &str,
    (bx, by, bw, bh): (u32, u32, u32, u32),
    lines: usize,
    ink: [u8; 3],
) {
    let Some(font) = text::label_font() else {
        return;
    };
    let px = text::FONT_GRID;
    let layout = text::fit_box(font, words, bw as f32, bh as f32, lines, px, px);
    let cover = text::coverage_in(font, bw, bh, &layout);
    let inked = |i: usize| cover[i] >= 128;
    let first =
        |along: &dyn Fn(usize) -> u32| (0..cover.len()).filter(|&i| inked(i)).map(along).min();
    let (Some(x0), Some(y0)) = (first(&|i| i as u32 % bw), first(&|i| i as u32 / bw)) else {
        return;
    };
    let (sx, sy) = ((bx + x0) % SPRITE_PX, (by + y0) % SPRITE_PX);
    for (i, _) in cover.iter().enumerate().filter(|(i, _)| inked(*i)) {
        let (x, y) = (bx + i as u32 % bw - sx, by + i as u32 / bw - sy);
        let d = ((y * DISC_PX + x) * 4) as usize;
        rgba[d..d + 3].copy_from_slice(&ink);
    }
}

/// The card `rom` is on: the folder holding the `Games` folder it is in, or the `Config` folder
/// a playlist made for a game on several discs is kept in.
fn card_root(rom: &Path) -> Option<PathBuf> {
    rom.ancestors()
        .skip(1)
        .find(|dir| {
            matches!(
                dir.file_name().and_then(|n| n.to_str()),
                Some("Games" | "Config")
            )
        })
        .and_then(Path::parent)
        .map(Path::to_path_buf)
}

/// The label of disc `disc` of `cart`, `paths` its discs: its own, kept under its own name,
/// or for disc 1 the game's.
fn label_of(cart: &Cart, paths: &[PathBuf], disc: usize) -> Option<PathBuf> {
    let own = card_root(&cart.rom)
        .zip(paths.get(disc))
        .map(|(root, path)| ps1::disc_label(&root, path))
        .filter(|p| p.is_file());
    own.or_else(|| cart.label.clone().filter(|_| disc == 0))
}

/// The label scans of `cart`'s discs, each with the size its disc's face shows it at.
pub(crate) fn label_scans(cart: &Cart) -> Vec<(PathBuf, u32, u32)> {
    let paths = ps1::discs(&cart.rom);
    (0..stack_of(cart))
        .filter_map(|disc| label_of(cart, &paths, disc))
        .map(|scan| (scan, DISC_PX, DISC_PX))
        .collect()
}

/// The faces of every disc of `cart`, side by side, disc 1 on the left, their labels drawn from
/// their scans when `scans` and generated when not.
pub(crate) fn face(cart: &Cart, scans: bool) -> Option<CartFace> {
    let discs = stack_of(cart);
    let paths = ps1::discs(&cart.rom);
    let title = label_text(cart);
    let w = DISC_PX * discs as u32;
    let mut rgba = vec![0u8; (w * DISC_PX * 4) as usize];
    for disc in 0..discs {
        let art = label_of(cart, &paths, disc)
            .filter(|_| scans)
            .and_then(|p| label_art(&p, DISC_PX, DISC_PX));
        let one = disc_face(&title, disc, discs, art.as_deref());
        let row = (DISC_PX * 4) as usize;
        for (y, line) in one.chunks_exact(row).enumerate() {
            let at = (y * w as usize + disc * DISC_PX as usize) * 4;
            rgba[at..at + row].copy_from_slice(line);
        }
    }
    Some(CartFace {
        rgba,
        w,
        h: DISC_PX,
    })
}

/// The disc's cover: all of it but the hole.
fn mask() -> &'static [u8] {
    static MASK: OnceLock<Vec<u8>> = OnceLock::new();
    MASK.get_or_init(|| {
        (0..DISC_PX * DISC_PX)
            .map(|i| {
                let (ax, ay) = (i % DISC_PX / SPRITE_PX, i / DISC_PX / SPRITE_PX);
                match ring(from_middle(ax, ay)) {
                    Ring::Off => 0,
                    _ => 255,
                }
            })
            .collect()
    })
}

fn no_detail() -> &'static Detail {
    static DETAIL: OnceLock<Detail> = OnceLock::new();
    DETAIL.get_or_init(|| Detail {
        shadow: vec![0; (DISC_PX * DISC_PX) as usize],
        highlight: vec![0; (DISC_PX * DISC_PX) as usize],
    })
}

/// How the disc's face is laid out. A disc draws its own face, so only its size and its cover
/// are read: there is no shell, relief or board to lay.
pub(crate) fn spec() -> Spec {
    let (x, y, w, h) = TITLE_BOX;
    Spec {
        w: DISC_PX,
        h: DISC_PX,
        label: (x, y, w, h),
        label_round: 0,
        mask: mask(),
        depth: mask(),
        detail: no_detail(),
        max_h: h as f32,
        rim: 0,
        inside: Inside::Nothing,
    }
}

/// The rim's cuts, and the hole's: enough that neither reads as a polygon on the 2-pixel grid,
/// few enough to keep the PlayStation's look.
const CUTS: usize = 24;
const HOLE_CUTS: usize = 12;

/// The disc's drawing is this many units across, as a cart's is 240 wide.
const UNITS: f32 = 240.0;

/// The face is laid over this many cells across and down, so its print bends a little as the
/// disc turns, the way big textured polygons did on the PlayStation.
const FACE_CELLS: usize = 4;

/// A loop round the disc's middle in drawing units, `cuts` corners, `radius` art pixels out:
/// clockwise for the rim, so its walls face out, and the other way for the hole, so its walls
/// face into it. The corners stand a little outside the circle and the cuts between them a
/// little inside, so the walls follow the edge the face's alpha cuts.
fn round(radius: f32, cuts: usize, clockwise: bool) -> Vec<(f32, f32)> {
    let half = UNITS / 2.0;
    let r = radius / R * half * (1.0 + 1.0 / (PI / cuts as f32).cos()) / 2.0;
    (0..cuts)
        .map(|i| {
            let turn = i as f32 / cuts as f32 * TAU;
            let a = -PI / 2.0 + if clockwise { turn } else { -turn };
            (half + r * a.cos(), half + r * a.sin())
        })
        .collect()
}

/// The disc's outline in drawing units, all the way round clockwise, and the drawing's size.
pub(crate) fn outline() -> (Vec<(f32, f32)>, (f32, f32)) {
    (round(R, CUTS, true), (UNITS, UNITS))
}

/// The hole's outline in drawing units, all the way round anticlockwise.
pub(crate) fn hole() -> Vec<(f32, f32)> {
    round(HOLE, HOLE_CUTS, false)
}

/// One flat wall for each cut of `points`, from `z` back a disc's thickness and `up` higher,
/// each all one colour: the face's `sample` art pixels out from the middle, in line with the
/// middle of its cut, `cell` placing a share across one face in the stack's texture.
#[allow(clippy::too_many_arguments)]
fn walls(
    out: &mut Vec<MeshVertex>,
    points: &[(f32, f32)],
    z: f32,
    up: f32,
    sample: f32,
    cell: &dyn Fn(f32) -> f32,
    shade: f32,
) {
    let half = UNITS / 2.0;
    for (i, &a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-3 {
            continue;
        }
        let normal = [dy / len, -dx / len, 0.0];
        let (mx, my) = (
            f32::midpoint(a.0, b.0) - half,
            f32::midpoint(a.1, b.1) - half,
        );
        let out_to = sample / R * half / (mx * mx + my * my).sqrt();
        let uv = [
            cell((half + mx * out_to) / UNITS),
            (half + my * out_to) / UNITS,
        ];
        let corner = |p: (f32, f32), z: f32, shade: f32| MeshVertex {
            xyz: [p.0 / UNITS, p.1 / UNITS - up, z],
            normal,
            uv,
            shade,
        };
        let (a0, b0) = (corner(a, z, shade), corner(b, z, shade));
        let back = shade * SIDE_BACK_SHADE;
        let (a1, b1) = (corner(a, z + THICK, back), corner(b, z + THICK, back));
        out.extend([a0, b0, b1, a0, b1, a1]);
    }
}

/// How much light the back edge of a wall gets against its front edge.
const SIDE_BACK_SHADE: f32 = 0.72;

/// Disc `disc` of a stack of `discs` in few polygons, standing `disc` places behind the front
/// one, `up` of its height higher, and taking its face from its own part of the stack's
/// texture: the rim's walls, then the hole's, then the face over them.
fn layer(disc: usize, discs: usize, up: f32) -> Vec<MeshVertex> {
    layer_at(disc, discs, disc as f32 * PITCH, up)
}

/// Disc `disc` of a stack of `discs`, taking its face from its own part of the stack's texture
/// as `layer` does, but standing `z` of its width behind where its rect is drawn.
fn layer_at(disc: usize, discs: usize, z: f32, up: f32) -> Vec<MeshVertex> {
    let shade = 1.0 - STACK_SHADE * disc as f32;
    let cell = |u: f32| (disc as f32 + u) / discs as f32;
    let mut out = Vec::new();
    walls(
        &mut out,
        &outline().0,
        z,
        up,
        f32::midpoint(EDGE, R),
        &cell,
        shade,
    );
    walls(
        &mut out,
        &hole(),
        z,
        up,
        f32::midpoint(HOLE, LIP),
        &cell,
        shade,
    );
    let at = |c: usize, r: usize| {
        let (u, v) = (c as f32 / FACE_CELLS as f32, r as f32 / FACE_CELLS as f32);
        MeshVertex {
            xyz: [u, v - up, z],
            normal: [0.0, 0.0, -1.0],
            uv: [cell(u), v],
            shade,
        }
    };
    for r in 0..FACE_CELLS {
        for c in 0..FACE_CELLS {
            let (p, q, s, t) = (at(c, r), at(c + 1, r), at(c + 1, r + 1), at(c, r + 1));
            out.extend([p, q, s, p, s, t]);
        }
    }
    out
}

/// A stack of `discs` in one model, each disc risen as `push_disc` raises it and the back disc
/// first, so drawn in one go it is drawn back to front. Turned in the canvas it turns as one
/// thing; `push_stack` turns each disc about its own middle.
fn stack_mesh(discs: usize) -> Vec<MeshVertex> {
    (0..discs)
        .rev()
        .flat_map(|disc| layer(disc, discs, disc as f32 * PITCH * RISE))
        .collect()
}

/// The disc in few polygons.
pub(crate) fn mesh() -> Vec<MeshVertex> {
    layer(0, 1, 0.0)
}

/// How far above the front disc of a stack, at `w, h`, disc `disc` is drawn: `RISE` of how far
/// it stands behind, seen as far off as it stands, so it shows over the front disc as much as
/// it would were it risen in its model.
fn rise(disc: usize, (w, h): (f32, f32)) -> f32 {
    let back = disc as f32 * PITCH;
    back * RISE * h * FOCAL / (FOCAL + back * w)
}

/// The models stacks of discs are drawn with, once they are on the GPU: each disc of each size
/// of stack on its own, and each stack whole.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct StackMeshes {
    discs: [[MeshId; MAX_STACK]; MAX_STACK],
    alone: [[MeshId; MAX_STACK]; MAX_STACK],
    whole: [MeshId; MAX_STACK],
}

impl StackMeshes {
    /// Every one of them, each handed to `make` to put on the GPU.
    pub fn build(mut make: impl FnMut(&[MeshVertex]) -> MeshId) -> StackMeshes {
        let mut discs = Vec::with_capacity(MAX_STACK);
        let mut alone = Vec::with_capacity(MAX_STACK);
        let mut whole = Vec::with_capacity(MAX_STACK);
        for n in 1..=MAX_STACK {
            let one: Vec<MeshId> = (0..n).map(|disc| make(&layer(disc, n, 0.0))).collect();
            discs.push(std::array::from_fn(|disc| one[disc.min(n - 1)]));
            let front: Vec<MeshId> = (0..n)
                .map(|disc| match disc {
                    0 => one[0],
                    _ => make(&layer_at(disc, n, 0.0, 0.0)),
                })
                .collect();
            alone.push(std::array::from_fn(|disc| front[disc.min(n - 1)]));
            whole.push(match n {
                1 => one[0],
                _ => make(&stack_mesh(n)),
            });
        }
        StackMeshes {
            discs: std::array::from_fn(|i| discs[i]),
            alone: std::array::from_fn(|i| alone[i]),
            whole: std::array::from_fn(|i| whole[i]),
        }
    }

    /// The model disc `disc` of a stack of `discs` is drawn with, 0 being disc 1 at the front.
    pub fn disc(&self, disc: usize, discs: usize) -> MeshId {
        let n = discs.clamp(1, MAX_STACK);
        self.discs[n - 1][disc.min(n - 1)]
    }

    /// Disc `disc` of a stack of `discs` on its own, standing where the front disc would.
    pub fn alone(&self, disc: usize, discs: usize) -> MeshId {
        let n = discs.clamp(1, MAX_STACK);
        self.alone[n - 1][disc.min(n - 1)]
    }

    /// A stack of `discs` whole, as one model.
    pub fn whole(&self, discs: usize) -> MeshId {
        self.whole[discs.clamp(1, MAX_STACK) - 1]
    }
}

/// A game's discs as the shelf holds them, ready to draw: how many there are, their faces side
/// by side in one texture, and the models they are drawn with.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct DiscStack {
    pub tex: TexId,
    pub discs: usize,
    pub meshes: StackMeshes,
}

impl DiscStack {
    /// The model disc `disc` is drawn with, in its place in the stack.
    pub fn mesh(&self, disc: usize) -> MeshId {
        self.meshes.disc(disc, self.discs)
    }

    /// The whole stack as one model.
    pub fn whole(&self) -> MeshId {
        self.meshes.whole(self.discs)
    }
}

/// Disc `disc` of `stack` (0 being disc 1), in its place behind the front disc, which stands at
/// `rect`, and [`rise`] above it: tinted `colour`, turned `turn` radians about its own middle
/// and spun `yaw` about the stack's upright middle, as `posed_cart` holds a cart.
pub fn push_disc(
    out: &mut Vec<Draw>,
    (x, y, w, h): (f32, f32, f32, f32),
    stack: &DiscStack,
    disc: usize,
    colour: [f32; 4],
    turn: f32,
    yaw: f32,
) {
    out.push(posed_cart(
        (x, y - rise(disc, (w, h)), w, h),
        stack.mesh(disc),
        stack.tex,
        colour,
        turn,
        yaw,
    ));
}

/// Disc `disc` of `stack` on its own at `rect`, out of the stack, as a screen that lays a game's
/// discs side by side shows each: tinted `colour`, turned `turn` and spun `yaw` about its own
/// middle.
pub fn push_disc_alone(
    out: &mut Vec<Draw>,
    rect: (f32, f32, f32, f32),
    stack: &DiscStack,
    disc: usize,
    colour: [f32; 4],
    turn: f32,
    yaw: f32,
) {
    out.push(posed_cart(
        rect,
        stack.meshes.alone(disc, stack.discs),
        stack.tex,
        colour,
        turn,
        yaw,
    ));
}

/// Every disc of `stack`, its front disc standing at `rect`, each its own model, from the back
/// disc to the front one, as `push_disc` draws each.
pub fn push_stack(
    out: &mut Vec<Draw>,
    rect: (f32, f32, f32, f32),
    stack: &DiscStack,
    colour: [f32; 4],
    turn: f32,
    yaw: f32,
) {
    for disc in (0..stack.discs).rev() {
        push_disc(out, rect, stack, disc, colour, turn, yaw);
    }
}
