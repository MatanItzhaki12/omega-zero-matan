//! The link screen a game opens with SELECT + MENU: Host or Join, over a cable or the wireless
//! adapter, then linking, linked, failed or unplugged. It is drawn in the kit over the theme's
//! backdrop, as the menus are: a lone panel titled LINK holding the ROLE and MODE rows and, under
//! them, what is happening; the link's art between it and the bottom bar; and the kit's bar,
//! B RETURN and A LINK while picking, with SELECT MODE and ◀▶ SWAP between them.
//!
//! The art is assets/ui/components/omega-link.js's (omega_ui::link_art): the link port along the
//! console's top edge, the plug on its cable or the wireless adapter over it, the radio arcs and
//! the clicks of a plug going home. Everything is placed in art pixels and drawn at 2x, so every
//! edge lands on the grid: the plug drops into the port and lifts out of it a whole art pixel at
//! a time, a failed link shakes it a pixel either way, and the arcs pulse in the four steps of
//! kit::fade.

use omega_gfx::{out_h, out_w};
use omega_ui::kit::action::Side;
use omega_ui::kit::bar::{action_swap, BarLayout, Hint};
use omega_ui::kit::fade::stepped;
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::pixel::quad;
use omega_ui::kit::row::panel_height;
use omega_ui::link_art::{
    ADAPTER_H, ADAPTER_W, CABLE_W, CLICKS_H, CLICKS_W, CONNECTOR_H, PLUG_H, RINGS, TIP_H,
};
use omega_ui::{ease, stage_y, Draw, Millis, TexId, UndoFace, SPRITE_PX};

use crate::app::{App, GameMenu, LinkRow};
use crate::link_kind::LinkKind;
use crate::list_view::{RowLabel, RowValue, TypingView, ViewRow};
use crate::nav::progress;
use crate::screens::bar::BarSet;
use crate::screens::lone::{hold, hold_aligned, notes, notes_height, rows_height, Lone, NOTE_GAP};

/// One of the art's faces uploaded: its texture and its size in art pixels.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Sprite {
    pub tex: TexId,
    pub w: u32,
    pub h: u32,
}

impl Sprite {
    /// The face with its top left at art pixel (`x`, `y`) of the screen, at 2x.
    fn at(self, x: i32, y: i32) -> Draw {
        quad(self.tex, self.w, self.h, px(x), px(y))
    }
}

/// Every piece of the art, uploaded.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct LinkSprites {
    pub plug_host: Sprite,
    pub plug_join: Sprite,
    pub cable: Sprite,
    pub adapter: Sprite,
    pub port: Sprite,
    pub arcs_right: [Sprite; RINGS],
    pub arcs_left: [Sprite; RINGS],
    pub clicks: Sprite,
}

/// An art pixel position as the screen pixels a Draw takes: always even.
fn px(art: i32) -> f32 {
    (art * SPRITE_PX as i32) as f32
}

/// The screen in art pixels, across and down.
fn page() -> (i32, i32) {
    ((out_w() / SPRITE_PX) as i32, (out_h() / SPRITE_PX) as i32)
}

/// How tall the port's strip is, from its top edge to the bottom of the screen; the bottom bar
/// stands over its lower half.
pub const PORT_UP: u32 = 36;

/// The strip's top edge, in art pixels down the screen.
pub fn port_top() -> i32 {
    page().1 - PORT_UP as i32
}

/// The middle of the screen across, in art pixels, which the plug and the adapter hang over.
fn centre() -> i32 {
    page().0 / 2
}

/// How wide the LINK panel is, and how far down the 720x480 stage its top is.
pub const PANEL_W: u32 = 200;
pub const PANEL_TOP: u32 = 8;

/// Where the plug's bottom edge is, in art pixels from the strip's top edge: seated, its tip all
/// the way into the port; hanging over the port while the player picks; and lifted higher when a
/// link failed.
const SEATED: i32 = TIP_H as i32;
const PICK: i32 = -18;
const FAILED: i32 = -30;
/// Where the adapter's body ends, from the strip's top edge: on it, its connector in the port, and
/// held over it while the player picks.
const ADAPTER_PICK: i32 = -30;

