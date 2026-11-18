//! omega-zero's own borders: where a game's picture leaves the screen bare, a little console
//! shell around it, in pixel art, in a pastel colour of its system's. A rounded shell with its
//! edge lit, the picture in a well with a dark rim, and the system's touches in the room either
//! side: a Game Boy's D-pad and slanted A and B, the DS's speakers and lights, the SNES's four
//! buttons; and a sparkle or two and a heart in the corners.
//!
//! Drawn at two panel pixels to an art pixel, as the rest of omega-zero is, around wherever the
//! picture is, so a DS's two screens stacked get one as much as one screen does. A border of the
//! player's own (Overlays/<system>/) is drawn instead where there is one.

use omega_store::Platform;

use crate::plate::UndoFace;

/// Panel pixels to an art pixel.
const ART: u32 = 2;

type Rgb = [u8; 3];

/// The pastel shell each system's border is.
pub fn border_colour(platform: Platform) -> Option<Rgb> {
    Some(match platform {
        Platform::Gb => [0xf2, 0xc6, 0xd6],
        Platform::Gbc => [0xb8, 0xe6, 0xd2],
        Platform::Nds => [0xc9, 0xbf, 0xec],
        Platform::Snes => [0xc6, 0xd8, 0xf2],
        Platform::Ps1 => [0xf6, 0xd6, 0xbc],
        Platform::Psp => [0x44, 0x46, 0x56],
        // the GBA's picture fills the screen it is drawn for: no room for a border
        Platform::Gba => return None,
    })
}

/// The canvas outside the shell, where its rounded corners leave it: the dark of omega-zero's
/// own background.
const OUTSIDE: Rgb = [0x12, 0x12, 0x16];
/// The picture's rim, a dark plum, softer than black against the pastels.
const RIM: Rgb = [0x2a, 0x22, 0x30];
const HEART: Rgb = [0xe8, 0x5a, 0x8a];
const SPARKLE_ON_PALE: Rgb = [0xff, 0xff, 0xff];
const SPARKLE_ON_DARK: Rgb = [0xff, 0xe0, 0x8a];

const SPARKLE: [&str; 5] = ["..X..", "..X..", "XXXXX", "..X..", "..X.."];
const HEART_ROWS: [&str; 6] = [
    ".XX.XX.", "XXXXXXX", "XXXXXXX", ".XXXXX.", "..XXX..", "...X...",
];

/// The smallest band, in art pixels, worth drawing a system's touches in.
const ROOM: i32 = 16;

fn shade(c: Rgb, f: f32) -> Rgb {
    c.map(|v| (v as f32 * f).round().clamp(0.0, 255.0) as u8)
}

/// A drawing at art resolution.
struct Art {
    w: i32,
    h: i32,
    px: Vec<Rgb>,
    /// Where the picture shows through, cleared at the end.
    hole: Vec<bool>,
}

impl Art {
    fn new(w: i32, h: i32, fill: Rgb) -> Art {
        Art {
            w,
            h,
            px: vec![fill; (w * h) as usize],
            hole: vec![false; (w * h) as usize],
        }
    }

    fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if (0..self.w).contains(&x) && (0..self.h).contains(&y) {
            self.px[(y * self.w + x) as usize] = c;
        }
    }

    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        for y in y0.max(0)..y1.min(self.h) {
            for x in x0.max(0)..x1.min(self.w) {
                self.px[(y * self.w + x) as usize] = c;
            }
        }
    }

    /// A rectangle with its corners rounded `r`.
    fn round_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, r: i32, c: Rgb) {
        for y in y0.max(0)..y1.min(self.h) {
            for x in x0.max(0)..x1.min(self.w) {
                if in_round(x, y, x0, y0, x1, y1, r) {
                    self.px[(y * self.w + x) as usize] = c;
                }
            }
        }
    }

    fn disc(&mut self, cx: i32, cy: i32, r: i32, c: Rgb) {
        for y in cy - r..cy + r {
            for x in cx - r..cx + r {
                let (dx, dy) = (x - cx, y - cy);
                if dx * dx + dy * dy + dx + dy < r * r {
                    self.put(x, y, c);
                }
            }
        }
    }

    fn sprite(&mut self, x: i32, y: i32, rows: &[&str], c: Rgb) {
        for (j, row) in rows.iter().enumerate() {
            for (i, ch) in row.bytes().enumerate() {
                if ch == b'X' {
                    self.put(x + i as i32, y + j as i32, c);
                }
            }
        }
    }
}

