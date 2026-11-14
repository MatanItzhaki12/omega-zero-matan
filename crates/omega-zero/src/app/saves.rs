//! Saves and save states: the battery save and resume state written as a cart comes out or
//! as the game plays on, and the states saved, loaded and picked in the switcher.

use super::*;

impl App {
    pub(super) fn flush_eject(&mut self, stem: &str) {
        // the time the game was played goes on the card as it comes out, and the shelf takes
        // it where it is sorted by it
        self.write_play_log();
        if self.state.shelf_by_play {
            self.sort_shelves();
        }
        self.settle_saves();
        let (Some(root), Some(snapshot)) = (&self.root, &self.snapshot) else {
            return;
        };
        // a running core's state is taken and written on a thread of its own: the core giving
        // a PSP game's state up takes a tenth of a second, and the card a moment to sync it,
        // which the cart coming out would otherwise wait out. The emulator answers the ask
        // before it stops, and the next core to open waits for the write before it reads a
        // resume. The empty slot is kept with the rest of omega-zero's state, which the card's
        // writer takes in order, rather than by rewriting the state file from that thread over
        // a newer one
        let taken = match snapshot.detached() {
            Some(detached) => Taken::Later(detached),
            None => match snapshot.state() {
                Some(state) => Taken::Now(trusted_write(snapshot.as_ref(), state, "eject")),
                None => {
                    crate::log!("eject: the core gave up no state");
                    return;
                }
            },
        };
        let behind = matches!(taken, Taken::Later(_));
        let eject_save = self.state.eject_save;
        let (root, platform, core, stem) =
            (root.clone(), self.platform, self.core, stem.to_owned());
        let stem_of_write = stem.clone();
        let write = move || {
            let (state, sav) = match taken {
                Taken::Now(written) => written,
                Taken::Later(snapshot) => {
                    let Some(state) = snapshot.state() else {
                        return Err(std::io::Error::other("the core gave up no state"));
                    };
                    trusted_write(snapshot.as_ref(), state, "eject")
                }
            };
            let state = state.filter(|_| eject_save);
            match behind {
                true => persist::flush(
                    &root,
                    platform,
                    core,
                    &stem,
                    state.as_deref(),
                    sav.as_deref(),
                ),
                false => persist::eject(
                    &root,
                    platform,
                    core,
                    &stem,
                    state.as_deref(),
                    sav.as_deref(),
                ),
            }
        };
        // the backup looks for the save once it is on the card
        let pass = self
            .save_backup
            .as_ref()
            .map(super::super::save_backup::SaveBackup::passer);
        let written = match behind {
            true => std::thread::Builder::new()
                .name("omega-eject-save".into())
                .spawn(move || match write() {
                    Ok(()) => pass.iter().for_each(|pass| pass()),
                    Err(e) => crate::log!("eject: {e}"),
                })
                .map(|h| {
                    self.pending_save = Some(h);
                    self.pending_for = Some((platform, stem_of_write));
                })
                .map_err(|e| crate::log!("eject: no writer thread: {e}"))
                .is_ok(),
            false => write().map_err(|e| crate::log!("eject: {e}")).is_ok(),
        };
        if written {
            self.state.cart = None;
            self.state.cart_platform = None;
            if behind {
                self.persist();
            }
        }
        if !behind {
            self.backup_pass();
        }
    }

    /// The save being written to the card of the game by `stem` on `platform`, handed to the
    /// thread its core opens on, which waits for it before it reads the game's resume. Another
    /// game's is left to finish on its own: the game starting reads nothing it writes.
    pub(crate) fn take_pending_save(
        &mut self,
        platform: Platform,
        stem: &str,
    ) -> Option<std::thread::JoinHandle<()>> {
        let ours = self
            .pending_for
            .as_ref()
            .is_none_or(|(p, s)| *p == platform && s == stem);
        ours.then(|| self.pending_save.take()).flatten()
    }

    pub fn settle_saves(&mut self) {
        if let Some(h) = self.pending_save.take() {
            let _ = h.join();
        }
    }

    /// How long between autosaves: a PSP state is 40 MB, which the core takes a moment to give
    /// up and the card some seconds to take, so a PSP game, which keeps saves of its own, is
    /// autosaved less often. A state is still written as the cart comes out and as the console
    /// sleeps.
    pub(super) fn autosave_every(&self) -> Millis {
        match self.platform {
            Platform::Psp => AUTOSAVE_PSP_MS,
            _ => AUTOSAVE_MS,
        }
    }

