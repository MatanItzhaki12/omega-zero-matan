use std::path::{Path, PathBuf};

use crate::atomic::atomic_write;
use crate::backdrop::{colour_index, BackdropStyle, BACKDROP_COLOURS};
use crate::blending::Blending;
use crate::font::Font;
use crate::gb_palette::GbColors;
use crate::platform::Platform;
use crate::screen_filter::ScreenFilter;

pub const BRIGHTNESS_MAX: u8 = 9;
pub const BLUE_LIGHT_MAX: u8 = 9;
pub const VOLUME_MAX: u8 = 100;

pub const UTC_OFFSET_MIN: i16 = -720;
pub const UTC_OFFSET_MAX: i16 = 840;

/// The fast-forward speeds Settings steps through; 0 is OFF, as on Slot.
pub const FF_SPEEDS: [u8; 5] = [0, 2, 3, 4, 6];

/// How long a shut lid sleeps before the handheld powers off, as Slot offers it.
pub const LID_OFF_MINUTES: [u8; 6] = [1, 3, 5, 10, 15, 30];

pub const FF_SPEED_DEFAULT: u8 = 6;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AppState {
    pub cart: Option<String>,
    pub cart_platform: Option<Platform>,
    pub brightness: u8,
    pub blue_light: u8,
    pub volume: u8,
    pub volume_hp: u8,
    pub muted: bool,
    pub muted_hp: bool,
    pub clock_set: bool,
    pub utc_offset_min: i16,
    pub rumble: bool,
    pub ff_speed: u8,
    pub ff_sound: bool,
    pub colour_correction: bool,
    /// What a Game Boy or GBA game at 3x is drawn through.
    pub screen_filter: ScreenFilter,
    /// Home Wi-Fi is switched on.
    pub wifi: bool,
    /// SSH is switched on: BaseOS's dev images start its server at boot, and it is left
    /// running unless this is off.
    pub ssh: bool,
    /// Bluetooth is switched on: omega-zero brings the radio up at boot, which BaseOS leaves
    /// blocked.
    pub bluetooth: bool,
    /// The shelf's background, once one was picked. Until then a card with a wallpaper shows
    /// it, and any other the default pattern.
    pub backdrop: Option<BackdropStyle>,
    /// The pattern drifts.
    pub backdrop_moving: bool,
    /// Which of [`BACKDROP_COLOURS`] the pattern is in.
    pub backdrop_colour: u8,
    /// The font the bar's actions write their big words in.
    pub font: Font,
    /// Putting a cart in picks up from the state it was ejected in.
    pub auto_load: bool,
    /// Ejecting a cart saves a state to pick up from.
    pub eject_save: bool,
    /// Holding L2 rewinds.
    pub rewind: bool,
    /// Held X and Y are turbo A and B.
    pub turbo: bool,
    /// Frame blending, on the GBA and the Game Boys.
    pub blending: Blending,
    /// The colours an original Game Boy game is drawn in.
    pub gb_colors: GbColors,
    /// A RECENTS shelf of the last games played stands first.
    pub show_recents: bool,
    /// The name of the cart picked is shown over the shelf.
    pub show_titles: bool,
    /// The clock reads 24-hour time, else 12-hour.
    pub clock_24h: bool,
    /// Minutes a shut lid sleeps before the handheld powers off: one of `LID_OFF_MINUTES`.
    pub lid_off_min: u8,
    /// Games are checked for RetroAchievements.
    pub cheevos: bool,
    /// RetroAchievements' hardcore: no state loads, no rewinding.
    pub cheevos_hardcore: bool,
    /// The opening's terminal plays at boot, before the shelf.
    pub boot_animation: bool,
    /// The cart picked on the shelf has the holo sheen over its label.
    pub holo: bool,
    /// Each present shows the frame after the one the game is at.
    pub run_ahead: bool,
    /// The name shown in online rooms, as typed; empty for the one made up for it.
    pub online_name: String,
    /// The relay an online room is opened through, by its place in RetroArch's list.
    pub relay: u8,
    /// The password an online room is hosted with; none when empty.
    pub online_password: String,
    /// The address an online room is joined at, under the link screen's ADVANCED: a host, and a
    /// port after a colon.
    pub online_address: String,
    /// Save Backup: saves are copied to a NAS as they are made.
    pub backup: bool,
    /// The NAS: its address, and the name it was found by, shown in its place.
    pub backup_host: String,
    pub backup_name: String,
    /// Who signs in to the NAS, and their password: no user is a guest.
    pub backup_user: String,
    pub backup_password: String,
    /// The share saves go to on the NAS, and the folder in it, its parts apart by `/`.
    pub backup_share: String,
    pub backup_dir: String,
    /// The shelf and the cart picked when omega-zero last went off, picked again at boot: the
    /// shelf by its key (`recents`, `favorites` or a system's), the cart by its system and stem.
    pub shelf: String,
    pub picked: String,
    pub picked_platform: Option<Platform>,
    /// How a PSP game is shown when it was not set for itself: `fit`, `actual` or `stretch`.
    pub psp_screen: String,
    /// The menus click, chime and swish as they are used.
    pub menu_sounds: bool,
    /// A soft tune plays on the shelf and in its menus, and stops for a game.
    pub menu_music: bool,
    /// How loud the menus' sounds and the tune are, each 1 to `MENU_VOLUME_MAX`.
    pub menu_sounds_volume: u8,
    pub menu_music_volume: u8,
    /// Settings' Update takes the test builds of the `test` branch, not the releases.
    pub test_updates: bool,
    /// How many automatic backups each game keeps, its save backed up by itself as it goes in:
    /// one of `AUTO_BACKUPS`, 0 for none made.
    pub auto_backups: u8,
}