const DROP_MS: Millis = 200;
const SEAT_MS: Millis = 160;
const LIFT_MS: Millis = 250;
const UNPLUG_MS: Millis = 260;
const ARC_MS: f32 = 1200.0;
const ARC_STAGGER_MS: f32 = 300.0;
/// A failed link shakes the plug or the adapter once it has lifted: a pixel to the right, to the
/// left, and back, `SHAKES` times, each `SHAKE_MS` long.
const SHAKE_MS: Millis = 50;
const SHAKES: Millis = 4;
/// How far the arcs stand from the adapter's sides, and from each other.
const ARC_GAP: i32 = 4;
const ARC_STEP: i32 = 6;

/// `a` to `b` `t` of the way, to the nearest art pixel.
fn between(a: i32, b: i32, t: f32) -> i32 {
    (a as f32 + (b - a) as f32 * t.clamp(0.0, 1.0)).round() as i32
}

fn eased(now: Millis, since: Millis, ms: Millis) -> f32 {
    ease((now.saturating_sub(since) as f32 / ms as f32).clamp(0.0, 1.0))
}

fn working_plug(since: Millis, now: Millis) -> i32 {
    between(PICK, SEATED, eased(now, since, DROP_MS))
}

fn working_adapter(since: Millis, now: Millis) -> i32 {
    between(ADAPTER_PICK, 0, eased(now, since, DROP_MS))
}

fn working_arcs(since: Millis, now: Millis) -> [f32; RINGS] {
    let t = now.saturating_sub(since);
    let mut out = [0.0; RINGS];
    if t < DROP_MS {
        return out;
    }
    for (i, a) in out.iter_mut().enumerate() {
        let u = (t - DROP_MS) as f32 - ARC_STAGGER_MS * i as f32;
        if u >= 0.0 {
            let phase = (u / ARC_MS).fract();
            *a = 1.0 - (2.0 * phase - 1.0).abs();
        }
    }
    out
}

/// Where the plug's bottom edge is at `now`, in art pixels from the strip's top edge.
pub fn plug_bottom(menu: GameMenu, now: Millis) -> i32 {
    match menu {
        GameMenu::Pick(_) | GameMenu::Rooms { .. } => PICK,
        GameMenu::Working { since, .. } => working_plug(since, now),
        GameMenu::Linked { worked, since, .. } => between(
            working_plug(worked, since),
            SEATED,
            eased(now, since, SEAT_MS),
        ),
        GameMenu::Failed { worked, since, .. } => between(
            working_plug(worked, since),
            FAILED,
            eased(now, since, LIFT_MS),
        ),
        GameMenu::Unplug { since, .. } => between(SEATED, PICK, eased(now, since, UNPLUG_MS)),
    }
}

/// Where the adapter's body ends at `now`, in art pixels from the strip's top edge.
pub fn adapter_bottom(menu: GameMenu, now: Millis) -> i32 {
    match menu {
        GameMenu::Pick(_) | GameMenu::Rooms { .. } => ADAPTER_PICK,
        GameMenu::Working { since, .. } => working_adapter(since, now),
        GameMenu::Linked { worked, since, .. } => between(
            working_adapter(worked, since),
            0,
            eased(now, since, SEAT_MS),
        ),
        GameMenu::Failed { worked, since, .. } => working_adapter(worked, since),
        GameMenu::Unplug { since, .. } => between(0, ADAPTER_PICK, eased(now, since, UNPLUG_MS)),
    }
}

/// How far a failed link has the plug or the adapter shaken across at `now`, in art pixels: once
/// it has lifted, a pixel right, a pixel left, `SHAKES` times; otherwise none.
pub fn shake(menu: GameMenu, now: Millis) -> i32 {
    let GameMenu::Failed { since, .. } = menu else {
        return 0;
    };
    let t = now.saturating_sub(since);
    if !(LIFT_MS..LIFT_MS + SHAKE_MS * SHAKES).contains(&t) {
        return 0;
    }
    match ((t - LIFT_MS) / SHAKE_MS) % 2 {
        0 => 1,
        _ => -1,
    }
}

