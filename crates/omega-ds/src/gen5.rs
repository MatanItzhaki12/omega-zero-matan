//! The fifth generation's Pokémon games on one screen: Black (USA/Europe, IRBO), White (IRAO),
//! Black 2 (USA, IREO) and White 2 (IRDO). Which of a game's screens is up, from the functions
//! that drew them, and the view laid out for each. After the 4:3 layouts of Pokémon Fulldex's
//! pickSceneBw2 (frontend/main.cpp), and its Black and White's own (frontend/bw1.h; GPL-3.0).
//!
//! The overworld, battles and most of the story are on the top screen, the C-Gear on the touch
//! screen beside it left out (L2 shows it). The touch screen's menus come with them where they
//! are needed: the X menu's buttons at the right edge over the world, a battle's commands, moves
//! or targets as a panel in the corner over the message box, a battle's question's two answers
//! beside it, the save's card and question in the corner and along the bottom. The games take
//! the D-pad and A on all of these and draw their own cursor on them, which the panels show.
//! The applications played on the touch screen (the party, the Bag, the town map, the trainer
//! card, the Pokédex) show it.

use crate::fallback::Fallback;
use crate::scene::{Back, Piece, Scene, LAYER_BACKDROP};
use crate::screen::{rgb, Screens, BOTTOM, H, TOP, W};
use crate::watch::Watch;

/// A game the layouts are for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Game {
    Black,
    White,
    Black2,
    White2,
}

impl Game {
    /// The game by the code in its header.
    pub fn of(code: &str) -> Option<Game> {
        match code {
            "IRBO" => Some(Game::Black),
            "IRAO" => Some(Game::White),
            "IREO" => Some(Game::Black2),
            "IRDO" => Some(Game::White2),
            _ => None,
        }
    }

    /// Its function table, cut to the functions watched here (data/gen5_tables.py).
    pub fn table(self) -> &'static str {
        match self {
            Game::Black => include_str!("../data/IRBO.txt"),
            Game::White => include_str!("../data/IRAO.txt"),
            Game::Black2 => include_str!("../data/IREO.txt"),
            Game::White2 => include_str!("../data/IRDO.txt"),
        }
    }

    /// Black or White, whose screens are drawn apart from their sequels' in places.
    fn first(self) -> bool {
        matches!(self, Game::Black | Game::White)
    }
}

/// The functions the layouts follow, by the names the tables have: Black 2's (the pokebw2
/// decompilation's), which Black's functions of the same roles carry too, and Black's own.
struct Funcs {
    field: Option<usize>,
    field_menu: Option<usize>,
    /// SAVE from the X menu: Black 2's, until it is done, and Black's.
    save: Option<usize>,
    report: Option<usize>,
    /// Waits for the save memory: while the save is written nothing else runs.
    save_write: Option<usize>,
    battle: Option<usize>,
    commands: Option<usize>,
    moves: Option<usize>,
    targets: Option<usize>,
    battle_party: Option<usize>,
    battle_bag: Option<usize>,
    /// A question in a battle, its two answers on the touch screen: Black 2's choice screen,
    /// its YES / NO, and the poll that lasts as long as either is up.
    battle_choice: Option<usize>,
    battle_yes_no: Option<usize>,
    battle_yes_no_poll: Option<usize>,
    evolution: Option<usize>,
    bag: Option<usize>,
    party: Option<usize>,
    trainer_card: Option<usize>,
    /// Black's trainer card's other side, the Badge Case, and its signature.
    badges: Option<usize>,
    card_edit: Option<usize>,
    town_map: Option<usize>,
    options: Option<usize>,
    summary: Option<usize>,
    dex_list: Option<usize>,
    dex_entry: Option<usize>,
    /// The Pokédex's opening: Black 2's, and Black's Pokédex with neither the list nor an entry.
    dex_open: Option<usize>,
    dex_app: Option<usize>,
    /// Black: a Pokémon registered in the Pokédex, and the battle's touch screen asking whether
    /// to nickname it.
    dex_register: Option<usize>,
    touch_input: Option<usize>,
    pc_box: Option<usize>,
    /// Black's Musical's dressing up, played on the touch screen.
    musical: Option<usize>,
    start_menu: Option<usize>,
    /// CONTINUE chosen, and the C-Gear's YES / NO question it asks.
    start_continue: Option<usize>,
    confirm: Option<usize>,
    intro: Option<usize>,
    naming: Option<usize>,
}

