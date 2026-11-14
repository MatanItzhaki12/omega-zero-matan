use crate::{Btn, Millis, RawEvent};

pub const SELECT_CHORD_MS: Millis = 600;

pub const SELECT_TAP_MS: Millis = 50;
pub const MENU_TAP_MS: Millis = 250;
pub const MENU_DOUBLE_TAP_MS: Millis = 350;
pub const MENU_HOLD_MS: Millis = 1000;
pub const MUTE_CHORD_MS: Millis = 200;
pub const POWER_HOLD_MS: Millis = 1000;

pub const VOLUME_REPEAT_DELAY_MS: Millis = 400;
pub const VOLUME_REPEAT_MS: Millis = 120;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Action {
    GbaDown(Btn),
    GbaUp(Btn),
    ShelfLeft,
    ShelfRight,
    Insert,
    Eject,
    Polaroids,
    SaveState,
    LoadState,
    RewindStart,
    RewindStop,
    FfStart,
    FfStop,
    BrightnessUp,
    BrightnessDown,
    BlueLightUp,
    BlueLightDown,
    VolumeUp,
    VolumeDown,
    QuickMenu,
    /// SELECT pressed and let go on its own, before it could become a chord. It comes after
    /// the `GbaDown(Select)` the tap also hands the game, and is never sent for a SELECT held
    /// past the chord window.
    SelectTap,
    GameMenu,
    MuteToggle,
    ColourCorrectionToggle,
    /// SELECT+A and SELECT+B, as on Slot: the next screen filter, and the one before.
    FilterNext,
    FilterPrev,
    /// SELECT+X: the game's screen kept as a picture.
    Screenshot,
    /// SELECT+START on the shelf: the cart picked opened in the cart editor.
    CartEditor,
    PowerPress,
    PowerTap,
    PowerHold,
    /// START pressed while POWER is held, before the hold powers off: a restart instead.
    PowerRestart,
    PowerOff,
    LidClose,
    LidOpen,
}

/// What the buttons the systems disagree about do for the game being played. The handhelds'
/// layout is the default: L2 rewinds, R2 fast-forwards, L2 and R2 together lock fast forward
/// on until they are pressed together again or R2 alone, and SELECT+Y toggles the colour
/// correction. A game with L2 and R2 of its own, as the PlayStation's have, is handed them, and
/// fast forward moves to SELECT+R2: the same hold, and the same lock with L2, SELECT held.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Keys {
    /// L2 and R2 are the game's, and fast forward is SELECT+R2.
    pub game_l2_r2: bool,
    /// Holding L2 rewinds, or SELECT+L2 where L2 is the game's. Off, nothing rewinds.
    pub rewind: bool,
    /// SELECT+Y toggles the colour correction. Off, a Y under SELECT is the game's.
    pub colour: bool,
    /// SELECT+START opens the cart editor: on the shelf alone, so a game's own SELECT+START
    /// (a soft reset, in many) is always the game's.
    pub shelf: bool,
}

impl Keys {
    /// The handhelds': L2 rewinds, R2 fast-forwards and SELECT+Y is the colour correction.
    pub const HANDHELD: Keys = Keys {
        game_l2_r2: false,
        rewind: true,
        colour: true,
        shelf: false,
    };
}

impl Default for Keys {
    fn default() -> Self {
        Keys::HANDHELD
    }
}

#[derive(Copy, Clone, Default)]
enum Select {
    #[default]
    Idle,
    Held {
        since: Millis,
        chorded: bool,
        sent: Option<Millis>,
    },
    ReleaseDue(Millis),
}

