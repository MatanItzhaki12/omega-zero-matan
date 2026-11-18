//! The fourth generation's Pokémon games on one screen: Platinum (USA, CPUE), HeartGold (IPKE)
//! and SoulSilver (IPGE). Which of a game's screens is up, from the functions that drew them, and
//! the view laid out for each. After Pokémon Fulldex's frontend/platinum.h, its AutoFocus for
//! HeartGold and SoulSilver, and the 4:3 layouts of its pickSceneBw2 and gen4BattleInput
//! (frontend/main.cpp, GPL-3.0).
//!
//! The overworld and its menus are on the top screen; Platinum's Pokétch, on the touch screen
//! beside it, is left out (L2 shows it). A battle's commands and moves, on the touch screen, drawn
//! alike in the three games, are cut out and laid in a band under the battle in place of the
//! message box; the D-pad moves an outline over them and A touches the one chosen. Platinum's
//! introduction's two questions that take only a touch stand at the right of the view, A and B
//! answering them. HeartGold's and SoulSilver's applications played on the touch screen (the
//! Bag, the party, the Pokégear, the PC's boxes and more) show the touch screen.

use crate::fallback::Fallback;
use crate::keys;
use crate::scene::{
    Back, Piece, Scene, LAYER_BACKDROP, LAYER_BG0, LAYER_BG1, LAYER_BG2, LAYER_BG3,
};
use crate::screen::{rgb, Screens, BOTTOM, H, TOP, W};
use crate::watch::Watch;

/// A game the layouts are for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Game {
    Platinum,
    HeartGold,
    SoulSilver,
}

impl Game {
    /// The game by the code in its header.
    pub fn of(code: &str) -> Option<Game> {
        match code {
            "CPUE" => Some(Game::Platinum),
            "IPKE" => Some(Game::HeartGold),
            "IPGE" => Some(Game::SoulSilver),
            _ => None,
        }
    }

    /// Its function table, cut to the functions watched here.
    pub fn table(self) -> &'static str {
        match self {
            Game::Platinum => include_str!("../data/CPUE.txt"),
            Game::HeartGold => include_str!("../data/IPKE.txt"),
            Game::SoulSilver => include_str!("../data/IPGE.txt"),
        }
    }

    fn hgss(self) -> bool {
        matches!(self, Game::HeartGold | Game::SoulSilver)
    }
}

/// HeartGold's and SoulSilver's applications played on the touch screen, which show it alone:
/// Fulldex's AutoFocus::bottomApps (the keyboard, NamingScreen, has a layout of its own).
const HGSS_TOUCH_APPS: [&str; 25] = [
    "Bag",
    "PartyMenu",
    "Pokegear",
    "GearConfigure",
    "GearMap",
    "GearPhone",
    "GearRadio",
    "TownMap",
    "PCBox",
    "EasyChat",
    "VoltorbFlip",
    "Pokedex",
    "ScratchOffCards",
    "AlphPuzzle",
    "TrainerCard",
    "TrainerCardSignature",
    "BugContestSwapMon",
    "WirelessTradeSelectMon",
    "OptionsMenu",
    "MainMenu",
    "CheckSave",
    "DeleteSave",
    "ApricornBox",
    "PalPad",
    "BerryPots",
];

/// How recently an application's functions must have run for it to count as up: Fulldex's six
/// frames, which cover the frames it fades in and out across.
const APP_FRAMES: u32 = 6;

/// The functions the layouts follow, by their places in the table.
struct Funcs {
    /// HeartGold's and SoulSilver's applications played on the touch screen: each one's
    /// per-frame function and the functions it starts and ends with.
    touch_apps: Vec<Option<usize>>,
    field: Option<usize>,
    start_menu: Option<usize>,
    dex: Option<usize>,
    party: Option<usize>,
    bag: Option<usize>,
    trainer_card: Option<usize>,
    options: Option<usize>,
    summary: Option<usize>,
    town_map: Option<usize>,
    pc_box: Option<usize>,
    naming: Option<usize>,
    intro: Option<usize>,
    starter: Option<usize>,
    journal: Option<usize>,
    move_reminder: Option<usize>,
    evolution: Option<usize>,
    egg_hatch: Option<usize>,
    battle: Option<usize>,
    battle_input: Option<usize>,
    battle_bag: Option<usize>,
    battle_party: Option<usize>,
    action_menu: Option<usize>,
    first_menu: Option<usize>,
    move_menu: Option<usize>,
    other_menus: Vec<Option<usize>>,
    saving: Option<usize>,
}