impl Funcs {
    fn of(w: &Watch) -> Funcs {
        Funcs {
            field: w.find("FieldmapProc_Update"),
            field_menu: w.find("EventFieldMenu_Callback"),
            save: w.find("EventSave_Callback"),
            report: w.find("REPORT_EVENT_Main"),
            save_write: w.find("CARD_WaitBackupAsync"),
            battle: w.find("BtlMain_ProcMain"),
            commands: w.find("BtlvCore_WaitCommandSelect"),
            moves: w.find("BtlvCore_WaitMoveSelect"),
            targets: w.find("BtlvCore_WaitTargetSelect"),
            battle_party: w.find("BPlistMain_Task"),
            battle_bag: w.find("BBagMain_Task"),
            battle_choice: w.find("BChoiceMain_Task"),
            battle_yes_no: w.find("BtlvInput_YesNoTask"),
            battle_yes_no_poll: w.find("BtlvCore_PollYesNo"),
            evolution: w.find("ShinkaDemo_Main"),
            bag: w.find("ItemMenuProc_Main"),
            party: w.find("PokeListProc_Main"),
            trainer_card: w.find("TrainerCard_Main"),
            badges: w.find("TRCARD_SYS_StateWaitBadgeProc"),
            card_edit: w.find("TRCARD_SYS_StateWaitEditProc"),
            town_map: w.find("TownMap_ProcMain"),
            options: w.find("ConfigPanel_Main"),
            summary: w.find("PStatus_ProcMain"),
            dex_list: w.find("ZknListSeq_Main"),
            dex_entry: w.find("ZukanDetail_Main"),
            dex_open: w.find("ZukanOpen_Main"),
            dex_app: w.find("ZUKAN_Update"),
            dex_register: w.find("ZUKAN_TOROKU_GRAPHIC_Main"),
            touch_input: w.find("BTLV_INPUT_ProcessTouchInput"),
            pc_box: w.find("Box2Proc_Main"),
            musical: w.find("MUSICAL_DRESSUP_SYS_Main"),
            start_menu: w.find("StartMenu_Main"),
            start_continue: w.find("StartMenu_Continue"),
            confirm: w.find("ConfirmDialog_Update"),
            intro: w.find("IntroProc_Main"),
            naming: w.find("NameEntry_ProcMain"),
        }
    }
}

/// The dark backing of a panel standing over the picture, and the room round it.
const PANEL: u32 = 0x14141C;
const PANEL_PAD: f32 = 2.0;

/// The X menu's grid on the touch screen, two buttons to a row, rows 48 apart: where its first
/// button is, each one's size, and how large they are drawn at the view's right edge.
struct MenuGrid {
    x: [i32; 2],
    y: i32,
    w: i32,
    h: i32,
    scale: f32,
}

const SEQUEL_MENU: MenuGrid = MenuGrid {
    x: [2, 130],
    y: 22,
    w: 126,
    h: 44,
    scale: 0.4,
};
const FIRST_MENU: MenuGrid = MenuGrid {
    x: [3, 131],
    y: 24,
    w: 123,
    h: 41,
    scale: 0.41,
};

/// A game's one-screen view.
pub struct Gen5 {
    game: Game,
    f: Funcs,
    /// Black's save screen was shown last: it stays until the overworld runs again, as nothing
    /// else runs while the save is written.
    saving: bool,
    fallback: Fallback,
}

impl Gen5 {
    pub fn new(game: Game, watch: &Watch) -> Gen5 {
        Gen5 {
            game,
            f: Funcs::of(watch),
            saving: false,
            fallback: Fallback::default(),
        }
    }

    /// The keys the game is handed for the next frame, and where the frontend touches the touch
    /// screen: the keys as they are, the games taking the D-pad and A on every menu laid out.
    pub fn input(&mut self, held: u16) -> (u16, Option<(i32, i32)>) {
        (held, None)
    }