#[derive(Default)]
pub struct Gestures {
    keys: Keys,
    select: Select,
    chord_held: u16,
    menu_down_at: Option<Millis>,
    menu_last_tap: Option<Millis>,
    menu_eject_fired: bool,
    power_down_at: Option<Millis>,
    power_hold_fired: bool,
    /// START was pressed for a restart under POWER: its release goes nowhere either.
    start_for_power: bool,
    /// START is held, as the game has it: POWER held under it restarts too.
    start_held: bool,
    /// START and POWER are held together: the hold, once it fires, restarts rather than
    /// powers off.
    restart_armed: bool,
    vol_up_at: Option<Millis>,
    vol_down_at: Option<Millis>,
    vol_up_ramp: Option<Millis>,
    vol_down_ramp: Option<Millis>,
    mute_fired: bool,
    ff_on: bool,
    ff_latched: bool,
    ff_latching_press: bool,
    r2_refused: bool,
    /// The shoulders held, as rewind and fast forward take them: L2 and R2 together lock fast
    /// forward on, or off.
    l2_held: bool,
    r2_held: bool,
    /// L2's press let go of a lock it found on: R2 joining it is the chord turning it off.
    l2_unlatched: bool,
    rewinding: bool,
}

fn ramp_due(down: Option<Millis>, last: Option<Millis>, now: Millis) -> bool {
    let Some(down) = down else {
        return false;
    };
    if now.saturating_sub(down) < VOLUME_REPEAT_DELAY_MS {
        return false;
    }
    last.is_none_or(|l| now.saturating_sub(l) >= VOLUME_REPEAT_MS)
}

impl Gestures {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ff_latched(&self) -> bool {
        self.ff_latched
    }

    /// What the buttons do from now on, for the game being played. A change lets go of a rewind
    /// or a fast forward under way, and of a shoulder's chord still held, so nothing is left
    /// running under buttons that now mean something else.
    pub fn set_keys(&mut self, keys: Keys) -> Vec<Action> {
        if self.keys == keys {
            return Vec::new();
        }
        self.keys = keys;
        let mut out = self.rewind_stop();
        out.extend(self.ff_clear());
        self.r2_refused = false;
        (self.l2_held, self.r2_held, self.l2_unlatched) = (false, false, false);
        self.chord_held &= !(L2_CHORD | R2_CHORD);
        out
    }

    pub fn keys(&self) -> Keys {
        self.keys
    }

    pub fn drop_ff_latch(&mut self) -> Vec<Action> {
        if !self.ff_latched {
            return Vec::new();
        }
        self.ff_clear()
    }

    pub fn feed(&mut self, ev: RawEvent, now: Millis) -> Vec<Action> {
        match ev {
            RawEvent::Down(b) => self.down(b, now),
            RawEvent::Up(b) => self.up(b, now),
        }
    }

    pub fn tick(&mut self, now: Millis) -> Vec<Action> {
        let mut out = Vec::new();
        if let Select::ReleaseDue(due) = self.select {
            if now >= due {
                self.select = Select::Idle;
                out.push(Action::GbaUp(Btn::Select));
            }
        }
        if let Select::Held {
            since,
            chorded: false,
            sent: None,
        } = self.select
        {
            if now.saturating_sub(since) >= SELECT_CHORD_MS {
                out.extend(self.hand_over_select(now));
            }
        }
        if let Some(d) = self.menu_down_at {
            if !self.menu_eject_fired && now.saturating_sub(d) >= MENU_HOLD_MS {
                self.menu_eject_fired = true;
                out.push(Action::Eject);
            }
        }
        if let Some(d) = self.power_down_at {
            if !self.power_hold_fired && now.saturating_sub(d) >= POWER_HOLD_MS {
                self.power_hold_fired = true;
                out.push(match self.restart_armed {
                    true => Action::PowerRestart,
                    false => Action::PowerHold,
                });
            }
        }
        if !self.mute_fired {
            if ramp_due(self.vol_up_at, self.vol_up_ramp, now) {
                self.vol_up_ramp = Some(now);
                out.push(Action::VolumeUp);
            }
            if ramp_due(self.vol_down_at, self.vol_down_ramp, now) {
                self.vol_down_ramp = Some(now);
                out.push(Action::VolumeDown);
            }
        }
        out
    }