impl Funcs {
    fn of(game: Game, w: &Watch) -> Funcs {
        if game.hgss() {
            return Funcs::hgss(w);
        }
        Funcs {
            touch_apps: Vec::new(),
            field: w.find("FieldMap_Main"),
            start_menu: w.find("StartMenu_Main"),
            dex: w.find("PokedexMain_Main"),
            party: w.find("PartyMenu_Main"),
            bag: w.find("BagApplication_Main"),
            trainer_card: w.find("TrainerCaseApp_Main"),
            options: w.find("OptionsMenu_Main"),
            summary: w.find("PokemonSummaryScreen_Main"),
            town_map: w.find("TownMap_Main"),
            pc_box: w.find("BoxAppMan_Main"),
            naming: w.find("NamingScreen_Main"),
            intro: w.find("RowanIntro_Main"),
            starter: w.find("ChooseStarter_Main"),
            journal: w.find("JournalController_Main"),
            move_reminder: w.find("MoveReminder_Main"),
            evolution: w.find("Evolution_Main"),
            egg_hatch: w.find("EggHatch_Main"),
            battle: w.find("Battle_Main"),
            battle_input: w.find("BattleSystem_MenuInput"),
            battle_bag: w.find("BattleBagTask_Tick"),
            battle_party: w.find("BattlePartyTask_Tick"),
            action_menu: w.find("BattleSubscreen_DrawActionMenu"),
            first_menu: w.find("BattleSubscreen_DrawFirstBattleMenu"),
            move_menu: w.find("BattleSubscreen_DrawMoveSelectMenu"),
            other_menus: [
                "BattleSubscreen_DrawPalParkMenu",
                "BattleSubscreen_DrawYesNoMenu",
                "BattleSubscreen_DrawForgetMoveMenu",
                "BattleSubscreen_DrawGiveUpMoveMenu",
                "BattleSubscreen_DrawStopMenu",
                "BattleSubscreen_DrawUseNextMonMenu",
                "BattleSubscreen_DrawSwitchMonMenu",
                "BattleSubscreen_DrawTargetSelectMenu",
            ]
            .iter()
            .map(|n| w.find(n))
            .collect(),
            saving: w.find("SaveData_CardSave_Main"),
        }
    }

    /// HeartGold's and SoulSilver's, by the names Fulldex's table has (data/hgss_tables.py): the
    /// battle's touch-screen panels are told by their colours alone (`read_panel`), its own Bag
    /// and party by the touch screen they show.
    fn hgss(w: &Watch) -> Funcs {
        let app = |name: &str| {
            [
                w.find(name),
                w.find(&format!("{name}:start")),
                w.find(&format!("{name}:end")),
            ]
        };
        Funcs {
            touch_apps: HGSS_TOUCH_APPS.iter().flat_map(|n| app(n)).collect(),
            field: w.find("FieldMap_Main"),
            start_menu: w.find("Task_StartMenu"),
            dex: None,
            party: None,
            bag: None,
            trainer_card: None,
            options: None,
            summary: None,
            town_map: None,
            pc_box: None,
            naming: w.find("NamingScreen"),
            intro: w.find("OakSpeech"),
            starter: None,
            journal: None,
            move_reminder: None,
            evolution: w.find("EvolutionScene_Task"),
            egg_hatch: w.find("HatchEgg"),
            battle: w.find("Battle"),
            battle_input: w.find("BattleInput_CheckTouch"),
            battle_bag: None,
            battle_party: None,
            action_menu: None,
            first_menu: None,
            move_menu: None,
            other_menus: Vec::new(),
            saving: w.find("HandleWriteSaveAsync_NormalData"),
        }
    }
}

/// The introduction's questions answered only by a touch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntroTouch {
    /// Professor Rowan's YES (red, x 100-140, y 70-92) and NO (blue under it): NO at `y`.
    YesNo { no_y: i32 },
    /// "Touch the button on the middle of the Poké Ball" (its pale blue button at 128, 96).
    Ball,
}

/// Which of the introduction's touch questions the touch screen shows, if one.
pub fn intro_touch(s: &Screens) -> Option<IntroTouch> {
    let is = |x: usize, y: usize, r: i32, g: i32, b: i32| {
        let (pr, pg, pb) = rgb(s.pixel(BOTTOM, x, y));
        (pr - r).abs() < 40 && (pg - g).abs() < 40 && (pb - b).abs() < 40
    };
    let mut red = 0;
    for y in 70..92 {
        for x in 100..140 {
            red += is(x, y, 251, 89, 89) as u32;
        }
    }
    let (mut blue, mut blue_y) = (0u32, 0u32);
    for y in 96..140 {
        for x in 100..140 {
            if is(x, y, 73, 154, 219) {
                blue += 1;
                blue_y += y as u32;
            }
        }
    }
    if red >= 250 && blue >= 250 {
        return Some(IntroTouch::YesNo {
            no_y: (blue_y / blue) as i32,
        });
    }
    if is(108, 76, 251, 89, 89)
        && is(132, 84, 251, 89, 89)
        && is(104, 108, 73, 154, 219)
        && is(136, 116, 73, 154, 219)
    {
        return Some(IntroTouch::YesNo { no_y: 113 });
    }
    let ball = is(100, 70, 219, 97, 97)
        && is(156, 70, 219, 97, 97)
        && is(128, 60, 219, 97, 97)
        && is(100, 120, 251, 251, 251)
        && is(156, 120, 251, 251, 251)
        && is(128, 130, 251, 251, 251)
        && is(128, 96, 211, 227, 251);
    ball.then_some(IntroTouch::Ball)
}

/// A battle button cut from the touch screen: its rectangle, the label's columns, and the plain
/// columns the button is one colour across.
#[derive(Clone, Copy)]
struct Cut {
    sx: i32,
    sy: i32,
    sw: i32,
    sh: i32,
    lx0: i32,
    lx1: i32,
    tile_x: i32,
    tile_w: i32,
}

