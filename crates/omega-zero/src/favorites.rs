//! Favorites, as Slot keeps them: Y on the shelf stars the cart picked, or takes its star off,
//! and every starred cart stands on a FAVORITES shelf of its own, the first of the shelves, once
//! there is one. A cart is starred by its system and its name, one to a line of
//! Config/favorites.txt as `gba/Pokemon - Emerald Version (USA, Europe)`, the file Slot writes,
//! so a card moved over from Slot keeps its favorites.
//!
//! FAVORITES stands its carts by system, in the shelves' order, and by name within each. Every
//! starred cart wears a gold star at its corner, on every shelf it stands on.
//!
//! RECENTS, once Settings > Shelf shows it, stands before FAVORITES: the last ten games put in,
//! the latest first, kept in Config/recents.txt in the same lines, as Slot keeps them.
//!
//! Both shelves draw each cart on the face its own system's shelf has for it, so a label that
//! arrives redraws them all, and a boot draws no face twice.

use std::path::{Path, PathBuf};

use omega_input::Millis;
use omega_store::{sort_key, Cart, Platform, CONFIG_DIR};
use omega_ui::Toast;

use crate::app::App;

pub const FAVORITES_FILE: &str = "favorites.txt";
pub const RECENTS_FILE: &str = "recents.txt";

/// How many games RECENTS holds.
pub const RECENTS_MAX: usize = 10;

/// What a shelf holds: the carts of one system, the last games played, or the starred carts of
/// every system.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ShelfKind {
    Recents,
    Favorites,
    System(Platform),
}

impl ShelfKind {
    /// The system whose carts the shelf holds; none for FAVORITES, which holds any.
    pub fn platform(self) -> Option<Platform> {
        match self {
            ShelfKind::System(p) => Some(p),
            ShelfKind::Recents | ShelfKind::Favorites => None,
        }
    }

    /// What the shelf's pill says.
    pub fn pill(self) -> &'static str {
        match self {
            ShelfKind::System(p) => p.dir_name(),
            ShelfKind::Favorites => "FAVORITES",
            ShelfKind::Recents => "RECENTS",
        }
    }

    /// The shelf's name in full.
    pub fn name(self) -> &'static str {
        match self {
            ShelfKind::System(p) => p.name(),
            ShelfKind::Favorites => "Favorites",
            ShelfKind::Recents => "Recents",
        }
    }
}

fn favorites_path(root: &Path) -> PathBuf {
    root.join(CONFIG_DIR).join(FAVORITES_FILE)
}

fn recents_path(root: &Path) -> PathBuf {
    root.join(CONFIG_DIR).join(RECENTS_FILE)
}

/// The games put in last, the latest first.
pub fn read_recents(root: &Path) -> Vec<(Platform, String)> {
    std::fs::read_to_string(recents_path(root))
        .map(|text| {
            text.lines()
                .filter_map(parse_line)
                .take(RECENTS_MAX)
                .collect()
        })
        .unwrap_or_default()
}

pub fn write_recents(root: &Path, recents: &[(Platform, String)]) -> std::io::Result<()> {
    let text: String = recents
        .iter()
        .map(|(p, stem)| line_of(*p, stem) + "\n")
        .collect();
    crate::card_writer::write(&recents_path(root), text.into_bytes())
}

/// `cart` as the latest game put in, ahead of `recents`, which keep it once and ten at most.
pub fn with_recent(recents: &[(Platform, String)], cart: &Cart) -> Vec<(Platform, String)> {
    std::iter::once((cart.platform, cart.stem.clone()))
        .chain(
            recents
                .iter()
                .filter(|(p, s)| !(*p == cart.platform && *s == cart.stem))
                .cloned(),
        )
        .take(RECENTS_MAX)
        .collect()
}

