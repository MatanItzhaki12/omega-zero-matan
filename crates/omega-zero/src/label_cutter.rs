//! The threads that draw carts' faces from their labels, so no label is decoded on the main
//! thread: at boot for every cart that wears a scan, and for each label a sync saves. The shelf
//! comes up with every cart in its generated face, and each one is drawn again in its label as
//! its face comes back. A scan with no cut yet is cut first, so its face is drawn from the cut,
//! and once its face is handed back the scan is shrunk to what its faces need.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use omega_store::{Cart, Platform};
use omega_ui::{cart_face, cut_label, shrink_label, CartFace};

/// Drawing a face from its label is mostly decoding it, on one core; the rest are left to the
/// shelf.
const MAX_WORKERS: usize = 3;

/// How many drawn faces can wait for compose to take them. A worker with one more waits for
/// room, so a boot's hundreds of faces are never all held at once on a handheld with 1 GB.
const READY_FACES: usize = 8;

/// A cart to draw from its label, with the ticket it was asked under.
struct Job {
    cart: Cart,
    ticket: u64,
}

/// A cart's face drawn from its label, for the cart's face on the shelf.
pub struct Drawn {
    pub platform: Platform,
    pub stem: String,
    pub face: CartFace,
}

/// The carts waiting to be drawn, in the order the workers take them, and whether the cutter is
/// gone, which sends the workers home. A deque rather than a channel, so the carts that come
/// into view can be moved to its front.
#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
    closed: bool,
}

type Inbox = Arc<(Mutex<Queue>, Condvar)>;

/// The next job, waiting for one; none once the cutter is gone.
fn next_job(inbox: &Inbox) -> Option<Job> {
    let (lock, ready) = &**inbox;
    let mut queue = lock.lock().ok()?;
    loop {
        if queue.closed {
            return None;
        }
        if let Some(job) = queue.jobs.pop_front() {
            return Some(job);
        }
        queue = ready.wait(queue).ok()?;
    }
}

pub struct LabelCutter {
    inbox: Inbox,
    drawn: Receiver<(u64, Drawn)>,
    /// The ticket of the last face asked for each cart whose face has not come back yet: a face
    /// drawn for an ask a newer one has overtaken is not drawn over the newer one.
    asked: HashMap<(u8, String), u64>,
    tickets: u64,
}

impl LabelCutter {
    pub fn spawn() -> Self {
        let (outbox, drawn) = mpsc::sync_channel(READY_FACES);
        let inbox: Inbox = Arc::new((Mutex::new(Queue::default()), Condvar::new()));
        let workers = thread::available_parallelism()
            .map_or(1, |n| n.get().saturating_sub(1))
            .clamp(1, MAX_WORKERS);
        for i in 0..workers {
            let inbox = Arc::clone(&inbox);
            let outbox = outbox.clone();
            let spawned = thread::Builder::new()
                .name(format!("omega-cut-{i}"))
                .spawn(move || {
                    // a boot that resumes a game draws its labels while the game runs
                    crate::spawn::stand_back();
                    loop {
                        let Some(Job { cart, ticket }) = next_job(&inbox) else {
                            return;
                        };
                        cut_label(&cart);
                        let face = Drawn {
                            platform: cart.platform,
                            stem: cart.stem.clone(),
                            face: cart_face(&cart),
                        };
                        if outbox.send((ticket, face)).is_err() {
                            return;
                        }
                        shrink_label(&cart);
                    }
                });
            if let Err(e) = spawned {
                crate::log!("labels: cutter thread failed to start: {e}");
            }
        }
        LabelCutter {
            inbox,
            drawn,
            asked: HashMap::new(),
            tickets: 0,
        }
    }

    /// Draws `cart`'s face from its label, after the carts asked for before it: its scans are
    /// cut first where they have no cut, and shrunk once its face is handed back.
    pub fn request(&mut self, cart: Cart) {
        self.tickets += 1;
        let key = (cart.platform as u8, cart.stem.clone());
        self.asked.insert(key, self.tickets);
        let (lock, ready) = &*self.inbox;
        if let Ok(mut queue) = lock.lock() {
            queue.jobs.push_back(Job {
                cart,
                ticket: self.tickets,
            });
            ready.notify_one();
        }
    }

    /// Moves the carts named, those still waiting, to the front of the queue in the order given:
    /// the carts that came into view are drawn next, on whichever shelf they are, FAVORITES and
    /// RECENTS too.
    pub fn hurry(&self, carts: &[(Platform, String)]) {
        let (lock, _) = &*self.inbox;
        let Ok(mut queue) = lock.lock() else {
            return;
        };
        for (platform, stem) in carts.iter().rev() {
            let at = queue
                .jobs
                .iter()
                .position(|j| j.cart.platform == *platform && j.cart.stem == *stem);
            if let Some(job) = at.and_then(|i| queue.jobs.remove(i)) {
                queue.jobs.push_front(job);
            }
        }
    }

    /// How many carts are still waiting to be drawn, not counting those being drawn.
    pub fn waiting(&self) -> usize {
        self.inbox.0.lock().map_or(0, |q| q.jobs.len())
    }

    /// The faces handed back and not taken yet, until `bytes` of them are taken, the rest left
    /// for the next time: always one at least, when one is waiting. A face drawn for a cart
    /// asked for again since is passed over.
    pub fn take(&mut self, bytes: usize) -> Vec<Drawn> {
        let mut taken = Vec::new();
        let mut spent = 0;
        while spent < bytes {
            let Ok((ticket, drawn)) = self.drawn.try_recv() else {
                break;
            };
            let key = (drawn.platform as u8, drawn.stem.clone());
            if self.asked.get(&key) != Some(&ticket) {
                continue;
            }
            self.asked.remove(&key);
            spent += drawn.face.rgba.len();
            taken.push(drawn);
        }
        taken
    }

    /// Whether every face asked for has come back and been taken.
    pub fn idle(&self) -> bool {
        self.asked.is_empty()
    }
}

impl Drop for LabelCutter {
    /// Sends the workers home, as dropping the channel's sender once did.
    fn drop(&mut self) {
        let (lock, ready) = &*self.inbox;
        if let Ok(mut queue) = lock.lock() {
            queue.closed = true;
        }
        ready.notify_all();
    }
}