const fn cut(c: [i32; 8]) -> Cut {
    Cut {
        sx: c[0],
        sy: c[1],
        sw: c[2],
        sh: c[3],
        lx0: c[4],
        lx1: c[5],
        tile_x: c[6],
        tile_w: c[7],
    }
}

/// The command buttons, FIGHT, BAG, POKéMON and RUN: their flat insides, the label all of each
/// but the plain columns at its ends.
const COMMANDS: [Cut; 4] = [
    cut([39, 64, 178, 34, 54, 202, 50, 4]),
    cut([12, 154, 56, 33, 18, 62, 14, 4]),
    cut([188, 154, 56, 33, 192, 240, 188, 4]),
    cut([100, 162, 56, 30, 106, 150, 102, 4]),
];
/// Where each command and each move is touched.
const COMMAND_AT: [(i32, i32); 4] = [(128, 84), (39, 168), (216, 168), (128, 172)];
const MOVE_AT: [(i32, i32); 4] = [(64, 52), (192, 52), (64, 116), (192, 116)];
/// Where the move list's CANCEL is touched.
const CANCEL_AT: (i32, i32) = (128, 172);
/// The commands' arrangement, FIGHT above BAG, RUN and POKéMON: where each arrow goes from each
/// (left, right, up, down).
const COMMAND_TO: [[usize; 4]; 4] = [[1, 2, 0, 3], [1, 3, 0, 1], [3, 2, 0, 2], [1, 2, 0, 3]];
const MOVE_TO: [[usize; 4]; 4] = [[0, 1, 0, 2], [0, 1, 1, 3], [2, 3, 0, 2], [2, 3, 1, 3]];
/// How many frames a touch the frontend makes is held.
const TOUCH_FRAMES: u32 = 4;

/// The band under the battle the buttons are laid in, and its colour.
const BAND_Y: f32 = 144.0;
const BAND_H: f32 = 48.0;
const BAND: u32 = 0x202020;
const BACKING: u32 = 0x101218;
const CHOSEN: u32 = 0xFFFFFF;

/// What the battle's touch screen shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Panel {
    Commands { fight_only: bool },
    Moves,
    YesNo,
    Idle,
    Unknown,
}

/// A game's one-screen view, and the buttons it answers for the touch screen.
pub struct Gen4 {
    game: Game,
    f: Funcs,
    /// The battle buttons' outline: 0 to 3, as COMMAND_AT or MOVE_AT number them.
    sel: usize,
    last_move: usize,
    last_strip: u32,
    strip_shown: &'static str,
    /// The commands stay up this long after they were last seen, while the next panel slides in.
    strip_until: u32,
    unknown_for: u32,
    fight_only: bool,
    /// A touch being held: frames left, and where.
    touch: Option<(u32, (i32, i32))>,
    prev_keys: u16,
    fallback: Fallback,
}

impl Gen4 {
    pub fn new(game: Game, watch: &Watch) -> Gen4 {
        Gen4 {
            game,
            f: Funcs::of(game, watch),
            sel: 0,
            last_move: 0,
            last_strip: 0,
            strip_shown: "",
            strip_until: 0,
            unknown_for: 0,
            fight_only: false,
            touch: None,
            prev_keys: 0,
            fallback: Fallback::default(),
        }
    }

    /// The keys the game is handed for the next frame, and where the frontend touches the touch
    /// screen, from the keys held and the frame shown last: in a battle's command or move band
    /// the arrows move the outline and A touches the button chosen, B in the moves touches
    /// CANCEL; in the introduction's touch questions A and B touch the answers.
    pub fn input(
        &mut self,
        frame: u32,
        held: u16,
        watch: &Watch,
        shown: Option<&Screens>,
    ) -> (u16, Option<(i32, i32)>) {
        let edge = held & !self.prev_keys;
        self.prev_keys = held;
        let mut keys = held;
        let strip = self.strip_shown;
        let (cmd, moves) = (strip == "battle-cmd", strip == "battle-moves");
        if cmd || moves {
            if frame.saturating_sub(self.last_strip) > 1800 {
                self.last_move = 0;
            }
            self.last_strip = frame;
            let usable = |i: usize| -> bool {
                if cmd {
                    return !self.fight_only || i == 0;
                }
                let Some(s) = shown else {
                    return true;
                };
                let (mx, my) = MOVE_AT[i];
                // a move's name is white on the pale panel; an empty slot's is a plain grey
                (my - 16..=my - 12).any(|y| {
                    (mx - 28..=mx + 28).any(|x| {
                        let (r, g, b) = rgb(s.pixel(BOTTOM, x as usize, y as usize));
                        r > 200 && g > 200 && b > 140
                    })
                })
            };
            let arrows = [keys::LEFT, keys::RIGHT, keys::UP, keys::DOWN];
            for (k, key) in arrows.iter().enumerate() {
                if edge & key != 0 {
                    let to = if cmd { COMMAND_TO } else { MOVE_TO }[self.sel][k];
                    if usable(to) {
                        self.sel = to;
                    }
                }
            }
            if edge & keys::A != 0 && self.touch.is_none() {
                if moves {
                    self.last_move = self.sel;
                }
                let at = if cmd { COMMAND_AT } else { MOVE_AT }[self.sel];
                self.touch = Some((TOUCH_FRAMES, at));
            }
            if moves && edge & keys::B != 0 && self.touch.is_none() {
                self.touch = Some((TOUCH_FRAMES, CANCEL_AT));
            }
            keys &= !(keys::LEFT | keys::RIGHT | keys::UP | keys::DOWN | keys::A | keys::B);
        } else if self.game == Game::Platinum && watch.ran(self.f.intro, 2) {
            if let Some(kind) = shown.and_then(intro_touch) {
                let answer = match kind {
                    IntroTouch::YesNo { .. } => keys::A | keys::B,
                    IntroTouch::Ball => keys::A,
                };
                if edge & answer != 0 && self.touch.is_none() {
                    let at = match kind {
                        IntroTouch::Ball => (128, 96),
                        IntroTouch::YesNo { .. } if edge & keys::A != 0 => (120, 81),
                        IntroTouch::YesNo { no_y } => (120, no_y),
                    };
                    self.touch = Some((TOUCH_FRAMES, at));
                }
                // given to the game, A would only bring up "Please touch a button"; the ball
                // shows while Rowan's words before it still want A
                if matches!(kind, IntroTouch::YesNo { .. }) {
                    keys &= !answer;
                }
            }
        }
        let touch = self.touch.map(|(_, at)| at);
        self.touch = match self.touch {
            Some((n, at)) if n > 1 => Some((n - 1, at)),
            _ => None,
        };
        (keys, touch)
    }