    pub(super) fn autosave(&mut self) {
        self.autosave_at = self.now() + self.autosave_every();
        self.write_play_log();
        self.settle_saves();
        let (Some(root), Some(snapshot), Some(cart)) = (&self.root, &self.snapshot, self.seated())
        else {
            return;
        };
        let (root, platform, core, cart) =
            (root.clone(), self.platform, self.core, cart.to_owned());
        let autosaved = cart.clone();
        // the state is taken on the writer's thread where it can be, as the core giving it up
        // takes a frame or more, which the screen would otherwise wait out every autosave
        let taken = match snapshot.detached() {
            Some(detached) => Taken::Later(detached),
            None => match snapshot.state() {
                Some(state) => Taken::Now(trusted_write(snapshot.as_ref(), state, "autosave")),
                None => {
                    crate::log!("autosave: the core gave up no state");
                    return;
                }
            },
        };
        let pass = self
            .save_backup
            .as_ref()
            .map(super::super::save_backup::SaveBackup::passer);
        let write = move || {
            let (state, sav) = match taken {
                Taken::Now(written) => written,
                Taken::Later(snapshot) => {
                    let Some(state) = snapshot.state() else {
                        crate::log!("autosave: the core gave up no state");
                        return;
                    };
                    trusted_write(snapshot.as_ref(), state, "autosave")
                }
            };
            if let Err(e) = persist::autosave_flush(
                &root,
                platform,
                core,
                &cart,
                state.as_deref(),
                sav.as_deref(),
            ) {
                crate::log!("autosave: {e}");
                return;
            }
            // the backup looks for the save once it is on the card
            if let Some(pass) = &pass {
                pass();
            }
        };
        match std::thread::Builder::new()
            .name("omega-autosave".into())
            .spawn(move || {
                crate::spawn::stand_back();
                write();
            }) {
            Ok(h) => {
                self.pending_save = Some(h);
                self.pending_for = Some((self.platform, autosaved));
            }
            Err(e) => crate::log!("autosave: no writer thread: {e}"),
        }
    }

    pub fn flush_resume(&mut self) {
        self.settle_saves();
        self.autosave_at = self.now() + self.autosave_every();
        let (Some(root), Some(snapshot), Some(cart)) = (&self.root, &self.snapshot, self.seated())
        else {
            return;
        };
        let Some(state) = snapshot.state() else {
            crate::log!("flush: the core gave up no state");
            return;
        };
        let (state, sav) = trusted_write(snapshot.as_ref(), state, "flush");
        if let Err(e) = persist::flush(
            root,
            self.platform,
            self.core,
            cart,
            state.as_deref(),
            sav.as_deref(),
        ) {
            crate::log!("flush: {e}");
        }
        self.backup_pass();
    }

    pub(crate) fn ring(&self) -> Option<StateRing> {
        let (Some(root), Some(cart)) = (&self.root, self.seated()) else {
            return None;
        };
        Some(StateRing::new(root, self.platform, self.core, cart))
    }

    pub(crate) fn seated(&self) -> Option<&str> {
        match &self.phase {
            Phase::Playing { cart } | Phase::Polaroids { cart } => Some(cart),
            _ => None,
        }
    }

    pub(crate) fn entries(&self) -> Vec<StateEntry> {
        self.ring().and_then(|r| r.list().ok()).unwrap_or_default()
    }

    pub(crate) fn open_polaroids(&mut self) {
        if self.link_active() {
            return self.refuse();
        }
        let entries = self.entries();
        if entries.is_empty() {
            return self.refuse();
        }
        let Phase::Playing { cart } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        self.polaroids = Some(Polaroids::new(entries));
        self.phase = Phase::Polaroids { cart };
    }

    pub(crate) fn close_polaroids(&mut self) {
        let Phase::Polaroids { cart } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        self.polaroids = None;
        self.delete_asked = false;
        self.phase = Phase::Playing { cart };
    }

    pub(super) fn load_selected(&mut self) {
        let state = self
            .polaroids
            .as_ref()
            .and_then(|p| p.selected())
            .map(|e| e.state.clone());
        let refused = state.map(|state| self.load_file(&state)) == Some(false);
        if !refused {
            self.close_polaroids();
        }
    }

    pub(crate) fn load_newest(&mut self) {
        let Some(newest) = self.entries().first().map(|e| e.state.clone()) else {
            return self.refuse();
        };
        self.load_file(&newest);
    }

    pub(super) fn load_file(&mut self, state: &Path) -> bool {
        if !self.may_load_state() {
            self.refuse();
            return false;
        }
        let Some(snapshot) = &self.snapshot else {
            return false;
        };
        let bytes = match std::fs::read(state) {
            Ok(bytes) => bytes,
            Err(e) => {
                crate::log!("load: {e}");
                return false;
            }
        };
        // the game as it was, for Undo Load to go back to
        let prior = snapshot.state();
        snapshot.load(bytes);
        if let Some(prior) = prior {
            self.note_loaded(prior);
        }
        self.hud.toast(Toast::StateLoaded, self.now());
        true
    }

    pub(crate) fn save_state(&mut self) {
        let (Some(ring), Some(snapshot)) = (self.ring(), &self.snapshot) else {
            return;
        };
        if !snapshot.resume_trusted() {
            crate::log!("save: the core refused the resume it was given, not pushing a state");
            return self.refuse();
        }
        let Some(state) = snapshot.state() else {
            crate::log!("save: the core gave up no state");
            return;
        };
        let thumb = snapshot.thumb().unwrap_or_default();
        let stamp = free_stamp(&ring, self.wall_secs());
        // the state a full ring drops for this one, for Undo Save to put back
        let evicted = self.oldest_if_full();
        if let Err(e) = ring.push(&state, &thumb, &stamp) {
            crate::log!("save: {e}");
            return;
        }
        self.note_saved(stamp, evicted);
        self.hud.toast(Toast::StateSaved, self.now());
    }
}
