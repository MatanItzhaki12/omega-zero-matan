//! Where the sound goes, and how loud: the speaker, wired headphones in the jack, or Bluetooth
//! headphones. The speaker has its volume and mute, and headphones of either kind share theirs.
//!
//! Every move between them goes through [`App::follow_output`]. Sound leaving headphones is
//! held silent at once ([`crate::audio::hush`]); leaving them for the speaker mutes the speaker,
//! so a game never plays aloud by surprise; and the hush lasts until what was queued at the
//! headphones' level has played out unheard. Bluetooth headphones play at a volume of their own,
//! kept as the console's by [`BtVolume`].

use super::*;

/// How often the headphone jack is read: a plug or unplug is believed two reads on.
pub(super) const HEADPHONES_POLL_MS: Millis = 150;

/// How long the sound stays silent after it leaves headphones: what was queued at their level
/// plays out unheard.
const HUSH_MS: Millis = 500;

/// How long Bluetooth headphones are given to take a volume sent them before what they say is
/// taken for a change made on them.
const BT_VOLUME_ANSWER_MS: Millis = 2_500;

/// Where the sound plays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Output {
    #[default]
    Speaker,
    Wired,
    Bluetooth,
}

impl Output {
    /// Headphones, of either kind: the volume and mute are theirs.
    pub fn headphones(self) -> bool {
        self != Output::Speaker
    }
}

/// The headphone jack, believed plugged or not once it reads the same twice running: one odd
/// read never moves the sound between the headphones and the speaker.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Jack {
    plugged: bool,
    last: bool,
}

impl Jack {
    pub(crate) fn new(plugged: bool) -> Self {
        Self {
            plugged,
            last: plugged,
        }
    }

    /// Takes a read of the jack, and says whether it disagrees with what is believed: one odd
    /// read, or the first of a plug or unplug.
    pub(crate) fn read(&mut self, raw: bool) -> bool {
        if std::mem::replace(&mut self.last, raw) == raw {
            self.plugged = raw;
        }
        raw != self.plugged
    }

    pub(crate) fn plugged(&self) -> bool {
        self.plugged
    }
}

/// What keeping Bluetooth headphones' volume as the console's calls for, one step at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BtStep {
    /// Send them the console's volume.
    Send(u8),
    /// Their volume was changed on them: the console takes it.
    Adopt(u8),
    Nothing,
}

/// Bluetooth headphones' own volume, kept as the console's: the buttons set theirs, and theirs
/// changed on the headphones is the console's. They take a volume sent them a second or so
/// late, saying the one they had until then: what they say while a volume is on its way is not
/// taken for a change of theirs, or the two would be sent back and forth.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BtVolume {
    sent: Option<u8>,
    seen: Option<u8>,
    sent_at: Option<Millis>,
}

impl BtVolume {
    /// One step, with what the headphones last said and the console's volume for them.
    pub(crate) fn step(&mut self, read: Option<u8>, level: u8, now: Millis) -> BtStep {
        if let Some(read) = read.filter(|r| self.seen != Some(*r)) {
            self.seen = Some(read);
            let answering = self
                .sent_at
                .is_some_and(|at| now.saturating_sub(at) < BT_VOLUME_ANSWER_MS);
            if read.abs_diff(level) <= 1 {
                // theirs is ours, to the rounding of AVRCP's steps: the volume sent is taken
                self.sent_at = None;
            } else if !answering && self.sent.is_some() {
                self.sent = Some(read);
                return BtStep::Adopt(read);
            }
        }
        if self.sent == Some(level) {
            return BtStep::Nothing;
        }
        self.sent = Some(level);
        self.seen = Some(level);
        self.sent_at = Some(now);
        BtStep::Send(level)
    }
}

impl App {
    /// Where the sound plays now.
    pub fn output(&self) -> Output {
        if crate::audio::on_bluetooth() {
            Output::Bluetooth
        } else if self.jack.plugged() {
            Output::Wired
        } else {
            Output::Speaker
        }
    }

