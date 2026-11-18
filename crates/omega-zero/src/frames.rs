use std::collections::VecDeque;
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// The emulator thread's pictures on their way to the screen: those waiting, oldest first, and
/// the buffers already drawn to be written again. A picture carries its own size, since a
/// PlayStation game changes resolution as it plays. One waits at most, the newest, unless more
/// are let wait (`set_depth`): a DS game keeps one in hand ahead of the display.
pub struct Frames {
    inner: Mutex<Inner>,
    size: usize,
    taken: AtomicU64,
}

struct Inner {
    ready: VecDeque<(Vec<u8>, (u32, u32))>,
    depth: usize,
    spare: Vec<Vec<u8>>,
    allocated: usize,
}

impl Frames {
    /// Room for pictures of `size` bytes, what a buffer is first made with.
    pub fn new(size: usize) -> Arc<Self> {
        Arc::new(Frames {
            inner: Mutex::new(Inner {
                ready: VecDeque::new(),
                depth: 1,
                spare: Vec::new(),
                allocated: 0,
            }),
            size,
            taken: AtomicU64::new(0),
        })
    }

    pub fn take_write(&self) -> Vec<u8> {
        let mut i = self.lock();
        match i.spare.pop() {
            Some(buf) => buf,
            None => {
                i.allocated += 1;
                Vec::with_capacity(self.size)
            }
        }
    }

    /// Takes back `buf`, from take_write, unused.
    pub fn give_back(&self, buf: Vec<u8>) {
        self.lock().spare.push(buf);
    }

    /// How many pictures may wait for the screen, the oldest let go past them: one, the newest,
    /// or more, each shown in its turn.
    pub fn set_depth(&self, depth: usize) {
        let mut i = self.lock();
        i.depth = depth.max(1);
        while i.ready.len() > i.depth {
            if let Some((dropped, _)) = i.ready.pop_front() {
                i.spare.push(dropped);
            }
        }
    }

    /// Hands on `buf`, a picture `size` pixels wide and high, XRGB8888.
    pub fn publish(&self, buf: Vec<u8>, size: (u32, u32)) {
        let mut i = self.lock();
        i.ready.push_back((buf, size));
        while i.ready.len() > i.depth {
            if let Some((dropped, _)) = i.ready.pop_front() {
                i.spare.push(dropped);
            }
        }
    }

    /// The pictures waiting for the screen.
    pub fn queued(&self) -> usize {
        self.lock().ready.len()
    }

    /// The oldest picture waiting, taken for the screen.
    pub fn latest(self: &Arc<Self>) -> Option<FrameRef> {
        let (buf, size) = self.lock().ready.pop_front()?;
        self.taken.fetch_add(1, Ordering::Relaxed);
        Some(FrameRef {
            frames: self.clone(),
            buf,
            size,
        })
    }

    pub fn is_ready(&self) -> bool {
        !self.lock().ready.is_empty()
    }

    pub fn taken(&self) -> u64 {
        self.taken.load(Ordering::Relaxed)
    }

    pub fn allocated(&self) -> usize {
        self.lock().allocated
    }

    fn recycle(&self, buf: Vec<u8>) {
        self.lock().spare.push(buf);
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// A picture taken from `Frames`, its buffer going back for another when it is dropped. It reads
/// as its pixels, XRGB8888 in rows `size().0` long.
pub struct FrameRef {
    frames: Arc<Frames>,
    buf: Vec<u8>,
    size: (u32, u32),
}

impl FrameRef {
    /// The picture's width and height.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }
}

impl Deref for FrameRef {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.buf
    }
}

impl Drop for FrameRef {
    fn drop(&mut self) {
        self.frames.recycle(std::mem::take(&mut self.buf));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(frames: &Frames, value: u8) {
        let mut buf = frames.take_write();
        buf.clear();
        buf.push(value);
        frames.publish(buf, (1, 1));
    }

    #[test]
    fn one_picture_waits_the_newest_and_a_deeper_queue_shows_each_in_turn() {
        let frames = Frames::new(1);
        picture(&frames, 1);
        picture(&frames, 2);
        assert_eq!(frames.queued(), 1, "the older one is let go");
        assert_eq!(frames.latest().unwrap()[0], 2);
        assert!(frames.latest().is_none());

        frames.set_depth(2);
        picture(&frames, 3);
        picture(&frames, 4);
        assert_eq!(frames.queued(), 2);
        assert_eq!(frames.latest().unwrap()[0], 3, "the oldest first");
        assert_eq!(frames.latest().unwrap()[0], 4);
        picture(&frames, 5);
        picture(&frames, 6);
        picture(&frames, 7);
        assert_eq!(frames.queued(), 2, "past the depth, the oldest goes");
        assert_eq!(frames.latest().unwrap()[0], 6);

        frames.set_depth(1);
        assert_eq!(frames.queued(), 1, "a shallower queue keeps the newest");
        assert_eq!(frames.latest().unwrap()[0], 7);
    }
}
