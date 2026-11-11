//! The small files omega-zero keeps on the card, written on a thread of their own once
//! [`defer`] is on, as the handheld has it: each write is synced to the card, and a sync waits
//! behind whatever else is being written there, a game's state as it comes out among them, which
//! the screen would otherwise wait out. Off, as tests have it, each is written at once.
//!
//! Writes to one file are taken in order and only the newest is kept, so a file written twice
//! before the first reached the card is written once. [`settle`] waits for every write asked
//! for to be on the card: before powering off or restarting.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, Once};

use omega_store::atomic_write;

static DEFER: AtomicBool = AtomicBool::new(false);
static QUEUE: Mutex<Queue> = Mutex::new(Queue {
    jobs: VecDeque::new(),
    busy: false,
});
static CHANGED: Condvar = Condvar::new();

struct Queue {
    jobs: VecDeque<Job>,
    /// A job taken off the queue is being written.
    busy: bool,
}

enum Job {
    Write(PathBuf, Vec<u8>),
    Remove(PathBuf),
}

impl Job {
    fn path(&self) -> &Path {
        match self {
            Job::Write(path, _) | Job::Remove(path) => path,
        }
    }

    fn run(self) {
        match self {
            Job::Write(path, bytes) => {
                if let Err(e) = atomic_write(&path, &bytes) {
                    crate::log!("{}: {e}", path.display());
                }
            }
            Job::Remove(path) => {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

fn lock() -> std::sync::MutexGuard<'static, Queue> {
    QUEUE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Writes on the card's own thread from now on, or, off, at once.
pub fn defer(on: bool) {
    if !on {
        settle();
    }
    DEFER.store(on, Ordering::Release);
}

/// Writes `bytes` to `path`, whole or not at all: at once, or on the card's thread.
pub fn write(path: &Path, bytes: Vec<u8>) -> std::io::Result<()> {
    if !DEFER.load(Ordering::Acquire) {
        return atomic_write(path, &bytes);
    }
    ask(Job::Write(path.to_path_buf(), bytes));
    Ok(())
}

/// Takes the file at `path` off the card: at once, or on the card's thread.
pub fn remove(path: &Path) {
    if !DEFER.load(Ordering::Acquire) {
        let _ = std::fs::remove_file(path);
        return;
    }
    ask(Job::Remove(path.to_path_buf()));
}

fn ask(job: Job) {
    static MADE: Once = Once::new();
    MADE.call_once(|| {
        let spawned = std::thread::Builder::new()
            .name("omega-card".into())
            .spawn(run);
        if let Err(e) = spawned {
            crate::log!("no thread to write the card on: {e}");
        }
    });
    let mut queue = lock();
    // the newest write to a file stands for the ones before it still waiting
    queue.jobs.retain(|waiting| waiting.path() != job.path());
    queue.jobs.push_back(job);
    CHANGED.notify_all();
}

fn run() {
    crate::spawn::stand_back();
    loop {
        let job = {
            let mut queue = lock();
            loop {
                if let Some(job) = queue.jobs.pop_front() {
                    queue.busy = true;
                    break job;
                }
                queue = CHANGED.wait(queue).unwrap_or_else(|e| e.into_inner());
            }
        };
        job.run();
        lock().busy = false;
        CHANGED.notify_all();
    }
}

/// Waits for every write asked for so far to be on the card.
pub fn settle() {
    let mut queue = lock();
    while !queue.jobs.is_empty() || queue.busy {
        queue = CHANGED.wait(queue).unwrap_or_else(|e| e.into_inner());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deferred_writes_land_once_settled_and_only_the_newest_of_a_file() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a.txt"), d.path().join("b.txt"));
        std::fs::write(&b, "there").unwrap();
        // through the queue itself: switching deferring on would defer the other tests' writes
        ask(Job::Write(a.clone(), b"one".to_vec()));
        ask(Job::Write(a.clone(), b"two".to_vec()));
        ask(Job::Remove(b.clone()));
        settle();
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "two");
        assert!(!b.exists());
    }
}
