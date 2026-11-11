use std::collections::HashMap;
use std::path::{Path, PathBuf};

use omega_gfx::{out_h, out_w};
use omega_input::{Action, Btn, Keys, MUTE_CHORD_MS};
use omega_power::{Battery, Charge, LedState, LidPolicy, Power};
use omega_retro::LinkChannel;
use omega_store::{
    format_stamp, read_app_state, scan, AppState, BackdropStyle, Cart, Core, Platform, StateEntry,
    StateRing, Theme, BLUE_LIGHT_MAX, BRIGHTNESS_MAX, VOLUME_MAX,
};
use omega_ui::kit::action::Font;
use omega_ui::kit::fade::stepped;
use omega_ui::{
    backdrop_shift, board_from, board_zoom, draw_backdrop, draw_pattern, draw_solid, ease, grown,
    lid_of, lift_of, model_box, model_cart_at, on_board, posed_cart, posed_face, push_cart_side,
    sockets_of, CartDive, CartMeshes, CartModel, ChromeColours, ClockPicker, CornerBadge, Draw,
    FfState, GaugeIcon, GbShell, Hud, HudKind, KeyboardFaces, LinkBadge, LinkFaces, Links, Millis,
    Placed, Polaroids, PowerChoice, Refusal, SettingsFaces, SettingsRow, Shelf, Sockets, TexId,
    Toast, GBA_SOCKETS, HOP_LIFT, LID_SHADOW_H, LID_SHADOW_W, SHADOW_AT, SHADOW_H, SPRITE_PX,
    TOP_BAR_H, TURN_PAD,
};

use crate::audio::Sfx;
use crate::background_screen::{Backdrop, BackgroundScreen};
use crate::bluetooth::bluetooth_jobs;
use crate::bluetooth_screen::{Bluetooth, BluetoothScreen};
use crate::boot_fade::BootFade;
use crate::core_picker::{Chip, CorePicker, Outcome, Press};
use crate::disc_screen::{DiscBar, DiscScreen};
use crate::favorites::{favorite_carts, read_favorites, read_recents, ShelfKind};
use crate::label_screen::{Labels, LabelsScreen};
use crate::label_sync::{label_jobs, Account, LabelJobs, NoKey, Want};
use crate::link_kind::{link_carried, link_kind, serial_option, LinkKind};
use crate::link_radio::{radio_jobs, LinkRole, RadioJob, RadioJobs};
use crate::link_screen::{LinkBar, LinkSprites};
use crate::link_start::{link_port, LinkFail, LinkProgress, LinkStarter, LinkStep};
use crate::nav::{even, offsets, Dir, End, Offsets, Slide, SLIDE_MS};
use crate::opening::Opening;
use crate::options::OptionsScreen;
use crate::pause_menu::{PauseMenu, PendingUndo};
use crate::persist::{self, Snapshot};
use crate::screens::bar::BarFaces;
use crate::screens::clock::clock_screen;
use crate::screens::info::InfoScreen;
use crate::screens::keyboard::typing_of;
use crate::screens::lone::Lone;
use crate::screens::power::{PowerScreen, PowerSlide};
use crate::ssh::{ssh_server, Ssh};
use crate::start_notice::StartNotice;
use crate::video_mode::{self, VideoMode};
use crate::wifi::{wifi_jobs, WifiJobs};
use crate::wifi_screen::{Summary, Watch, Wifi, WifiScreen};

mod link;
mod saves;

pub const INSERT_S: f32 = 0.88;
const INSERT_HOLD_S: f32 = 0.28;

pub const SEATED_AT: f32 = INSERT_S - INSERT_HOLD_S;

pub const EJECT_S: f32 = SEATED_AT;

/// How far into its way in a UMD waits while its disc spins up: at the top of its rise.
const LOADING_HOLD: f32 = 0.2;

/// How a UMD's disc spins in its window as its game loads: up from still the way a drive spins
/// one up, slowly off the mark, then faster and faster, to `DISC_SPIN` turns a second, near
/// enough by `DISC_SPIN_UP` seconds. A disc that fast is a blur to the eye, and is drawn as one.
/// The UMD is held up at the top of its rise until it has spun at full speed for a moment,
/// `DISC_SPUN` seconds from the press, and its game opens only once it is all the way in and
/// the screen is dark, so the core's opening never stalls a frame of the spin.
const DISC_SPIN: f32 = 20.0;
const DISC_SPIN_UP: f32 = 1.6;
const DISC_SPUN: f32 = DISC_SPIN_UP + 0.4;

const EJECT_HOLD_S: f32 = 0.2;

const POWER_ON_S: f32 = 0.6;

const POWER_OFF_S: f32 = 0.5;

const VOLUME_STEP: u8 = 5;

const AUTOSAVE_MS: Millis = 60_000;

const BATTERY_CRITICAL: u8 = 5;

const BATTERY_POLL_MS: Millis = 10_000;

const CHARGE_POLL_MS: Millis = 1_000;
const HEADPHONES_POLL_MS: Millis = 500;

const BATTERY_LOW: u8 = 20;

pub const LINK_LOST_MS: Millis = 2000;

const PLAY_HOLD_MS: Millis = 500;

/// How long the screen after the power menu's choice is drawn before omega-zero may restart or
/// power off: its slide in, and a few frames at rest, so the frame left on the panel while the
/// platform takes the handheld down is the still one.
const SHUTDOWN_SHOW_MS: Millis = SLIDE_MS + 120;

/// How long the shelf's pill takes to rise into place with a new system or letter.
const PILL_IN_MS: Millis = 220;
/// How long a letter jumped to stays in the pill before the system comes back.
const LETTER_SHOWN_MS: Millis = 2200;
/// The core picker's lid, the cart's own face, turned `turn` as it lifts and spun `yaw` as it
/// leaves the shelf. Unspun, it is drawn exactly as it was before carts swayed.
fn push_lid(out: &mut Vec<Draw>, at: (f32, f32, f32, f32), tex: TexId, turn: f32, yaw: f32) {
    let (x, y, w, h) = at;
    push_cart_side(out, at, tex, turn, yaw, 1.0, 1.0);
    out.push(match yaw == 0.0 {
        true => Draw::Turned {
            x,
            y,
            w,
            h,
            tex,
            alpha: 1.0,
            turn,
        },
        false => posed_face(at, tex, 1.0, turn, yaw),
    });
}

/// How far under the resting lid's bottom edge the middle of its shadow falls.
const LID_SHADOW_DROP: f32 = 29.0;
const FACES_WAIT_MS: Millis = 1500;

const ALERT_HOLD: f32 = 0.45;
const ALERT_GONE: f32 = 0.9;

const CLOCK_FLOOR: i64 = 1_577_836_800;

struct LinkSession {
    client_id: u16,
    lost_at: Option<Millis>,
}

struct LinkStarting {
    starter: LinkStarter,
    client_id: u16,
}

