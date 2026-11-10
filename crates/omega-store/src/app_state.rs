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
}

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

/// `s` as the state file has it.
pub fn app_state_text(s: &AppState) -> String {
    let text = format!(
        "cart={}\ncart_platform={}\nbrightness={}\nblue_light={}\nvolume={}\nvolume_hp={}\nmuted={}\nmuted_hp={}\nclock_set={}\nutc_offset_min={}\nrumble={}\nff_speed={}\nff_sound={}\ncolour_correction={}\nscreen_filter={}\nwifi={}\nssh={}\nbluetooth={}\nbackdrop={}\nbackdrop_moving={}\nbackdrop_colour={}\nfont={}\nauto_load={}\neject_save={}\nrewind={}\nturbo={}\nblending={}\ngb_palette={}\nshow_recents={}\nshow_titles={}\nclock_24h={}\nlid_off_min={}\ncheevos={}\ncheevos_hardcore={}\nboot_animation={}\nholo={}\nrun_ahead={}\nonline_name={}\nrelay={}\nonline_password={}\nonline_address={}\n",
        s.cart.as_deref().unwrap_or(""),
        s.cart_platform.map_or(String::new(), platform_key),
        s.brightness,
        s.blue_light,
        s.volume,
        s.volume_hp,
        s.muted as u8,
        s.muted_hp as u8,
        s.clock_set as u8,
        s.utc_offset_min,
        s.rumble as u8,
        s.ff_speed,
        s.ff_sound as u8,
        s.colour_correction as u8,
        s.screen_filter.key(),
        s.wifi as u8,
        if s.ssh { "on" } else { "off" },
        if s.bluetooth { "on" } else { "off" },
        s.backdrop.map_or("", BackdropStyle::key),
        s.backdrop_moving as u8,
        BACKDROP_COLOURS[(s.backdrop_colour as usize).min(BACKDROP_COLOURS.len() - 1)],
        s.font.key(),
        s.auto_load as u8,
        s.eject_save as u8,
        s.rewind as u8,
        s.turbo as u8,
        s.blending.key(),
        s.gb_colors.key(),
        s.show_recents as u8,
        s.show_titles as u8,
        s.clock_24h as u8,
        s.lid_off_min,
        s.cheevos as u8,
        s.cheevos_hardcore as u8,
        s.boot_animation as u8,
        s.holo as u8,
        s.run_ahead as u8,
        online_name(&s.online_name),
        s.relay,
        online_password(&s.online_password),
        online_password(&s.online_address),
    );
    let text = text
        + &format!(
            "backup={}\nbackup_host={}\nbackup_name={}\nbackup_user={}\nbackup_password={}\nbackup_share={}\nbackup_dir={}\nshelf={}\npicked={}\npicked_platform={}\npsp_screen={}\n",
            s.backup as u8,
            kept_text(&s.backup_host),
            kept_text(&s.backup_name),
            kept_text(&s.backup_user),
            kept_text(&s.backup_password),
            kept_text(&s.backup_share),
            kept_text(&s.backup_dir),
            kept_text(&s.shelf),
            kept_text(&s.picked),
            s.picked_platform.map_or(String::new(), platform_key),
            kept_text(&s.psp_screen),
        );
    text
}