    /// The view for the frame just drawn.
    pub fn scene(&mut self, frame: u32, watch: &Watch, s: &Screens) -> Scene {
        let mut picked = self.pick(frame, watch, s);
        let strip = match picked.name {
            "battle-cmd" | "battle-moves" => picked.name,
            _ => "",
        };
        // a band newly up starts on FIGHT, or the moves on the move chosen last, as the game's
        // own cursor does
        if strip != self.strip_shown && !strip.is_empty() {
            self.sel = if strip == "battle-moves" {
                self.last_move
            } else {
                0
            };
            picked = self.pick(frame, watch, s);
        }
        self.strip_shown = strip;
        picked
    }

    fn pick(&mut self, frame: u32, w: &Watch, s: &Screens) -> Scene {
        let ran = |f: Option<usize>, n: u32| w.ran(f, n);
        let f = &self.f;
        let known = if ran(f.naming, 2) {
            // the question above the keyboard that answers it
            Some(Scene::stack(
                "naming",
                vec![Piece::crop(TOP, 0, 144, 256, 48), Piece::full(BOTTOM)],
                false,
            ))
        } else if ran(f.battle, 2) {
            Some(self.battle(frame, w, s))
        } else if let Some(last) = self.fallback.last().filter(|_| ran(f.saving, 8)) {
            // nothing else runs while the save is written: the layout up stays
            return last.clone();
        } else if let Some(app) = self.app(w, s) {
            Some(app)
        } else if self.game == Game::Platinum && ran(f.field, 16) && underground(s) {
            // the Underground swaps the screens: the world on the touch screen, tapped to send
            // the radar's ping, and the radar's map on the top one, over the world's corner
            Some(Scene {
                name: "underground",
                pieces: vec![
                    Piece::full(BOTTOM),
                    Piece::crop(TOP, 55, 4, 146, 138)
                        .scaled(0.5)
                        .at(4.0, 4.0)
                        .bordered(),
                ],
                back: Back::Colour(0xAE695D),
            })
        } else if self.game.hgss() && ran(f.field, 16) && ran(f.start_menu, 2) {
            Some(hgss_start_menu(s))
        } else if ran(f.field, 16) {
            // the overworld and the X menu: the Pokétch on the touch screen is left out
            Some(Scene::only(
                if ran(f.start_menu, 2) {
                    "start-menu"
                } else {
                    "top"
                },
                TOP,
            ))
        } else {
            None
        };
        if let Some(sc) = known {
            self.fallback.known(&sc, frame);
            return sc;
        }
        self.fallback.unknown(frame, s, w.ran(self.f.field, 120))
    }