/// How lit each radio arc is at `now`, in the four steps of a kit fade: they call out in turn
/// while linking, all lit once linked, and fade away when the link fails or ends.
pub fn arc_alphas(menu: GameMenu, now: Millis) -> [f32; RINGS] {
    let smooth = match menu {
        GameMenu::Pick(_) | GameMenu::Rooms { .. } => [0.0; RINGS],
        GameMenu::Working { since, .. } => working_arcs(since, now),
        GameMenu::Linked { worked, since, .. } => {
            let from = working_arcs(worked, since);
            let t = eased(now, since, SEAT_MS);
            from.map(|a| a + (1.0 - a) * t)
        }
        GameMenu::Failed { worked, since, .. } => {
            let fade = 1.0 - eased(now, since, LIFT_MS);
            working_arcs(worked, since).map(|a| a * fade)
        }
        GameMenu::Unplug { since, .. } => [1.0 - eased(now, since, UNPLUG_MS); RINGS],
    };
    smooth.map(stepped)
}

/// Whether the clicks of a plug going home show: once it has seated on a link.
pub fn clicks_shown(menu: GameMenu, now: Millis) -> bool {
    matches!(menu, GameMenu::Linked { since, .. } if now.saturating_sub(since) >= SEAT_MS)
}

fn role_of(menu: GameMenu) -> LinkRow {
    match menu {
        GameMenu::Pick(role)
        | GameMenu::Working { role, .. }
        | GameMenu::Linked { role, .. }
        | GameMenu::Failed { role, .. }
        | GameMenu::Unplug { role, .. } => role,
        GameMenu::Rooms { .. } => LinkRow::Join,
    }
}

/// The art for `menu` at `now` over `kind` of link, the cable running up from `cable_top` (art
/// pixels down the screen) to the plug: the cable, the plug or the adapter, then the port over
/// them, so the plug's tip and the adapter's connector go into it, then the clicks or the arcs.
pub fn draw_link_art(
    menu: GameMenu,
    kind: LinkKind,
    now: Millis,
    s: &LinkSprites,
    cable_top: i32,
    out: &mut Vec<Draw>,
) {
    let (port, mid, dx) = (port_top(), centre(), shake(menu, now));
    match kind {
        LinkKind::Cable => {
            let plug = match role_of(menu) {
                LinkRow::Host => s.plug_host,
                LinkRow::Join => s.plug_join,
            };
            let top = port + plug_bottom(menu, now) - PLUG_H as i32;
            let cable = top - cable_top;
            if cable > 0 {
                let x = mid - CABLE_W as i32 / 2 + dx;
                out.push(quad(
                    s.cable.tex,
                    CABLE_W,
                    cable as u32,
                    px(x),
                    px(cable_top),
                ));
            }
            out.push(plug.at(mid - plug.w as i32 / 2 + dx, top));
            out.push(s.port.at(0, port));
            if clicks_shown(menu, now) {
                out.push(
                    s.clicks
                        .at(mid - CLICKS_W as i32 / 2, port - CLICKS_H as i32),
                );
            }
        }
        LinkKind::Wireless => {
            let body = (ADAPTER_H - CONNECTOR_H) as i32;
            let top = port + adapter_bottom(menu, now) - body;
            let half = ADAPTER_W as i32 / 2;
            out.push(s.adapter.at(mid - half + dx, top));
            out.push(s.port.at(0, port));
            for (i, alpha) in arc_alphas(menu, now).into_iter().enumerate() {
                if alpha <= 0.0 {
                    continue;
                }
                let (right, left) = (s.arcs_right[i], s.arcs_left[i]);
                let y = top + body / 2 - right.h as i32 / 2;
                let reach = half + ARC_GAP + ARC_STEP * i as i32;
                for (arc, x) in [
                    (right, mid + reach + dx),
                    (left, mid - reach - left.w as i32 + dx),
                ] {
                    out.push(Draw::Tex {
                        x: px(x),
                        y: px(y),
                        w: px(arc.w as i32),
                        h: px(arc.h as i32),
                        tex: arc.tex,
                        alpha,
                    });
                }
            }
        }
    }
}