fn in_round(x: i32, y: i32, x0: i32, y0: i32, x1: i32, y1: i32, r: i32) -> bool {
    if x < x0 || y < y0 || x >= x1 || y >= y1 {
        return false;
    }
    let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
    let r = r as f32;
    let dx = (x0 as f32 + r - cx).max(cx - (x1 as f32 - r)).max(0.0);
    let dy = (y0 as f32 + r - cy).max(cy - (y1 as f32 - r)).max(0.0);
    dx * dx + dy * dy <= r * r
}

/// The border for `platform` on a `w` by `h` canvas, the game's picture at `picture` (x, y,
/// width, height in panel pixels) left clear: none for a system that has none, or a picture
/// that leaves no room for one.
pub fn border_face(platform: Platform, w: u32, h: u32, picture: [f32; 4]) -> Option<UndoFace> {
    let body = border_colour(platform)?;
    let (aw, ah) = ((w / ART) as i32, (h / ART) as i32);
    // the picture, in art pixels, rounded outwards so no row of it is covered
    let px0 = (picture[0] / ART as f32).floor() as i32;
    let py0 = (picture[1] / ART as f32).floor() as i32;
    let px1 = ((picture[0] + picture[2]) / ART as f32).ceil() as i32;
    let py1 = ((picture[1] + picture[3]) / ART as f32).ceil() as i32;
    let (left, right, top, bottom) = (px0, aw - px1, py0, ah - py1);
    if left.max(right).max(top).max(bottom) < 4 {
        return None;
    }
    let mut a = Art::new(aw, ah, OUTSIDE);
    // the shell, its edge lit along the top and left and in shadow along the right and foot
    a.round_rect(1, 1, aw - 1, ah - 1, 14, shade(body, 0.75));
    a.round_rect(1, 1, aw - 2, ah - 2, 14, shade(body, 1.12));
    a.round_rect(2, 2, aw - 2, ah - 2, 13, body);
    // the well the picture sits in, as deep as the room round it allows, and its dark rim
    let lip = left.min(right).min(top).min(bottom).clamp(0, 5);
    if lip >= 2 {
        let r = 6.min(lip + 1);
        a.round_rect(
            px0 - lip,
            py0 - lip,
            px1 + lip,
            py1 + lip,
            r,
            shade(body, 1.18),
        );
        a.round_rect(
            px0 - lip + 1,
            py0 - lip + 1,
            px1 + lip,
            py1 + lip,
            r,
            shade(body, 0.72),
        );
        a.round_rect(
            px0 - lip + 1,
            py0 - lip + 1,
            px1 + lip - 1,
            py1 + lip - 1,
            r - 1,
            shade(body, 0.86),
        );
    }
    let rim = lip.min(2);
    a.round_rect(px0 - rim, py0 - rim, px1 + rim, py1 + rim, rim + 1, RIM);
    for y in py0.max(0)..py1.min(ah) {
        for x in px0.max(0)..px1.min(aw) {
            a.hole[(y * aw + x) as usize] = true;
        }
    }
    touches(&mut a, platform, body, (px0, py0, px1, py1));
    // a sparkle at two corners and a heart at a third, where the shell has room for them
    let pale = body.iter().map(|&c| c as u32).sum::<u32>() > 3 * 128;
    let sparkle = if pale {
        SPARKLE_ON_PALE
    } else {
        SPARKLE_ON_DARK
    };
    if left >= 10 || top >= 10 {
        a.sprite(8, 7, &SPARKLE, sparkle);
    }
    if right >= 10 || bottom >= 10 {
        a.sprite(aw - 13, ah - 12, &SPARKLE, sparkle);
    }
    if right >= 11 || top >= 10 {
        a.sprite(aw - 15, 6, &HEART_ROWS, HEART);
    }
    Some(upscale(&a, w, h))
}