/// How many automatic backups a game may keep, as Saves › Auto Backup steps them: none, or
/// the newest few. Each is a copy of a save, a few KB for most games and some MB for a PSP's.
pub const AUTO_BACKUPS: [u8; 5] = [0, 1, 3, 5, 10];
pub const AUTO_BACKUPS_DEFAULT: u8 = 5;

/// The top of the menu sounds' and the tune's volumes.
pub const MENU_VOLUME_MAX: u8 = 10;
/// Where each starts, halfway: the loudness each was tuned to on the handheld, with as much
/// again above it.
pub const MENU_VOLUME_DEFAULT: u8 = 5;

impl Default for AppState {
    fn default() -> Self {
        AppState {
            cart: None,
            cart_platform: None,
            brightness: 5,
            blue_light: 0,
            volume: 60,
            volume_hp: 60,
            muted: false,
            muted_hp: false,
            clock_set: false,
            utc_offset_min: 0,
            rumble: true,
            ff_speed: FF_SPEED_DEFAULT,
            ff_sound: false,
            colour_correction: false,
            screen_filter: ScreenFilter::Lcd,
            wifi: false,
            ssh: true,
            bluetooth: false,
            backdrop: None,
            backdrop_moving: true,
            backdrop_colour: 0,
            font: Font::Bold,
            auto_load: true,
            eject_save: true,
            rewind: true,
            turbo: true,
            blending: Blending::Off,
            gb_colors: GbColors::Auto,
            show_recents: false,
            show_titles: false,
            clock_24h: true,
            lid_off_min: 3,
            cheevos: false,
            cheevos_hardcore: false,
            boot_animation: false,
            holo: true,
            run_ahead: false,
            online_name: String::new(),
            // Madrid's, the nearest of RetroArch's relays to most of Europe and the Middle East
            relay: 1,
            online_password: String::new(),
            online_address: String::new(),
            backup: false,
            backup_host: String::new(),
            backup_name: String::new(),
            backup_user: String::new(),
            backup_password: String::new(),
            backup_share: String::new(),
            backup_dir: String::new(),
            shelf: String::new(),
            picked: String::new(),
            picked_platform: None,
            psp_screen: "fit".to_string(),
            menu_sounds: true,
            menu_music: true,
            menu_sounds_volume: MENU_VOLUME_DEFAULT,
            menu_music_volume: MENU_VOLUME_DEFAULT,
            test_updates: false,
            auto_backups: AUTO_BACKUPS_DEFAULT,
        }
    }
}

/// Where omega-zero's state is kept on the card at `root`.
pub fn state_path(root: &Path) -> PathBuf {
    root.join("Config").join("omega-zero.state")
}