/// Where the LINK panel goes, `[x, y]` in art pixels: across the middle, near the top of the
/// 720x480 stage, which a taller screen has against its bottom.
pub fn panel_at(panel: &Lone) -> [i32; 2] {
    let stage_top = (stage_y().max(0.0) as u32 / SPRITE_PX) as i32;
    [
        (page().0 - panel.w as i32).div_euclid(2),
        stage_top + PANEL_TOP as i32,
    ]
}

/// A row of the LINK panel, which the cursor moves between as it does a list of settings.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum LinkField {
    /// Local or online, for a game that can link online.
    Play,
    /// Local: another handheld on omega-zero, or RetroArch on this network.
    With,
    Role,
    /// The cable or the wireless adapter, for a game that has both.
    Mode,
    /// Online: the name the room shows, typed on the keyboard.
    Name,
    /// Online: the room's password, typed on the keyboard.
    Password,
    /// Online: the relay a room is opened through.
    Server,
    /// Online, joining: the rows few need, shown or not.
    Advanced,
    /// Under ADVANCED: an address to join at, past the lobby, typed on the keyboard.
    Address,
    /// No row: the keyboard asking for the password of the room being joined.
    RoomPassword,
}

impl LinkField {
    pub fn label(self) -> &'static str {
        match self {
            LinkField::Play => "PLAY",
            LinkField::With => "WITH",
            LinkField::Role => "ROLE",
            LinkField::Mode => "MODE",
            LinkField::Name => "NAME",
            LinkField::Password => "PASSWORD",
            LinkField::Server => "SERVER",
            LinkField::Advanced => "ADVANCED",
            LinkField::Address => "ADDRESS",
            LinkField::RoomPassword => "PASSWORD",
        }
    }

    /// A opens the keyboard on the row rather than linking.
    pub fn typed(self) -> bool {
        matches!(
            self,
            LinkField::Name | LinkField::Password | LinkField::Address | LinkField::RoomPassword
        )
    }
}

/// What the LINK panel shows: the role and the link's kind, the role lit with its carets while
/// the player picks, and under them what is happening, if anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkView {
    pub role: LinkRow,
    pub kind: LinkKind,
    pub picking: bool,
    pub status: Option<&'static str>,
    /// Online or nearby, when the game can link online: None when it cannot.
    pub online: Option<bool>,
    /// A line more under the status: who this handheld is online, or who it is linked with.
    pub detail: Option<String>,
    /// The rooms open online, while one is being picked.
    pub rooms: Option<RoomsView>,
    /// The panel's rows, top to bottom, and the one under the cursor while the player picks:
    /// none to show ROLE and MODE alone.
    pub fields: Vec<(LinkField, RowValue)>,
    pub lit: Option<usize>,
}

/// The rooms open online, as the ROOMS panel lists them: each host's name and their game, the
/// lit one, and the first in view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomsView {
    pub lines: Vec<(String, String)>,
    pub lit: usize,
    pub first: usize,
}

/// How many rooms the ROOMS panel shows at once.
pub const ROOM_ROWS: usize = 5;

/// How many characters of a room's game its row has room for, beside its host's name.
const GAME_CHARS: usize = 14;

/// A room's game as its row prints it: its name without its region and revision, a Pokémon
/// game by its colour, another by its series, cut to fit.
pub fn short_game(name: &str) -> String {
    let name = name.split(" (").next().unwrap_or(name).trim();
    let name = match name.strip_prefix("Pokemon - ") {
        Some(colour) => colour.trim_end_matches(" Version").trim(),
        None => name.split(" - ").next().unwrap_or(name).trim(),
    };
    match name.chars().count() > GAME_CHARS {
        true => format!(
            "{}…",
            name.chars()
                .take(GAME_CHARS - 1)
                .collect::<String>()
                .trim_end()
        ),
        false => name.to_string(),
    }
}

/// The kind's word on the MODE row.
fn kind_word(kind: LinkKind) -> &'static str {
    match kind {
        LinkKind::Cable => "Cable",
        LinkKind::Wireless => "Wireless",
    }
}