    /// The view for the frame just drawn.
    pub fn scene(&mut self, frame: u32, w: &Watch, s: &Screens) -> Scene {
        if let Some(sc) = self.pick(w, s) {
            self.fallback.known(&sc, frame);
            return sc;
        }
        self.fallback.unknown(frame, s, w.ran(self.f.field, 120))
    }

    fn pick(&mut self, w: &Watch, s: &Screens) -> Option<Scene> {
        let f = &self.f;
        let ran = |f: Option<usize>, n: u32| w.ran(f, n);
        let first = self.game.first();
        // Black's own screens before the ones the four games share
        if first {
            // (Black's save skips a few frames as it opens, and runs nothing for a second or
            // two while it writes)
            self.saving = ran(f.report, 10) || (self.saving && !ran(f.field, 1));
            if self.saving {
                return Some(save(s));
            }
            if ran(f.dex_register, 2) {
                // the Pokémon's page on the top screen; the nickname's YES / NO with it
                return Some(if ran(f.touch_input, 2) {
                    both("dex-register-ask")
                } else {
                    Scene::only("dex-register", TOP)
                });
            }
            if ran(f.badges, 2) || ran(f.card_edit, 2) {
                return Some(Scene::only("trainer-card", BOTTOM));
            }
        }
        Some(if ran(f.start_continue, 2) && ran(f.confirm, 2) {
            // after CONTINUE, the C-Gear's question and its YES / NO, which A and B answer
            Scene::only("continue", BOTTOM)
        } else if ran(f.naming, 2) {
            // the question above the keyboard that answers it; ahead of the battle, which still
            // runs as a Pokémon just caught is named
            Scene::stack(
                "naming",
                vec![Piece::crop(TOP, 0, 144, 256, 48), Piece::full(BOTTOM)],
                false,
            )
        } else if ran(f.battle, 2) {
            self.battle(w, s)
        } else if !first
            && (ran(f.save, 8)
                || (ran(f.save_write, 8)
                    && self.fallback.last().is_some_and(|sc| sc.name == "save")))
        {
            save(s)
        } else if ran(f.field_menu, 8) {
            // (it skips a few frames as the menu opens)
            x_menu(s, if first { &FIRST_MENU } else { &SEQUEL_MENU })
        } else if ran(f.field, 16) {
            Scene::only("top", TOP)
        } else {
            self.app(w, s)?
        })
    }

    /// The games' screens other than battles, the overworld and its menus; none for a screen
    /// they have no layout for.
    fn app(&self, w: &Watch, s: &Screens) -> Option<Scene> {
        let f = &self.f;
        let ran = |f: Option<usize>| w.ran(f, 2);
        let first = self.game.first();
        Some(if ran(f.bag) {
            bag(s)
        } else if ran(f.options) {
            // the chosen option's explanation, near the bottom of the top screen, above the list
            Scene::stack(
                "options",
                vec![Piece::crop(TOP, 0, 146, 256, 44), Piece::full(BOTTOM)],
                false,
            )
        } else if ran(f.summary) {
            // both pages use both screens
            both("summary")
        } else if ran(f.pc_box) {
            // the chosen Pokémon's card beside the box
            both("pc-box")
        } else if ran(f.dex_open) || (ran(f.dex_app) && !ran(f.dex_list) && !ran(f.dex_entry)) {
            // the opening: the Poké Ball alone, until the list takes over
            Scene::only("pokedex-open", BOTTOM)
        } else if ran(f.dex_entry) {
            // the other screen is an empty grid
            Scene::only("pokedex-entry", BOTTOM)
        } else if ran(f.dex_list) {
            // Black 2's list runs on from one screen into the other; Black's is on the touch
            // screen, below the title and the counts of those seen and obtained
            let top = if first {
                Piece::crop(TOP, 0, 0, 256, 51)
            } else {
                Piece::full(TOP)
            };
            Scene::stack("pokedex-list", vec![top, Piece::full(BOTTOM)], false)
        } else if ran(f.town_map) {
            Scene::only("town-map", BOTTOM)
        } else if ran(f.party) {
            // the top screen only has a backdrop
            Scene::only("party", BOTTOM)
        } else if ran(f.trainer_card) {
            Scene::only("trainer-card", BOTTOM)
        } else if ran(f.musical) {
            // the top screen has only the curtain and a line of text
            Scene::only("musical-dressing", BOTTOM)
        } else if ran(f.start_menu) && !ran(f.start_continue) {
            launch_menu(first)
        } else if ran(f.intro) {
            // the professor, the questions and the choice of character are all on the top screen
            Scene::only("intro", TOP)
        } else if ran(f.evolution) {
            Scene::only("evolution", TOP)
        } else {
            return None;
        })
    }

