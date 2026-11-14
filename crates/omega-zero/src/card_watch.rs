//! The card watched for what changes on it behind omega-zero's back: a game copied in or taken
//! away, a label added or drawn over, a cart's colours set in the label configs, all as over
//! SSH or with the card in a computer while omega-zero runs.
//!
//! A thread of its own, at the low priority the card's other background work runs at, takes a
//! fingerprint of the card every [`EVERY`]: when each system's Games and Labels folders and
//! every label in them were last changed, and the label configs. Only where the fingerprint
//! moved does it scan the card again, as boot does, and tell what came of it apart from what it
//! had: the games added, with their faces already drawn for them; the games gone; and the
//! games whose label, colours or title changed, whose faces are to be drawn again. All of that
//! is done on the thread, so the shelf on the main thread only uploads the few faces new games
//! bring and stands its rows again.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use omega_store::{Cart, Platform};
use omega_ui::CartFace;

/// How often the card is looked at: soon enough that a game copied in is on the shelf by the time
/// the copy is looked for, rarely enough that the looks cost nothing beside the game.
pub const EVERY: Duration = Duration::from_secs(2);

/// What the card's games, labels and label configs were last seen as: a folder's or a file's
/// path, its size and when it was last changed.
pub type Fingerprint = Vec<(PathBuf, u64, Option<SystemTime>)>;

/// The card's games, labels and label configs as they stand, cheaply: the folders and files
/// looked at, not read.
pub fn fingerprint(root: &Path) -> Fingerprint {
    let stamp = |path: PathBuf| {
        let meta = std::fs::metadata(&path).ok();
        let size = meta.as_ref().map_or(0, std::fs::Metadata::len);
        let when = meta.and_then(|m| m.modified().ok());
        (path, size, when)
    };
    let mut out = Vec::new();
    for platform in Platform::ALL {
        out.push(stamp(root.join("Games").join(platform.dir_name())));
        let labels = root.join("Labels").join(platform.dir_name());
        out.push(stamp(labels.clone()));
        // a label drawn over in place leaves its folder as it was: each label is looked at
        let Ok(entries) = std::fs::read_dir(&labels) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .map(|e| e.path())
            .collect();
        files.sort();
        out.extend(files.into_iter().map(stamp));
    }
    for config in [
        omega_store::CART_SHELL_FILE,
        omega_store::LABELS_SHELL_FILE,
        omega_store::SLOT_SHELL_FILE,
    ] {
        out.push(stamp(root.join(config)));
    }
    out
}

/// What changed on the card since the shelf was last stood.
pub struct Change {
    /// Every game on the card now, as boot would find them.
    pub carts: Vec<Cart>,
    /// The face each game added is drawn on until its label is cut, drawn on the watching
    /// thread, by its system and name.
    pub faces: Vec<(Platform, String, CartFace)>,
    /// The games whose faces are to be drawn again: their label, its picture, their colours or
    /// their title changed, or they were added wearing a label.
    pub redraw: Vec<(Platform, String)>,
}

impl Change {
    /// What `now` holds that `was` did not: the faces of the games added, and the games to draw
    /// again; none where nothing a shelf shows changed. `labels_moved` names the games whose
    /// label pictures were drawn over in place, which their carts alone do not show.
    pub fn between(
        was: &[Cart],
        now: Vec<Cart>,
        labels_moved: &[(Platform, String)],
    ) -> Option<Change> {
        let before: HashMap<(Platform, &str), &Cart> = was
            .iter()
            .map(|c| ((c.platform, c.stem.as_str()), c))
            .collect();
        let mut faces = Vec::new();
        let mut redraw = Vec::new();
        for cart in &now {
            match before.get(&(cart.platform, cart.stem.as_str())) {
                None => {
                    faces.push((
                        cart.platform,
                        cart.stem.clone(),
                        omega_ui::generated_face(cart),
                    ));
                    if omega_ui::wears_label(cart) {
                        redraw.push((cart.platform, cart.stem.clone()));
                    }
                }
                Some(old) if *old != cart => redraw.push((cart.platform, cart.stem.clone())),
                Some(_) if labels_moved.contains(&(cart.platform, cart.stem.clone())) => {
                    redraw.push((cart.platform, cart.stem.clone()));
                }
                Some(_) => {}
            }
        }
        let gone = was.len() + faces.len() != now.len();
        if faces.is_empty() && redraw.is_empty() && !gone {
            return None;
        }
        Some(Change {
            carts: now,
            faces,
            redraw,
        })
    }
}

/// The games whose label pictures changed between two fingerprints of the card, by the label's
/// file name, as `<system>/<name>.<ext>`: the games of `carts` wearing one of them.
fn labels_moved(was: &Fingerprint, now: &Fingerprint, carts: &[Cart]) -> Vec<(Platform, String)> {
    let before: HashMap<&PathBuf, (u64, Option<SystemTime>)> = was
        .iter()
        .map(|(p, size, when)| (p, (*size, *when)))
        .collect();
    let moved: Vec<&PathBuf> = now
        .iter()
        .filter(|(p, size, when)| before.get(p).is_some_and(|b| *b != (*size, *when)))
        .map(|(p, ..)| p)
        .collect();
    carts
        .iter()
        .filter(|c| c.label.as_ref().is_some_and(|l| moved.contains(&l)))
        .map(|c| (c.platform, c.stem.clone()))
        .collect()
}