struct Reload {
    stem: String,
    role: LinkRow,
    cancelled: bool,
    from: LinkKind,
    from_serial: &'static str,
    fallback: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GameMenu {
    Pick(LinkRow),
    /// The rooms open online, the lit one's place in the list.
    Rooms {
        lit: usize,
    },
    Working {
        role: LinkRow,
        step: LinkStep,
        since: Millis,
    },
    Linked {
        role: LinkRow,
        worked: Millis,
        since: Millis,
        opened: bool,
    },
    Failed {
        role: LinkRow,
        fail: LinkFail,
        worked: Millis,
        since: Millis,
    },
    Unplug {
        role: LinkRow,
        since: Millis,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LinkRow {
    Host,
    Join,
}

impl LinkRow {
    pub const ALL: [LinkRow; 2] = [LinkRow::Host, LinkRow::Join];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn text(self) -> &'static str {
        match self {
            LinkRow::Host => "Host",
            LinkRow::Join => "Join",
        }
    }

    pub fn role(self) -> LinkRole {
        match self {
            LinkRow::Host => LinkRole::Host,
            LinkRow::Join => LinkRole::Join,
        }
    }

    pub fn client_id(self) -> u16 {
        match self {
            LinkRow::Host => 0,
            LinkRow::Join => 1,
        }
    }

    pub fn other(self) -> LinkRow {
        match self {
            LinkRow::Host => LinkRow::Join,
            LinkRow::Join => LinkRow::Host,
        }
    }

    pub fn from_client_id(id: u16) -> LinkRow {
        if id == 0 {
            LinkRow::Host
        } else {
            LinkRow::Join
        }
    }
}

pub const LINKED_HOLD_MS: Millis = 1000;

pub const UNPLUG_HOLD_MS: Millis = 420;

#[derive(Clone, Debug)]
pub enum Phase {
    SetClock {
        picker: ClockPicker,
        seed: i64,
        from_menu: bool,
    },
    Shelf,
    /// The settings START opens on the shelf, with `row` lit.
    Settings {
        row: SettingsRow,
    },
    /// What the buttons do, on the shelf and in a game, opened with a SELECT tap on the shelf.
    Controls,
    Inserting {
        cart: String,
        t: f32,
        core_ready: bool,
        resumed: bool,
        clean: bool,
    },
    Playing {
        cart: String,
    },
    Ejecting {
        cart: String,
        t: f32,
    },
    Polaroids {
        cart: String,
    },
    About,
    Network(WifiScreen),
    /// Bluetooth and its devices, opened from Settings.
    Bluetooth(BluetoothScreen),
    /// The panel Y opened on one of Network's rows: Wi-Fi, network time or SSH.
    Info(InfoScreen),
    Labels(LabelsScreen),
    Background(BackgroundScreen),
    /// The GAMEPLAY or SHELF panel, opened from Settings.
    Options(OptionsScreen),
    Doze {
        cart: Option<String>,
    },
}

/// What the bar across the top prints beside its icons: the time, the battery's figure, and the
/// bolt shown while the handheld charges, baked in its ink. The save-state switcher shows the
/// same bar.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Readouts {
    pub clock: omega_ui::Printed,
    pub percent: omega_ui::Printed,
    pub bolt: Option<TexId>,
}

pub struct App {
    pub(crate) phase: Phase,
    /// FAVORITES first, then each system's shelf, in `Platform::ALL`'s order. A shelf with
    /// nothing on it is passed over.
    pub(crate) shelves: Vec<(ShelfKind, Shelf)>,
    /// The carts starred with Y, as Config/favorites.txt lists them.
    pub(crate) favorites: Vec<(Platform, String)>,
    /// The menu a tap of MENU opens over a game, as on Slot.
    pub(crate) pause_menu: Option<PauseMenu>,
    /// The MENU panel, once baked.
    pub(crate) pause_panel: Option<Lone>,
    /// The save or load the menu's Undo can take back, and when it was made.
    pub(crate) pending_undo: Option<(PendingUndo, Millis)>,
    pub(crate) shelf_at: usize,
    pub(crate) play_held: Option<Millis>,
    pub(crate) refusal: Option<Refusal>,
    pub(crate) refused_from: Option<f32>,
    pub(crate) alert_face: Option<TexId>,
    /// The disc of the UMD going in, by its game's stem, uploaded to spin in its window while
    /// the game loads.
    spin_disc: Option<(String, TexId, TexId, TexId)>,
    /// How long the UMD going in has spun, in seconds, and how far it has turned, in radians.
    pub(crate) spin: (f32, f32),
    /// What holding POWER has put over the screen: its menu, or the screen after its choice.
    pub(crate) power_screen: Option<PowerScreen>,
    /// The slide to or from a power screen under way.
    pub(crate) power_slide: Option<PowerSlide>,
    /// The POWER menu's panel, and the one shown after its choice, once baked.
    pub(crate) power_menu_panel: Option<Lone>,
    pub(crate) going_panel: Option<Lone>,
    pub(crate) core_picker: Option<CorePicker>,
    pub(crate) core_board_face: Option<TexId>,
    pub(crate) core_lid_face: Option<TexId>,
    pub(crate) core_faces_stem: Option<String>,
    pub(crate) core_socket_faces: Vec<TexId>,
    pub(crate) core_chip_faces: Vec<TexId>,
    pub(crate) core_blank_chip_face: Option<TexId>,
    pub(crate) core_chip_shadow_face: Option<TexId>,
    /// The blank chip and its shadow for each board whose sockets are not the GBA's.
    pub(crate) core_chip_faces_for: Vec<(Sockets, TexId, TexId)>,
    pub(crate) core_lid_shadow_face: Option<TexId>,
    pub(crate) game_menu: Option<GameMenu>,
    /// The DISC screen SELECT + MENU opens over a PlayStation game on several discs.
    pub(crate) disc_screen: Option<DiscScreen>,
    /// The game a disc was swapped into the drive of, and that disc, counting from 0.
    pub(crate) disc_in: Option<(String, usize)>,
    /// A disc swapped in that the core has yet to be handed.
    pub(crate) disc_swap: Option<usize>,
    /// The DISC screen's panel, and its bar.
    pub(crate) disc_panel: Option<Lone>,
    pub(crate) disc_bar: Option<DiscBar>,
    pub(crate) link_sprites: Option<LinkSprites>,
    pub(crate) link_hardware: LinkKind,
    pub(crate) last_role: LinkRow,
    pub(crate) link_choices: HashMap<String, LinkKind>,
    pub(crate) link_loaded: Option<&'static str>,
    pub(crate) link_reload: Option<(String, &'static str)>,
    reload: Option<Reload>,
    /// The link screen's LINK panel, and its bar.
    pub(crate) link_panel: Option<Lone>,
    pub(crate) link_bar: Option<LinkBar>,
    starting: Option<LinkStarting>,
    pub(crate) link_transport: Option<(u16, Box<dyn LinkChannel>)>,
    /// The other player's name, when an online link gave one.
    pub(crate) link_peer: Option<String>,
    /// The link screen goes online rather than nearby, for a game that can.
    pub(crate) link_online: bool,
    /// The rooms open online, as the lobby last listed them, and their listing under way.
    pub(crate) online_rooms: Vec<crate::netplay_lobby::Room>,
    pub(crate) rooms_fetch: Option<crate::link_start::RoomsFetch>,
    pub(crate) rooms_failed: bool,
    /// The room picked to join.
    pub(crate) online_room: Option<crate::netplay_lobby::Room>,
    /// The LINK panel's row under the cursor, and the keyboard up over it for NAME or PASSWORD.
    pub(crate) link_field: crate::link_screen::LinkField,
    pub(crate) link_typing: Option<(crate::link_screen::LinkField, omega_ui::Keyboard)>,
    /// Save Backup's thread, what its panels are doing, and whether a network was joined at
    /// the last frame, for a pass once one is.
    pub(crate) save_backup: Option<crate::save_backup::SaveBackup>,
    pub(crate) backup_ui: crate::backup_settings::BackupUi,
    pub(crate) backup_joined: bool,
    /// The passes that copied and failed, as last toasted, and whether a failure was told of
    /// since the last copy: a failure is told once, until saves copy again.
    pub(crate) backup_seen: (u64, u64),
    pub(crate) backup_fail_told: bool,
    /// When Save Backup last asked Wi-Fi to join again, for saves waiting on a network.
    pub(crate) backup_woke_at: Option<Millis>,
    /// The stick a game's menu set, for the session's pad to take.
    pub(crate) stick_pending: Option<crate::input::Stick>,
    /// The keyboard asks again for a room's password, the one given having been turned away.
    pub(crate) link_refused: bool,
    /// Local, the link goes to RetroArch on this network rather than to another handheld.
    pub(crate) link_retroarch: bool,
    /// The LINK panel shows its ADVANCED rows.
    pub(crate) link_advanced: bool,
    /// The room being joined, kept to ask for its password again, and the password given.
    pub(crate) pending_join: Option<crate::netplay_lobby::Room>,
    pub(crate) join_password: String,
    pub(crate) restarting: bool,
    pub(crate) act_at: Millis,
    pub(crate) root: Option<PathBuf>,
    pub(crate) state: AppState,
    pub(crate) vol_before: Vec<(u8, bool, Millis)>,
    /// Bluetooth headphones' own volume as last set from here, and as last read from them.
    pub(crate) bt_volume_sent: Option<u8>,
    pub(crate) bt_volume_seen: Option<u8>,
    /// Bluetooth headphones connected without their sound, since when, and when they were last
    /// asked to connect it.
    pub(crate) bt_audio_wait: Option<(String, Millis, Option<Millis>)>,
    /// How many times the headphones connected now were asked to connect their sound.
    pub(crate) bt_audio_asks: u8,
    /// Headphones connected whose own volume is being set before sound goes to them, and since
    /// when.
    pub(crate) bt_volume_wait: Option<(String, Millis)>,
    /// Sound went to Bluetooth headphones at the last tick.
    pub(crate) bt_was_on: bool,
    /// The check for an update as Wi-Fi first joins a network: since when it has been joined,
    /// whether the check was made, and whether an update found was told of.
    pub(crate) update_online_since: Option<Millis>,
    pub(crate) update_checked: bool,
    pub(crate) update_told: bool,
    pub(crate) snapshot: Option<Box<dyn Snapshot>>,
    pub(crate) core: Core,
    pub(crate) colour_pending: Option<bool>,
    pub(crate) link_player: Option<u8>,
    pub(crate) platform: Platform,
    pub(crate) named_core: bool,
    pub(crate) video_mode: VideoMode,
    link: Option<LinkSession>,
    pub(crate) sfx: Option<Sfx>,
    /// The shelf and the cart picked as the menus' sounds last saw them, for the tick as the
    /// pick moves.
    pub(crate) sound_pick: Option<(usize, usize)>,
    pub(crate) polaroids: Option<Polaroids>,
    /// The switcher asking whether to delete the save it shows, that question's panel, and the
    /// X DELETE line between RETURN and LOAD.
    pub(crate) delete_asked: bool,
    pub(crate) delete_panel: Option<Lone>,
    pub(crate) delete_hint: Option<Lone>,
    pub(crate) clock_panel: Option<Lone>,
    pub(crate) settings_faces: Option<SettingsFaces>,
    pub(crate) about_modal: Option<Lone>,
    pub(crate) wallpaper: Option<TexId>,
    /// The top bar's clock, figure and bolt, baked in its ink.
    pub(crate) top_readouts: Readouts,
    pub(crate) shelf_platform: omega_ui::Printed,
    pub(crate) name_pending: bool,
    /// A game's start took omega-zero down last time: the boot leaves it on the shelf, or the
    /// seated cart where that game is not there, rather than resume either.
    hold_resume: bool,
    /// The letter jumped to with Up or Down, shown in the pill for a while in place of the
    /// system.
    pub(crate) pill_letter: Option<char>,
    /// When the pill rose with what it shows now.
    pub(crate) shelf_named: Option<Millis>,
    pub(crate) hud: Hud,
    pub(crate) screen: f32,
    pub(crate) game_ready: bool,
    pub(crate) clock: f64,
    pub(crate) power: Option<Power>,
    pub(crate) dozed_at: Millis,
    pub(crate) woke_on_press: bool,
    pub(crate) autosave_at: Millis,
    pub(crate) pending_save: Option<std::thread::JoinHandle<()>>,
    /// The game whose save that is.
    pending_for: Option<(Platform, String)>,
    pub(crate) battery_at: Millis,
    pub(crate) charge_at: Millis,
    pub(crate) headphones: bool,
    pub(crate) headphones_at: Millis,
    pub(crate) battery: Option<Battery>,
    pub(crate) last_led: Option<LedState>,
    pub(crate) powering_off: bool,
    pub(crate) radio: Box<dyn RadioJobs>,
    pub(crate) wifi: Wifi,
    pub(crate) ssh: Ssh,
    pub(crate) network_faces: Option<omega_ui::network::NetworkFaces>,
    pub(crate) info_faces: Option<omega_ui::network::InfoFaces>,
    pub(crate) controls_panel: Option<Lone>,
    pub(crate) keyboard_faces: Option<KeyboardFaces>,
    pub(crate) link_faces: Option<LinkFaces>,
    pub(crate) bluetooth: Bluetooth,
    /// The channel to the headphones connected, AirPods or Galaxy Buds, for their noise
    /// control and battery.
    pub(crate) phones: crate::headphones::Keeper,
    /// The Bluetooth modal's faces, laid out as Network's.
    pub(crate) bluetooth_faces: Option<omega_ui::network::NetworkFaces>,
    pub(crate) labels: Labels,
    pub(crate) labels_panel: Option<Lone>,
    /// Carts whose labels arrived and whose faces are still drawn without them.
    pub(crate) stale_faces: Vec<(Platform, String)>,
    pub(crate) background_panel: Option<Lone>,
    /// The pattern the shelf is drawn over, the width of its picture, and what it was drawn
    /// as, so a stale one is not shown while the new one is made.
    pub(crate) backdrop_face: Option<(TexId, u32, BackdropStyle, u8)>,
    /// The move between screens under way, while it slides.
    pub(crate) slide: Option<Slide>,
    /// The card's Config/theme.txt, which the shelf's chrome is drawn in when there is one.
    pub(crate) theme: Option<Theme>,
    /// The bar along the bottom: its actions and the hints between them.
    pub(crate) bar_faces: Option<BarFaces>,
    /// The boot logo over the shelf while it fades out at boot.
    pub(crate) boot_fade: Option<BootFade>,
    /// The opening over the shelf while it plays at boot.
    pub(crate) opening: Option<Opening>,
    /// The notice that the last start of a game took omega-zero down, from the boot that found
    /// it noted until it is put away.
    pub(crate) start_notice: Option<StartNotice>,
    /// The games put in last, the latest first, as Config/recents.txt lists them.
    pub(crate) recents: Vec<(Platform, String)>,
    /// The GAMEPLAY or SHELF panel, once baked.
    /// Each options panel as last baked, by its title.
    pub(crate) options_panels: Vec<(&'static str, Lone)>,
    /// Each system's games switched off its default core by hand, as EMULATORS last counted.
    pub(crate) switched: Vec<(Platform, Vec<String>)>,
    /// Settings' Update: omega-zero fetching its latest release.
    pub(crate) updater: crate::update::Updater,
    /// Frame blending or the Game Boy's palette changed, for the game in play to take.
    pub(crate) core_options_pending: bool,
    /// The picked cart's name over the shelf, baked, with its size in art pixels.
    pub(crate) title_face: Option<(TexId, u32, u32)>,
    /// Where signing in to RetroAchievements stands.
    pub(crate) cheevos: crate::achievements::Cheevos,
}

fn shelves_of(carts: Vec<Cart>, favorites: &[(Platform, String)]) -> Vec<(ShelfKind, Shelf)> {
    let starred = favorite_carts(&carts, favorites);
    let mut rows: Vec<(Platform, Vec<Cart>)> =
        Platform::ALL.iter().map(|p| (*p, Vec::new())).collect();
    for cart in carts {
        if let Some((_, row)) = rows.iter_mut().find(|(p, _)| *p == cart.platform) {
            row.push(cart);
        }
    }
    [
        (ShelfKind::Recents, Shelf::new(Vec::new())),
        (ShelfKind::Favorites, Shelf::new(starred)),
    ]
    .into_iter()
    .chain(
        rows.into_iter()
            .map(|(platform, carts)| (ShelfKind::System(platform), Shelf::new(carts))),
    )
    .collect()
}

impl App {
    pub fn new(carts: Vec<Cart>) -> Self {
        App::with_favorites(carts, Vec::new())
    }

    /// The shelves of `carts`, the ones of them in `favorites` on FAVORITES too.
    pub fn with_favorites(carts: Vec<Cart>, favorites: Vec<(Platform, String)>) -> Self {
        let shelves = shelves_of(carts, &favorites);
        let shelf_at = shelves
            .iter()
            .position(|(_, s)| !s.carts.is_empty())
            .unwrap_or(0);
        App {
            radio: radio_jobs(),
            wifi: Wifi::new(wifi_jobs()),
            ssh: Ssh::new(ssh_server()),
            network_faces: None,
            info_faces: None,
            controls_panel: None,
            keyboard_faces: None,
            link_faces: None,
            bluetooth: Bluetooth::new(bluetooth_jobs()),
            phones: Default::default(),
            bluetooth_faces: None,
            labels: Labels::new(Box::new(NoKey::default()), None),
            labels_panel: None,
            stale_faces: Vec::new(),
            background_panel: None,
            backdrop_face: None,
            slide: None,
            theme: None,
            bar_faces: None,
            boot_fade: None,
            opening: None,
            start_notice: None,
            recents: Vec::new(),
            options_panels: Vec::new(),
            switched: Vec::new(),
            updater: Default::default(),
            core_options_pending: false,
            title_face: None,
            cheevos: Default::default(),
            phase: Phase::Shelf,
            shelves,
            favorites,
            pause_menu: None,
            pause_panel: None,
            pending_undo: None,
            shelf_at,
            play_held: None,
            refusal: None,
            refused_from: None,
            alert_face: None,
            spin_disc: None,
            spin: (0.0, 0.0),
            power_screen: None,
            power_slide: None,
            power_menu_panel: None,
            going_panel: None,
            core_picker: None,
            core_board_face: None,
            core_lid_face: None,
            core_faces_stem: None,
            core_socket_faces: Vec::new(),
            core_chip_faces: Vec::new(),
            core_blank_chip_face: None,
            core_chip_shadow_face: None,
            core_chip_faces_for: Vec::new(),
            core_lid_shadow_face: None,
            game_menu: None,
            disc_screen: None,
            disc_in: None,
            disc_swap: None,
            disc_panel: None,
            disc_bar: None,
            link_sprites: None,
            link_hardware: LinkKind::Cable,
            last_role: LinkRow::Host,
            link_choices: HashMap::new(),
            link_loaded: None,
            link_reload: None,
            reload: None,
            link_panel: None,
            link_bar: None,
            starting: None,
            link_transport: None,
            link_peer: None,
            link_online: false,
            online_rooms: Vec::new(),
            rooms_fetch: None,
            rooms_failed: false,
            online_room: None,
            link_field: crate::link_screen::LinkField::Role,
            link_typing: None,
            save_backup: None,
            backup_ui: Default::default(),
            backup_joined: false,
            backup_seen: (0, 0),
            backup_fail_told: false,
            backup_woke_at: None,
            stick_pending: None,
            link_refused: false,
            link_retroarch: false,
            link_advanced: false,
            pending_join: None,
            join_password: String::new(),
            restarting: false,
            act_at: 0,
            root: None,
            state: AppState::default(),
            vol_before: Vec::new(),
            bt_volume_sent: None,
            bt_volume_seen: None,
            bt_audio_wait: None,
            bt_audio_asks: 0,
            bt_volume_wait: None,
            bt_was_on: false,
            update_online_since: None,
            update_checked: false,
            update_told: false,
            snapshot: None,
            core: Core::default(),
            colour_pending: None,
            link_player: None,
            platform: Platform::default(),
            named_core: false,
            video_mode: VideoMode::default(),
            link: None,
            sfx: None,
            sound_pick: None,
            polaroids: None,
            delete_asked: false,
            delete_panel: None,
            delete_hint: None,
            clock_panel: None,
            settings_faces: None,
            about_modal: None,
            wallpaper: None,
            top_readouts: Readouts::default(),
            shelf_platform: omega_ui::Printed::default(),
            name_pending: false,
            hold_resume: false,
            pill_letter: None,
            shelf_named: None,
            hud: Hud::new(),
            screen: 0.0,
            game_ready: false,
            clock: 0.0,
            power: None,
            dozed_at: 0,
            woke_on_press: false,
            autosave_at: AUTOSAVE_MS,
            pending_save: None,
            pending_for: None,
            battery_at: BATTERY_POLL_MS,
            charge_at: CHARGE_POLL_MS,
            headphones: false,
            headphones_at: 0,
            battery: None,
            last_led: None,
            powering_off: false,
        }
    }

    pub fn boot(root: &Path) -> Self {
        crate::root::ensure(root);
        omega_ui::set_theme(Theme::read(root));
        let mut app = App::with_favorites(scan(root).unwrap_or_default(), read_favorites(root));
        app.root = Some(root.to_path_buf());
        app.theme = Theme::read_override(root);
        app.state = read_app_state(root);
        crate::time_zone::follow(app.state.utc_offset_min);
        app.show_holo();
        app.wifi.start(app.state.wifi);
        app.ssh.start(app.state.ssh, Some(root));
        app.bluetooth.start(app.state.bluetooth);
        app.labels = Labels::new(label_jobs(root), Account::load(root));
        app.start_achievements();
        app.start_save_backup();
        app.recents = read_recents(root);
        app.refresh_recents();
        app.refresh_marks();
        // RECENTS, when shown, is the shelf a boot comes up on, as on Slot
        if app.state.show_recents && !app.shelves[0].1.carts.is_empty() {
            app.shelf_at = 0;
        }
        if let Some(fell) = persist::take_starting(root) {
            crate::log!("{fell} took omega-zero down as it started; it is not resumed");
            app.hold_resume = true;
            app.start_notice = StartNotice::of(root, &fell);
        }
        if app.state.clock_set {
            app.start();
        } else {
            app.phase = clock_screen(system_secs(), 0, false);
        }
        app
    }

    pub(crate) fn start(&mut self) {
        // the game whose start took omega-zero down is the one left on the shelf; a notice
        // naming a game not on the shelf would be wrong to say it was left there
        let fell = self
            .start_notice_names()
            .and_then(|f| self.seat_of(&f.stem, Some(f.platform)));
        if fell.is_none() {
            self.start_notice = None;
        }
        let seated = if fell.is_some() {
            fell
        } else if self.single_cart() {
            Some((self.shelf_at, 0))
        } else {
            let stem = self.state.cart.clone();
            let platform = self.state.cart_platform;
            stem.and_then(|stem| self.seat_of(&stem, platform))
        };
        self.phase = Phase::Shelf;
        let held = std::mem::take(&mut self.hold_resume);
        match seated {
            Some((at, i)) if held => {
                self.shelf_at = at;
                self.shelf_mut().select(i);
                self.state.cart = None;
                self.state.cart_platform = None;
                self.name_pending = true;
            }
            Some((at, i)) => {
                self.shelf_at = at;
                self.shelf_mut().select(i);
                self.insert(false);
                if let Phase::Inserting { resumed, t, .. } = &mut self.phase {
                    *resumed = true;
                    *t = INSERT_S;
                }
            }
            None => {
                self.state.cart = None;
                self.state.cart_platform = None;
                self.name_pending = true;
                self.pick_last();
            }
        }
    }

    /// The key a shelf is kept by: `recents`, `favorites`, or its system's.
    fn shelf_key(kind: ShelfKind) -> String {
        match kind {
            ShelfKind::Recents => "recents".to_string(),
            ShelfKind::Favorites => "favorites".to_string(),
            ShelfKind::System(p) => p.dir_name().to_ascii_lowercase(),
        }
    }

    /// Keeps the shelf and the cart picked, for the next boot to pick again.
    pub(crate) fn remember_pick(&mut self) {
        let kind = self.shelves[self.shelf_at].0;
        let cart = self.shelf().carts.get(self.shelf().index);
        let (stem, platform) = match cart {
            Some(c) => (c.stem.clone(), Some(c.platform)),
            None => (String::new(), None),
        };
        let shelf = Self::shelf_key(kind);
        let s = &mut self.state;
        if s.shelf == shelf && s.picked == stem && s.picked_platform == platform {
            return;
        }
        s.shelf = shelf;
        s.picked = stem;
        s.picked_platform = platform;
        self.persist();
    }

    /// The shelf and the cart picked when omega-zero last went off, picked again: on the shelf
    /// it was on, else on its system's, the shelf staying as it is when the cart is gone.
    fn pick_last(&mut self) {
        let (key, stem, platform) = (
            self.state.shelf.clone(),
            self.state.picked.clone(),
            self.state.picked_platform,
        );
        if stem.is_empty() {
            return;
        }
        let holds = |shelf: &Shelf| {
            shelf
                .carts
                .iter()
                .position(|c| c.stem == stem && platform.is_none_or(|p| c.platform == p))
        };
        let on_kept = self
            .shelves
            .iter()
            .position(|(kind, _)| Self::shelf_key(*kind) == key)
            .and_then(|at| holds(&self.shelves[at].1).map(|i| (at, i)));
        let on_system = || {
            self.shelves
                .iter()
                .enumerate()
                .find_map(|(at, (kind, shelf))| {
                    matches!(kind, ShelfKind::System(_))
                        .then(|| holds(shelf))
                        .flatten()
                        .map(|i| (at, i))
                })
        };
        if let Some((at, i)) = on_kept.or_else(on_system) {
            self.shelf_at = at;
            self.shelf_mut().select(i);
        }
    }

    pub(crate) fn shelf(&self) -> &Shelf {
        &self.shelves[self.shelf_at].1
    }

    pub(crate) fn shelf_mut(&mut self) -> &mut Shelf {
        &mut self.shelves[self.shelf_at].1
    }

    fn switch_shelf(&mut self, by: i32) {
        let Some(to) = self.next_shelf(by) else {
            return;
        };
        self.shelf_mut().release_hold();
        self.play_held = None;
        self.shelf_at = to;
        self.shelf_platform = omega_ui::Printed::default();
        self.pill_letter = None;
        self.shelf_named = None;
        self.name_pending = true;
    }

    /// Moves off a shelf left with nothing on it, as FAVORITES is once its last star is taken
    /// off, to the next shelf that has carts.
    pub(crate) fn leave_empty_shelf(&mut self) {
        self.switch_shelf(1);
    }

    /// R2 on the shelf: a cart other than the one picked, at random, for when nothing comes
    /// to mind.
    fn pick_at_random(&mut self) {
        let n = self.shelf().carts.len();
        if n < 2 {
            return;
        }
        // the clock's milliseconds through a step of xorshift: random enough to pick a game
        let mut x = self.now().wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        x ^= x >> 33;
        x ^= x << 21;
        let here = self.shelf().index;
        let other = (here + 1 + (x % (n as u64 - 1)) as usize) % n;
        self.shelf_mut().glide_to(other);
    }

    fn jump_letter(&mut self, dir: i32) {
        let before = self.shelf().index;
        match dir > 0 {
            true => self.shelf_mut().jump_next_letter(),
            false => self.shelf_mut().jump_prev_letter(),
        }
        if self.shelf().index == before {
            return;
        }
        let Some(letter) = self.selected_stem().map(omega_store::initial) else {
            return;
        };
        if self.pill_letter != Some(letter) {
            self.shelf_platform = omega_ui::Printed::default();
        }
        self.pill_letter = Some(letter);
        self.shelf_named = None;
        self.name_pending = true;
    }

    fn next_shelf(&self, by: i32) -> Option<usize> {
        let n = self.shelves.len() as i32;
        (1..n)
            .map(|step| (self.shelf_at as i32 + by * step).rem_euclid(n) as usize)
            .find(|at| !self.shelves[*at].1.carts.is_empty())
    }

    fn seat_of(&self, stem: &str, platform: Option<Platform>) -> Option<(usize, usize)> {
        self.shelves
            .iter()
            .enumerate()
            .filter(|(_, (kind, _))| match kind.platform() {
                // a cart is seated from its own system's shelf, never from FAVORITES
                None => false,
                Some(p) => platform.is_none_or(|want| p == want),
            })
            .find_map(|(at, (_, shelf))| {
                shelf
                    .carts
                    .iter()
                    .position(|c| c.stem == stem)
                    .map(|i| (at, i))
            })
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub fn wall_secs(&self) -> i64 {
        self.utc_secs() + i64::from(self.state.utc_offset_min) * 60
    }

    pub(crate) fn utc_secs(&self) -> i64 {
        self.power.as_ref().map_or_else(system_secs, |p| p.now())
    }

    pub fn set_cart_shadow(&mut self, face: TexId) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_shadow(face);
        }
    }

    pub fn set_cart_meshes(&mut self, meshes: CartMeshes) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_meshes(meshes);
        }
    }