    fn battle(&self, w: &Watch, s: &Screens) -> Scene {
        let f = &self.f;
        let ran = |f: Option<usize>, n: u32| w.ran(f, n);
        if ran(f.battle_party, 2) || ran(f.battle_bag, 2) {
            // the party or the Bag, a whole page of the touch screen; one colour for a few frames
            // as it fades in and out, while the battle stays
            return if one_colour(s, BOTTOM) {
                Scene::only("battle", TOP)
            } else {
                Scene::only("battle-list", BOTTOM)
            };
        }
        if ran(f.battle_choice, 4) || ran(f.battle_yes_no, 4) || ran(f.battle_yes_no_poll, 1) {
            return battle_choice();
        }
        let mut sc = Scene::only("battle", TOP);
        let moves = ran(f.moves, 1);
        let commands = !moves && ran(f.commands, 1);
        let targets = !moves && !commands && ran(f.targets, 1);
        // the touch screen's panel in the lower right corner, over the message box's empty
        // right end: the four moves two by two, FIGHT above BAG, RUN and POKéMON, or the targets
        let panel = if moves {
            Some((Piece::crop(BOTTOM, 0, 30, 256, 98), 0.5, "battle-moves"))
        } else if commands {
            Some((Piece::crop(BOTTOM, 0, 78, 256, 114), 0.44, "battle-cmd"))
        } else if targets {
            Some((Piece::crop(BOTTOM, 0, 22, 256, 110), 0.45, "battle-targets"))
        } else {
            None
        };
        if let Some((crop, scale, name)) = panel {
            corner_panel(&mut sc, crop, scale);
            sc.name = name;
        }
        sc
    }
}

/// Both screens side by side.
fn both(name: &'static str) -> Scene {
    Scene::stack(name, vec![Piece::full(TOP), Piece::full(BOTTOM)], true)
}

/// A pixel's brightness, 0 to 255.
fn lum(s: &Screens, screen: usize, x: usize, y: usize) -> i32 {
    let (r, g, b) = rgb(s.pixel(screen, x, y));
    (r + g + b) / 3
}

/// Whether a screen is all one colour, as it is between two pages.
fn one_colour(s: &Screens, screen: usize) -> bool {
    let c = s.pixel(screen, 0, 0) & 0xFF_FFFF;
    (0..H).all(|y| (0..W).all(|x| s.pixel(screen, x, y) & 0xFF_FFFF == c))
}

/// `crop` at `scale` in the lower right corner on a dark bordered backing, above the view's
/// bottom edge.
fn corner_panel(sc: &mut Scene, crop: Piece, scale: f32) {
    let (w, h) = (crop.sw as f32 * scale, crop.sh as f32 * scale);
    let x = W as f32 - w - PANEL_PAD - 2.0;
    let y = H as f32 - h - PANEL_PAD - 1.0;
    sc.pieces.push(
        Piece::fill(
            x - PANEL_PAD,
            y - PANEL_PAD,
            w + 2.0 * PANEL_PAD,
            h + 2.0 * PANEL_PAD,
            PANEL,
        )
        .bordered(),
    );
    sc.pieces.push(crop.scaled(scale).at(x, y));
}