/// The thread watching the card, and what it found that the shelf has not taken yet.
pub struct CardWatch {
    found: Receiver<Change>,
    stop: Arc<AtomicBool>,
}

impl CardWatch {
    /// Watches the card at `root`, whose games were found as `carts` as the shelf was stood.
    pub fn spawn(root: PathBuf, carts: Vec<Cart>) -> Option<CardWatch> {
        let (tell, found) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        std::thread::Builder::new()
            .name("omega-card-watch".into())
            .spawn(move || {
                crate::spawn::stand_back();
                // none to begin with, so the first look scans the card and finds what changed
                // between boot's scan and the thread starting
                let mut seen = Fingerprint::new();
                let mut carts = carts;
                while !stopped.load(Ordering::Acquire) {
                    std::thread::sleep(EVERY);
                    let now = fingerprint(&root);
                    if now == seen {
                        continue;
                    }
                    let Ok(found) = omega_store::scan(&root) else {
                        continue;
                    };
                    let moved = labels_moved(&seen, &now, &found);
                    seen = now;
                    let Some(change) = Change::between(&carts, found.clone(), &moved) else {
                        continue;
                    };
                    crate::log!(
                        "card: {} games, {} added, {} to draw again",
                        change.carts.len(),
                        change.faces.len(),
                        change.redraw.len()
                    );
                    carts = found;
                    if tell.send(change).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some(CardWatch { found, stop })
    }

    /// The latest change found, the ones before it folded in: the games as the newest has
    /// them, and every face drawn or to draw again since the shelf last took one.
    pub fn take(&self) -> Option<Change> {
        let mut out: Option<Change> = None;
        loop {
            match self.found.try_recv() {
                Ok(next) => {
                    out = Some(match out {
                        None => next,
                        Some(mut was) => {
                            was.carts = next.carts;
                            was.faces.extend(next.faces);
                            was.redraw.extend(next.redraw);
                            was
                        }
                    });
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return out,
            }
        }
    }
}

impl Drop for CardWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cart(stem: &str) -> Cart {
        Cart {
            platform: Platform::Gba,
            stem: stem.into(),
            rom: format!("Games/GBA/{stem}.gba").into(),
            label: None,
            title: String::new(),
            code: String::new(),
            shell: None,
        }
    }

    #[test]
    fn a_game_added_comes_with_its_face_and_one_changed_is_drawn_again() {
        let was = vec![cart("Emerald"), cart("Fusion")];
        assert!(
            Change::between(&was, was.clone(), &[]).is_none(),
            "nothing changed"
        );

        let mut relabelled = cart("Fusion");
        relabelled.label = Some("Labels/GBA/Fusion.png".into());
        let now = vec![cart("Emerald"), relabelled, cart("Zero")];
        let change = Change::between(&was, now, &[]).expect("a change");
        assert_eq!(change.carts.len(), 3);
        assert_eq!(change.faces.len(), 1);
        assert_eq!(change.faces[0].1, "Zero");
        assert_eq!(change.redraw, [(Platform::Gba, "Fusion".to_string())]);

        // a game taken away alone is a change too, with nothing to draw
        let change = Change::between(&was, vec![cart("Emerald")], &[]).expect("a change");
        assert!(change.faces.is_empty() && change.redraw.is_empty());

        // a label drawn over in place leaves the cart as it was
        let change = Change::between(&was, was.clone(), &[(Platform::Gba, "Emerald".into())])
            .expect("a change");
        assert_eq!(change.redraw, [(Platform::Gba, "Emerald".to_string())]);
    }

    #[test]
    fn a_label_drawn_over_in_place_moves_the_fingerprint_and_names_its_game() {
        let d = tempfile::tempdir().unwrap();
        let label = d.path().join("Labels/GBA/Emerald.png");
        std::fs::create_dir_all(label.parent().unwrap()).unwrap();
        std::fs::write(&label, b"first").unwrap();
        let was = fingerprint(d.path());
        assert_eq!(
            was,
            fingerprint(d.path()),
            "looked at twice, the card is the same"
        );
        std::fs::write(&label, b"drawn again, longer").unwrap();
        let now = fingerprint(d.path());
        assert_ne!(was, now);
        let mut emerald = cart("Emerald");
        emerald.label = Some(label);
        assert_eq!(
            labels_moved(&was, &now, &[emerald, cart("Fusion")]),
            [(Platform::Gba, "Emerald".to_string())]
        );
    }

    #[test]
    fn a_game_copied_onto_the_card_is_found_by_the_watching_thread() {
        let d = tempfile::tempdir().unwrap();
        let games = d.path().join("Games/GBA");
        std::fs::create_dir_all(&games).unwrap();
        std::fs::write(games.join("Emerald.gba"), [0u8; 512]).unwrap();
        let first = omega_store::scan(d.path()).unwrap();
        let watch = CardWatch::spawn(d.path().to_path_buf(), first).expect("the thread");
        std::fs::write(games.join("Zero.gba"), [0u8; 512]).unwrap();
        let began = std::time::Instant::now();
        let change = loop {
            if let Some(change) = watch.take() {
                break change;
            }
            assert!(
                began.elapsed() < EVERY * 4,
                "the game copied in was never found"
            );
            std::thread::sleep(Duration::from_millis(50));
        };
        let stems: Vec<&str> = change.carts.iter().map(|c| c.stem.as_str()).collect();
        assert_eq!(stems, ["Emerald", "Zero"]);
        assert_eq!(change.faces.len(), 1);
    }
}
