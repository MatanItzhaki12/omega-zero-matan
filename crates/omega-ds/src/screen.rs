//! The DS's two screens as the core drew them last: their pixels, the top screen above the touch
//! screen in one 256 by 384 frame of XRGB8888, and which layer drew each pixel.

/// One screen's size.
pub const W: usize = 256;
pub const H: usize = 192;
/// One screen's pixels.
pub const PIXELS: usize = W * H;

/// The top screen.
pub const TOP: usize = 0;
/// The touch screen.
pub const BOTTOM: usize = 1;

/// The two screens of one frame.
pub struct Screens<'a> {
    /// The core's frame: the top screen's rows, then the touch screen's, each pixel four bytes
    /// of XRGB8888, little-endian.
    frame: &'a [u8],
    /// Which layer drew each pixel, a screen at a time; empty where the core cannot say.
    layers: [&'a [u8]; 2],
    /// Whether each screen is one colour all over, worked out the first time it is asked.
    blank: [std::cell::Cell<Option<bool>>; 2],
}

impl<'a> Screens<'a> {
    /// The screens in `frame`, which must be the two of them stacked, and their layers. None for a
    /// frame of another size.
    pub fn new(frame: &'a [u8], size: (u32, u32), layers: [&'a [u8]; 2]) -> Option<Screens<'a>> {
        if size != (W as u32, 2 * H as u32) || frame.len() < 2 * PIXELS * 4 {
            return None;
        }
        let layers = layers.map(|l| if l.len() >= PIXELS { l } else { &[][..] });
        Some(Screens {
            frame,
            layers,
            blank: Default::default(),
        })
    }

    /// The pixel at `x, y` of `screen`, 0xRRGGBB.
    #[inline]
    pub fn pixel(&self, screen: usize, x: usize, y: usize) -> u32 {
        let i = ((screen * H + y) * W + x) * 4;
        u32::from_le_bytes([self.frame[i], self.frame[i + 1], self.frame[i + 2], 0])
    }

    /// The flag of the layer that drew `x, y` of `screen`: the backdrop's where none is known.
    #[inline]
    pub fn layer(&self, screen: usize, x: usize, y: usize) -> u8 {
        self.layers[screen].get(y * W + x).copied().unwrap_or(0x20)
    }

    /// Whether `screen` is one colour all over: dark, or a fade's. Read once a frame, however
    /// often it is asked.
    pub fn blank(&self, screen: usize) -> bool {
        if let Some(known) = self.blank[screen].get() {
            return known;
        }
        let rows = self.rows(screen);
        // (the unused fourth byte of each pixel left out, as a core may leave it anything)
        let first = [rows[0], rows[1], rows[2]];
        let one = rows.chunks_exact(4).all(|p| p[..3] == first);
        self.blank[screen].set(Some(one));
        one
    }

    /// The rows of `screen`, as the core drew them, XRGB8888 bytes.
    pub fn rows(&self, screen: usize) -> &'a [u8] {
        &self.frame[screen * PIXELS * 4..(screen + 1) * PIXELS * 4]
    }
}

/// The red, green and blue of 0xRRGGBB.
#[inline]
pub fn rgb(c: u32) -> (i32, i32, i32) {
    (
        ((c >> 16) & 0xFF) as i32,
        ((c >> 8) & 0xFF) as i32,
        (c & 0xFF) as i32,
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A frame of the two screens, each pixel `top` or `bottom` (0xRRGGBB).
    pub(crate) fn frame(top: u32, bottom: u32) -> Vec<u8> {
        let mut f = Vec::with_capacity(2 * PIXELS * 4);
        for i in 0..2 * PIXELS {
            let c = if i < PIXELS { top } else { bottom };
            f.extend_from_slice(&[c as u8, (c >> 8) as u8, (c >> 16) as u8, 0]);
        }
        f
    }

    #[test]
    fn a_frame_of_two_screens_reads_each_screen_apart() {
        let f = frame(0x102030, 0xa0b0c0);
        let s = Screens::new(&f, (256, 384), [&[], &[]]).unwrap();
        assert_eq!(s.pixel(TOP, 10, 10), 0x102030);
        assert_eq!(s.pixel(BOTTOM, 255, 191), 0xa0b0c0);
        assert!(s.blank(TOP) && s.blank(BOTTOM));
        assert_eq!(s.layer(TOP, 0, 0), 0x20, "no layers: the backdrop");
        assert!(
            Screens::new(&f, (256, 192), [&[], &[]]).is_none(),
            "one screen"
        );
    }
}