/// Where a cart stands on FAVORITES: its system's place among the shelves, then its name.
fn favorite_key(cart: &Cart) -> (usize, (u8, String)) {
    let system = Platform::ALL
        .iter()
        .position(|p| *p == cart.platform)
        .unwrap_or(usize::MAX);
    (system, sort_key(&cart.stem))
}

/// A favorite's line: its system's folder in small letters, a slash, then its name.
fn line_of(platform: Platform, stem: &str) -> String {
    format!("{}/{stem}", platform.dir_name().to_ascii_lowercase())
}

fn parse_line(line: &str) -> Option<(Platform, String)> {
    let (dir, stem) = line.trim_end_matches('\r').split_once('/')?;
    let platform = Platform::ALL
        .into_iter()
        .find(|p| dir.eq_ignore_ascii_case(p.dir_name()))?;
    (!stem.is_empty()).then(|| (platform, stem.to_string()))
}

/// The carts starred on the card, in the order they were starred.
pub fn read_favorites(root: &Path) -> Vec<(Platform, String)> {
    std::fs::read_to_string(favorites_path(root))
        .map(|text| text.lines().filter_map(parse_line).collect())
        .unwrap_or_default()
}

pub fn write_favorites(root: &Path, favorites: &[(Platform, String)]) -> std::io::Result<()> {
    let text: String = favorites
        .iter()
        .map(|(p, stem)| line_of(*p, stem) + "\n")
        .collect();
    crate::card_writer::write(&favorites_path(root), text.into_bytes())
}

/// The starred carts among `carts`, in the order the shelf shows them: by name, as every shelf
/// is.
pub fn favorite_carts(carts: &[Cart], favorites: &[(Platform, String)]) -> Vec<Cart> {
    let mut found: Vec<Cart> = carts
        .iter()
        .filter(|c| {
            favorites
                .iter()
                .any(|(p, s)| *p == c.platform && *s == c.stem)
        })
        .cloned()
        .collect();
    found.sort_by_key(favorite_key);
    found
}

/// Where `cart` goes on a FAVORITES shelf of `carts`, kept by system, then by name.
fn place_in(carts: &[Cart], cart: &Cart) -> usize {
    let key = favorite_key(cart);
    carts
        .iter()
        .position(|c| favorite_key(c) > key)
        .unwrap_or(carts.len())
}

impl App {
    /// The FAVORITES shelf's place among the shelves.
    pub(crate) fn favorites_at(&self) -> Option<usize> {
        self.shelves
            .iter()
            .position(|(kind, _)| *kind == ShelfKind::Favorites)
    }

    /// The shelf in view is FAVORITES.
    pub fn on_favorites(&self) -> bool {
        self.shelves.get(self.shelf_at).map(|(k, _)| *k) == Some(ShelfKind::Favorites)
    }

    /// Whether `cart` is starred.
    pub fn is_favorite(&self, cart: &Cart) -> bool {
        self.favorites
            .iter()
            .any(|(p, s)| *p == cart.platform && *s == cart.stem)
    }

    /// Draws each cart on FAVORITES on its own system's face, once the faces are set.
    pub(crate) fn share_favorite_faces(&mut self) {
        for kind in [ShelfKind::Favorites, ShelfKind::Recents] {
            let Some(at) = self.shelves.iter().position(|(k, _)| *k == kind) else {
                continue;
            };
            let faces: Option<Vec<_>> = self.shelves[at]
                .1
                .carts
                .iter()
                .map(|c| self.face_of(c.platform, &c.stem))
                .collect();
            if let Some(faces) = faces {
                self.shelves[at].1.set_faces(faces);
            }
        }
        self.refresh_marks();
    }

    /// Puts the star on every starred cart, on every shelf.
    pub(crate) fn refresh_marks(&mut self) {
        let favorites = std::mem::take(&mut self.favorites);
        for (_, shelf) in &mut self.shelves {
            let marks = shelf
                .carts
                .iter()
                .map(|c| {
                    favorites
                        .iter()
                        .any(|(p, s)| *p == c.platform && *s == c.stem)
                })
                .collect();
            shelf.set_marks(marks);
        }
        self.favorites = favorites;
    }