    /// The game's screens other than battles and the overworld; none for a screen it has no
    /// layout for.
    fn app(&self, w: &Watch, s: &Screens) -> Option<Scene> {
        if self.game.hgss() {
            return self.hgss_app(w);
        }
        let ran = |f: Option<usize>| w.ran(f, 2);
        let f = &self.f;
        let both = |name| Scene::stack(name, vec![Piece::full(TOP), Piece::full(BOTTOM)], true);
        Some(if ran(f.party) {
            // the party on the top screen; the touch screen has only Poké Balls
            let mut sc = Scene::only("party", TOP);
            sc.back = Back::Sample(TOP, 1, 140);
            sc
        } else if ran(f.bag) {
            Scene::only("bag", TOP)
        } else if ran(f.summary) {
            Scene::only("summary", TOP)
        } else if ran(f.options) {
            Scene::only("options", TOP)
        } else if ran(f.journal) {
            Scene::only("journal", TOP)
        } else if ran(f.move_reminder) {
            Scene::only("move-reminder", TOP)
        } else if ran(f.town_map) {
            Scene::only("map", TOP)
        } else if ran(f.dex) {
            both("pokedex")
        } else if ran(f.trainer_card) {
            both("trainer-card")
        } else if ran(f.pc_box) {
            both("pc-box")
        } else if ran(f.evolution) || ran(f.egg_hatch) {
            Scene::only("evolution", TOP)
        } else if ran(f.intro) {
            // Rowan, his words and the controls' pages are on the top screen; his two questions
            // answered by a touch stand at the right edge, above the message box
            let mut sc = Scene::only("intro", TOP);
            if let Some(kind) = intro_touch(s) {
                let (crop, scale) = match kind {
                    IntroTouch::YesNo { .. } => (Piece::crop(BOTTOM, 94, 64, 54, 74), 1.0),
                    IntroTouch::Ball => (Piece::crop(BOTTOM, 56, 24, 146, 148), 0.6),
                };
                let pad = 2.0;
                let (cw, ch) = (crop.sw as f32 * scale, crop.sh as f32 * scale);
                let (x, y) = (W as f32 - cw - pad - 2.0, 72.0 - ch / 2.0);
                sc.pieces.push(
                    Piece::fill(x - pad, y - pad, cw + 2.0 * pad, ch + 2.0 * pad, 0x14141C)
                        .bordered(),
                );
                sc.pieces.push(crop.scaled(scale).at(x, y));
                sc.name = "intro-touch";
            }
            sc
        } else if ran(f.starter) {
            Scene::only("starter", TOP)
        } else {
            return None;
        })
    }

    /// HeartGold's and SoulSilver's screens other than battles and the overworld: an application
    /// played on the touch screen shows it, as long as it runs, from the frames it starts in to
    /// the ones it ends in; its evolutions and hatchings are on the top screen.
    fn hgss_app(&self, w: &Watch) -> Option<Scene> {
        let f = &self.f;
        if f.touch_apps.iter().any(|&a| w.ran(a, APP_FRAMES)) {
            return Some(Scene::only("touch-app", BOTTOM));
        }
        if w.ran(f.evolution, 2) || w.ran(f.egg_hatch, APP_FRAMES) {
            return Some(Scene::only("evolution", TOP));
        }
        None
    }

    /// The last of the battle's touch-screen panels drawn: 1 the commands, 2 FIGHT alone, 3 the
    /// moves, 4 another, 0 none seen yet (a state loaded in a battle).
    fn battle_panel(&self, w: &Watch) -> u8 {
        let mut best = 0;
        let mut kind = 0;
        let mut take = |f: Option<usize>, k: u8| {
            let t = w.last_frame(f);
            if t != 0 && t >= best {
                best = t;
                kind = k;
            }
        };
        for f in &self.f.other_menus {
            take(*f, 4);
        }
        take(self.f.action_menu, 1);
        take(self.f.first_menu, 2);
        take(self.f.move_menu, 3);
        kind
    }

    fn battle(&mut self, frame: u32, w: &Watch, s: &Screens) -> Scene {
        let f = &self.f;
        let bottom_blank = s.blank(BOTTOM);
        if w.ran(f.battle_bag, 2) || w.ran(f.battle_party, 2) {
            return Scene::only("battle-submenu", BOTTOM);
        }
        let input = w.ran(f.battle_input, 2) || (frame < self.strip_until && !bottom_blank);
        if !input {
            return Scene::only("battle", TOP);
        }
        let panel = self.read_panel(w, s);
        match panel {
            Panel::Commands { .. } => self.strip_until = frame + 20,
            Panel::Moves | Panel::Idle => self.strip_until = 0,
            _ => {}
        }
        match panel {
            Panel::Commands { fight_only } => {
                self.fight_only = fight_only;
                commands(s, fight_only, self.sel)
            }
            Panel::Moves => moves(s, self.sel),
            _ if frame < self.strip_until => commands(s, self.fight_only, self.sel),
            Panel::YesNo => yes_no(),
            _ => battle_menu(s),
        }
    }

    /// What the battle's touch screen shows, from its colours and the panel drawn last.
    fn read_panel(&mut self, w: &Watch, s: &Screens) -> Panel {
        let px = |x, y| rgb(s.pixel(BOTTOM, x, y));
        let blue = |x, y| {
            let (r, g, b) = px(x, y);
            b > 150 && r < 110 && g > 90 && g < 200 && b > r + 60
        };
        let red = |x, y| {
            let (r, g, b) = px(x, y);
            r > 180 && g < 120 && b < 120
        };
        let orange = |x, y| {
            let (r, g, b) = px(x, y);
            r > 180 && g > 110 && g < 200 && b < 110
        };
        // a button's outline: a grey, never the screen's white or green
        let edge = |x, y| {
            let (r, g, b) = px(x, y);
            r.max(g).max(b) - r.min(g).min(b) < 16 && r < 200
        };
        let lit = |x, y| {
            let (r, g, b) = px(x, y);
            r > 160 && g > 160 && b > 160
        };
        let panel = self.battle_panel(w);
        let fight_only = (panel == 0 || panel == 2)
            && red(128, 60)
            && !orange(40, 160)
            && !blue(128, 166)
            && !blue(40, 166);
        let cmd = (red(128, 60) && orange(40, 160) && blue(128, 166) && panel != 4) || fight_only;
        let moves = !cmd && panel != 4 && blue(40, 166) && blue(128, 166) && blue(216, 166);
        let (yes, no) = (
            red(128, 50) || red(128, 64),
            blue(128, 124) || blue(128, 136),
        );
        let yes_no = !blue(128, 166)
            && !orange(40, 160)
            && ((yes && no)
                || (yes && lit(128, 124) && edge(128, 112))
                || (no && lit(128, 50) && edge(128, 40)));
        let blank = s.blank(BOTTOM);
        let idle = blank || (!blue(128, 166) && !red(128, 60));
        self.unknown_for = if cmd || moves || idle || yes_no {
            0
        } else {
            self.unknown_for + 1
        };
        if cmd {
            Panel::Commands { fight_only }
        } else if moves {
            Panel::Moves
        } else if yes_no {
            Panel::YesNo
        } else if idle {
            Panel::Idle
        } else {
            Panel::Unknown
        }
    }
}