    pub(super) fn on_headphones(&self) -> bool {
        self.output().headphones()
    }

    pub fn volume(&self) -> u8 {
        self.level()
    }

    pub(super) fn level(&self) -> u8 {
        match self.on_headphones() {
            true => self.state.volume_hp,
            false => self.state.volume,
        }
    }

    pub(super) fn level_mut(&mut self) -> &mut u8 {
        match self.on_headphones() {
            true => &mut self.state.volume_hp,
            false => &mut self.state.volume,
        }
    }

    pub fn muted(&self) -> bool {
        match self.on_headphones() {
            true => self.state.muted_hp,
            false => self.state.muted,
        }
    }

    pub(super) fn muted_mut(&mut self) -> &mut bool {
        match self.on_headphones() {
            true => &mut self.state.muted_hp,
            false => &mut self.state.muted,
        }
    }

    /// The volume as heard: none muted, and turned down while AirPods hear their wearer talk.
    pub fn output_volume(&self) -> u8 {
        if self.muted() {
            return 0;
        }
        (f32::from(self.level()) * self.duck()).round() as u8
    }

    /// How loud the samples are made: on Bluetooth headphones, which play at a volume of their
    /// own, full but for mute and the duck; else the volume as heard.
    pub fn sample_volume(&self) -> u8 {
        if self.output() != Output::Bluetooth {
            return self.output_volume();
        }
        if self.muted() {
            return 0;
        }
        (100.0 * self.duck()).round() as u8
    }

    fn duck(&self) -> f32 {
        self.phones.phones().map_or(1.0, |p| p.duck)
    }

    /// Reads the jack, when due. A read that disagrees with what is believed of the
    /// headphones in it holds the sound silent at once: the codec moves the sound to the
    /// speaker by itself as they come out, before the unplug is believed and the speaker muted.
    pub(super) fn read_jack(&mut self, now: Millis) {
        if now < self.headphones_at {
            return;
        }
        self.headphones_at = now + HEADPHONES_POLL_MS;
        let raw = self.power.as_ref().is_some_and(|p| p.headphones());
        let doubt = self.jack.read(raw);
        if self.output != Output::Wired {
            return;
        }
        if doubt && !self.jack_doubt {
            self.jack_doubt = true;
            crate::audio::hush(true);
        } else if !doubt && self.jack_doubt {
            self.jack_doubt = false;
            if self.hush_until.is_none() {
                crate::audio::hush(false);
            }
        }
    }

    /// Follows the sound to where it plays now: the gauge's icon, the toasts, the speaker
    /// muted as headphones go, and the hush over the move.
    pub(super) fn follow_output(&mut self, now: Millis) {
        let to = self.output();
        let from = std::mem::replace(&mut self.output, to);
        self.hud.set_headphones(to.headphones());
        if from == to {
            return;
        }
        self.jack_doubt = false;
        if to == Output::Bluetooth {
            // one toast for both: connected, and the mode they connected in where it is known
            let said = self
                .phones
                .phones()
                .and_then(|p| p.noise.map(|n| n.connected(p.kind)))
                .unwrap_or("Headphones connected");
            self.hud.toast(Toast::Headphones(said), now);
        }
        if from == Output::Bluetooth {
            self.hud
                .toast(Toast::Headphones("Headphones disconnected"), now);
        }
        if from.headphones() && to != Output::Bluetooth {
            // what was queued went at the headphones' level: Bluetooth's full scale, or the
            // wired level, too loud for the speaker or for wired headphones
            crate::audio::hush(true);
            self.hush_until = Some(now + HUSH_MS);
        }
        if from.headphones() && to == Output::Speaker && !self.state.muted {
            self.state.muted = true;
            self.persist();
        }
        if from != Output::Bluetooth && to != Output::Bluetooth {
            // wired in or out: the gauge shows the level the buttons now set
            self.hud
                .show(HudKind::Volume, self.output_volume(), self.muted(), now);
        }
    }