    /// The black outline that backs a `model` cart not drawn as a model, on every shelf.
    pub fn set_model_shadow(&mut self, model: CartModel, face: TexId) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_model_shadow(model, face);
        }
    }

    /// `model`'s holo sheen, on every shelf: its frames, the light from one end of the label to
    /// the other.
    pub fn set_model_holo(&mut self, model: CartModel, frames: Vec<TexId>) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_model_holo(model, frames.clone());
        }
    }

    /// Has every shelf show the holo sheen, or none, as Labels > Holo Sheen says.
    pub(crate) fn show_holo(&mut self) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_holo_shown(self.state.holo);
        }
    }

    pub fn set_gb_cart_shadows(&mut self, notched: TexId, rounded: TexId) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_gb_shadow(GbShell::Notched, notched);
            shelf.set_gb_shadow(GbShell::Rounded, rounded);
        }
    }

    pub fn set_wallpaper(&mut self, face: TexId) {
        self.wallpaper = Some(face);
    }

    pub fn set_link_faces(&mut self, faces: LinkFaces) {
        self.link_faces = Some(faces);
    }

    /// What the bar across the top shows the handheld connected to: Bluetooth while it is on
    /// (none on a handheld without it), and SSH, with SFTP, while it is switched on and a
    /// network is joined.
    pub fn links(&self) -> Links {
        let joined = self.wifi.status().joined();
        Links {
            wifi: joined,
            bluetooth: self.bluetooth.present().then(|| self.bluetooth.enabled()),
            ssh: joined && self.state.ssh && self.ssh.present(),
        }
    }

    /// The top bar's clock, figure and bolt, baked in `chrome_colours().ink`.
    pub fn set_top_readouts(&mut self, readouts: Readouts) {
        self.top_readouts = readouts;
    }

    pub fn set_shelf_platform_face(&mut self, face: TexId, w: u32) {
        self.shelf_platform = omega_ui::Printed::new(face, w);
    }

    pub fn clear_shelf_platform(&mut self) {
        self.shelf_platform = omega_ui::Printed::default();
    }

    /// What the shelf's pill shows: the letter jumped to, or the shelf's system.
    pub fn pill_text(&self) -> Option<String> {
        match self.pill_letter {
            Some(letter) => Some(letter.to_string()),
            None => self
                .shelves
                .get(self.shelf_at)
                .map(|(kind, _)| kind.pill().to_string()),
        }
    }

    /// How far below its place the shelf's pill is, in art pixels: it rises into place as a
    /// new system or letter is shown, and is out of sight while its face is still to come.
    pub(crate) fn pill_sink(&self) -> u32 {
        let Some(at) = self.shelf_named else {
            return omega_ui::kit::bar::SINK;
        };
        let t = self.now().saturating_sub(at) as f32 / PILL_IN_MS as f32;
        ((1.0 - ease(t.min(1.0))) * omega_ui::kit::bar::SINK as f32).round() as u32
    }

    pub fn shelf_platform_name(&self) -> Option<&'static str> {
        self.next_shelf(1)?;
        Some(self.shelves[self.shelf_at].0.name())
    }

    /// Moves to the screen `to`, the one shown now sliding away as it comes in.
    pub fn go(&mut self, to: Phase, dir: Dir) {
        let from = std::mem::replace(&mut self.phase, to);
        self.slide_from(from, dir);
    }

    /// Slides from `from` to the phase there is now, when a screen changed in place into
    /// another, as the Network screen does putting its keyboard up.
    pub(crate) fn slide_from(&mut self, from: Phase, dir: Dir) {
        self.slide = Some(Slide {
            from,
            dir,
            started: self.now(),
        });
    }

    /// Ends the slide under way, if there is one, as though its time were up: between two
    /// screens, or to or from a power screen.
    pub fn finish_slide(&mut self) {
        self.slide = None;
        self.power_slide = None;
    }

    /// The move between screens under way.
    pub fn slide(&self) -> Option<&Slide> {
        self.slide.as_ref()
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    /// Every cart on the card, once: FAVORITES holds only carts its systems' shelves do.
    pub fn carts(&self) -> impl Iterator<Item = &Cart> {
        self.system_shelves()
            .flat_map(|(_, shelf)| shelf.carts.iter())
    }

    /// Each system's shelf, with its system.
    fn system_shelves(&self) -> impl Iterator<Item = (Platform, &Shelf)> {
        self.shelves
            .iter()
            .filter_map(|(kind, shelf)| Some((kind.platform()?, shelf)))
    }

    pub fn seated_cart(&self) -> Option<&Cart> {
        let stem = match &self.phase {
            Phase::Inserting { cart, .. }
            | Phase::Playing { cart }
            | Phase::Ejecting { cart, .. }
            | Phase::Polaroids { cart } => cart,
            Phase::Doze { cart: Some(cart) } => cart,
            _ => return None,
        };
        self.shelf().carts.iter().find(|c| c.stem == *stem)
    }

    pub fn single_cart(&self) -> bool {
        self.carts().count() == 1
    }

    /// Sets each cart's face, in `carts()`'s order; FAVORITES draws its carts on the same ones.
    pub fn set_faces(&mut self, faces: Vec<TexId>) {
        let mut faces = faces.into_iter();
        for (kind, shelf) in &mut self.shelves {
            if *kind == ShelfKind::Favorites {
                continue;
            }
            let n = shelf.carts.len();
            shelf.set_faces(faces.by_ref().take(n).collect());
        }
        self.share_favorite_faces();
    }

    pub fn set_snapshot(&mut self, snapshot: Box<dyn Snapshot>) {
        self.snapshot = Some(snapshot);
    }

    pub fn set_core(&mut self, core: Core) {
        self.core = core;
    }

    pub fn set_platform(&mut self, platform: Platform) {
        self.platform = platform;
    }

    pub fn set_named_core(&mut self, named: bool) {
        self.named_core = named;
    }

    pub fn set_video_mode(&mut self, mode: VideoMode) {
        self.video_mode = mode;
    }

    /// How a PSP game is shown when it was not set for itself: Settings > Gameplay's.
    pub fn psp_screen(&self) -> VideoMode {
        VideoMode::parse(&self.state.psp_screen).unwrap_or(VideoMode::Fit)
    }

    /// The game's menu: the PSP game's picture stepped on, kept for the game.
    pub(crate) fn step_picture(&mut self, right: bool) {
        let to = self.video_mode.step_in(&VideoMode::PSP, right);
        self.set_picture(to);
    }

    /// How the D-pad stands in for the stick in the game in play.
    pub(crate) fn stick_now(&self) -> crate::input::Stick {
        match (&self.root, &self.phase) {
            (Some(root), Phase::Playing { cart }) => {
                crate::input::stick_for(root, cart, self.platform)
            }
            _ => crate::input::Stick::Off,
        }
    }

    /// The game's menu: the PSP game's stick stepped on, kept for the game and handed to the pad.
    pub(crate) fn step_stick(&mut self, right: bool) {
        use crate::input::Stick;
        const ORDER: [Stick; 3] = [Stick::HoldL2, Stick::ToggleL2, Stick::Off];
        let at = ORDER
            .iter()
            .position(|s| *s == self.stick_now())
            .unwrap_or(0);
        let to = ORDER[if right { (at + 1) % 3 } else { (at + 2) % 3 }];
        if let (Some(root), Phase::Playing { cart }) = (&self.root, &self.phase) {
            if let Err(e) = crate::input::write_stick(root, cart, to) {
                crate::log!("psp_stick.ini: {e}");
            }
        }
        self.stick_pending = Some(to);
    }

    /// The stick a game's menu set since the last call, for the session's pad.
    pub fn take_stick(&mut self) -> Option<crate::input::Stick> {
        self.stick_pending.take()
    }

    pub fn source_rect(&self) -> [f32; 4] {
        video_mode::source_rect(self.platform, self.video_mode)
    }

    pub fn screen(&self) -> video_mode::Screen {
        video_mode::screen(self.platform, self.video_mode)
    }

    /// The system of the cart in the handheld, or of the last one there.
    pub fn platform(&self) -> Platform {
        self.platform
    }

    fn frontend_owns_the_shoulders(&self) -> bool {
        matches!(self.phase, Phase::Playing { .. }) && self.platform.owns_shoulders()
    }

    /// What the buttons the systems disagree about do, for the game in the handheld or the last
    /// one there: a PlayStation game has L2 and R2 and nothing rewinds it, and only a handheld's
    /// screen has a colour correction for SELECT+Y to toggle.
    pub fn keys(&self) -> Keys {
        Keys {
            game_l2_r2: self.platform.game_owns_l2_r2(),
            rewind: self.platform.rewinds() && self.state.rewind && !self.hardcore(),
            colour: self.platform.colour_correction(),
        }
    }

    pub fn taken_buttons(&self) -> &'static [Btn] {
        if self.frontend_owns_the_shoulders() {
            &[Btn::L1, Btn::R1]
        } else {
            &[]
        }
    }

    pub fn takes_from_the_game(&self, action: Action) -> bool {
        match action {
            Action::GbaDown(btn) | Action::GbaUp(btn) => self.taken_buttons().contains(&btn),
            _ => false,
        }
    }

    pub(crate) fn set_picture(&mut self, mode: VideoMode) {
        if self.video_mode == mode {
            return;
        }
        self.video_mode = mode;
        let (Some(root), Phase::Playing { cart }) = (self.root.clone(), &self.phase) else {
            return;
        };
        if let Err(e) = video_mode::write_video_mode(&root, cart, mode) {
            crate::log!("video: could not write video_mode.ini: {e}");
        }
    }

    pub fn set_link_loaded(&mut self, serial: &'static str) {
        self.link_loaded = Some(serial);
    }

    pub fn link_mode(&self, stem: &str) -> (LinkKind, &'static str) {
        let Some((cart, auto)) = self.auto_link(stem) else {
            return (LinkKind::Cable, "auto");
        };
        let chosen = self.link_choices.get(stem).copied().unwrap_or(auto);
        (chosen, serial_option(chosen, auto, &cart.code, &cart.title))
    }

    pub fn link_active(&self) -> bool {
        self.link.is_some()
    }

    pub fn begin_link(&mut self, client_id: u16) {
        self.link = Some(LinkSession {
            client_id,
            lost_at: None,
        });
        self.sync_link_badge();
    }

    pub fn link_client_id(&self) -> Option<u16> {
        self.link.as_ref().map(|s| s.client_id)
    }

    pub fn may_rewind(&self) -> bool {
        !self.link_active() && !self.hardcore()
    }

    /// No state loads while linked, nor in RetroAchievements' hardcore.
    pub fn may_load_state(&self) -> bool {
        !self.link_active() && !self.hardcore()
    }

    pub fn may_fast_forward(&self) -> bool {
        !self.link_active() && self.state.ff_speed != 0 && self.platform.fast_forwards()
    }

    /// Whether held X and Y are turbo A and B, as Settings > Gameplay has it.
    pub fn turbo_on(&self) -> bool {
        self.state.turbo
    }

    /// Whether a game shows the frame after the one it is at, as Settings > Gameplay has it.
    pub fn run_ahead_on(&self) -> bool {
        self.state.run_ahead
    }

    /// The options frame blending and the Game Boy's palette set on the game in play: blending
    /// on the GBA's and the Game Boys' cores, the palette on an original Game Boy game on mGBA.
    pub fn core_extras(&self) -> Vec<(&'static str, &'static str)> {
        let mut set = Vec::new();
        if matches!(self.platform, Platform::Gba | Platform::Gb | Platform::Gbc) {
            set.extend(self.state.blending.option(self.core));
        }
        if self.platform == Platform::Gb && self.core == Core::Mgba {
            match self.state.gb_colors {
                omega_store::GbColors::Auto => set.extend([
                    ("mgba_gb_colors_preset", "1"),
                    ("mgba_gb_colors", "GBC Dark Green →A"),
                ]),
                omega_store::GbColors::Palette(p) => set.extend([
                    ("mgba_gb_colors_preset", "0"),
                    ("mgba_gb_colors", p.core_name()),
                ]),
            }
        }
        set
    }

    pub(crate) fn queue_core_options(&mut self) {
        self.core_options_pending = true;
    }

    /// Whether `core_extras` changed since the game in play last took them.
    pub fn take_core_options(&mut self) -> bool {
        std::mem::take(&mut self.core_options_pending)
    }

    pub fn end_link(&mut self) {
        self.link = None;
        self.link_player = None;
        self.link_peer = None;
        self.online_room = None;
        self.sync_link_badge();
        self.radio.ask(RadioJob::Down);
        self.radio.ask(RadioJob::Cool);
    }

    pub fn peer_lost(&mut self) {
        if matches!(self.game_menu, Some(GameMenu::Linked { .. })) {
            self.game_menu = None;
        }
        let now = self.now();
        if let Some(session) = &mut self.link {
            session.lost_at.get_or_insert(now);
        }
        self.sync_link_badge();
    }

    pub fn bios_mismatch(&mut self) {
        if !self.link_active() {
            return;
        }
        let role = self
            .link_client_id()
            .map_or(self.last_role, LinkRow::from_client_id);
        self.end_link();
        self.hud.toast(Toast::BiosMismatch, self.now());
        self.unplug(role);
    }

    pub fn peer_ended(&mut self) {
        if !self.link_active() {
            return;
        }
        let role = self
            .link_client_id()
            .map_or(self.last_role, LinkRow::from_client_id);
        self.end_link();
        self.hud.toast(Toast::PeerEnded, self.now());
        self.unplug(role);
    }

    pub fn link_badge(&self) -> LinkBadge {
        self.hud.link()
    }

    fn sync_link_badge(&mut self) {
        let badge = match &self.link {
            None => LinkBadge::Off,
            Some(s) => match (s.client_id == 0, s.lost_at.is_some()) {
                (true, false) => LinkBadge::Hosting,
                (false, false) => LinkBadge::Joined,
                (true, true) => LinkBadge::HostingLost,
                (false, true) => LinkBadge::JoinedLost,
            },
        };
        self.hud.set_link(badge);
    }

    pub fn take_sfx(&mut self) -> Option<Sfx> {
        self.sfx.take()
    }

    pub fn set_power(&mut self, mut power: Power) {
        power.set_backlight(self.state.brightness);
        self.headphones = power.headphones();
        self.hud.set_headphones(self.headphones);
        let secs = power.now();
        if matches!(self.phase, Phase::SetClock { .. }) || secs < CLOCK_FLOOR {
            self.phase = clock_screen(secs, 0, false);
            self.slide = None;
        }
        self.power = Some(power);
        self.battery_at = self.now();
        self.charge_at = self.now();
        self.last_led = None;
    }

    pub fn set_rumble(&mut self, strength: u16) {
        if let Some(power) = &mut self.power {
            power.set_rumble(strength);
        }
    }

    pub fn rumble_enabled(&self) -> bool {
        self.state.rumble
    }

    pub fn ff_speed(&self) -> u8 {
        self.state.ff_speed
    }

    pub fn ff_sound(&self) -> bool {
        self.state.ff_sound
    }

    pub fn colour_correction(&self) -> bool {
        self.state.colour_correction
    }

    /// How long a shut lid sleeps before powering off, as Settings > Shelf has it.
    pub fn lid_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(u64::from(self.state.lid_off_min.max(1)) * 60)
    }

    /// The clock reads 24-hour time, else 12-hour.
    pub fn clock_24h(&self) -> bool {
        self.state.clock_24h
    }

    /// What a Game Boy or GBA game at 3x is drawn through.
    pub fn screen_filter(&self) -> omega_store::ScreenFilter {
        self.state.screen_filter
    }

    /// SELECT+A and SELECT+B in a game, as on Slot: the next screen filter, or the one before,
    /// named in a toast.
    pub(crate) fn step_screen_filter(&mut self, forward: bool) {
        self.state.screen_filter = self.state.screen_filter.step(forward);
        let now = self.now();
        self.hud.toast(Toast::Filter(self.state.screen_filter), now);
        self.persist();
    }

    pub fn powering_off(&self) -> bool {
        self.powering_off
    }

    pub fn ready_to_power_off(&self) -> bool {
        self.powering_off && self.now() >= self.act_at
    }

    pub fn ready_to_restart(&self) -> bool {
        self.restarting && self.now() >= self.act_at
    }

    pub fn shutting_down(&self) -> bool {
        self.powering_off || self.restarting
    }

    pub fn restarting(&self) -> bool {
        self.restarting
    }

    pub fn restart(&mut self) {
        self.settle_saves();
        crate::card_writer::settle();
        if let Some(power) = &mut self.power {
            power.restart();
        }
    }

    pub fn core_picker(&self) -> Option<Core> {
        self.core_picker.map(|p| p.seat())
    }

    pub fn core_picker_chip(&self) -> Option<Chip> {
        self.core_picker.map(|p| p.chip(self.now()))
    }

    /// Whether a screen over the game holds it paused: the link screen or the DISC screen.
    pub fn game_menu_open(&self) -> bool {
        self.game_menu.is_some() || self.disc_screen.is_some() || self.pause_menu.is_some()
    }

    pub fn game_menu(&self) -> Option<GameMenu> {
        self.game_menu
    }

    pub fn take_link_transport(&mut self) -> Option<(u16, Box<dyn LinkChannel>)> {
        self.link_transport.take()
    }

    pub fn take_link_reload(&mut self) -> Option<(String, &'static str)> {
        self.link_reload.take()
    }

    pub fn link_player(&self) -> Option<u8> {
        self.link_player
    }

    pub fn take_colour_correction(&mut self) -> Option<bool> {
        self.colour_pending.take()
    }

    pub fn core(&self) -> Core {
        self.core
    }

    pub fn link_reload_done(&mut self) {
        let Some(reload) = self.reload.take() else {
            return;
        };
        if reload.fallback {
            self.link_choices.insert(reload.stem, reload.from);
            self.close_game_menu();
            return self.refuse();
        }
        if reload.cancelled {
            return self.close_game_menu();
        }
        let since = match self.game_menu {
            Some(GameMenu::Working { since, .. }) => since,
            _ => self.now(),
        };
        let starter = self.link_starter(reload.role);
        self.start_link_from(starter, reload.role.client_id(), since);
    }

    pub fn link_reload_failed(&mut self) {
        let Some(mut reload) = self.reload.take() else {
            return;
        };
        if !reload.fallback {
            self.link_reload = Some((reload.stem.clone(), reload.from_serial));
            reload.fallback = true;
            self.reload = Some(reload);
            return;
        }
        self.link_choices.insert(reload.stem, reload.from);
        self.refuse_seated();
    }

    pub fn selected_stem(&self) -> Option<&str> {
        self.shelf()
            .carts
            .get(self.shelf().index)
            .map(|c| c.stem.as_str())
    }

    pub fn battery(&self) -> Option<Battery> {
        self.battery
    }

    pub fn poweroff(&mut self) {
        self.remember_pick();
        self.settle_saves();
        crate::card_writer::settle();
        if let Some(power) = &mut self.power {
            power.poweroff();
        }
    }

    /// Acts on a press, and makes the menus' sound for what it changed.
    pub fn apply(&mut self, action: Action) {
        let before = self.sound_mark();
        let began = std::time::Instant::now();
        self.press(action);
        let took = began.elapsed().as_millis();
        if took >= 50 {
            crate::log!("slow press: {action:?} took {took} ms");
        }
        self.sound_for_press(&before, action);
    }

    fn press(&mut self, action: Action) {
        let taken = self.opening_takes(action) || self.start_notice_takes(action);
        self.press_ends_boot_fade(action);
        if taken {
            return;
        }
        match self.power_screen {
            Some(PowerScreen::Menu(_)) => match action {
                Action::LidClose => return self.doze(),
                Action::LidOpen => return self.wake(),
                _ => return self.power_input(action),
            },
            // nothing can be pressed while the handheld goes down
            Some(PowerScreen::Going(_)) => return,
            None => {}
        }
        match action {
            Action::LidClose => return self.doze(),
            Action::LidOpen => return self.wake(),
            Action::PowerPress => {
                if self.link_active() {
                    self.end_link();
                    return;
                }
                self.woke_on_press = matches!(self.phase, Phase::Doze { .. });
                if self.woke_on_press {
                    self.wake();
                }
                return self.flush_resume();
            }
            Action::PowerTap => return self.power_press(),
            // holding POWER powers off at once, with no menu between; START pressed while it
            // is held restarts instead
            Action::PowerHold => return self.power_off_now(),
            Action::PowerRestart => return self.restart_now(),
            Action::PowerOff => return,
            _ => {}
        }
        let clock = matches!(self.phase, Phase::SetClock { .. });
        if !clock && self.adjust(action) {
            return;
        }
        // A press during a slide ends it, then acts on the screen it arrived at. A release
        // does not: a tap's own release, or the one SELECT hands the game after its tap,
        // would end the slide its press began.
        if !matches!(action, Action::GbaUp(_)) {
            self.finish_slide();
        }
        if clock {
            return self.clock_input(action);
        }
        match action {
            Action::GbaUp(Btn::Left) => self.shelf_mut().release_left(),
            Action::GbaUp(Btn::Right) => self.shelf_mut().release_right(),
            _ => {}
        }
        if self.pause_menu.is_some() {
            return self.pause_input(action);
        }
        if self.game_menu.is_some() {
            return self.game_menu_input(action);
        }
        if self.disc_screen.is_some() {
            return self.disc_input(action);
        }
        let now = self.now();
        match self.phase {
            Phase::Shelf => match action {
                // the shelf's buttons are Slot's: START picks the core (X too, as it always
                // did, but on RECENTS, where X takes the game off it), MENU opens Settings and
                // Y stars the cart
                Action::GbaDown(Btn::X) if self.core_picker.is_none() && self.on_recents() => {
                    self.remove_recent(now)
                }
                Action::GbaDown(Btn::Start | Btn::X) if self.core_picker.is_none() => {
                    self.open_core_picker()
                }
                _ if self.core_picker.is_some() => self.core_picker_input(action),
                Action::QuickMenu => self.open_settings(),
                Action::GbaDown(Btn::Y) => self.toggle_favorite(now),
                Action::SelectTap => self.go(Phase::Controls, Dir::Deeper),
                Action::GbaDown(Btn::Up) => self.jump_letter(-1),
                Action::GbaDown(Btn::Down) => self.jump_letter(1),
                Action::ShelfLeft | Action::GbaDown(Btn::Left) => self.shelf_mut().hold_left(now),
                Action::ShelfRight | Action::GbaDown(Btn::Right) => {
                    self.shelf_mut().hold_right(now)
                }
                Action::GbaDown(Btn::A) => self.play_held = Some(now),
                Action::GbaUp(Btn::A) => {
                    if self.play_held.take().is_some() {
                        self.insert(!self.state.auto_load);
                    }
                }
                Action::Insert => self.insert(!self.state.auto_load),
                Action::GbaDown(Btn::L1) => self.switch_shelf(-1),
                Action::GbaDown(Btn::R1) => self.switch_shelf(1),
                // L2 back to the shelf's first cart, R2 to one picked at random
                Action::RewindStart | Action::GbaDown(Btn::L2) => self.shelf_mut().glide_to(0),
                Action::FfStart | Action::GbaDown(Btn::R2) => self.pick_at_random(),
                _ => {}
            },
            Phase::Inserting { .. } if action == Action::Eject => self.eject(),
            Phase::Playing { .. } => match action {
                Action::GbaDown(Btn::L1) if self.frontend_owns_the_shoulders() => {
                    self.set_picture(VideoMode::Stretch)
                }
                Action::GbaDown(Btn::R1) if self.frontend_owns_the_shoulders() => {
                    self.set_picture(VideoMode::Actual)
                }
                Action::Eject => self.eject(),
                Action::QuickMenu => self.open_pause_menu(),
                Action::GameMenu => self.game_menu_shortcut(),
                Action::Polaroids => self.open_polaroids(),
                Action::FilterNext => self.step_screen_filter(true),
                Action::FilterPrev => self.step_screen_filter(false),
                Action::Screenshot => self.take_screenshot(),
                Action::SaveState => self.save_state(),
                Action::LoadState => self.load_newest(),
                Action::RewindStart if !self.may_rewind() => self.refuse(),
                // Fast Forward OFF: R2 does nothing, as on Slot
                Action::FfStart if self.state.ff_speed == 0 => {}
                Action::FfStart if !self.may_fast_forward() => self.refuse(),
                _ => {}
            },
            Phase::Polaroids { .. } if self.delete_asked => self.delete_input(action),
            Phase::Polaroids { .. } => match action {
                Action::GbaDown(Btn::X) => self.ask_delete(),
                Action::ShelfLeft | Action::GbaDown(Btn::Left) => self.flick(Polaroids::left),
                Action::ShelfRight | Action::GbaDown(Btn::Right) => self.flick(Polaroids::right),
                Action::GbaDown(Btn::A) => self.load_selected(),
                Action::GbaDown(Btn::B) | Action::Polaroids => self.close_polaroids(),
                _ => {}
            },
            Phase::About => self.about_input(action),
            Phase::Settings { row } => self.settings_input(row, action),
            Phase::Controls => self.controls_input(action),
            Phase::Network(_) => self.network_input(action),
            Phase::Bluetooth(_) => self.bluetooth_input(action),
            Phase::Info(_) => self.info_input(action),
            Phase::Labels(_) => self.labels_input(action),
            Phase::Background(_) => self.background_input(action),
            Phase::Options(_) => self.options_input(action),
            _ => {}
        }
    }

    /// What the shelf is drawn over now.
    pub fn backdrop(&self) -> Backdrop {
        Backdrop::resolve(
            self.state.backdrop,
            self.state.backdrop_moving,
            self.state.backdrop_colour,
            self.wallpaper.is_some(),
        )
    }

    /// The backdrop colour the kit's faces are drawn in, as an index in BACKDROP_COLOURS: a sync
    /// keys its faces on it, so a new colour in Settings > Theme re-tints every screen.
    pub fn palette_colour(&self) -> u8 {
        self.state.backdrop_colour
    }

    /// The kit's colours for the backdrop colour the player picked, wallpaper or not.
    pub fn palette(&self) -> &'static omega_ui::kit::palette::Palette {
        omega_ui::kit::palette::palette(self.palette_colour())
    }

    /// The font the bar's actions write their big words in: the bar's sync keys their faces on
    /// it, so a new font in Settings > Theme redraws the bar at once.
    pub fn font(&self) -> Font {
        match self.state.font {
            omega_store::Font::Bold => Font::Bold,
            omega_store::Font::Thin => Font::Thin,
        }
    }

    /// What the shelf's top bar is drawn in: the card's theme.txt when it has one,
    /// or else the kit's palette, so a new colour in Settings > Theme re-tints them too.
    pub fn chrome_colours(&self) -> ChromeColours {
        match &self.theme {
            Some(theme) => ChromeColours::themed(theme),
            None => ChromeColours::of(self.palette()),
        }
    }

    /// Draws the shelf's chrome in the card's own colours, as a Config/theme.txt does.
    pub fn set_theme(&mut self, theme: Option<Theme>) {
        self.theme = theme;
    }

    /// The pattern the frontend should make a picture of, when the background is one.
    pub fn backdrop_pattern(&self) -> Option<(BackdropStyle, u8)> {
        let b = self.backdrop();
        b.style.moves().then_some((b.style, b.colour))
    }

    pub fn set_backdrop_face(&mut self, tex: TexId, w: u32, style: BackdropStyle, colour: u8) {
        self.backdrop_face = Some((tex, w, style, colour));
    }

    pub(crate) fn draw_background(&self, out: &mut Vec<Draw>) {
        let b = self.backdrop();
        match (b.style, self.backdrop_face) {
            (BackdropStyle::Wallpaper, _) => draw_backdrop(self.wallpaper, out),
            (style, Some((tex, w, built, colour)))
                if style.moves() && built == style && colour == b.colour =>
            {
                draw_pattern((tex, w), backdrop_shift(style, b.moving, self.now()), out);
            }
            _ => draw_solid(b.colour, out),
        }
    }

    /// How closely the radio is watched: the Network screen, and SSH opened from it, keep what
    /// X's scans found, Settings names the network joined, and the shelf's top bar and Labels
    /// only need to know whether there is one.
    pub fn wifi_watch(&self) -> Watch {
        if self.pause_page() == Some(crate::pause_menu::PausePage::Networks) {
            return Watch::Networks;
        }
        match self.phase {
            Phase::Network(_) | Phase::Info(_) => Watch::Networks,
            Phase::Settings { .. } => Watch::Status,
            Phase::Shelf | Phase::Inserting { .. } | Phase::Ejecting { .. } | Phase::Labels(_) => {
                Watch::Glance
            }
            _ => Watch::Idle,
        }
    }

    /// Wi-Fi has a network joined.
    pub(crate) fn online(&self) -> bool {
        self.wifi.status().joined()
    }

    /// The carts with no label, and the PlayStation discs with none, for a sync to look up.
    pub(crate) fn label_wants(&self) -> Vec<Want> {
        crate::label_sync::wants(self.root.as_deref(), self.carts())
    }

    /// Puts each label a sync saved on its cart, for the frontend to redraw the cart with.
    fn take_labels(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        for (platform, stem) in self.labels.take_fresh() {
            let path = crate::label_sync::label_path(&root, platform, &stem);
            let mut found = false;
            for (kind, shelf) in &mut self.shelves {
                if *kind != ShelfKind::Favorites && kind.platform() != Some(platform) {
                    continue;
                }
                let cart = shelf
                    .carts
                    .iter_mut()
                    .find(|c| c.platform == platform && c.stem == stem);
                let Some(cart) = cart else {
                    continue;
                };
                found = true;
                // a later disc's label, saved under the disc's own name, leaves the game's as
                // it was
                if path.is_file() {
                    cart.label = Some(path.clone());
                }
            }
            if !found {
                continue;
            }
            self.stale_faces.push((platform, stem));
        }
    }

    /// The carts whose faces must be drawn again, with the faces they are drawn on now.
    pub fn take_stale_faces(&mut self) -> Vec<(Cart, TexId)> {
        let stale = std::mem::take(&mut self.stale_faces);
        self.faces_of(stale)
    }

    /// The carts named, with the faces they are drawn on now.
    pub fn faces_of(&self, carts: Vec<(Platform, String)>) -> Vec<(Cart, TexId)> {
        carts
            .into_iter()
            .filter_map(|(platform, stem)| {
                let (_, shelf) = self.system_shelves().find(|(p, _)| *p == platform)?;
                let (cart, face) = shelf.find(&stem)?;
                Some((cart.clone(), face?))
            })
            .collect()
    }

    /// The face the cart `stem` of `platform` is drawn on now.
    pub fn face_of(&self, platform: Platform, stem: &str) -> Option<TexId> {
        let (_, shelf) = self.system_shelves().find(|(p, _)| *p == platform)?;
        shelf.find(stem)?.1
    }

    /// Every cart once, the shelf in view first and on each shelf the carts nearest its picked
    /// one first, either way round: the order a boot draws their labels in, so the ones on
    /// screen come first, on FAVORITES and RECENTS as on a system's shelf.
    pub fn carts_nearest_first(&self) -> Vec<&Cart> {
        let n = self.shelves.len();
        let mut carts = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (_, shelf) in self.shelves.iter().cycle().skip(self.shelf_at).take(n) {
            let len = shelf.carts.len();
            let mut row: Vec<(usize, &Cart)> = shelf.carts.iter().enumerate().collect();
            row.sort_by_key(|(i, _)| {
                let d = i.abs_diff(shelf.index);
                d.min(len - d)
            });
            let fresh = row
                .into_iter()
                .map(|(_, c)| c)
                .filter(|c| seen.insert((c.platform as u8, c.stem.as_str())));
            carts.extend(fresh);
        }
        carts
    }

    /// Which shelf is in view and which cart on it is picked: a change brings the carts now on
    /// screen to the front of the label queue.
    pub fn view_key(&self) -> (usize, usize) {
        (self.shelf_at, self.shelf().index)
    }

    /// The carts on screen now, the picked one first and then out to either side.
    pub fn carts_in_view(&self) -> Vec<(Platform, String)> {
        let shelf = self.shelf();
        [0, -1, 1, -2, 2, -3, 3]
            .into_iter()
            .filter_map(|off| shelf.cart_at_offset(off))
            .map(|i| (shelf.carts[i].platform, shelf.carts[i].stem.clone()))
            .collect()
    }

    pub fn set_label_jobs(&mut self, jobs: Box<dyn LabelJobs>) {
        let account = self.labels.account().cloned();
        self.labels = Labels::new(jobs, account);
    }

    pub fn labels(&self) -> &Labels {
        &self.labels
    }

    pub fn set_wifi_jobs(&mut self, jobs: Box<dyn WifiJobs>) {
        self.wifi = Wifi::new(jobs);
        self.wifi.start(self.state.wifi);
    }

    pub fn wifi(&self) -> &Wifi {
        &self.wifi
    }

    pub fn wifi_summary(&self) -> Summary {
        self.wifi.summary()
    }

    pub fn apply_at(&mut self, action: Action, now: Millis) {
        self.clock = now as f64;
        self.apply(action);
    }

    fn adjust(&mut self, action: Action) -> bool {
        if action == Action::MuteToggle {
            self.mute_toggle();
            return true;
        }
        if action == Action::ColourCorrectionToggle {
            self.change_setting(SettingsRow::ColourCorrection, true);
            let said = match self.state.colour_correction {
                true => Toast::ColourOn,
                false => Toast::ColourOff,
            };
            self.hud.toast(said, self.now());
            return true;
        }
        let s = &self.state;
        let (kind, value) = match action {
            Action::BrightnessUp => (HudKind::Brightness, up(s.brightness, 1, BRIGHTNESS_MAX)),
            Action::BrightnessDown => (HudKind::Brightness, s.brightness.saturating_sub(1)),
            Action::BlueLightUp => (HudKind::BlueLight, up(s.blue_light, 1, BLUE_LIGHT_MAX)),
            Action::BlueLightDown => (HudKind::BlueLight, s.blue_light.saturating_sub(1)),
            Action::VolumeUp => (HudKind::Volume, up(self.level(), VOLUME_STEP, VOLUME_MAX)),
            Action::VolumeDown => (HudKind::Volume, self.level().saturating_sub(VOLUME_STEP)),
            _ => return false,
        };
        if kind == HudKind::Volume {
            self.remember_volume();
        }
        let level = match kind {
            HudKind::Brightness => &mut self.state.brightness,
            HudKind::BlueLight => &mut self.state.blue_light,
            HudKind::Volume if self.on_headphones() => &mut self.state.volume_hp,
            HudKind::Volume => &mut self.state.volume,
            HudKind::Rewind => return false,
        };
        let moved = *level != value;
        *level = value;
        let unmuted = kind == HudKind::Volume && std::mem::take(self.muted_mut());
        let (shown, now) = (self.hud_value(kind, value), self.now());
        self.hud.show(kind, shown, self.muted(), now);
        if let (HudKind::Brightness, Some(power)) = (kind, &mut self.power) {
            power.set_backlight(value);
        }
        if moved || unmuted {
            self.persist();
        }
        true
    }

    fn remember_volume(&mut self) {
        if self.vol_before.len() == 2 {
            self.vol_before.remove(0);
        }
        self.vol_before
            .push((self.level(), self.muted(), self.now()));
    }

    fn mute_toggle(&mut self) {
        let now = self.now();
        if let Some((volume, muted, _)) = self
            .vol_before
            .iter()
            .find(|(_, _, at)| now.saturating_sub(*at) <= MUTE_CHORD_MS)
            .copied()
        {
            *self.level_mut() = volume;
            *self.muted_mut() = muted;
        }
        self.vol_before.clear();
        let muted = !self.muted();
        *self.muted_mut() = muted;
        self.hud
            .show(HudKind::Volume, self.output_volume(), muted, now);
        self.persist();
    }

    fn hud_value(&self, kind: HudKind, value: u8) -> u8 {
        match kind {
            HudKind::Volume => self.output_volume(),
            _ => value,
        }
    }

    pub fn show_rewind(&mut self, fill: u8) {
        let now = self.now();
        self.hud.show(HudKind::Rewind, fill, false, now);
    }

    pub fn hide_rewind(&mut self) {
        self.hud.release_rewind();
    }

    /// Fast forward as `ff`, its badge saying the speed Settings sets.
    pub fn set_ff(&mut self, ff: FfState) {
        self.hud.set_ff(ff, self.state.ff_speed);
    }

    pub fn ff_badge(&self) -> Option<CornerBadge> {
        self.hud.ff_badge()
    }

    pub fn blue_light(&self) -> u8 {
        self.state.blue_light
    }

    pub fn brightness(&self) -> u8 {
        self.state.brightness
    }

    pub fn volume(&self) -> u8 {
        self.level()
    }

    /// Headphones have their volume and mute of their own: wired ones in the jack, or
    /// Bluetooth ones the sound is going to.
    fn on_headphones(&self) -> bool {
        self.headphones || crate::audio::on_bluetooth()
    }

    fn level(&self) -> u8 {
        if self.on_headphones() {
            self.state.volume_hp
        } else {
            self.state.volume
        }
    }

    fn level_mut(&mut self) -> &mut u8 {
        if self.on_headphones() {
            &mut self.state.volume_hp
        } else {
            &mut self.state.volume
        }
    }

    pub fn muted(&self) -> bool {
        if self.on_headphones() {
            self.state.muted_hp
        } else {
            self.state.muted
        }
    }

    fn muted_mut(&mut self) -> &mut bool {
        if self.on_headphones() {
            &mut self.state.muted_hp
        } else {
            &mut self.state.muted
        }
    }

    /// Checks for an update once a boot, quietly, once Wi-Fi has been joined to a network for
    /// 5 s, at boot or after; and says when one is found.
    fn check_for_update(&mut self, now: Millis) {
        const SETTLE_MS: Millis = 5_000;
        if !self.update_checked {
            if !self.online() {
                self.update_online_since = None;
            } else {
                let since = *self.update_online_since.get_or_insert(now);
                if now.saturating_sub(since) >= SETTLE_MS {
                    self.update_checked = true;
                    self.updater.check_quietly(self.root.clone());
                }
            }
        }
        let found = matches!(
            self.updater.state(),
            crate::update::Update::Available { .. }
        );
        // in Settings the row says so itself
        if found && !self.update_told {
            self.update_told = true;
            if !matches!(self.phase, Phase::Settings { .. }) {
                self.hud
                    .toast(Toast::Update("Update available in Settings"), now);
            }
        }
    }

    /// Switches Wi-Fi on or off, from the game's menu.
    pub(crate) fn toggle_wifi(&mut self) {
        let now = self.now();
        self.wifi.set_enabled(!self.wifi.enabled(), now);
        self.state.wifi = self.wifi.enabled();
        self.persist();
    }

    /// Switches Bluetooth on or off, from the game's menu.
    pub(crate) fn toggle_bluetooth(&mut self) {
        let now = self.now();
        self.bluetooth.set_enabled(!self.bluetooth.enabled(), now);
        self.state.bluetooth = self.bluetooth.enabled();
        self.persist();
    }

    /// How loud the samples are made: on Bluetooth headphones, which play at a volume of their
    /// own, full but for mute and conversational awareness; else the volume.
    pub fn sample_volume(&self) -> u8 {
        if !crate::audio::on_bluetooth() {
            return self.output_volume();
        }
        if self.muted() {
            return 0;
        }
        let duck = self.phones.phones().map_or(1.0, |p| p.duck);
        (100.0 * duck).round() as u8
    }

    /// Whether sound may go to the headphones at `audio`: once their own volume is the one
    /// they are kept at here, so they never play for a moment at whatever they were left at.
    /// Should it not be set within 2.5 s, the sound goes to them all the same.
    fn bluetooth_volume_ready(&mut self, audio: Option<&str>, now: Millis) -> bool {
        const GIVE_UP_MS: Millis = 2_500;
        let Some(mac) = audio else {
            self.bt_volume_wait = None;
            return false;
        };
        if crate::audio::on_bluetooth() {
            return true;
        }
        let want = self.state.volume_hp;
        if self.bt_volume_wait.as_ref().is_none_or(|(m, _)| m != mac) {
            crate::audio::bt_volume::set(want);
            self.bt_volume_wait = Some((mac.to_string(), now));
        }
        let since = self.bt_volume_wait.as_ref().map_or(now, |(_, at)| *at);
        crate::audio::bt_volume::level().is_some_and(|l| l.abs_diff(want) <= 1)
            || now.saturating_sub(since) >= GIVE_UP_MS
    }

    /// Says when the sound goes to Bluetooth headphones and when it comes back from them, and
    /// mutes the speaker as it does, so a game playing aloud never surprises anyone.
    fn tell_bluetooth_sound(&mut self) {
        let on = crate::audio::on_bluetooth();
        if on == self.bt_was_on {
            return;
        }
        self.bt_was_on = on;
        let now = self.now();
        if on {
            // one toast for both: connected, and the mode they connected in where it is known
            let said = self
                .phones
                .phones()
                .and_then(|p| p.noise.map(|n| n.connected(p.kind)))
                .unwrap_or("Headphones connected");
            self.hud.toast(Toast::Headphones(said), now);
        } else {
            self.hud
                .toast(Toast::Headphones("Headphones disconnected"), now);
            if !self.headphones && !self.state.muted {
                self.state.muted = true;
                self.persist();
            }
        }
    }

    /// Headphones that connect by themselves can come without their sound: bluetoothd has
    /// them, but not the A2DP stream. Asking them to connect again, once they have gone a
    /// moment without it, brings it, and every 15 s while it stays away.
    fn connect_bluetooth_sound(&mut self, audio: Option<&str>, now: Millis) {
        const WAIT_MS: Millis = 5_000;
        const AGAIN_MS: Millis = 15_000;
        let Some(mac) = audio.filter(|_| !crate::audio::on_bluetooth()) else {
            self.bt_audio_wait = None;
            if audio.is_none() {
                self.bt_audio_asks = 0;
            }
            return;
        };
        if self.bt_audio_wait.as_ref().is_none_or(|(m, ..)| m != mac) {
            self.bt_audio_wait = Some((mac.to_string(), now, None));
        }
        let Some((_, since, asked)) = &mut self.bt_audio_wait else {
            return;
        };
        let due = match asked {
            None => now.saturating_sub(*since) >= WAIT_MS,
            Some(at) => now.saturating_sub(*at) >= AGAIN_MS,
        };
        // twice at most: headphones a phone is taking back are left to it
        if due && self.bt_audio_asks < 2 {
            self.bt_audio_asks += 1;
            *asked = Some(now);
            crate::log!("bluetooth: {mac} connected without its sound, asked again");
            self.bluetooth.connect_quietly(mac);
        }
    }

    /// Keeps the headphones' own volume and the console's as one while sound goes to
    /// Bluetooth headphones: the buttons set theirs, and theirs changed on the headphones shows
    /// on the gauge and is kept.
    fn follow_bluetooth_volume(&mut self, now: Millis) {
        use crate::audio::bt_volume;
        if !crate::audio::on_bluetooth() {
            self.bt_volume_sent = None;
            self.bt_volume_seen = None;
            return;
        }
        let level = self.state.volume_hp;
        if let Some(read) = bt_volume::level() {
            if self.bt_volume_seen != Some(read) {
                self.bt_volume_seen = Some(read);
                // changed on the headphones: a step of their own, not the rounding of ours
                if self.bt_volume_sent.is_some() && read.abs_diff(level) > 1 {
                    self.state.volume_hp = read;
                    self.bt_volume_sent = Some(read);
                    self.hud
                        .show(HudKind::Volume, self.output_volume(), self.muted(), now);
                    self.persist();
                    return;
                }
            }
        }
        if self.bt_volume_sent != Some(level) {
            bt_volume::set(level);
            self.bt_volume_sent = Some(level);
            self.bt_volume_seen = Some(level);
        }
    }

    pub fn output_volume(&self) -> u8 {
        if self.muted() {
            0
        } else {
            // AirPods hearing their wearer talk turn the sound down until they stop
            let duck = self.phones.phones().map_or(1.0, |p| p.duck);
            (f32::from(self.level()) * duck).round() as u8
        }
    }

    pub fn hud_icon(&self) -> GaugeIcon {
        self.hud.glyph()
    }

    pub fn now(&self) -> Millis {
        self.clock as Millis
    }

    fn flick(&mut self, step: fn(&mut Polaroids)) {
        if let Some(p) = &mut self.polaroids {
            step(p);
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.clock += dt as f64 * 1000.0;
        self.step_boot_fade(dt);
        self.step_opening(dt);
        self.step_start_notice(dt);
        // a letter jumped to shows in the pill for a while, then the system comes back
        if self.pill_letter.is_some()
            && self
                .shelf_named
                .is_some_and(|at| self.now().saturating_sub(at) > LETTER_SHOWN_MS)
        {
            self.pill_letter = None;
            self.shelf_platform = omega_ui::Printed::default();
            self.name_pending = true;
        }
        // the pill rises with the shelf's system once the shelf can be seen
        if self.name_pending
            && self.shelf_platform.face.is_some()
            && !self.opening_covers()
            && !self.start_notice_hides_shelf()
        {
            self.name_pending = false;
            self.shelf_named = Some(self.now());
        }
        self.timers();
        self.poll_link();
        self.poll_rooms();
        self.poll_backup();
        let now = self.now();
        let ready = self.core_faces_ready();
        if let Some(picker) = &mut self.core_picker {
            if picker.waiting() && (ready || picker.waited(now) >= FACES_WAIT_MS) {
                picker.start(now);
            }
        }
        if self.core_picker.is_some_and(|p| p.finished(now)) {
            self.core_picker = None;
        }
        if !self.on_shelf() {
            self.shelf_mut().release_hold();
        }
        self.tick_shelf_sound();
        // the shelf keeps swaying while it slides away or back
        if self.shelf_shown() {
            let picking = self.core_picker.is_some();
            let shelf = self.shelf_mut();
            shelf.tick(now);
            shelf.hold_still(picking);
            shelf.update(dt);
        }
        if self.slide.as_ref().is_some_and(|s| s.over(now)) {
            self.slide = None;
        }
        if self.power_slide.is_some_and(|s| s.over(now)) {
            self.power_slide = None;
        }
        self.step_spin(dt);
        let mut touched = false;
        // a UMD whose disc is spinning in its window waits up for its game
        let spinning = match &self.phase {
            Phase::Inserting { cart, .. } => self
                .spin_disc
                .as_ref()
                .is_some_and(|(stem, ..)| stem == cart),
            _ => false,
        };
        let spun = self.spin.0 >= DISC_SPUN;
        let next = match &mut self.phase {
            Phase::Inserting {
                cart,
                t,
                core_ready,
                resumed,
                ..
            } => {
                let was = *t;
                *t += dt;
                // a UMD is held up at the top of its rise while its disc spins up, and goes
                // in once it has spun at full speed a moment; its game opens behind the dark
                let hold = SEATED_AT * LOADING_HOLD;
                if spinning && !spun && !*resumed && was <= hold {
                    *t = t.min(hold);
                }
                let at = SEATED_AT - Sfx::Insert.lead();
                touched = !*resumed && was < at && *t >= at;
                (*t >= INSERT_S && *core_ready).then(|| Phase::Playing {
                    cart: std::mem::take(cart),
                })
            }
            Phase::Ejecting { t, .. } => {
                if self.screen <= 0.0 {
                    let was = *t;
                    *t += dt;
                    touched = was < 0.0 && *t >= 0.0;
                }
                (*t >= EJECT_S).then_some(Phase::Shelf)
            }
            _ => None,
        };
        if touched {
            self.sfx = Some(match self.phase {
                Phase::Ejecting { .. } => Sfx::Eject,
                _ => Sfx::Insert,
            });
        }
        if let Some(phase) = next {
            self.phase = phase;
            self.refused_from = None;
            let seated = match &self.phase {
                Phase::Playing { cart } => Some(cart.clone()),
                _ => None,
            };
            if seated.is_none() {
                self.link_loaded = None;
            }
            self.record_cart(seated);
        }
        self.step_screen(dt);
    }

    fn step_screen(&mut self, dt: f32) {
        let lit = matches!(self.phase, Phase::Playing { .. } | Phase::Polaroids { .. });
        let step = if lit {
            dt / POWER_ON_S
        } else {
            -dt / POWER_OFF_S
        };
        self.screen = (self.screen + step).clamp(0.0, 1.0);
    }

    pub fn screen_power(&self) -> f32 {
        self.screen
    }

    pub fn set_game_ready(&mut self, ready: bool) {
        self.game_ready = ready;
    }

    pub fn game_visible(&self) -> bool {
        self.game_ready && self.screen > 0.0
    }

    pub fn tick_ms(&mut self, now: Millis) {
        self.clock = self.clock.max(now as f64);
        self.timers();
    }

    fn timers(&mut self) {
        self.tick_achievements();
        let (now, watch) = (self.now(), self.wifi_watch());
        self.wifi.tick(now, watch);
        self.bluetooth.tick(now, self.bluetooth_watch());
        let audio = self.bluetooth.connected_audio();
        crate::audio::bt_volume::watch(audio.as_deref());
        let ready = self.bluetooth_volume_ready(audio.as_deref(), now);
        crate::audio::route_bluetooth(audio.as_deref().filter(|_| ready));
        self.connect_bluetooth_sound(audio.as_deref(), now);
        self.follow_bluetooth_volume(now);
        self.tell_bluetooth_sound();
        self.tell_headphones_noise();
        self.check_for_update(now);
        let phones = self.bluetooth.connected_headphones();
        self.phones.keep(
            phones.as_ref().map(|(mac, kind)| (mac.as_str(), *kind)),
            now,
        );
        self.tell_low_headphones();
        self.leave_gone_headphones();
        self.ssh.tick(now, self.state.ssh);
        self.labels.tick();
        self.take_labels();
        self.play_hold();
        if self.doze_expired() {
            self.on_doze_timeout();
        }
        if self.now() >= self.autosave_at {
            self.autosave();
        }
        if self.now() >= self.battery_at {
            self.battery_at = self.now() + BATTERY_POLL_MS;
            self.battery = self.power.as_ref().and_then(|p| p.battery());
            if let Some(b) = self.battery {
                self.on_battery(b);
            }
        }
        if self.now() >= self.headphones_at {
            self.headphones_at = self.now() + HEADPHONES_POLL_MS;
            let on = self.power.as_ref().is_some_and(|p| p.headphones());
            if on != self.headphones {
                self.headphones = on;
                self.hud.set_headphones(on);
                let now = self.now();
                self.hud
                    .show(HudKind::Volume, self.output_volume(), self.muted(), now);
            }
        }
        if self.now() >= self.charge_at {
            self.charge_at = self.now() + CHARGE_POLL_MS;
            if let (Some(power), Some(b)) = (self.power.as_ref(), self.battery.as_mut()) {
                b.charge = power.charge();
            }
            let state = self.led_state();
            self.set_led(state);
        }
        if let Some(GameMenu::Linked {
            since,
            opened: false,
            ..
        }) = self.game_menu
        {
            if self.now().saturating_sub(since) >= LINKED_HOLD_MS {
                self.game_menu = None;
            }
        }
        if let Some(GameMenu::Unplug { since, .. }) = self.game_menu {
            if self.now().saturating_sub(since) >= UNPLUG_HOLD_MS {
                self.game_menu = None;
            }
        }
        if let Some(at) = self.link.as_ref().and_then(|s| s.lost_at) {
            if self.now().saturating_sub(at) >= LINK_LOST_MS {
                self.end_link();
            }
        }
    }

    pub fn led_state(&self) -> LedState {
        let Some(b) = self.battery else {
            return LedState::Running;
        };
        match b.charge {
            Charge::Charging => LedState::Charging,
            Charge::Full => LedState::Charged,
            _ if b.percent <= BATTERY_LOW => LedState::Low,
            _ => LedState::Running,
        }
    }

    fn set_led(&mut self, state: LedState) {
        if self.shutting_down() && state != LedState::Off {
            return;
        }
        if self.last_led == Some(state) {
            return;
        }
        self.last_led = Some(state);
        if let Some(power) = self.power.as_mut() {
            power.set_led(state);
        }
    }

    fn record_cart(&mut self, cart: Option<String>) {
        let platform = self.seated_cart().map(|c| c.platform);
        if self.state.cart == cart && self.state.cart_platform == platform {
            return;
        }
        self.state.cart = cart;
        self.state.cart_platform = platform;
        self.persist();
    }

    pub(crate) fn persist(&self) {
        let Some(root) = &self.root else {
            return;
        };
        let text = omega_store::app_state_text(&self.state);
        if let Err(e) = crate::card_writer::write(&omega_store::state_path(root), text.into_bytes())
        {
            crate::log!("omega-zero.state: {e}");
        }
    }

    pub fn on_core_ready(&mut self) {
        let Phase::Inserting {
            cart, core_ready, ..
        } = &mut self.phase
        else {
            return;
        };
        *core_ready = true;
        let cart = cart.clone();
        self.retire_refused_resume(&cart);
    }

    fn retire_refused_resume(&mut self, stem: &str) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        if snapshot.resume_trusted() {
            return;
        }
        if !self.named_core {
            return;
        }
        let Some(root) = &self.root else {
            return;
        };
        if let Some(h) = self.pending_save.take() {
            let _ = h.join();
        }
        let ring = StateRing::new(root, self.platform, self.core, stem);
        match ring.retire_resume(&format_stamp(self.wall_secs())) {
            Ok(Some(to)) => crate::log!(
                "resume: {} refused this state, moved it to {}",
                self.core.as_str(),
                to.display()
            ),
            Ok(None) => {}
            Err(e) => crate::log!("resume: could not move the refused state aside: {e}"),
        }
    }

    pub fn on_core_failed(&mut self) {
        let caught = self.seat();
        let Phase::Inserting { cart, .. } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        self.refuse_out(cart, caught);
    }

    fn refuse_out(&mut self, cart: String, caught: f32) {
        let t = (1.0 - caught) * EJECT_S;
        self.phase = Phase::Ejecting { cart, t };
        self.refused_from = Some(t);
    }

    pub fn refuse(&mut self) {
        self.refusal = Some(Refusal::started(self.now()));
    }

    pub fn refusal_active(&self, now: Millis) -> bool {
        self.refusal.is_some_and(|r| r.active(now))
    }

    pub fn seat(&self) -> f32 {
        match &self.phase {
            Phase::Shelf => 0.0,
            Phase::Inserting { t, resumed, .. } => {
                if *resumed {
                    1.0
                } else {
                    (t / SEATED_AT).clamp(0.0, 1.0)
                }
            }
            Phase::Ejecting { t, .. } => 1.0 - (t / EJECT_S).clamp(0.0, 1.0),
            _ => 1.0,
        }
    }

    pub fn draw(&self, out: &mut Vec<Draw>) {
        // nothing under the opening's black, or the start notice's, need be drawn
        if self.opening_covers() || self.start_notice_covers() {
            self.draw_opening(out);
            self.draw_start_notice(out);
            return self.draw_boot_logo(out);
        }
        // the power screens stand over whatever POWER was held on
        if self.power_shown() {
            return self.draw_power(out);
        }
        // the backdrop never moves: the screens slide over it
        if backdrop_under(&self.phase) {
            self.draw_background(out);
        }
        match &self.slide {
            Some(slide) => {
                let (p, w) = (slide.progress(self.now()), out_w() as f32);
                self.draw_screen(&slide.from, offsets(slide.dir, End::Leaving, p, w), out);
                self.draw_screen(&self.phase, offsets(slide.dir, End::Arriving, p, w), out);
            }
            None => self.draw_screen(&self.phase, Offsets::NONE, out),
        }
        if self.pause_menu.is_some() && matches!(self.phase, Phase::Playing { .. }) {
            self.draw_pause_menu(out);
        }
        self.draw_bar(out);
        if matches!(
            self.phase,
            Phase::SetClock { .. } | Phase::About | Phase::Doze { .. }
        ) {
            return;
        }
        if let Some(picker) = self.core_picker_shown() {
            self.draw_core_picker(&picker, out);
        }
        if let Some(menu) = self.game_menu {
            self.draw_game_menu(menu, out);
        }
        if let Some(screen) = &self.disc_screen {
            self.draw_disc_screen(screen, out);
        }
        match self.phase {
            // the save states' panel holds the top: a toast stands over the bar instead
            Phase::Polaroids { .. } => {
                let bar = omega_ui::kit::action::TALL * omega_ui::SPRITE_PX;
                self.hud
                    .draw_above(self.now(), out_h().saturating_sub(bar) as f32, out)
            }
            _ => self.hud.draw_at(self.now(), self.top_bar_reach(), out),
        }
        self.draw_opening(out);
        self.draw_start_notice(out);
        self.draw_boot_logo(out);
    }

    /// How far down the screen the bar across the top reaches right now: all of it on the
    /// screens that show it, none on the rest, and what is left of it while it slides away or
    /// back with the shelf.
    fn top_bar_reach(&self) -> f32 {
        let shows = |phase: &Phase| match phase {
            Phase::Shelf | Phase::Ejecting { .. } | Phase::Polaroids { .. } => true,
            Phase::Inserting { resumed, .. } => !resumed,
            _ => false,
        };
        let reach = |phase: &Phase, off: Offsets| match shows(phase) {
            true => (TOP_BAR_H + off.bar_dy).max(0.0),
            false => 0.0,
        };
        match &self.slide {
            Some(slide) => {
                let (p, w) = (slide.progress(self.now()), out_w() as f32);
                let from = reach(&slide.from, offsets(slide.dir, End::Leaving, p, w));
                let to = reach(&self.phase, offsets(slide.dir, End::Arriving, p, w));
                from.max(to)
            }
            None => reach(&self.phase, Offsets::NONE),
        }
    }

    /// Draws `phase`, a screen of a slide or the one shown, moved by `off`. A menu screen
    /// moves as one; the shelf moves its carts and its top bar each their own way.
    pub(crate) fn draw_screen(&self, phase: &Phase, off: Offsets, out: &mut Vec<Draw>) {
        let start = out.len();
        match phase {
            Phase::Shelf => return self.draw_shelf(off, out),
            // the lone panels stand over the backdrop, with nothing between
            Phase::SetClock { .. } => self.draw_clock(out),
            // the modal stands over the backdrop, which shows round it
            Phase::Settings { row } => self.draw_settings(*row, out),
            Phase::Controls => self.draw_controls(out),
            Phase::Background(_) => self.draw_background_screen(out),
            Phase::Options(screen) if screen.typing.is_none() => {
                self.draw_options(screen.page, out)
            }
            Phase::Labels(_) if typing_of(phase).is_none() => self.draw_labels(out),
            // the modal and its panels stand over the backdrop, with nothing between
            Phase::Network(screen) if screen.typing.is_none() => self.draw_network(out),
            // the keyboard's modal stands on the backdrop itself
            Phase::Network(_) | Phase::Labels(_) | Phase::Options(_) => {
                if let Some(t) = typing_of(phase) {
                    self.draw_keyboard(&t, out);
                }
            }
            Phase::Bluetooth(_) => self.draw_bluetooth(out),
            Phase::Info(_) => self.draw_info(out),
            Phase::About => self.draw_about(out),
            Phase::Inserting { cart, resumed, .. } => {
                if !resumed {
                    self.shelf()
                        .draw_row(Some(cart), 0.0, self.seat(), 1.0, out);
                    self.draw_top_bar(out);
                }
                self.dive(cart, self.seat(), out);
            }
            Phase::Ejecting { cart, .. } => {
                self.shelf()
                    .draw_row(Some(cart), 0.0, self.seat(), 1.0, out);
                self.draw_top_bar(out);
                self.dive(cart, self.seat(), out);
            }
            Phase::Playing { cart } if self.screen < 1.0 => self.dive(cart, 0.0, out),
            Phase::Playing { .. } => self.push_game(out),
            // the save states over the game, under the shelf's top bar; the bar along the bottom
            // is drawn after every screen
            Phase::Polaroids { .. } => {
                self.push_game(out);
                if let Some(p) = &self.polaroids {
                    p.draw(out);
                }
                self.draw_top_bar(out);
                self.draw_delete(out);
            }
            Phase::Doze { .. } => out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: out_w() as f32,
                h: out_h() as f32,
                colour: [0.0, 0.0, 0.0, 1.0],
            }),
        }
        for d in &mut out[start..] {
            *d = d.shifted(off.dx, off.dy);
        }
    }

    pub fn screen_shake(&self) -> f32 {
        self.shake_at(self.now())
    }

    pub fn shake_at(&self, now: Millis) -> f32 {
        self.shake_when(matches!(self.phase, Phase::Playing { .. }), now)
    }

    pub fn shelf_shake(&self) -> f32 {
        self.shake_when(self.on_shelf() && self.core_picker.is_none(), self.now())
    }

    fn shake_when(&self, mine: bool, now: Millis) -> f32 {
        if !mine {
            return 0.0;
        }
        self.refusal.map_or(0.0, |r| r.offset(now))
    }

    fn push_game(&self, out: &mut Vec<Draw>) {
        if self.game_visible() {
            out.push(Draw::Game);
        }
    }

    /// The cart `stem` on its way into the handheld or out of it, the screen behind it `dim`.
    fn dive(&self, stem: &str, dim: f32, out: &mut Vec<Draw>) {
        let Some((cart, face)) = self.shelf().find(stem) else {
            return;
        };
        let alpha = self.alert_alpha();
        let (rest, scale) = self.shelf().selected_at();
        CartDive {
            cart,
            face,
            mesh: self.shelf().mesh_for(stem),
            stack: self.shelf().stack_for(stem),
            rest,
            scale,
            pose: self.shelf().pose(),
            seat: self.seat(),
            alert: self.alert_face.filter(|_| alpha > 0.0).map(|t| (t, alpha)),
            dim,
            screen: self.screen,
            game: self.game_ready,
            disc: self.spinning_disc(stem),
        }
        .draw(out);
    }

    /// The UMD picked on the shelf, or going in, while its disc has no texture yet: for the
    /// frontend to make one, ahead of the game starting.
    pub fn wants_spin_disc(&self) -> Option<omega_store::Cart> {
        let c = match &self.phase {
            Phase::Inserting { cart, .. } => self.shelf().find(cart)?.0,
            Phase::Shelf => {
                let shelf = self.shelf();
                &shelf.carts[shelf.cart_at_offset(0)?]
            }
            _ => return None,
        };
        if self.spin_disc.as_ref().is_some_and(|(s, ..)| *s == c.stem) {
            return None;
        }
        (omega_ui::model_of(c) == omega_ui::CartModel::Umd).then(|| c.clone())
    }

    /// The disc of the UMD by `stem`, the same disc as a blur of rings, and the hub drawn still
    /// over it.
    pub fn set_spin_disc(&mut self, stem: String, tex: TexId, blur: TexId, hub: TexId) {
        self.spin_disc = Some((stem, tex, blur, hub));
    }

    /// The disc of the UMD going in, how far it has turned, and how far over the last frame:
    /// while it goes in.
    fn spinning_disc(&self, stem: &str) -> Option<omega_ui::SpinningDisc> {
        if !matches!(self.phase, Phase::Inserting { .. }) {
            return None;
        }
        let (s, tex, blur, hub) = self.spin_disc.as_ref()?;
        let (age, turn) = self.spin;
        // seen turning up to 3 turns a second, a blur of rings from 9, the one into the other
        let k = ((disc_speed(age) - 3.0) / 6.0).clamp(0.0, 1.0);
        (s == stem).then_some(omega_ui::SpinningDisc {
            tex: *tex,
            turn,
            blur: *blur,
            blurred: k * k * (3.0 - 2.0 * k),
            hub: *hub,
        })
    }

    /// Whether a UMD is going in: its disc spins, and its game waits for the spin.
    fn umd_going_in(&self) -> bool {
        match &self.phase {
            Phase::Inserting { cart, .. } => self
                .shelf()
                .find(cart)
                .is_some_and(|(c, _)| omega_ui::model_of(c) == omega_ui::CartModel::Umd),
            _ => false,
        }
    }

    /// Spins the UMD going in on by `dt`: its speed `DISC_SPIN` times 1 - e^-(t/τ)², slow off
    /// the mark and then fast to full, τ being half of `DISC_SPIN_UP`.
    fn step_spin(&mut self, dt: f32) {
        if !self.umd_going_in() {
            self.spin = (0.0, 0.0);
            return;
        }
        let (age, turn) = self.spin;
        let age = age + dt;
        let speed = disc_speed(age);
        self.spin = (
            age,
            (turn + speed * std::f32::consts::TAU * dt) % std::f32::consts::TAU,
        );
    }

    /// How fast the UMD going in spins, in turns a second; 0 for any other.
    pub fn disc_turns(&self) -> f32 {
        if self.umd_going_in() {
            disc_speed(self.spin.0)
        } else {
            0.0
        }
    }

    /// Whether the core may be opened for the game going in. A UMD's waits until the UMD is
    /// all the way in and the screen dark, so the core's opening, the heaviest work the
    /// handheld does, never lands on the spin or the UMD's way in: the game starts a little
    /// later, and the animation is clean.
    pub fn core_may_open(&self) -> bool {
        !self.umd_going_in() || self.seat() >= 1.0
    }

    pub fn alert_visible(&self) -> bool {
        self.alert_alpha() > 0.0
    }

    /// How much of the refused cart's alert shows: all of it until the cart is `ALERT_HOLD` of
    /// the way out, then fading in the kit's four steps until it is gone at `ALERT_GONE`.
    pub fn alert_alpha(&self) -> f32 {
        let (Some(from), Phase::Ejecting { t, .. }) = (self.refused_from, &self.phase) else {
            return 0.0;
        };
        let span = EJECT_S - from;
        if span <= 0.0 {
            return 0.0;
        }
        let u = ((t - from) / span).clamp(0.0, 1.0);
        stepped((ALERT_GONE - u) / (ALERT_GONE - ALERT_HOLD))
    }

    pub fn set_alert_face(&mut self, face: TexId) {
        self.alert_face = Some(face);
    }

    pub fn set_core_board_faces(&mut self, board: TexId, lid: TexId) {
        self.core_board_face = Some(board);
        self.core_lid_face = Some(lid);
        self.core_faces_stem = self.selected_stem().map(str::to_string);
    }

    /// The core picker's parts: each core's socket and named chip, indexed by `Core::index`; the
    /// blank chip in the air and its shadow, both sized for the GBA board's sockets; and the
    /// lid's shadow.
    pub fn set_core_part_faces(
        &mut self,
        sockets: Vec<TexId>,
        chips: Vec<TexId>,
        blank: TexId,
        [chip_shadow, lid_shadow]: [TexId; 2],
    ) {
        self.core_socket_faces = sockets;
        self.core_chip_faces = chips;
        self.core_blank_chip_face = Some(blank);
        self.core_chip_shadow_face = Some(chip_shadow);
        self.core_lid_shadow_face = Some(lid_shadow);
    }

    /// The blank chip in the air and its shadow for a board whose sockets are not the GBA's,
    /// such as the SNES's, each sized for them.
    pub fn set_chip_faces_for(&mut self, sockets: Sockets, blank: TexId, shadow: TexId) {
        self.core_chip_faces_for
            .retain(|(fits, _, _)| *fits != sockets);
        self.core_chip_faces_for.push((sockets, blank, shadow));
    }

    /// The blank chip and its shadow that fit `sockets`.
    fn chip_faces_for(&self, sockets: Sockets) -> (Option<TexId>, Option<TexId>) {
        if sockets == GBA_SOCKETS {
            return (self.core_blank_chip_face, self.core_chip_shadow_face);
        }
        self.core_chip_faces_for
            .iter()
            .find(|(fits, _, _)| *fits == sockets)
            .map_or((None, None), |(_, blank, shadow)| {
                (Some(*blank), Some(*shadow))
            })
    }

    pub(crate) fn core_picker_shown(&self) -> Option<CorePicker> {
        self.core_picker.filter(|p| !p.waiting())
    }

    /// The model the cart the shelf is on stands as.
    fn selected_model(&self) -> CartModel {
        self.shelf()
            .model_at(self.shelf().index)
            .unwrap_or(CartModel::Gba)
    }

    fn draw_core_picker(&self, picker: &CorePicker, out: &mut Vec<Draw>) {
        let now = self.now();
        let progress = picker.openness(now);
        let lift = lift_of(progress);
        let (rest, scale) = self.shelf().selected_at();
        // The cart leaves the shelf held the way it was swaying there, and squares up as it
        // opens.
        let pose = self.shelf().pose().toward_rest(lift);
        let model = self.selected_model();
        let shelf = model_cart_at(model, rest, scale);
        let shelf = Placed {
            y: shelf.y - pose.lift,
            ..shelf
        };
        let board = board_from(shelf, progress);
        let zoom = board_zoom(board);
        let ready = self.core_faces_ready();
        let cores = picker.cores();
        let sockets = sockets_of(cores[0]);

        if ready {
            if let Some(tex) = self.core_board_face {
                out.push(posed_face(
                    (board.x, board.y, board.w, board.h),
                    tex,
                    1.0,
                    0.0,
                    pose.yaw,
                ));
            }
            // the parts are pixel art like the board, a texel to `px` screen pixels as it is
            // drawn, each put down on whole art pixels
            let px = SPRITE_PX as f32 * zoom;
            for (core, u) in cores.iter().zip(sockets.u) {
                let Some(tex) = self.core_socket_faces.get(core.index()).copied() else {
                    continue;
                };
                let (x, y) = on_board(board, u, sockets.v);
                out.push(Draw::Tex {
                    x: even(x),
                    y: even(y),
                    w: sockets.w as f32 * px,
                    h: sockets.h as f32 * px,
                    tex,
                    alpha: 1.0,
                });
            }

            // the chip crosses and hops a whole art pixel at a time, level: a turned face
            // would leave the grid
            let chip = picker.chip(now);
            let (first, second) = (sockets.chip_u(0), sockets.chip_u(1));
            let u = (first + (second - first) * chip.across).round();
            let hop = (HOP_LIFT * chip.lift).round();
            let (blank, shadow) = self.chip_faces_for(sockets);
            if hop > 0.0 {
                if let Some(tex) = shadow {
                    let (x, y) = on_board(board, u + SHADOW_AT[0], sockets.chip_v() + SHADOW_AT[1]);
                    out.push(Draw::Tex {
                        x: even(x),
                        y: even(y),
                        w: sockets.shadow_w() as f32 * px,
                        h: SHADOW_H as f32 * px,
                        tex,
                        alpha: stepped(chip.lift * lift),
                    });
                }
            }
            let face = match chip.seated {
                Some(core) => self.core_chip_faces.get(core.index()).copied(),
                None => blank,
            };
            if let Some(tex) = face {
                let (x, y) = on_board(board, u, sockets.chip_v() - hop);
                out.push(Draw::Tex {
                    x: even(x + chip.shake),
                    y: even(y),
                    w: sockets.chip_w as f32 * px,
                    h: sockets.chip_h as f32 * px,
                    tex,
                    alpha: 1.0,
                });
            }
        }

        // the lid's shadow, a dithered block at the size it rests at, fading in steps as it lifts
        if let Some(tex) = self.core_lid_shadow_face {
            let (lid, _) = lid_of(model, shelf, progress);
            let (w, h) = (
                (LID_SHADOW_W * SPRITE_PX) as f32,
                (LID_SHADOW_H * SPRITE_PX) as f32,
            );
            out.push(Draw::Tex {
                x: even(lid.x + (lid.w - w) / 2.0),
                y: even(lid.y + lid.h + LID_SHADOW_DROP - h / 2.0),
                w,
                h,
                tex,
                alpha: stepped(lift),
            });
        }

        let meshed = self.selected_stem().and_then(|stem| {
            let (_, face) = self.shelf().find(stem)?;
            face.zip(self.shelf().mesh_for(stem))
        });
        let (lid, turn) = lid_of(model, shelf, progress);
        if let Some((tex, mesh)) = meshed {
            // the lid is the cart itself, sides and all, in the model the shelf drew it with
            out.push(posed_cart(
                (lid.x, lid.y, lid.w, lid.h),
                mesh,
                tex,
                [1.0; 4],
                turn,
                pose.yaw,
            ));
        } else if ready {
            if let Some(tex) = self.core_lid_face {
                // the lid's face is padded `TURN_PAD` texels each side, so it can turn unclipped
                let face_w = model_box(model).0 as f32;
                let at = grown(lid, TURN_PAD as f32 * lid.w / face_w);
                push_lid(out, (at.x, at.y, at.w, at.h), tex, turn, pose.yaw);
            }
        } else if let Some((_, Some(tex))) = self
            .selected_stem()
            .and_then(|stem| self.shelf().find(stem))
        {
            push_lid(out, (lid.x, lid.y, lid.w, lid.h), tex, turn, pose.yaw);
        }
    }

    fn auto_link(&self, stem: &str) -> Option<(&Cart, LinkKind)> {
        let cart = self.shelf().carts.iter().find(|c| c.stem == stem)?;
        let auto = link_kind(
            &cart.code,
            &cart.title,
            omega_store::header_clean(&cart.rom),
        );
        Some((cart, auto))
    }

    fn on_shelf(&self) -> bool {
        matches!(self.phase, Phase::Shelf)
    }

    /// The shelf is on the screen: shown, or sliding away or back.
    fn shelf_shown(&self) -> bool {
        self.on_shelf()
            || self
                .slide
                .as_ref()
                .is_some_and(|s| matches!(s.from, Phase::Shelf))
    }

    fn insert(&mut self, clean: bool) {
        // no game starts under the start notice
        if !self.on_shelf() || self.start_notice_shown() {
            return;
        }
        let Some(picked) = self.shelf().carts.get(self.shelf().index).cloned() else {
            return;
        };
        let cart = picked.stem.clone();
        self.note_recent(&picked);
        self.play_held = None;
        self.refusal = None;
        self.refused_from = None;
        // a game goes in with its first disc in the drive
        self.disc_in = None;
        self.disc_swap = None;
        self.phase = Phase::Inserting {
            cart,
            t: 0.0,
            core_ready: false,
            resumed: false,
            clean,
        };
    }

    pub fn starting_clean(&self) -> bool {
        matches!(self.phase, Phase::Inserting { clean: true, .. })
    }

    fn play_hold(&mut self) {
        let Some(at) = self.play_held else {
            return;
        };
        if !self.on_shelf() {
            self.play_held = None;
            return;
        }
        if self.now().saturating_sub(at) >= PLAY_HOLD_MS {
            self.insert(true);
        }
    }

    pub(crate) fn eject(&mut self) {
        if self.single_cart() {
            return self.refuse();
        }
        let cart = match &mut self.phase {
            Phase::Playing { cart } | Phase::Inserting { cart, .. } => std::mem::take(cart),
            _ => return,
        };
        self.end_link();
        self.close_game_menu();
        self.close_pause_menu();
        self.flush_eject(&cart);
        self.refusal = None;
        self.refused_from = None;
        self.phase = Phase::Ejecting {
            cart,
            t: -EJECT_HOLD_S,
        };
    }

    fn doze(&mut self) {
        if self.link_active() {
            self.end_link();
        }
        self.close_game_menu();
        self.close_pause_menu();
        self.core_picker = None;
        self.slide = None;
        if matches!(self.phase, Phase::Doze { .. }) {
            return;
        }
        self.flush_resume();
        let cart = match &mut self.phase {
            Phase::Playing { cart } | Phase::Polaroids { cart } => Some(std::mem::take(cart)),
            _ => None,
        };
        self.polaroids = None;
        self.phase = Phase::Doze { cart };
        self.dozed_at = self.now();
        self.radio.ask(RadioJob::Cool);
        if let Some(power) = &mut self.power {
            power.on_close();
        }
    }

    fn wake(&mut self) {
        let Phase::Doze { cart } = &mut self.phase else {
            return;
        };
        self.phase = match cart.take() {
            Some(cart) => Phase::Playing { cart },
            None => Phase::Shelf,
        };
        if let Some(power) = &mut self.power {
            power.on_open();
        }
    }

    pub fn on_doze_timeout(&mut self) {
        if !matches!(self.phase, Phase::Doze { .. }) {
            return;
        }
        self.begin_power_off();
    }

    fn power_press(&mut self) {
        if std::mem::take(&mut self.woke_on_press) {
            return;
        }
        match self.phase {
            Phase::Doze { .. } => self.wake(),
            _ => self.doze(),
        }
    }

    /// Opens the core picker over the cart the shelf is on, with a socket for each core that runs
    /// its system; a system with one core has nothing to pick, and X does nothing.
    fn open_core_picker(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let Some(cart) = self.shelf().carts.get(self.shelf().index) else {
            return;
        };
        let cores = Core::for_platform(cart.platform);
        if cores.len() < 2 {
            return;
        }
        let seat = omega_store::core_for_platform(&root, &cart.stem, cart.platform);
        let default = omega_store::default_core(&root, cart.platform);
        let switched = seat != default;
        let now = self.now();
        let mut picker = CorePicker::open(cores, seat, default, now);
        // a game switched by hand: Y puts it back on the default
        if switched {
            self.hud
                .toast(Toast::Emulator("Y puts it on the default"), now);
        }
        if self.core_faces_ready() {
            picker.start(now);
        }
        self.core_picker = Some(picker);
        self.shelf_mut().release_hold();
        self.play_held = None;
    }

    fn core_faces_ready(&self) -> bool {
        self.core_faces_stem
            .as_deref()
            .is_some_and(|stem| self.selected_stem() == Some(stem))
    }

    fn core_picker_input(&mut self, action: Action) {
        let press = match action {
            Action::GbaDown(Btn::Left) | Action::ShelfLeft => Press::Left,
            Action::GbaDown(Btn::Right) | Action::ShelfRight => Press::Right,
            Action::GbaDown(Btn::A) => Press::Keep,
            Action::GbaDown(Btn::B) => Press::Back,
            Action::GbaDown(Btn::Y) => Press::Default,
            _ => return,
        };
        let now = self.now();
        let Some(picker) = &mut self.core_picker else {
            return;
        };
        let outcome = picker.press(press, now);
        if let Outcome::Write(core) = outcome {
            self.write_core(core);
        }
    }

    /// Keeps `core` for the cart the shelf is on: its system's default is kept as none, so the
    /// cart follows the default wherever Settings moves it.
    fn write_core(&self, core: Core) {
        let (Some(root), Some(cart)) = (
            self.root.clone(),
            self.shelf().carts.get(self.shelf().index),
        ) else {
            return;
        };
        let written = match core == omega_store::default_core(&root, cart.platform) {
            true => omega_store::clear_selected_core(&root, &cart.stem),
            false => omega_store::write_selected_core(&root, &cart.stem, core),
        };
        if let Err(e) = written {
            crate::log!("core: could not write selected_core.ini: {e}");
        }
    }

    /// Powers off once the screen saying so has slid in, ending a link session first. Once the
    /// handheld is going down, either way, nothing changes that.
    /// Holding POWER: the game's resume is made durable, then the handheld powers off, with
    /// the screen saying so. A link under way is ended first.
    pub(crate) fn power_off_now(&mut self) {
        if self.shutting_down() {
            return;
        }
        self.close_pause_menu();
        self.flush_resume();
        self.finish_slide();
        self.begin_power_off();
    }

    /// Restarts at once, as holding POWER powers off: START pressed while it is held.
    pub(crate) fn restart_now(&mut self) {
        if self.shutting_down() {
            return;
        }
        self.close_pause_menu();
        self.flush_resume();
        self.finish_slide();
        if self.link_active() {
            self.end_link();
        }
        self.begin_restart();
    }

    pub(crate) fn begin_power_off(&mut self) {
        if self.shutting_down() {
            return;
        }
        self.remember_pick();
        if self.link_active() {
            self.end_link();
        }
        self.close_game_menu();
        self.powering_off = true;
        self.act_at = self.now() + SHUTDOWN_SHOW_MS;
        self.set_led(LedState::Off);
        self.show_going(PowerChoice::PowerOff);
    }

    /// Restarts once the screen saying so has slid in.
    pub(crate) fn begin_restart(&mut self) {
        if self.shutting_down() {
            return;
        }
        self.remember_pick();
        self.close_game_menu();
        self.restarting = true;
        self.act_at = self.now() + SHUTDOWN_SHOW_MS;
        self.set_led(LedState::Off);
        self.show_going(PowerChoice::Restart);
    }

    pub fn on_battery(&mut self, b: Battery) {
        if b.percent > BATTERY_CRITICAL || self.powering_off {
            return;
        }
        if matches!(b.charge, Charge::Charging | Charge::Full) {
            return;
        }
        self.flush_resume();
        self.begin_power_off();
    }

    fn doze_expired(&self) -> bool {
        if self.link_active() {
            return false;
        }
        let (Phase::Doze { .. }, Some(power)) = (&self.phase, &self.power) else {
            return false;
        };
        self.now().saturating_sub(self.dozed_at) >= power.timeout().as_millis() as Millis
    }

    /// What shows over the screen for a moment or while it lasts: the gauge, the toast and the
    /// badge in the corner.
    pub fn hud(&self) -> &Hud {
        &self.hud
    }

    /// The HUD, for the frontend to hand it the faces it asks for.
    pub fn hud_mut(&mut self) -> &mut Hud {
        &mut self.hud
    }

    pub fn toast(&self) -> Option<Toast> {
        self.hud.said(self.now())
    }

    pub fn polaroid_entries(&self) -> &[StateEntry] {
        match &self.polaroids {
            Some(p) => &p.entries,
            None => &[],
        }
    }

    pub fn set_polaroid_faces(&mut self, faces: Vec<TexId>) {
        if let Some(p) = &mut self.polaroids {
            p.set_faces(faces);
        }
    }

    /// The save-state switcher while it is open.
    pub fn polaroids(&self) -> Option<&Polaroids> {
        self.polaroids.as_ref()
    }

    /// The switcher's SAVE STATES panel, uploaded as `tex`, `w` by `h` art pixels.
    pub fn set_polaroid_panel(&mut self, tex: TexId, w: u32, h: u32) {
        if let Some(p) = &mut self.polaroids {
            p.set_panel_face(tex, w, h);
        }
    }
}