    fn down(&mut self, b: Btn, now: Millis) -> Vec<Action> {
        match b {
            Btn::Select => self.select_down(now),
            Btn::Menu => self.menu_down(now),
            // POWER held under a held START restarts once the hold fires, as POWER held alone
            // powers off: START, which the game had, is let go of, and its release goes nowhere
            Btn::Power if self.start_held => {
                self.power_down_at = Some(now);
                self.power_hold_fired = false;
                self.restart_armed = true;
                self.start_held = false;
                self.start_for_power = true;
                vec![Action::GbaUp(Btn::Start)]
            }
            Btn::Power => {
                self.power_down_at = Some(now);
                self.power_hold_fired = false;
                vec![Action::PowerPress]
            }
            Btn::Lid => vec![Action::LidClose],
            // START under a held POWER makes the hold restart rather than power off, once it
            // fires; neither the game nor a menu sees START
            Btn::Start if self.power_down_at.is_some() && !self.power_hold_fired => {
                self.restart_armed = true;
                self.start_for_power = true;
                Vec::new()
            }
            Btn::VolUp | Btn::VolDown => self.volume_press(b, now),
            Btn::L2 if !self.keys.game_l2_r2 => self.l2_down(),
            Btn::R2 if !self.keys.game_l2_r2 => self.r2_down(),
            _ => {
                if let (true, Some((bit, action))) = (self.chording(now), self.chord(b)) {
                    self.mark_chorded();
                    self.chord_held |= bit;
                    return match action {
                        Action::RewindStart => self.l2_down(),
                        Action::FfStart => self.r2_down(),
                        action => vec![action],
                    };
                }
                if b == Btn::Start {
                    self.start_held = true;
                }
                let mut out = self.hand_over_select(now);
                out.push(Action::GbaDown(b));
                out
            }
        }
    }

    fn up(&mut self, b: Btn, now: Millis) -> Vec<Action> {
        match b {
            Btn::Select => self.select_up(now),
            Btn::Menu => self.menu_up(now),
            Btn::Power => self.power_up(),
            Btn::Lid => vec![Action::LidOpen],
            Btn::Start if self.start_for_power => {
                self.start_for_power = false;
                Vec::new()
            }
            // a START that made the cart editor's chord ends it, and goes nowhere
            Btn::Start if self.chord_held & CART_EDITOR_CHORD != 0 => {
                self.chord_held &= !CART_EDITOR_CHORD;
                Vec::new()
            }
            Btn::Start => {
                self.start_held = false;
                vec![Action::GbaUp(Btn::Start)]
            }
            Btn::VolUp | Btn::VolDown => self.volume_release(b),
            Btn::L2 if !self.keys.game_l2_r2 => self.l2_up(),
            Btn::R2 if !self.keys.game_l2_r2 => self.r2_up(),
            _ => {
                if let Some((bit, action)) = self.chord(b) {
                    if self.chord_held & bit != 0 {
                        self.chord_held &= !bit;
                        return match action {
                            Action::RewindStart => self.l2_up(),
                            Action::FfStart => self.r2_up(),
                            _ => Vec::new(),
                        };
                    }
                }
                vec![Action::GbaUp(b)]
            }
        }
    }

    /// What `b` does under SELECT, and the bit its chord holds while it is down: most are a
    /// press, and L2 and R2, where they are the game's, a hold that rewinds or fast-forwards
    /// until the shoulder is let go. L2 joining R2 there locks fast forward even where nothing
    /// rewinds.
    fn chord(&self, b: Btn) -> Option<(u16, Action)> {
        Some(match b {
            Btn::Up => (1, Action::BrightnessUp),
            Btn::Down => (2, Action::BrightnessDown),
            Btn::Left => (4, Action::BlueLightDown),
            Btn::Right => (8, Action::BlueLightUp),
            Btn::L1 => (16, Action::LoadState),
            Btn::R1 => (32, Action::SaveState),
            Btn::Y if self.keys.colour => (64, Action::ColourCorrectionToggle),
            Btn::A => (512, Action::FilterNext),
            Btn::B => (1024, Action::FilterPrev),
            Btn::X => (2048, Action::Screenshot),
            Btn::Start if self.keys.shelf => (CART_EDITOR_CHORD, Action::CartEditor),
            // L2 joining R2 locks fast forward; its release ends the chord it began
            Btn::L2
                if self.keys.game_l2_r2
                    && (self.keys.rewind || self.chord_held & (R2_CHORD | L2_CHORD) != 0) =>
            {
                (L2_CHORD, Action::RewindStart)
            }
            Btn::R2 if self.keys.game_l2_r2 => (R2_CHORD, Action::FfStart),
            _ => return None,
        })
    }