    /// The star every favorite wears, uploaded.
    pub fn set_mark_face(&mut self, tex: omega_ui::TexId) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_mark(tex);
        }
    }

    /// The carts RECENTS shows now: the last games put in that are still on the card, or none
    /// while Settings > Shelf hides it.
    fn recent_carts(&self) -> Vec<(Cart, Option<omega_ui::TexId>)> {
        if !self.state.show_recents {
            return Vec::new();
        }
        self.recents
            .iter()
            .filter_map(|(p, stem)| {
                let (_, shelf) = self
                    .shelves
                    .iter()
                    .find(|(k, _)| *k == ShelfKind::System(*p))?;
                let (cart, face) = shelf.find(stem)?;
                Some((cart.clone(), face))
            })
            .collect()
    }

    /// Stands RECENTS again as the list and the setting have it, moving off it once it is
    /// empty.
    pub(crate) fn refresh_recents(&mut self) {
        let Some(at) = self
            .shelves
            .iter()
            .position(|(k, _)| *k == ShelfKind::Recents)
        else {
            return;
        };
        let carts = self.recent_carts();
        self.shelves[at].1.replace(carts);
        self.refresh_marks();
        if self.shelf_at == at && self.shelf().carts.is_empty() {
            self.leave_empty_shelf();
        }
    }

    /// `cart` goes in: it is the latest game on RECENTS.
    pub(crate) fn note_recent(&mut self, cart: &Cart) {
        self.recents = with_recent(&self.recents, cart);
        if let Some(root) = &self.root {
            if let Err(e) = write_recents(root, &self.recents) {
                eprintln!("omega-zero: {RECENTS_FILE}: {e}");
            }
        }
        // on RECENTS, the cart going in is its first now, and stays the one picked
        if self.state.show_recents {
            self.refresh_recents();
        }
    }

    /// Y on the shelf: stars the cart picked, or takes its star off, and says which.
    pub(crate) fn toggle_favorite(&mut self, now: Millis) {
        let Some(cart) = self.shelf().carts.get(self.shelf().index).cloned() else {
            return;
        };
        let Some(at) = self.favorites_at() else {
            return;
        };
        let starred = self.is_favorite(&cart);
        if starred {
            self.favorites
                .retain(|(p, s)| !(*p == cart.platform && *s == cart.stem));
            let fav = &mut self.shelves[at].1;
            if let Some(i) = fav
                .carts
                .iter()
                .position(|c| c.platform == cart.platform && c.stem == cart.stem)
            {
                fav.remove(i);
            }
        } else {
            self.favorites.push((cart.platform, cart.stem.clone()));
            let face = self.face_of(cart.platform, &cart.stem);
            let fav = &mut self.shelves[at].1;
            let i = place_in(&fav.carts, &cart);
            fav.insert(i, cart, face);
        }
        self.refresh_marks();
        if let Some(root) = &self.root {
            if let Err(e) = write_favorites(root, &self.favorites) {
                eprintln!("omega-zero: {FAVORITES_FILE}: {e}");
            }
        }
        self.hud.toast(
            match starred {
                true => Toast::Unfavorited,
                false => Toast::Favorited,
            },
            now,
        );
        // the last star taken off FAVORITES leaves nothing there: on to the next shelf
        if self.on_favorites() && self.shelf().carts.is_empty() {
            self.leave_empty_shelf();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_lines_slot_writes() {
        assert_eq!(
            parse_line("gba/Pokemon - Emerald Version (USA, Europe)"),
            Some((
                Platform::Gba,
                "Pokemon - Emerald Version (USA, Europe)".into()
            ))
        );
        assert_eq!(
            parse_line("gbc/Pokemon - Crystal Version (USA, Europe) (Rev 1)\r"),
            Some((
                Platform::Gbc,
                "Pokemon - Crystal Version (USA, Europe) (Rev 1)".into()
            ))
        );
        assert_eq!(
            parse_line("gb/Tetris (World) (Rev 1)").unwrap().0,
            Platform::Gb
        );
        assert_eq!(parse_line("snes/a/b").unwrap().1, "a/b");
        assert_eq!(parse_line("nes/Zelda"), None);
        assert_eq!(parse_line("gba/"), None);
        assert_eq!(parse_line(""), None);
    }

    #[test]
    fn writes_them_back_the_same_way() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(CONFIG_DIR)).unwrap();
        let favs = vec![
            (Platform::Gba, "Mega Man Zero (USA, Europe)".to_string()),
            (Platform::Gb, "Tetris (World) (Rev 1)".to_string()),
        ];
        write_favorites(dir.path(), &favs).unwrap();
        let text = std::fs::read_to_string(favorites_path(dir.path())).unwrap();
        assert_eq!(
            text,
            "gba/Mega Man Zero (USA, Europe)\ngb/Tetris (World) (Rev 1)\n"
        );
        assert_eq!(read_favorites(dir.path()), favs);
    }

    #[test]
    fn a_new_favorite_takes_its_place_by_name() {
        let cart = |stem: &str| Cart {
            platform: Platform::Gba,
            stem: stem.into(),
            rom: PathBuf::new(),
            label: None,
            title: String::new(),
            code: String::new(),
            shell: None,
        };
        let row = [cart("Advance Wars"), cart("Metroid Fusion"), cart("Zelda")];
        let gb = Cart {
            platform: Platform::Gb,
            ..cart("Alleyway")
        };
        assert_eq!(
            place_in(&row, &gb),
            3,
            "a Game Boy cart goes after the GBA's"
        );
        assert_eq!(place_in(&row, &cart("Golden Sun")), 1);
        assert_eq!(place_in(&row, &cart("4 Games")), 0);
        assert_eq!(place_in(&row, &cart("Zoo")), 3);
    }
}