/// A question in a battle ("Will you switch your Pokémon?"), its two answers on the touch screen
/// (bars at rows 33 and 80, the cursor's brackets with them): the battle, the bars beside the
/// question's end on the message box's band (rows 147 to 189).
fn battle_choice() -> Scene {
    let mut sc = Scene::only("battle-choice", TOP);
    let right = W as f32 - 4.0;
    // (the question's text ends near x 166)
    let k = ((right - 170.0) / 252.0).clamp(0.3, 0.42);
    let (h, gap) = (47.0 * k, 2.0);
    let mut y = 148.0 + (41.0 - (2.0 * h + gap)) / 2.0;
    for top in [33, 80] {
        sc.pieces.push(
            Piece::crop(BOTTOM, 2, top, 252, 47)
                .scaled(k)
                .at(right - 252.0 * k, y),
        );
        y += h + gap;
    }
    sc
}

/// Saving from the X menu: the question, YES / NO, the saved game's card and the save's progress
/// are all on the touch screen, laid out alike in the four games. The overworld, with the touch
/// screen's message box (rows 2 to 45) along its bottom, where the games' other messages are,
/// and the rest (the card, YES / NO over its corner, the time saved or the progress) as a panel
/// in the upper right corner: each while it is there, the card once its white row of Pokémon is
/// drawn, the box while it is white at both ends.
fn save(s: &Screens) -> Scene {
    let mut sc = Scene::only("save", TOP);
    if lum(s, BOTTOM, 20, 108) > 200 {
        let scale = 0.6;
        sc.pieces.push(
            Piece::crop(BOTTOM, 0, 46, 256, 146)
                .scaled(scale)
                .at(W as f32 - 256.0 * scale - PANEL_PAD - 2.0, PANEL_PAD + 2.0)
                .bordered(),
        );
    }
    if lum(s, BOTTOM, 5, 24) > 230 && lum(s, BOTTOM, 250, 24) > 230 {
        sc.pieces
            .push(Piece::crop(BOTTOM, 0, 2, 256, 44).at(0.0, H as f32 - 44.0));
    }
    sc
}

/// The X menu, on the touch screen: its buttons at the view's right edge, beside the world, in
/// the game's own grid of two columns, so that the D-pad moves the highlight where it looks to.
/// None while it opens or closes.
fn x_menu(s: &Screens, grid: &MenuGrid) -> Scene {
    let mut sc = Scene::only("start-menu", TOP);
    let mut buttons = Vec::new();
    let mut rows = 0;
    for row in 0..3 {
        for (col, &bx) in grid.x.iter().enumerate() {
            let by = grid.y + row * 48;
            if menu_slot_used(s, grid, bx, by) {
                buttons.push((bx, by, row, col));
                rows = row + 1;
            }
        }
    }
    if buttons.is_empty() {
        return sc;
    }
    let (gap, pad) = (2.0, 3.0);
    let (bw, bh) = (grid.w as f32 * grid.scale, grid.h as f32 * grid.scale);
    let (w, h) = (2.0 * bw + gap, rows as f32 * bh + (rows - 1) as f32 * gap);
    let x = W as f32 - w - pad - 2.0;
    let y = (H as f32 - h) / 2.0;
    sc.pieces
        .push(Piece::fill(x - pad, y - pad, w + 2.0 * pad, h + 2.0 * pad, PANEL).bordered());
    for (bx, by, row, col) in buttons {
        sc.pieces.push(
            Piece::crop(BOTTOM, bx, by, grid.w, grid.h)
                .scaled(grid.scale)
                .at(x + col as f32 * (bw + gap), y + row as f32 * (bh + gap)),
        );
    }
    sc
}

/// Whether the X menu's slot at `bx, by` holds a button: most of its middle row dark, where an
/// empty slot is the light grid, and its icon or name light on it (as the menu opens its buttons
/// are empty dark boxes for a few frames).
fn menu_slot_used(s: &Screens, grid: &MenuGrid, bx: i32, by: i32) -> bool {
    let at = |x: i32, y: i32| lum(s, BOTTOM, x as usize, y as usize);
    let dark = (bx..bx + grid.w)
        .filter(|&x| at(x, by + grid.h / 2) <= 150)
        .count() as i32;
    let light = (by + 8..by + grid.h - 6)
        .step_by(2)
        .flat_map(|y| (bx + 8..bx + grid.w - 8).step_by(2).map(move |x| (x, y)))
        .filter(|&(x, y)| at(x, y) > 200)
        .count();
    dark * 10 >= grid.w * 3 && light >= 8
}