    fn chording(&self, now: Millis) -> bool {
        match self.select {
            Select::Held { since, chorded, .. } => {
                chorded || now.saturating_sub(since) < SELECT_CHORD_MS
            }
            _ => false,
        }
    }

    fn mark_chorded(&mut self) {
        if let Select::Held { chorded, .. } = &mut self.select {
            *chorded = true;
        }
    }

    fn select_down(&mut self, now: Millis) -> Vec<Action> {
        let mut out = Vec::new();
        if matches!(self.select, Select::ReleaseDue(_)) {
            out.push(Action::GbaUp(Btn::Select));
        }
        if !matches!(self.select, Select::Held { .. }) {
            self.select = Select::Held {
                since: now,
                chorded: false,
                sent: None,
            };
        }
        out
    }

    fn hand_over_select(&mut self, now: Millis) -> Vec<Action> {
        match &mut self.select {
            Select::Held {
                chorded: false,
                sent: sent @ None,
                ..
            } => {
                *sent = Some(now);
                vec![Action::GbaDown(Btn::Select)]
            }
            _ => Vec::new(),
        }
    }

    fn select_up(&mut self, now: Millis) -> Vec<Action> {
        let Select::Held { chorded, sent, .. } = std::mem::take(&mut self.select) else {
            return Vec::new();
        };
        match (sent, chorded) {
            (Some(at), _) if now.saturating_sub(at) >= SELECT_TAP_MS => {
                vec![Action::GbaUp(Btn::Select)]
            }
            (Some(at), _) => {
                self.select = Select::ReleaseDue(at + SELECT_TAP_MS);
                Vec::new()
            }
            (None, true) => Vec::new(),
            (None, false) => {
                self.select = Select::ReleaseDue(now + SELECT_TAP_MS);
                vec![Action::GbaDown(Btn::Select), Action::SelectTap]
            }
        }
    }

    fn menu_down(&mut self, now: Millis) -> Vec<Action> {
        if self.chording(now) {
            self.mark_chorded();
            self.menu_down_at = None;
            self.menu_last_tap = None;
            return vec![Action::GameMenu];
        }
        if let Some(tap) = self.menu_last_tap {
            if now.saturating_sub(tap) <= MENU_DOUBLE_TAP_MS {
                self.menu_last_tap = None;
                self.menu_down_at = None;
                return vec![Action::Polaroids];
            }
        }
        self.menu_down_at = Some(now);
        self.menu_eject_fired = false;
        Vec::new()
    }

    fn menu_up(&mut self, now: Millis) -> Vec<Action> {
        let Some(d) = self.menu_down_at.take() else {
            return Vec::new();
        };
        let ejected = self.menu_eject_fired;
        self.menu_eject_fired = false;
        let tapped = !ejected && now.saturating_sub(d) < MENU_TAP_MS;
        self.menu_last_tap = tapped.then_some(now);
        match tapped {
            true => vec![Action::QuickMenu],
            false => Vec::new(),
        }
    }

    fn power_up(&mut self) -> Vec<Action> {
        let held = self.power_hold_fired;
        let armed = std::mem::take(&mut self.restart_armed);
        self.power_down_at = None;
        self.power_hold_fired = false;
        // START and POWER let go of before the hold: nothing, as POWER tapped with START held
        if armed && !held {
            return Vec::new();
        }
        vec![if held {
            Action::PowerOff
        } else {
            Action::PowerTap
        }]
    }

    fn volume_press(&mut self, b: Btn, now: Millis) -> Vec<Action> {
        let (mine, other, action) = match b {
            Btn::VolUp => (&mut self.vol_up_at, self.vol_down_at, Action::VolumeUp),
            _ => (&mut self.vol_down_at, self.vol_up_at, Action::VolumeDown),
        };
        *mine = Some(now);
        match b {
            Btn::VolUp => self.vol_up_ramp = None,
            _ => self.vol_down_ramp = None,
        }
        let mut out = vec![action];
        let paired = other.is_some_and(|t| now.abs_diff(t) <= MUTE_CHORD_MS);
        if paired && !self.mute_fired {
            self.mute_fired = true;
            out.push(Action::MuteToggle);
        }
        out
    }