/// HeartGold's and SoulSilver's X menu, on the touch screen: the overworld, the menu's panel (x
/// 5-162, y 0-185 of the touch screen, inside its grey frame) at three quarters of its size at the
/// right edge; the touch screen alone once SAVE's orange card is up, and the world alone while
/// the touch screen is black, the menu closing.
fn hgss_start_menu(s: &Screens) -> Scene {
    let orange = |x, y| {
        let (r, g, b) = rgb(s.pixel(BOTTOM, x, y));
        r > 200 && g > 100 && g < 190 && b < 110
    };
    if orange(40, 20) && orange(20, 60) && orange(236, 20) {
        return Scene::only("save", BOTTOM);
    }
    let mut sc = Scene::only("start-menu", TOP);
    if s.blank(BOTTOM) {
        return sc;
    }
    let (scale, pad) = (0.75, 2.0);
    let panel = Piece::crop(BOTTOM, 5, 0, 158, 186);
    let (pw, ph) = (panel.sw as f32 * scale, panel.sh as f32 * scale);
    let (x, y) = (W as f32 - pw - pad - 2.0, (H as f32 - ph) / 2.0);
    sc.pieces
        .push(Piece::fill(x - pad, y - pad, pw + 2.0 * pad, ph + 2.0 * pad, 0x14141C).bordered());
    sc.pieces.push(panel.scaled(scale).at(x, y));
    sc
}

/// Whether the touch screen shows the 3D world: the Underground, which swaps the screens.
fn underground(s: &Screens) -> bool {
    let mut n = 0;
    for y in (0..H).step_by(8) {
        for x in (0..W).step_by(8) {
            if crate::scene::layer_class(s.layer(BOTTOM, x, y)) == crate::scene::LAYER_3D {
                n += 1;
            }
        }
    }
    n > (W / 8) * (H / 8) / 2
}

/// The band's layout: where the buttons go, two rows of them.
struct Band {
    inner_x: f32,
    inner_w: f32,
    row_h: f32,
    row_y: f32,
    row2_y: f32,
    gap: f32,
}

const BAND_LAYOUT: Band = Band {
    inner_x: 3.0,
    inner_w: W as f32 - 3.0 - 2.0,
    row_h: (BAND_H - 2.0 * 3.0 - 4.0) / 2.0,
    row_y: BAND_Y + 3.0,
    row2_y: BAND_Y + 3.0 + (BAND_H - 2.0 * 3.0 - 4.0) / 2.0 + 4.0,
    gap: 4.0,
};

/// The band under the battle: the battle above it, the buttons in it.
fn band_scene(name: &'static str) -> Scene {
    Scene {
        name,
        pieces: vec![
            Piece::full(TOP),
            Piece::fill(0.0, BAND_Y, W as f32 + 1.0, BAND_H, BAND),
        ],
        back: Back::Colour(0),
    }
}

/// The colour most of the plain columns of `c`'s row `y` have.
fn row_colour(s: &Screens, c: &Cut, y: i32) -> u32 {
    let cols: Vec<u32> = (c.tile_x..c.tile_x + c.tile_w)
        .map(|x| s.pixel(BOTTOM, x as usize, y as usize))
        .collect();
    cols.iter()
        .copied()
        .max_by_key(|c0| cols.iter().filter(|c1| *c1 == c0).count())
        .unwrap_or(0)
}

