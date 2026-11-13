use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// The emulator thread's pictures on their way to the screen: the newest one waiting, and the
/// buffers already drawn to be written again. A picture carries its own size, since a
/// PlayStation game changes resolution as it plays.
pub struct Frames {
    inner: Mutex<Inner>,
    size: usize,
    taken: AtomicU64,
}

struct Inner {
    ready: Option<(Vec<u8>, (u32, u32))>,
    spare: Vec<Vec<u8>>,
    allocated: usize,
}

impl Frames {
    /// Room for pictures of `size` bytes, what a buffer is first made with.
    pub fn new(size: usize) -> Arc<Self> {
        Arc::new(Frames {
            inner: Mutex::new(Inner {
                ready: None,
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

    /// Hands on `buf`, a picture `size` pixels wide and high, XRGB8888.
    pub fn publish(&self, buf: Vec<u8>, size: (u32, u32)) {
        let mut i = self.lock();
        if let Some((dropped, _)) = i.ready.replace((buf, size)) {
            i.spare.push(dropped);
        }
    }

    pub fn latest(self: &Arc<Self>) -> Option<FrameRef> {
        let (buf, size) = self.lock().ready.take()?;
        self.taken.fetch_add(1, Ordering::Relaxed);
        Some(FrameRef {
            frames: self.clone(),
            buf,
            size,
        })
    }

    pub fn is_ready(&self) -> bool {
        self.lock().ready.is_some()
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