    fn volume_release(&mut self, b: Btn) -> Vec<Action> {
        match b {
            Btn::VolUp => (self.vol_up_at, self.vol_up_ramp) = (None, None),
            _ => (self.vol_down_at, self.vol_down_ramp) = (None, None),
        }
        if self.vol_up_at.is_none() && self.vol_down_at.is_none() {
            self.mute_fired = false;
        }
        Vec::new()
    }

    fn rewind_start(&mut self) -> Vec<Action> {
        if self.rewinding {
            return Vec::new();
        }
        let mut out = Vec::new();
        out.extend(self.ff_clear());
        self.rewinding = true;
        out.push(Action::RewindStart);
        out
    }

    fn rewind_stop(&mut self) -> Vec<Action> {
        if !self.rewinding {
            return Vec::new();
        }
        self.rewinding = false;
        vec![Action::RewindStop]
    }

    /// L2 pressed: a rewind, where rewinding is on; with R2 already held, the chord that locks
    /// fast forward on or off.
    fn l2_down(&mut self) -> Vec<Action> {
        self.l2_held = true;
        if self.r2_held {
            return self.ff_toggle_lock();
        }
        self.l2_unlatched = self.ff_latched;
        match self.keys.rewind {
            true => self.rewind_start(),
            false => Vec::new(),
        }
    }

    fn l2_up(&mut self) -> Vec<Action> {
        self.l2_held = false;
        self.l2_unlatched = false;
        self.rewind_stop()
    }

    /// R2 pressed: fast forward while it is held; with L2 already held, the chord that locks it
    /// on or off.
    fn r2_down(&mut self) -> Vec<Action> {
        self.r2_held = true;
        if self.l2_held {
            return self.ff_toggle_lock();
        }
        self.ff_down()
    }

    fn r2_up(&mut self) -> Vec<Action> {
        self.r2_held = false;
        self.ff_up()
    }

    /// L2 and R2 together, either first: fast forward locked on, to run with neither held, or,
    /// locked already, off. A rewind the first of them began stops.
    fn ff_toggle_lock(&mut self) -> Vec<Action> {
        let was_locked = self.ff_latched || std::mem::take(&mut self.l2_unlatched);
        let mut out = self.rewind_stop();
        self.r2_refused = false;
        if was_locked {
            out.extend(self.ff_clear());
            return out;
        }
        self.ff_latched = true;
        // R2's release, still to come, leaves it running
        self.ff_latching_press = true;
        if !self.ff_on {
            self.ff_on = true;
            out.push(Action::FfStart);
        }
        out
    }

    fn ff_down(&mut self) -> Vec<Action> {
        if self.rewinding {
            self.r2_refused = true;
            return Vec::new();
        }
        // locked, a press of R2 on its own is the one that lets it go, on its release
        self.ff_latching_press = false;
        if self.ff_on {
            return Vec::new();
        }
        self.ff_on = true;
        vec![Action::FfStart]
    }

    fn ff_up(&mut self) -> Vec<Action> {
        if std::mem::take(&mut self.r2_refused) {
            return Vec::new();
        }
        if self.ff_latching_press {
            self.ff_latching_press = false;
            return Vec::new();
        }
        self.ff_clear()
    }

    fn ff_clear(&mut self) -> Vec<Action> {
        self.ff_latched = false;
        self.ff_latching_press = false;
        if !self.ff_on {
            return Vec::new();
        }
        self.ff_on = false;
        vec![Action::FfStop]
    }
}

/// The chord bits of SELECT+L2 and SELECT+R2, held for as long as the shoulder is.
/// The bit SELECT+START's chord holds while START is down.
const CART_EDITOR_CHORD: u16 = 4096;
const L2_CHORD: u16 = 128;
const R2_CHORD: u16 = 256;