/// Button `c` cut from the touch screen and drawn `dw` wide and a row high at `dx, dy`: its
/// rows' flat colours across it, its ends at its ends and its label in its middle, on a dark
/// backing that carries its outline. Returns where the backing went among the pieces.
fn button(sc: &mut Scene, s: &Screens, c: Cut, dx: f32, dy: f32, dw: f32, drop: u8) -> usize {
    let b = &BAND_LAYOUT;
    let scl = b.row_h / c.sh as f32;
    let backing = sc.pieces.len();
    sc.pieces.push(Piece::fill(dx, dy, dw, b.row_h, BACKING));
    let piece = |sx: i32, sw: i32, x: f32| {
        Piece::crop(BOTTOM, sx, c.sy, sw, c.sh)
            .scaled(scl)
            .at(x, dy)
            .dropping(drop)
    };
    if dw <= c.sw as f32 * scl + 0.5 {
        sc.pieces
            .push(piece(c.sx, c.sw, dx + (dw - c.sw as f32 * scl) / 2.0));
        return backing;
    }
    // the label as drawn: from the first column after the plain ones that is not plain to the
    // last before the plain columns at the right end, never less than the cut's own
    let (mut lx0, mut lx1) = (c.lx0, c.lx1);
    {
        let rows = c.sh.min(64);
        let flat: Vec<u32> = (0..rows).map(|y| row_colour(s, &c, c.sy + y)).collect();
        let plain = |x: i32| {
            (0..rows).all(|y| s.pixel(BOTTOM, x as usize, (c.sy + y) as usize) == flat[y as usize])
        };
        let end = c.sx + c.sw;
        let mut a = c.tile_x + c.tile_w;
        while a < end && plain(a) {
            a += 1;
        }
        let mut r = end - 1;
        while r > a && !plain(r) {
            r -= 1;
        }
        let mut e = r;
        while e > a && plain(e) {
            e -= 1;
        }
        if a < end && r > a && e >= a && e < r {
            lx0 = lx0.min(a);
            lx1 = lx1.max(e + 1);
        }
    }
    // one flat colour a run of rows across the button; the ends and the label over it
    let mut y0 = c.sy;
    while y0 < c.sy + c.sh {
        let col = row_colour(s, &c, y0);
        let mut y1 = y0 + 1;
        while y1 < c.sy + c.sh && row_colour(s, &c, y1) == col {
            y1 += 1;
        }
        let top = (y0 - c.sy) as f32 * scl;
        let bottom = if y1 < c.sy + c.sh {
            (y1 - c.sy) as f32 * scl + 0.5
        } else {
            b.row_h
        };
        sc.pieces
            .push(Piece::fill(dx, dy + top, dw, bottom - top, col));
        y0 = y1;
    }
    sc.pieces.push(piece(c.sx, lx0 - c.sx, dx));
    let right = c.sx + c.sw - lx1;
    sc.pieces
        .push(piece(lx1, right, dx + dw - right as f32 * scl));
    sc.pieces.push(piece(
        lx0,
        lx1 - lx0,
        dx + (dw - (lx1 - lx0) as f32 * scl) / 2.0,
    ));
    backing
}

/// The chosen button outlined in white, the others shaded.
fn outline(sc: &mut Scene, backings: &[usize], sel: usize) {
    let mut shades = Vec::new();
    for (k, &i) in backings.iter().enumerate() {
        let chosen = k == sel;
        let p = &mut sc.pieces[i];
        p.border = Some(if chosen {
            (CHOSEN, 2.0)
        } else {
            (BACKING, 0.75)
        });
        if !chosen {
            let mut shade = p.clone();
            shade.border = None;
            shade.source = crate::scene::Source::Fill(0);
            shade.alpha = 120;
            shades.push(shade);
        }
    }
    sc.pieces.extend(shades);
}

/// The commands in the band: FIGHT above BAG, RUN and POKéMON, the party's balls beside FIGHT.
fn commands(s: &Screens, fight_only: bool, sel: usize) -> Scene {
    let b = &BAND_LAYOUT;
    let mut sc = band_scene("battle-cmd");
    let cell = (b.inner_w - 2.0 * b.gap) / 3.0;
    let mut backings = vec![button(
        &mut sc,
        s,
        COMMANDS[0],
        b.inner_x,
        b.row_y,
        2.0 * cell + b.gap,
        0,
    )];
    let party_x = b.inner_x + 2.0 * (cell + b.gap);
    if !fight_only {
        let row2 = b.row2_y;
        backings.push(button(&mut sc, s, COMMANDS[1], b.inner_x, row2, cell, 0));
        backings.push(button(
            &mut sc,
            s,
            COMMANDS[2],
            party_x,
            row2,
            b.inner_w - 2.0 * (cell + b.gap),
            0,
        ));
        backings.push(button(
            &mut sc,
            s,
            COMMANDS[3],
            b.inner_x + cell + b.gap,
            row2,
            cell,
            0,
        ));
    }
    // the party: the balls on the touch screen's top row, three over three, without their
    // dark outlines, which would ring each in grey
    let party_w = b.inner_w - 2.0 * (cell + b.gap);
    let (ball, gap) = (17.0, 4.0);
    let ps = (party_w / (3.0 * ball + 2.0 * gap)).min((b.row_h - 1.0) / (2.0 * ball + gap));
    let px = party_x + (party_w - (3.0 * ball + 2.0 * gap) * ps) / 2.0;
    let py = b.row_y + (b.row_h - (2.0 * ball + gap) * ps) / 2.0;
    let drop = LAYER_BG0 | LAYER_BG1 | LAYER_BG2 | LAYER_BG3 | LAYER_BACKDROP;
    for i in 0..6 {
        let mut p = Piece::crop(BOTTOM, 4 + 19 * i, 5, ball as i32, ball as i32)
            .scaled(ps)
            .at(
                px + (i % 3) as f32 * (ball + gap) * ps,
                py + (i / 3) as f32 * (ball + gap) * ps,
            )
            .dropping(drop);
        p.keys = [0x383030, 0x414141];
        sc.pieces.push(p);
    }
    outline(&mut sc, &backings, sel.min(backings.len() - 1));
    sc
}