/// The LINK panel in `pal`: ROLE with its value and carets, MODE with the link's kind, and the
/// status as a note under them in the dim colour, the panel as tall as what it holds.
pub fn link_panel_face(view: &LinkView, pal: &Palette) -> UndoFace {
    if let Some(rooms) = &view.rooms {
        return rooms_panel_face(rooms, view.status, pal);
    }
    let rows: Vec<ViewRow> = match view.fields.is_empty() {
        true => vec![
            ViewRow {
                label: RowLabel::Setting("ROLE"),
                value: Some(RowValue::Choice(view.role.text())),
                dim: false,
            },
            ViewRow {
                label: RowLabel::Setting("MODE"),
                value: Some(RowValue::Said(kind_word(view.kind))),
                dim: false,
            },
        ],
        false => view
            .fields
            .iter()
            .map(|(field, value)| ViewRow {
                label: RowLabel::Setting(field.label()),
                value: Some(value.clone()),
                dim: false,
            })
            .collect(),
    };
    let held = rows_height(rows.len());
    let lines: Vec<String> = view
        .status
        .iter()
        .map(std::string::ToString::to_string)
        .chain(view.detail.clone())
        .collect();
    let content = match lines.len() {
        0 => held,
        n => held + notes_height(n),
    };
    let mut panel = panel_face(PANEL_W, panel_height(content), "LINK", true, pal);
    let lit = match view.fields.is_empty() {
        true => view.picking.then_some(0),
        false => view.lit,
    };
    // values with carets and without, as ROLE's and NAME's, end in one line
    hold_aligned(&mut panel, &rows, lit, pal);
    notes(&mut panel, &lines, held + NOTE_GAP, pal);
    panel.face
}

/// The ROOMS panel in `pal`: the rooms in view, each host's name and their game, the lit one
/// lit, and `status` under them while there are none to show.
fn rooms_panel_face(rooms: &RoomsView, status: Option<&'static str>, pal: &Palette) -> UndoFace {
    let rows: Vec<ViewRow> = rooms
        .lines
        .iter()
        .skip(rooms.first)
        .take(ROOM_ROWS)
        .map(|(host, game)| ViewRow {
            label: RowLabel::Name(host.clone()),
            value: Some(RowValue::Name(game.clone())),
            dim: false,
        })
        .collect();
    let held = rows_height(rows.len());
    let lines: Vec<String> = status
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    let content = match lines.len() {
        0 => held,
        n => held + notes_height(n),
    };
    let mut panel = panel_face(PANEL_W, panel_height(content.max(1)), "ROOMS", true, pal);
    hold(
        &mut panel,
        &rows,
        Some(rooms.lit.saturating_sub(rooms.first)),
        pal,
    );
    notes(&mut panel, &lines, held + NOTE_GAP, pal);
    panel.face
}

/// The pair of actions the bar shows over the link screen.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum LinkActions {
    /// Picking: B RETURN, A LINK.
    Pick,
    /// Linking: B CANCEL, and nothing for A.
    Working,
    /// Linked, opened over a link under way: B RETURN, A END LINK.
    Linked,
    /// Failed: A OK alone.
    Failed,
    /// Picking, on a row A types on: B RETURN, A EDIT.
    Edit,
    /// Linked a moment ago, or unplugging: nothing to press, both actions empty.
    Idle,
}