pub fn read_app_state(root: &Path) -> AppState {
    std::fs::read(state_path(root))
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .and_then(|s| parse(&s))
        .unwrap_or_default()
}

pub fn write_app_state(root: &Path, s: &AppState) -> std::io::Result<()> {
    atomic_write(&state_path(root), app_state_text(s).as_bytes())
}

/// `s` as the state file has it: a `key=value` line for each of [`FIELDS`], in their order.
pub fn app_state_text(s: &AppState) -> String {
    FIELDS.iter().fold(String::new(), |mut text, f| {
        text.push_str(f.key);
        text.push('=');
        text.push_str(&(f.write)(s));
        text.push('\n');
        text
    })
}

/// One setting the state file keeps: its key, how its value is written, and how a value read
/// back is taken in.
///
/// `read` sets the field from the value. A value it cannot take leaves the field as it was, at
/// its default, except for a [`Need::Must`] setting, where it is None: the whole file is then
/// unreadable and read as the default, so a half-written file never boots half set.
struct Field {
    key: &'static str,
    need: Need,
    write: fn(&AppState) -> String,
    read: fn(&mut AppState, &str) -> Option<()>,
}

/// Whether a file without the setting can still be read.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Need {
    /// The file is unreadable without it: the settings the first versions wrote.
    Must,
    /// Without it the setting keeps its default: every setting added since.
    May,
}

/// A field that keeps its default where the value cannot be read.
macro_rules! may {
    ($key:literal, |$s:ident| $write:expr, |$t:ident, $v:ident| $read:expr) => {
        Field {
            key: $key,
            need: Need::May,
            write: |$s: &AppState| $write,
            read: |$t: &mut AppState, $v: &str| {
                $read;
                Some(())
            },
        }
    };
}

/// An on/off field written as 1 or 0.
macro_rules! flag {
    ($key:literal, $field:ident) => {
        may!(
            $key,
            |s| (s.$field as u8).to_string(),
            |t, v| if let Some(b) = flag(v) {
                t.$field = b
            }
        )
    };
}

/// A free-text field, a line to itself.
macro_rules! text {
    ($key:literal, $field:ident) => {
        may!($key, |s| kept_text(&s.$field), |t, v| t.$field =
            kept_text(v))
    };
}