/// Black 2's and Black's Bag: the touch screen alone while the pockets are browsed; once an item
/// is chosen (its card on the top screen, words in white), both side by side on the card's blue.
fn bag(s: &Screens) -> Scene {
    let text = (28..152)
        .step_by(2)
        .flat_map(|y| (8..248).step_by(2).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let (r, g, b) = rgb(s.pixel(TOP, x, y));
            r > 200 && g > 200 && b > 200
        })
        .count();
    if text < 12 {
        let mut sc = Scene::only("bag", BOTTOM);
        sc.back = Back::Sample(BOTTOM, 2, 2);
        return sc;
    }
    let blue = s.pixel(TOP, 4, 100) & 0xFF_FFFF;
    let mut sc = both("bag");
    sc.pieces.insert(
        0,
        Piece::fill(-1.0, -1.0, W as f32 + 3.0, H as f32 + 2.0, blue),
    );
    sc
}

/// The start menu (CONTINUE, NEW GAME...). Black's is all on the top screen. Black 2's list is
/// on the top screen and the saved game's cards on the touch screen: both side by side on the
/// menu's own dark grey, without the backgrounds behind them.
fn launch_menu(first: bool) -> Scene {
    if first {
        let mut sc = Scene::only("launch-menu", TOP);
        sc.back = Back::Sample(TOP, 0, 0);
        return sc;
    }
    let list = Piece::crop(TOP, 32, 15, 194, 177).dropping(LAYER_BACKDROP);
    let cards = Piece::crop(BOTTOM, 22, 14, 214, 166).dropping(LAYER_BACKDROP);
    let mut sc = Scene::stack("launch-menu", vec![list, cards], true);
    sc.back = Back::Colour(0x1C1C1C);
    sc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watch_of(game: Game) -> Watch {
        Watch::new(crate::watch::load(game.table(), std::path::Path::new("")))
    }

    #[test]
    fn each_game_is_known_by_its_code_and_has_its_table() {
        for (code, game) in [
            ("IRBO", Game::Black),
            ("IRAO", Game::White),
            ("IREO", Game::Black2),
            ("IRDO", Game::White2),
        ] {
            assert_eq!(Game::of(code), Some(game));
            assert!(game
                .table()
                .lines()
                .any(|l| l.ends_with(" FieldmapProc_Update")));
        }
        assert_eq!(
            Game::of("CPUE"),
            None,
            "Platinum is the fourth generation's"
        );
    }

    #[test]
    fn every_game_has_the_functions_its_layouts_follow() {
        for game in [Game::Black, Game::White, Game::Black2, Game::White2] {
            let f = Funcs::of(&watch_of(game));
            let shared = [
                f.field,
                f.field_menu,
                f.save_write,
                f.battle,
                f.commands,
                f.moves,
                f.targets,
                f.battle_party,
                f.battle_bag,
                f.battle_yes_no,
                f.battle_yes_no_poll,
                f.evolution,
                f.bag,
                f.party,
                f.trainer_card,
                f.town_map,
                f.options,
                f.summary,
                f.dex_list,
                f.dex_entry,
                f.pc_box,
                f.start_menu,
                f.start_continue,
                f.confirm,
                f.intro,
                f.naming,
            ];
            assert!(shared.iter().all(Option::is_some), "{game:?}");
            let own = if game.first() {
                vec![
                    f.report,
                    f.badges,
                    f.card_edit,
                    f.dex_app,
                    f.dex_register,
                    f.touch_input,
                    f.musical,
                ]
            } else {
                vec![f.save, f.battle_choice, f.dex_open]
            };
            assert!(own.iter().all(Option::is_some), "{game:?}");
        }
    }

    #[test]
    fn the_x_menu_grids_fit_the_touch_screen() {
        for grid in [&SEQUEL_MENU, &FIRST_MENU] {
            assert!(grid.x[1] + grid.w <= W as i32);
            assert!(grid.y + 2 * 48 + grid.h <= H as i32);
        }
    }
}