impl LinkActions {
    pub const ALL: [LinkActions; 6] = [
        LinkActions::Pick,
        LinkActions::Working,
        LinkActions::Linked,
        LinkActions::Failed,
        LinkActions::Idle,
        LinkActions::Edit,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// The big word and the button in its pill of the action at `side`.
    pub fn action(self, side: Side) -> (&'static str, &'static str) {
        match (self, side) {
            (LinkActions::Pick | LinkActions::Linked | LinkActions::Edit, Side::Left) => {
                ("RETURN", "B")
            }
            (LinkActions::Edit, Side::Right) => ("EDIT", "A"),
            (LinkActions::Working, Side::Left) => ("CANCEL", "B"),
            (LinkActions::Pick, Side::Right) => ("LINK", "A"),
            (LinkActions::Linked, Side::Right) => ("END LINK", "A"),
            (LinkActions::Failed, Side::Right) => ("OK", "A"),
            (LinkActions::Failed | LinkActions::Idle, Side::Left)
            | (LinkActions::Working | LinkActions::Idle, Side::Right) => ("", ""),
        }
    }

    /// The actions `menu` has.
    pub fn of(menu: GameMenu) -> LinkActions {
        match menu {
            GameMenu::Pick(_) | GameMenu::Rooms { .. } => LinkActions::Pick,
            GameMenu::Working { .. } => LinkActions::Working,
            GameMenu::Linked { opened: true, .. } => LinkActions::Linked,
            GameMenu::Failed { .. } => LinkActions::Failed,
            GameMenu::Linked { .. } | GameMenu::Unplug { .. } => LinkActions::Idle,
        }
    }

    /// The actions the bar had before `menu`'s and when it changed to them, for the swap: picking
    /// gives way to linking, and linking to linked or failed.
    fn before(menu: GameMenu) -> Option<(LinkActions, Millis)> {
        match menu {
            GameMenu::Working { since, .. } => Some((LinkActions::Pick, since)),
            GameMenu::Linked {
                opened: false,
                since,
                ..
            }
            | GameMenu::Failed { since, .. } => Some((LinkActions::Working, since)),
            _ => None,
        }
    }
}

/// The actions the bar shows over `menu` at `now`, and how far they are sunk: when the actions
/// change, the old ones sink out of the screen and the new ones rise into place, as a menu's do
/// through a slide.
pub fn link_bar_place(menu: GameMenu, now: Millis) -> (LinkActions, u32) {
    let to = LinkActions::of(menu);
    match LinkActions::before(menu) {
        Some((from, since)) => {
            let swap = action_swap(progress(since, now));
            (if swap.new { to } else { from }, swap.sink)
        }
        None => (to, 0),
    }
}

/// The link screen's bar: each pair of actions uploaded, and the hints between picking's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinkBar {
    pub sets: [BarSet; 6],
    pub hints: Option<TexId>,
}

/// Y on the link screen, for a game that can link online: nearby or online.
pub const VIA: Hint<'static> = Hint {
    key: "Y",
    word: "VIA",
};

/// ◀▶ on the link screen: the lit row's value changes.
pub const CHANGE: Hint<'static> = Hint {
    key: "◀▶",
    word: "CHANGE",
};

/// Y on the ROOMS panel: the lobby asked for its rooms again.
pub const REFRESH: Hint<'static> = Hint {
    key: "Y",
    word: "REFRESH",
};

/// SELECT on the link screen: it switches between the cable and the wireless adapter.
pub const MODE: Hint<'static> = Hint {
    key: "SELECT",
    word: "MODE",
};

impl App {
    /// What the LINK panel shows, while the link screen is open.
    pub fn link_view(&self) -> Option<LinkView> {
        let menu = self.game_menu?;
        let online = self.online_available().then_some(self.link_online);
        let netplay = self.via_netplay();
        let status = match menu {
            GameMenu::Working { step, .. } if netplay => Some(step.line()),
            GameMenu::Working { step, .. } => Some(step.shown(self.radio.warmed()).line()),
            GameMenu::Linked { .. } => Some("Linked"),
            GameMenu::Failed { fail, .. } => fail.shown().map(|_| fail.line()),
            GameMenu::Rooms { lit } => self.rooms_status(lit),
            GameMenu::Pick(_) | GameMenu::Unplug { .. } => None,
        };
        let detail = match menu {
            GameMenu::Linked { .. } => self.link_peer.as_ref().map(|p| format!("With {p}")),
            GameMenu::Working { role, .. } if netplay => Some(match role {
                LinkRow::Host if self.going_lan() => {
                    format!("As {} on this Wi-Fi", self.online_name())
                }
                LinkRow::Host => format!(
                    "As {} through {}",
                    self.online_name(),
                    crate::options::relay_word(self.state.relay)
                ),
                LinkRow::Join => format!("As {}", self.online_name()),
            }),
            // what the link goes through, while it is picked
            GameMenu::Pick(_) if self.going_online() => Some("Uses RetroArch netplay".into()),
            GameMenu::Pick(_) if self.going_lan() => Some("RetroArch netplay over Wi-Fi".into()),
            _ => None,
        };
        let fields = self.link_field_values();
        let lit = match menu {
            GameMenu::Pick(_) => fields.iter().position(|(f, _)| *f == self.lit_field()),
            _ => None,
        };
        let rooms = match menu {
            GameMenu::Rooms { lit } => Some(RoomsView {
                lines: self
                    .online_rooms
                    .iter()
                    .map(|r| (r.username.clone(), short_game(&r.game_name)))
                    .collect(),
                lit,
                first: lit.saturating_sub(ROOM_ROWS - 1),
            }),
            _ => None,
        };
        Some(LinkView {
            role: role_of(menu),
            kind: self.link_hardware,
            picking: matches!(menu, GameMenu::Pick(_)),
            status,
            online,
            detail,
            rooms,
            fields,
            lit,
        })
    }