/// What an autosave writes: taken already, or to be taken through a snapshot on the writer's
/// thread.
enum Taken {
    Now((Option<Vec<u8>>, Option<Vec<u8>>)),
    Later(Box<dyn Snapshot + Send>),
}

fn trusted_write(
    snapshot: &dyn Snapshot,
    state: Vec<u8>,
    verb: &str,
) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let state = if snapshot.resume_trusted() {
        Some(state)
    } else {
        crate::log!(
            "{verb}: the core refused the resume it was given, not overwriting the saved one"
        );
        None
    };
    let sav = snapshot.save_ram();
    let sav = if snapshot.save_ram_trusted() {
        sav
    } else {
        if sav.is_some() {
            crate::log!(
                "{verb}: the core refused the save ram it was given, not overwriting the saved one"
            );
        }
        None
    };
    (state, sav)
}

/// How fast a UMD's disc spins `age` seconds after it starts, in turns a second.
fn disc_speed(age: f32) -> f32 {
    let tau = DISC_SPIN_UP / 2.0;
    DISC_SPIN * (1.0 - (-(age / tau).powi(2)).exp())
}

fn up(level: u8, step: u8, max: u8) -> u8 {
    level.saturating_add(step).min(max)
}

/// Whether the backdrop is drawn under `phase`: under the shelf and every menu, and under a
/// cart going in or coming out, but not under a game, nor a cart resumed at boot.
pub(crate) fn backdrop_under(phase: &Phase) -> bool {
    !matches!(
        phase,
        Phase::Playing { .. }
            | Phase::Polaroids { .. }
            | Phase::Doze { .. }
            | Phase::Inserting { resumed: true, .. }
    )
}

fn system_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn free_stamp(ring: &StateRing, now: i64) -> String {
    let taken: Vec<String> = ring
        .list()
        .map(|l| l.into_iter().map(|e| e.stamp).collect())
        .unwrap_or_default();
    let mut secs = now;
    let mut stamp = format_stamp(secs);
    while taken.contains(&stamp) {
        secs += 1;
        stamp = format_stamp(secs);
    }
    stamp
}