/// The four moves in the band, two by two; B cancels.
fn moves(s: &Screens, sel: usize) -> Scene {
    let b = &BAND_LAYOUT;
    let mut sc = band_scene("battle-moves");
    let cell = (b.inner_w - b.gap) / 2.0;
    let slot_x = [1, 129, 1, 129];
    let slot_y = [25, 25, 89, 89];
    let mut backings = Vec::new();
    for i in 0..4 {
        // the button inside its outline, its pale middle kept, the dark band inside its sides
        let c = cut([
            slot_x[i] + 2,
            slot_y[i] + 10,
            122,
            40,
            slot_x[i] + 14,
            slot_x[i] + 112,
            slot_x[i] + 5,
            4,
        ]);
        let x = b.inner_x + (i & 1) as f32 * (cell + b.gap);
        let y = if i >> 1 == 1 { b.row2_y } else { b.row_y };
        backings.push(button(&mut sc, s, c, x, y, cell, LAYER_BG2));
    }
    outline(&mut sc, &backings, sel);
    sc
}

/// YES and NO beside the battle, small, at its right, over its upper half: the game's own cursor
/// on them answers with the D-pad.
fn yes_no() -> Scene {
    let mut sc = Scene::only("battle-yesno", TOP);
    let (scale, gap) = (0.4, 4.0);
    let (w, h) = (244.0 * scale, 60.0 * scale);
    let (x, y) = (W as f32 - w - 4.0, BAND_Y - 2.0 * h - gap - 4.0);
    sc.pieces.push(
        Piece::crop(BOTTOM, 6, 35, 244, 60)
            .scaled(scale)
            .at(x, y)
            .bordered(),
    );
    sc.pieces.push(
        Piece::crop(BOTTOM, 6, 107, 244, 60)
            .scaled(scale)
            .at(x, y + h + gap)
            .bordered(),
    );
    sc
}

/// Another of the battle's panels: the whole touch screen small at the lower left, over the
/// battle's corner.
fn battle_menu(s: &Screens) -> Scene {
    let mut sc = Scene::only("battle-menu", TOP);
    if !s.blank(BOTTOM) {
        let scale = (126.0 / 256.0f32).min(98.0 / 192.0);
        sc.pieces.push(
            Piece::full(BOTTOM)
                .scaled(scale)
                .at(1.0, BAND_Y - 192.0 * scale)
                .bordered(),
        );
    }
    sc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watch;

    fn watch_of(game: Game) -> Watch {
        // (no ROM: the overlays' first bytes go unchecked)
        Watch::new(watch::load(game.table(), std::path::Path::new("")))
    }

    #[test]
    fn each_game_is_known_by_its_code_and_has_its_table() {
        assert_eq!(Game::of("CPUE"), Some(Game::Platinum));
        assert_eq!(Game::of("IPKE"), Some(Game::HeartGold));
        assert_eq!(Game::of("IPGE"), Some(Game::SoulSilver));
        assert_eq!(Game::of("IRDO"), None, "White 2 is the fifth generation's");
        assert!(watch_of(Game::Platinum).find("FieldMap_Main").is_some());
    }

    /// Every function HeartGold's and SoulSilver's layouts go by is in both tables: a name that
    /// is not would leave its screen to the fallback, silently.
    #[test]
    fn heartgold_and_soulsilver_have_every_function_their_layouts_follow() {
        for game in [Game::HeartGold, Game::SoulSilver] {
            let w = watch_of(game);
            for name in HGSS_TOUCH_APPS.iter().copied().chain([
                "FieldMap_Main",
                "Task_StartMenu",
                "NamingScreen",
                "OakSpeech",
                "EvolutionScene_Task",
                "HatchEgg",
                "Battle",
                "BattleInput_CheckTouch",
                "HandleWriteSaveAsync_NormalData",
            ]) {
                assert!(w.find(name).is_some(), "{game:?} has no {name}");
            }
            let f = Funcs::of(game, &w);
            assert_eq!(f.touch_apps.iter().filter(|a| a.is_some()).count() % 3, 0);
            assert!(
                f.touch_apps.iter().all(Option::is_some),
                "{game:?}: an app's start or end"
            );
        }
    }

    /// HeartGold's code is SoulSilver's but for three ranges shifted: in the Pokédex's overlay four
    /// bytes on, in the main menu's (past its first functions) four back.
    #[test]
    fn heartgold_is_soulsilver_shifted_where_its_code_differs() {
        let (hg, ss) = (Game::HeartGold.table(), Game::SoulSilver.table());
        let addr = |table: &str, name: &str| {
            table
                .lines()
                .find(|l| l.ends_with(&format!(" {name}")))
                .and_then(|l| u32::from_str_radix(l.split_whitespace().next()?, 16).ok())
                .unwrap()
        };
        assert_eq!(addr(hg, "Pokedex"), addr(ss, "Pokedex") + 4);
        assert_eq!(addr(hg, "MainMenu"), addr(ss, "MainMenu"));
        assert_eq!(
            addr(hg, "MainMenu_SelectOption_MigrateFromAgb"),
            addr(ss, "MainMenu_SelectOption_MigrateFromAgb") - 4
        );
        assert_eq!(addr(hg, "Bag"), addr(ss, "Bag"));
    }
}