#[cfg(test)]
mod recents_tests {
    use super::*;

    fn cart(platform: Platform, stem: &str) -> Cart {
        Cart {
            platform,
            stem: stem.into(),
            rom: PathBuf::new(),
            label: None,
            title: String::new(),
            code: String::new(),
            shell: None,
        }
    }

    #[test]
    fn the_latest_comes_first_once_and_ten_at_most() {
        let mut recents: Vec<(Platform, String)> = (0..RECENTS_MAX)
            .map(|i| (Platform::Gba, format!("Game {i}")))
            .collect();
        recents = with_recent(&recents, &cart(Platform::Gba, "Game 4"));
        assert_eq!(recents[0].1, "Game 4");
        assert_eq!(recents.len(), RECENTS_MAX);
        assert_eq!(recents.iter().filter(|(_, s)| s == "Game 4").count(), 1);
        recents = with_recent(&recents, &cart(Platform::Gb, "Tetris"));
        assert_eq!(recents[0], (Platform::Gb, "Tetris".into()));
        assert_eq!(recents.len(), RECENTS_MAX, "the oldest falls off");
    }

    #[test]
    fn favorites_stand_by_system_then_by_name() {
        let carts = [
            cart(Platform::Gbc, "Pokemon Crystal"),
            cart(Platform::Gb, "Tetris"),
            cart(Platform::Gba, "Zero"),
            cart(Platform::Gba, "Emerald"),
            cart(Platform::Gb, "Alleyway"),
        ];
        let all: Vec<_> = carts.iter().map(|c| (c.platform, c.stem.clone())).collect();
        let order: Vec<_> = favorite_carts(&carts, &all)
            .into_iter()
            .map(|c| c.stem)
            .collect();
        assert_eq!(
            order,
            ["Emerald", "Zero", "Alleyway", "Tetris", "Pokemon Crystal"]
        );
    }
}