/// Every setting, in the order the file has them. A new setting is a field of [`AppState`],
/// its default, and a line at the end of this list.
const FIELDS: &[Field] = &[
    Field {
        key: "cart",
        need: Need::Must,
        write: |s| s.cart.clone().unwrap_or_default(),
        read: |t, v| {
            t.cart = (!v.is_empty()).then(|| v.to_string());
            Some(())
        },
    },
    may!(
        "cart_platform",
        |s| s.cart_platform.map_or(String::new(), platform_key),
        |t, v| t.cart_platform = platform_value(v)
    ),
    Field {
        key: "brightness",
        need: Need::Must,
        write: |s| s.brightness.to_string(),
        read: |t, v| {
            t.brightness = level(v, BRIGHTNESS_MAX)?;
            Some(())
        },
    },
    Field {
        key: "blue_light",
        need: Need::Must,
        write: |s| s.blue_light.to_string(),
        read: |t, v| {
            t.blue_light = level(v, BLUE_LIGHT_MAX)?;
            Some(())
        },
    },
    Field {
        key: "volume",
        need: Need::Must,
        write: |s| s.volume.to_string(),
        read: |t, v| {
            t.volume = level(v, VOLUME_MAX)?;
            Some(())
        },
    },
    // without its own line the headphones' volume and mute are the speaker's
    Field {
        key: "volume_hp",
        need: Need::May,
        write: |s| s.volume_hp.to_string(),
        read: |t, v| {
            t.volume_hp = level(v, VOLUME_MAX)?;
            Some(())
        },
    },
    Field {
        key: "muted",
        need: Need::Must,
        write: |s| (s.muted as u8).to_string(),
        read: |t, v| {
            t.muted = level(v, 1)? == 1;
            Some(())
        },
    },
    Field {
        key: "muted_hp",
        need: Need::May,
        write: |s| (s.muted_hp as u8).to_string(),
        read: |t, v| {
            t.muted_hp = level(v, 1)? == 1;
            Some(())
        },
    },
    Field {
        key: "clock_set",
        need: Need::Must,
        write: |s| (s.clock_set as u8).to_string(),
        read: |t, v| {
            t.clock_set = level(v, 1)? == 1;
            Some(())
        },
    },
    Field {
        key: "utc_offset_min",
        need: Need::Must,
        write: |s| s.utc_offset_min.to_string(),
        read: |t, v| {
            t.utc_offset_min = offset(v)?;
            Some(())
        },
    },
    flag!("rumble", rumble),
    may!(
        "ff_speed",
        |s| s.ff_speed.to_string(),
        |t, v| if let Some(n) = ff_speed_value(v) {
            t.ff_speed = n;
        }
    ),
    flag!("ff_sound", ff_sound),
    flag!("colour_correction", colour_correction),
    may!(
        "screen_filter",
        |s| s.screen_filter.key().to_string(),
        |t, v| if let Some(f) = ScreenFilter::from_key(v) {
            t.screen_filter = f;
        }
    ),
    flag!("wifi", wifi),
    may!(
        "ssh",
        |s| on_off_text(s.ssh),
        |t, v| if let Some(b) = on_off(v) {
            t.ssh = b;
        }
    ),
    may!(
        "bluetooth",
        |s| on_off_text(s.bluetooth),
        |t, v| if let Some(b) = on_off(v) {
            t.bluetooth = b;
        }
    ),
    may!(
        "backdrop",
        |s| s.backdrop.map_or("", BackdropStyle::key).to_string(),
        |t, v| t.backdrop = BackdropStyle::from_key(v)
    ),
    flag!("backdrop_moving", backdrop_moving),
    may!(
        "backdrop_colour",
        |s| BACKDROP_COLOURS[(s.backdrop_colour as usize).min(BACKDROP_COLOURS.len() - 1)]
            .to_string(),
        |t, v| if let Some(c) = colour_index(v) {
            t.backdrop_colour = c;
        }
    ),
    may!(
        "font",
        |s| s.font.key().to_string(),
        |t, v| if let Some(f) = Font::from_key(v) {
            t.font = f;
        }
    ),
    flag!("auto_load", auto_load),
    flag!("eject_save", eject_save),
    flag!("rewind", rewind),
    flag!("turbo", turbo),
    may!(
        "blending",
        |s| s.blending.key().to_string(),
        |t, v| if let Some(b) = Blending::from_key(v) {
            t.blending = b;
        }
    ),
    may!(
        "gb_palette",
        |s| s.gb_colors.key().to_string(),
        |t, v| if let Some(c) = GbColors::from_key(v) {
            t.gb_colors = c;
        }
    ),
    flag!("show_recents", show_recents),
    flag!("show_titles", show_titles),
    flag!("clock_24h", clock_24h),
    may!(
        "lid_off_min",
        |s| s.lid_off_min.to_string(),
        |t, v| if let Some(m) = v.parse().ok().filter(|m| LID_OFF_MINUTES.contains(m)) {
            t.lid_off_min = m;
        }
    ),
    flag!("cheevos", cheevos),
    flag!("cheevos_hardcore", cheevos_hardcore),
    flag!("boot_animation", boot_animation),
    flag!("holo", holo),
    flag!("run_ahead", run_ahead),
    may!("online_name", |s| online_name(&s.online_name), |t, v| t
        .online_name =
        online_name(v)),
    may!(
        "relay",
        |s| s.relay.to_string(),
        |t, v| if let Ok(r) = v.trim().parse() {
            t.relay = r;
        }
    ),
    may!(
        "online_password",
        |s| online_password(&s.online_password),
        |t, v| t.online_password = online_password(v)
    ),
    may!(
        "online_address",
        |s| online_password(&s.online_address),
        |t, v| t.online_address = online_password(v)
    ),
    flag!("backup", backup),
    text!("backup_host", backup_host),
    text!("backup_name", backup_name),
    text!("backup_user", backup_user),
    text!("backup_password", backup_password),
    text!("backup_share", backup_share),
    text!("backup_dir", backup_dir),
    text!("shelf", shelf),
    text!("picked", picked),
    may!(
        "picked_platform",
        |s| s.picked_platform.map_or(String::new(), platform_key),
        |t, v| if let Some(p) = platform_value(v) {
            t.picked_platform = Some(p);
        }
    ),
    text!("psp_screen", psp_screen),
    flag!("menu_sounds", menu_sounds),
    flag!("menu_music", menu_music),
    may!(
        "menu_sounds_volume",
        |s| s.menu_sounds_volume.to_string(),
        |t, v| if let Some(n) = menu_volume(v) {
            t.menu_sounds_volume = n;
        }
    ),
    may!(
        "menu_music_volume",
        |s| s.menu_music_volume.to_string(),
        |t, v| if let Some(n) = menu_volume(v) {
            t.menu_music_volume = n;
        }
    ),
    flag!("test_updates", test_updates),
    may!(
        "auto_backups",
        |s| s.auto_backups.to_string(),
        |t, v| if let Some(n) = v.parse().ok().filter(|n| AUTO_BACKUPS.contains(n)) {
            t.auto_backups = n;
        }
    ),
];