    /// The LINK panel's rows: PLAY on top where the game can go online; local, WITH, whom the
    /// link is with; ROLE; then, through RetroArch's netplay, NAME, and PASSWORD for a host, and
    /// online SERVER for a host and ADVANCED for whoever joins; else MODE.
    pub(crate) fn link_shown_fields(&self) -> Vec<LinkField> {
        let role = match self.game_menu {
            Some(menu) => role_of(menu),
            None => self.last_role,
        };
        let mut fields = Vec::new();
        if self.online_possible() {
            fields.push(LinkField::Play);
            if !self.going_online() {
                fields.push(LinkField::With);
            }
        }
        fields.push(LinkField::Role);
        if !self.via_netplay() {
            fields.push(LinkField::Mode);
            return fields;
        }
        fields.push(LinkField::Name);
        match (role, self.going_online()) {
            (LinkRow::Host, true) => fields.extend([LinkField::Password, LinkField::Server]),
            (LinkRow::Host, false) => fields.push(LinkField::Password),
            (LinkRow::Join, true) => {
                fields.push(LinkField::Advanced);
                if self.link_advanced {
                    fields.push(LinkField::Address);
                }
            }
            (LinkRow::Join, false) => {}
        }
        fields
    }

    /// The rows the cursor stops on: all but a MODE that cannot change.
    pub(crate) fn link_fields(&self) -> Vec<LinkField> {
        let switchable = self.seated().is_some_and(|stem| self.link_switchable(stem));
        self.link_shown_fields()
            .into_iter()
            .filter(|f| *f != LinkField::Mode || switchable)
            .collect()
    }

    /// The row under the cursor: the one last on, while it is shown, else the top one.
    pub(crate) fn lit_field(&self) -> LinkField {
        let fields = self.link_fields();
        match fields.contains(&self.link_field) {
            true => self.link_field,
            false => fields.first().copied().unwrap_or(LinkField::Role),
        }
    }

    /// Each row shown, and its value.
    pub(crate) fn link_field_values(&self) -> Vec<(LinkField, RowValue)> {
        let switchable = self.seated().is_some_and(|stem| self.link_switchable(stem));
        let role = match self.game_menu {
            Some(menu) => role_of(menu),
            None => self.last_role,
        };
        self.link_shown_fields()
            .into_iter()
            .map(|f| {
                let value = match f {
                    LinkField::Play => RowValue::Choice(if self.going_online() {
                        "Online"
                    } else {
                        "Local"
                    }),
                    LinkField::With => RowValue::Choice(match self.going_lan() {
                        true => "RetroArch",
                        false => "omega-zero",
                    }),
                    LinkField::Role => RowValue::Choice(role.text()),
                    LinkField::Mode if switchable => {
                        RowValue::Choice(kind_word(self.link_hardware))
                    }
                    LinkField::Mode => RowValue::Said(kind_word(self.link_hardware)),
                    LinkField::Name => RowValue::Name(self.online_name()),
                    LinkField::Password if self.state.online_password.is_empty() => {
                        RowValue::Said("None")
                    }
                    LinkField::Password => RowValue::Name(self.state.online_password.clone()),
                    LinkField::Server => {
                        RowValue::Choice(crate::options::relay_word(self.state.relay))
                    }
                    LinkField::Advanced => RowValue::Setting(self.link_advanced),
                    LinkField::Address | LinkField::RoomPassword
                        if self.state.online_address.is_empty() =>
                    {
                        RowValue::Said("None")
                    }
                    LinkField::Address | LinkField::RoomPassword => {
                        RowValue::Name(self.state.online_address.clone())
                    }
                };
                (f, value)
            })
            .collect()
    }