    /// Lets the sound play again once what was queued as headphones went has played out.
    pub(super) fn end_hush(&mut self, now: Millis) {
        if self.hush_until.is_some_and(|t| now >= t) {
            self.hush_until = None;
            if !self.jack_doubt {
                crate::audio::hush(false);
            }
        }
    }

    /// Whether sound may go to the headphones at `audio`: once their own volume is the one
    /// they are kept at here, so they never play for a moment at whatever they were left at.
    /// Should it not be set within 2.5 s, the sound goes to them all the same.
    pub(super) fn bluetooth_volume_ready(&mut self, audio: Option<&str>, now: Millis) -> bool {
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

    /// Headphones that connect by themselves can come without their sound: bluetoothd has
    /// them, but not the A2DP stream. Asking them to connect again, once they have gone a
    /// moment without it, brings it, and every 15 s while it stays away.
    pub(super) fn connect_bluetooth_sound(&mut self, audio: Option<&str>, now: Millis) {
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

    /// Keeps Bluetooth headphones' own volume as the console's while the sound goes to them.
    pub(super) fn follow_bluetooth_volume(&mut self, now: Millis) {
        use crate::audio::bt_volume;
        if self.output() != Output::Bluetooth {
            self.bt_volume = BtVolume::default();
            return;
        }
        match self
            .bt_volume
            .step(bt_volume::level(), self.state.volume_hp, now)
        {
            BtStep::Send(level) => bt_volume::set(level),
            BtStep::Adopt(level) => {
                self.state.volume_hp = level;
                self.hud
                    .show(HudKind::Volume, self.output_volume(), self.muted(), now);
                self.persist();
            }
            BtStep::Nothing => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_jack_is_believed_once_it_reads_the_same_twice() {
        let mut jack = Jack::new(true);
        assert!(jack.read(false), "the first empty read is not in doubt");
        assert!(jack.plugged(), "one empty read was believed");
        assert!(!jack.read(true), "a read back in is still in doubt");
        assert!(jack.plugged());
        assert!(jack.read(false));
        assert!(!jack.read(false), "two empty reads are still in doubt");
        assert!(!jack.plugged(), "two empty reads were not believed");
    }

    /// The headphones as they answered on the console: each volume sent them taken a second
    /// late, the one they had said until then.
    #[test]
    fn headphones_answering_late_never_bounce_the_volume() {
        let mut bt = BtVolume::default();
        // connected at 62, and in step
        assert_eq!(bt.step(Some(62), 62, 0), BtStep::Send(62));
        assert_eq!(bt.step(Some(62), 62, 900), BtStep::Nothing);
        // two presses up, 0.4 s apart; they say 62, then 67, as they catch up
        assert_eq!(bt.step(Some(62), 67, 1_000), BtStep::Send(67));
        assert_eq!(bt.step(Some(62), 72, 1_400), BtStep::Send(72));
        for (t, said) in [(1_800, 62), (2_600, 67), (3_400, 72), (4_200, 72)] {
            assert_eq!(
                bt.step(Some(said), 72, t),
                BtStep::Nothing,
                "{said} said at {t} ms moved the volume"
            );
        }
    }

    #[test]
    fn a_volume_changed_on_the_headphones_is_taken() {
        let mut bt = BtVolume::default();
        assert_eq!(bt.step(Some(50), 50, 0), BtStep::Send(50));
        assert_eq!(bt.step(Some(50), 50, 3_000), BtStep::Nothing);
        // a swipe on the AirPods, long after the console last sent theirs
        assert_eq!(bt.step(Some(40), 50, 6_000), BtStep::Adopt(40));
        assert_eq!(bt.step(Some(40), 40, 6_100), BtStep::Nothing);
    }

    #[test]
    fn the_rounding_of_avrcp_s_steps_is_not_a_change() {
        let mut bt = BtVolume::default();
        assert_eq!(bt.step(None, 55, 0), BtStep::Send(55));
        assert_eq!(bt.step(Some(54), 55, 5_000), BtStep::Nothing);
    }
}