fn menu_volume(value: &str) -> Option<u8> {
    value
        .parse()
        .ok()
        .filter(|n| (1..=MENU_VOLUME_MAX).contains(n))
}

fn parse(text: &str) -> Option<AppState> {
    let mut s = AppState::default();
    let mut seen = vec![false; FIELDS.len()];
    for line in text.lines().filter(|l| !l.is_empty()) {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if let Some(at) = FIELDS.iter().position(|f| f.key == key) {
            (FIELDS[at].read)(&mut s, value)?;
            seen[at] = true;
        }
    }
    let read = |key: &str| FIELDS.iter().zip(&seen).any(|(f, &s)| f.key == key && s);
    let whole = FIELDS
        .iter()
        .zip(&seen)
        .all(|(f, &seen)| seen || f.need == Need::May);
    if !whole {
        return None;
    }
    if !read("volume_hp") {
        s.volume_hp = s.volume;
    }
    if !read("muted_hp") {
        s.muted_hp = s.muted;
    }
    Some(s)
}

fn on_off_text(on: bool) -> String {
    if on { "on" } else { "off" }.to_string()
}

fn platform_key(platform: Platform) -> String {
    platform.dir_name().to_ascii_lowercase()
}

fn platform_value(value: &str) -> Option<Platform> {
    Platform::ALL
        .into_iter()
        .find(|p| value.eq_ignore_ascii_case(p.dir_name()))
}

fn offset(value: &str) -> Option<i16> {
    value
        .parse()
        .ok()
        .filter(|n| (UTC_OFFSET_MIN..=UTC_OFFSET_MAX).contains(n))
}

fn ff_speed_value(value: &str) -> Option<u8> {
    value.parse().ok().filter(|n| FF_SPEEDS.contains(n))
}

fn level(value: &str, max: u8) -> Option<u8> {
    value.parse().ok().filter(|n| *n <= max)
}

fn flag(value: &str) -> Option<bool> {
    level(value, 1).map(|n| n == 1)
}

/// A setting written as a word, `on` or `off`.
fn on_off(value: &str) -> Option<bool> {
    match value {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

/// A name for online rooms as it can be kept and sent: printable characters only, trimmed, and
/// no longer than the 31 bytes RetroArch's nick field holds.
pub fn online_name(name: &str) -> String {
    let kept: String = name
        .chars()
        .filter(|c| !c.is_control() && *c != '=')
        .collect();
    let mut kept = kept.trim().to_string();
    while kept.len() > 31 {
        kept.pop();
    }
    kept.trim_end().to_string()
}

/// A room's password as it can be kept: printable characters only, trimmed, and no longer than
/// 63 bytes.
pub fn online_password(password: &str) -> String {
    let kept: String = password
        .chars()
        .filter(|c| !c.is_control() && *c != '=')
        .collect();
    let mut kept = kept.trim().to_string();
    while kept.len() > 63 {
        kept.pop();
    }
    kept
}

/// Free text as the state can keep it, a line to itself: control characters left out, and no
/// longer than 255 bytes. Nothing else is touched, so a password keeps its spaces and signs.
pub fn kept_text(text: &str) -> String {
    let mut kept: String = text.chars().filter(|c| !c.is_control()).collect();
    while kept.len() > 255 {
        kept.pop();
    }
    kept
}