fn parse(text: &str) -> Option<AppState> {
    let mut cart = None;
    let mut cart_platform = None;
    let mut brightness = None;
    let mut blue_light = None;
    let mut volume = None;
    let mut volume_hp = None;
    let mut muted = None;
    let mut muted_hp = None;
    let mut clock_set = None;
    let mut utc_offset_min = None;
    let mut rumble = None;
    let mut ff_speed = None;
    let mut ff_sound = None;
    let mut colour_correction = None;
    let mut screen_filter = None;
    let mut wifi = None;
    let mut ssh = None;
    let mut bluetooth = None;
    let mut backdrop = None;
    let mut backdrop_moving = None;
    let mut backdrop_colour = None;
    let mut font = None;
    let mut auto_load = None;
    let mut eject_save = None;
    let mut rewind = None;
    let mut turbo = None;
    let mut blending = None;
    let mut gb_colors = None;
    let mut show_recents = None;
    let mut show_titles = None;
    let mut clock_24h = None;
    let mut lid_off_min = None;
    let mut cheevos = None;
    let mut cheevos_hardcore = None;
    let mut boot_animation = None;
    let mut holo = None;
    let mut run_ahead = None;
    let mut online_name_read = None;
    let mut relay = None;
    let mut online_password_read = None;
    let mut online_address_read = None;
    let mut backup = None;
    let mut backup_text: [Option<String>; 9] = Default::default();
    let mut picked_platform = None;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "cart" => cart = Some(value.to_string()),
            "cart_platform" => cart_platform = platform_value(value),
            "brightness" => brightness = Some(level(value, BRIGHTNESS_MAX)?),
            "blue_light" => blue_light = Some(level(value, BLUE_LIGHT_MAX)?),
            "volume" => volume = Some(level(value, VOLUME_MAX)?),
            "volume_hp" => volume_hp = Some(level(value, VOLUME_MAX)?),
            "muted" => muted = Some(level(value, 1)? == 1),
            "muted_hp" => muted_hp = Some(level(value, 1)? == 1),
            "clock_set" => clock_set = Some(level(value, 1)? == 1),
            "utc_offset_min" => utc_offset_min = Some(offset(value)?),
            "rumble" => rumble = flag(value),
            "ff_speed" => ff_speed = ff_speed_value(value),
            "ff_sound" => ff_sound = flag(value),
            "colour_correction" => colour_correction = flag(value),
            "screen_filter" => screen_filter = ScreenFilter::from_key(value),
            "wifi" => wifi = flag(value),
            "ssh" => ssh = on_off(value),
            "bluetooth" => bluetooth = on_off(value),
            "backdrop" => backdrop = BackdropStyle::from_key(value),
            "backdrop_moving" => backdrop_moving = flag(value),
            "backdrop_colour" => backdrop_colour = colour_index(value),
            "font" => font = Font::from_key(value),
            "auto_load" => auto_load = flag(value),
            "eject_save" => eject_save = flag(value),
            "rewind" => rewind = flag(value),
            "turbo" => turbo = flag(value),
            "blending" => blending = Blending::from_key(value),
            "gb_palette" => gb_colors = GbColors::from_key(value),
            "show_recents" => show_recents = flag(value),
            "show_titles" => show_titles = flag(value),
            "clock_24h" => clock_24h = flag(value),
            "cheevos" => cheevos = flag(value),
            "cheevos_hardcore" => cheevos_hardcore = flag(value),
            "boot_animation" => boot_animation = flag(value),
            "holo" => holo = flag(value),
            "run_ahead" => run_ahead = flag(value),
            "online_name" => online_name_read = Some(online_name(value)),
            "relay" => relay = value.trim().parse().ok(),
            "online_password" => online_password_read = Some(online_password(value)),
            "online_address" => online_address_read = Some(online_password(value)),
            "backup" => backup = flag(value),
            "picked_platform" => picked_platform = platform_value(value),
            "lid_off_min" => {
                lid_off_min = value.parse().ok().filter(|m| LID_OFF_MINUTES.contains(m))
            }
            key => {
                if let Some(at) = TEXT_KEYS.iter().position(|k| *k == key) {
                    backup_text[at] = Some(kept_text(value));
                }
            }
        }
    }
    let cart = cart?;
    let fallback = AppState::default();
    Some(AppState {
        cart: (!cart.is_empty()).then_some(cart),
        cart_platform,
        brightness: brightness?,
        blue_light: blue_light?,
        volume: volume?,
        volume_hp: volume_hp.or(volume)?,
        muted: muted?,
        muted_hp: muted_hp.or(muted)?,
        clock_set: clock_set?,
        utc_offset_min: utc_offset_min?,
        rumble: rumble.unwrap_or(fallback.rumble),
        ff_speed: ff_speed.unwrap_or(fallback.ff_speed),
        ff_sound: ff_sound.unwrap_or(fallback.ff_sound),
        colour_correction: colour_correction.unwrap_or(fallback.colour_correction),
        screen_filter: screen_filter.unwrap_or(fallback.screen_filter),
        wifi: wifi.unwrap_or(fallback.wifi),
        ssh: ssh.unwrap_or(fallback.ssh),
        bluetooth: bluetooth.unwrap_or(fallback.bluetooth),
        backdrop,
        backdrop_moving: backdrop_moving.unwrap_or(fallback.backdrop_moving),
        backdrop_colour: backdrop_colour.unwrap_or(fallback.backdrop_colour),
        font: font.unwrap_or(fallback.font),
        auto_load: auto_load.unwrap_or(fallback.auto_load),
        eject_save: eject_save.unwrap_or(fallback.eject_save),
        rewind: rewind.unwrap_or(fallback.rewind),
        turbo: turbo.unwrap_or(fallback.turbo),
        blending: blending.unwrap_or(fallback.blending),
        gb_colors: gb_colors.unwrap_or(fallback.gb_colors),
        show_recents: show_recents.unwrap_or(fallback.show_recents),
        show_titles: show_titles.unwrap_or(fallback.show_titles),
        clock_24h: clock_24h.unwrap_or(fallback.clock_24h),
        lid_off_min: lid_off_min.unwrap_or(fallback.lid_off_min),
        cheevos: cheevos.unwrap_or(fallback.cheevos),
        cheevos_hardcore: cheevos_hardcore.unwrap_or(fallback.cheevos_hardcore),
        boot_animation: boot_animation.unwrap_or(fallback.boot_animation),
        holo: holo.unwrap_or(fallback.holo),
        run_ahead: run_ahead.unwrap_or(fallback.run_ahead),
        online_name: online_name_read.unwrap_or(fallback.online_name),
        relay: relay.unwrap_or(fallback.relay),
        online_password: online_password_read.unwrap_or(fallback.online_password),
        online_address: online_address_read.unwrap_or(fallback.online_address),
        backup: backup.unwrap_or(fallback.backup),
        backup_host: text_or(&mut backup_text[0], fallback.backup_host),
        backup_name: text_or(&mut backup_text[1], fallback.backup_name),
        backup_user: text_or(&mut backup_text[2], fallback.backup_user),
        backup_password: text_or(&mut backup_text[3], fallback.backup_password),
        backup_share: text_or(&mut backup_text[4], fallback.backup_share),
        backup_dir: text_or(&mut backup_text[5], fallback.backup_dir),
        shelf: text_or(&mut backup_text[6], fallback.shelf),
        picked: text_or(&mut backup_text[7], fallback.picked),
        psp_screen: text_or(&mut backup_text[8], fallback.psp_screen),
        picked_platform: picked_platform.or(fallback.picked_platform),
    })
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

/// The keys of the state's free text, read into one place by their order here.
const TEXT_KEYS: [&str; 9] = [
    "backup_host",
    "backup_name",
    "backup_user",
    "backup_password",
    "backup_share",
    "backup_dir",
    "shelf",
    "picked",
    "psp_screen",
];

fn text_or(read: &mut Option<String>, fallback: String) -> String {
    read.take().unwrap_or(fallback)
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