    /// The link screen's keyboard, while NAME or PASSWORD is being typed.
    pub(crate) fn link_typing_view(&self) -> Option<TypingView<'_>> {
        let (field, keys) = self.link_typing.as_ref()?;
        Some(TypingView {
            prompt: match field {
                LinkField::Name => "Your name in rooms".to_string(),
                LinkField::Address => "Host address, and :port if not 55435".to_string(),
                LinkField::RoomPassword if self.link_refused => {
                    "Wrong password: try again".to_string()
                }
                LinkField::RoomPassword => match &self.pending_join {
                    Some(room) => format!("Password for {}'s room", room.username),
                    None => "The room's password".to_string(),
                },
                _ => "Your room's password, or none".to_string(),
            },
            keys,
            refused: self.link_refused,
        })
    }

    /// The keys between picking's actions: ◀▶ CHANGE on a row with a choice, and Y REFRESH over
    /// the rooms.
    pub fn link_hints(&self) -> Vec<Hint<'static>> {
        match self.game_menu {
            Some(GameMenu::Rooms { .. }) => vec![REFRESH],
            // the rows move and change as a settings list's do: ◀▶ on one with a choice
            _ if self.lit_field().typed() => Vec::new(),
            _ => vec![CHANGE],
        }
    }

    /// The actions shown over `menu`: EDIT in place of LINK while the cursor is on a row A
    /// types on.
    fn shown_actions(&self, actions: LinkActions) -> LinkActions {
        let picking = matches!(self.game_menu, Some(GameMenu::Pick(_)));
        match actions {
            LinkActions::Pick if picking && self.lit_field().typed() => LinkActions::Edit,
            actions => actions,
        }
    }

    /// The link screen's actions shown now, and how far they are sunk.
    pub fn link_bar_shown(&self) -> Option<(LinkActions, u32)> {
        self.game_menu.map(|menu| {
            let (actions, sink) = link_bar_place(menu, self.now());
            (self.shown_actions(actions), sink)
        })
    }

    pub fn set_link_sprites(&mut self, sprites: LinkSprites) {
        self.link_sprites = Some(sprites);
    }

    pub fn set_link_panel(&mut self, panel: Lone) {
        self.link_panel = Some(panel);
    }

    pub fn set_link_bar(&mut self, bar: LinkBar) {
        self.link_bar = Some(bar);
    }

    /// The link screen over the game: the theme's backdrop, the art, the LINK panel over the top
    /// of the cable, and the bar.
    pub(crate) fn draw_game_menu(&self, menu: GameMenu, out: &mut Vec<Draw>) {
        self.draw_background(out);
        // the keyboard's modal stands on the backdrop, as it does in Settings
        if let Some(t) = self.link_typing_view() {
            return self.draw_keyboard(&t, out);
        }
        let panel = self.link_panel.map(|p| (p, panel_at(&p)));
        if let Some(sprites) = &self.link_sprites {
            let cable_top = panel.map_or(0, |(_, [_, y])| y);
            draw_link_art(
                menu,
                self.link_hardware,
                self.now(),
                sprites,
                cable_top,
                out,
            );
        }
        if let Some((p, [x, y])) = panel {
            out.push(quad(p.tex, p.w, p.h, px(x), px(y)));
        }
        self.draw_link_bar(menu, out);
    }

    fn draw_link_bar(&self, menu: GameMenu, out: &mut Vec<Draw>) {
        let Some(bar) = &self.link_bar else {
            return;
        };
        let (actions, sink) = link_bar_place(menu, self.now());
        let actions = self.shown_actions(actions);
        let set = &bar.sets[actions.index()];
        let h = out_h();
        if actions == LinkActions::Pick {
            if let Some(tex) = bar.hints {
                out.push(set.layout.hints_quad(tex, h));
            }
        }
        for ((tex, w, th), [x, y]) in [(set.left, set.layout.left), (set.right, set.layout.right)] {
            let y = BarLayout::top(h) + px(y + sink as i32);
            out.push(quad(tex, w, th, px(x), y));
        }
    }
}