/// The system's own touches, in the bands beside or under the picture.
fn touches(a: &mut Art, platform: Platform, body: Rgb, (x0, y0, x1, y1): (i32, i32, i32, i32)) {
    let (lx, rx) = (x0 / 2, x1 + (a.w - x1) / 2);
    let mid = i32::midpoint(y0, y1);
    let sides = x0 >= ROOM && a.w - x1 >= ROOM;
    match platform {
        Platform::Gb | Platform::Gbc if sides => {
            // the D-pad left, A and B slanted right, the power light over the D-pad
            let pad = shade(body, 0.62);
            a.rect(lx - 9, mid - 3, lx + 9, mid + 3, pad);
            a.rect(lx - 3, mid - 9, lx + 3, mid + 9, pad);
            a.disc(lx, mid - 60, 2, HEART);
            for (dx, dy) in [(8, -6), (-8, 6)] {
                a.disc(rx + dx + 1, mid + dy + 1, 6, shade(body, 0.68));
                a.disc(rx + dx, mid + dy, 6, shade(body, 0.8));
                a.put(rx + dx - 2, mid + dy - 3, shade(body, 1.2));
            }
        }
        Platform::Nds if sides => {
            // the speakers either side of the screen, as the DS Lite's top screen has them
            for sx in [lx - 6, rx - 6] {
                for i in 0..4 {
                    for j in 0..7 {
                        a.rect(
                            sx + i * 4,
                            mid - 12 + j * 4,
                            sx + i * 4 + 1,
                            mid - 11 + j * 4,
                            shade(body, 0.72),
                        );
                    }
                }
            }
        }
        Platform::Snes if sides => {
            // a D-pad left, the four buttons in their diamond right, in their colours softened
            let pad = shade(body, 0.62);
            a.rect(lx - 9, mid - 3, lx + 9, mid + 3, pad);
            a.rect(lx - 3, mid - 9, lx + 3, mid + 9, pad);
            for (dx, dy, c) in [
                (0, -10, [0x7c, 0x9c, 0xe0]),
                (10, 0, [0xe8, 0x84, 0x88]),
                (0, 10, [0xf0, 0xd0, 0x7c]),
                (-10, 0, [0x84, 0xc8, 0x98]),
            ] {
                a.disc(rx + dx + 1, mid + dy + 1, 5, shade(body, 0.7));
                a.disc(rx + dx, mid + dy, 5, c);
            }
        }
        Platform::Ps1 if a.w - x1 >= 10 => {
            // four soft coloured dots down the right
            for (i, c) in [
                [0x84, 0xd0, 0xb0],
                [0xe8, 0x8c, 0x8c],
                [0x8c, 0xb0, 0xe8],
                [0xe0, 0xa0, 0xd0],
            ]
            .into_iter()
            .enumerate()
            {
                a.disc(rx, mid - 24 + i as i32 * 16, 3, c);
            }
        }
        Platform::Psp if a.h - y1 >= 10 => {
            // the speaker on the foot's right, the power light on its left
            let fy = y1 + (a.h - y1) / 2;
            for i in 0..10 {
                for j in 0..2 {
                    a.rect(
                        a.w - 48 + i * 4,
                        fy - 3 + j * 4,
                        a.w - 47 + i * 4,
                        fy - 2 + j * 4,
                        shade(body, 0.62),
                    );
                }
            }
            a.disc(24, fy, 2, [0x60, 0xd0, 0x6c]);
        }
        _ => {}
    }
    // the DS's lights, under its screen where there is room
    if platform == Platform::Nds && a.h - y1 >= 8 {
        let ly = y1 + (a.h - y1) / 2 - 2;
        a.disc(i32::midpoint(x0, x1) - 4, ly, 2, [0x5c, 0xd0, 0x7a]);
        a.disc(i32::midpoint(x0, x1) + 4, ly, 2, [0xff, 0xa4, 0x3c]);
    }
}

/// The drawing at the canvas's size, each art pixel two by two, the picture's clear.
fn upscale(a: &Art, w: u32, h: u32) -> UndoFace {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let (ax, ay) = ((x / ART) as i32, (y / ART) as i32);
            if ax >= a.w || ay >= a.h {
                // an odd last row or column: the shell's own edge
                let o = ((y * w + x) * 4) as usize;
                rgba[o..o + 4].copy_from_slice(&[OUTSIDE[0], OUTSIDE[1], OUTSIDE[2], 255]);
                continue;
            }
            let i = (ay * a.w + ax) as usize;
            if a.hole[i] {
                continue;
            }
            let c = a.px[i];
            let o = ((y * w + x) * 4) as usize;
            rgba[o..o + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    UndoFace { rgba, w, h }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_border_leaves_the_picture_clear_and_covers_the_rest() {
        let picture = [104.0, 48.0, 512.0, 384.0];
        let face = border_face(Platform::Nds, 720, 480, picture).expect("a DS border");
        let alpha = |x: u32, y: u32| face.rgba[((y * 720 + x) * 4 + 3) as usize];
        assert_eq!(alpha(104, 48), 0, "the picture's corner shows");
        assert_eq!(alpha(615, 431), 0, "and its far corner");
        assert_eq!(alpha(103, 200), 255, "the shell beside it");
        assert_eq!(alpha(10, 10), 255);
        assert!(crate::bezel::overlay_clear(&face.rgba, 720, 480, picture));
    }

    #[test]
    fn a_picture_that_fills_the_screen_or_a_gba_has_no_border() {
        assert!(border_face(Platform::Gba, 720, 480, [0.0, 0.0, 720.0, 480.0]).is_none());
        assert!(border_face(Platform::Psp, 720, 480, [0.0, 0.0, 720.0, 480.0]).is_none());
        assert!(border_face(Platform::Gb, 720, 480, [120.0, 24.0, 480.0, 432.0]).is_some());
    }
}
