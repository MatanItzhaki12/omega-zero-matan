//! The start notice: the opening's terminal telling the player that the last start of a game
//! took omega-zero down, naming the game, its system and the core it was started on, saying it
//! was left on the shelf rather than resumed, and how to try it another way. A boot that finds
//! the game noted as starting shows it where the shelf would first be seen: the opening's black
//! stays, in place of the shelf coming in, and the notice comes up on it; with no opening, the
//! shelf fades to black first. Its lines come up in turn, in the opening's lettering on its
//! grid. Presses are ignored until its prompt is up, so a press meant to cut the opening short
//! cannot put it away unread; then A, B or START fades it out to the shelf, on the game it
//! names, and goes no further.

use std::path::Path;

use omega_input::{Action, Btn};
use omega_store::{Core, Platform};
use omega_ui::{clean_label, ease, Draw, TexId};

use crate::app::{App, Phase};
use crate::audio::Sfx;
use crate::boot_fade::STEP_MS;
use crate::opening::{self, draw_terminal, Facts, COLS, IN_MS, LINES};
use crate::persist;

/// How long the shelf takes to fade to black under a notice with no opening's black to come up
/// on, in ms.
pub const BLACK_MS: f32 = 300.0;

/// When the prompt comes up, in ms from the start of the terminal's fade in. No press puts the
/// notice away before then.
pub const PROMPT_MS: f32 = 2_300.0;

/// How long a press takes to fade the terminal out, and the shelf in after it, as a press
/// cutting the opening short does.
pub const OUT_MS: f32 = opening::CUT_OUT_MS;
pub const REVEAL_MS: f32 = opening::CUT_REVEAL_MS;

/// How long the cursor after the prompt stays lit, and then dark, as it blinks.
const BLINK_MS: f32 = 500.0;

/// How far in a value stands after its label, `game : `, in letters.
const VALUE_AT: usize = 7;

/// The most lines a game's name takes.
const NAME_LINES: usize = 2;

/// The game whose start took omega-zero down, as the notice names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fell {
    pub platform: Platform,
    pub stem: String,
    /// The core chosen for it, which it was started on and would be started on again.
    pub core: Core,
}

/// How one row of the notice comes up.
enum Line {
    /// Printed whole.
    Shown(String),
    /// Printed whole, with a cursor blinking after it.
    Prompt(String),
}

/// A line, the terminal's row it is printed on, and when it comes up, in ms from the start of
/// the terminal's fade in.
struct Row {
    row: usize,
    at: f32,
    line: Line,
}

/// The notice's rows and the sounds they make, for `fell` on the handheld `model`.
fn script(fell: &Fell, model: &str) -> (Vec<Row>, Vec<(f32, Sfx)>) {
    let mut rows = Vec::new();
    let mut shown = |row: usize, at: f32, text: String| {
        rows.push(Row {
            row,
            at,
            line: Line::Shown(text),
        });
    };
    shown(0, 0.0, "OMEGA ZERO MATAN HANDHELD".into());
    let model: String = model.chars().take(COLS as usize - 6).collect();
    shown(1, 0.0, format!("Model {model}"));
    shown(3, 400.0, "!! LAST START FAILED".into());
    let mut row = 5;
    let name = clean_label(&fell.stem);
    for (i, part) in wrap(&name, COLS as usize - VALUE_AT, NAME_LINES)
        .into_iter()
        .enumerate()
    {
        let label = if i == 0 { "game : " } else { "       " };
        shown(row, 800.0, format!("{label}{part}"));
        row += 1;
    }
    shown(row, 950.0, format!("sys  : {}", fell.platform.dir_name()));
    shown(row + 1, 1100.0, format!("core : {}", fell.core.name()));
    row += 3;
    for text in [
        "The game took the system down",
        "as it started. It was left on",
        "the shelf, not resumed.",
    ] {
        shown(row, 1500.0, text.into());
        row += 1;
    }
    row += 1;
    // the core picker opens only on a system with another core to pick
    let advice = match Core::for_platform(fell.platform).len() > 1 {
        true => ["X on the shelf picks another", "core for it."],
        false => [
            "Hold A on the shelf to start",
            "it clean, without its resume.",
        ],
    };
    for text in advice {
        shown(row, 1900.0, text.into());
        row += 1;
    }
    rows.push(Row {
        row: row + 1,
        at: PROMPT_MS,
        line: Line::Prompt(">press A".into()),
    });
    let sounds = vec![(400.0, Sfx::Reading), (PROMPT_MS, Sfx::Key)];
    (rows, sounds)
}

/// `text` broken at its spaces into lines of `width` letters at most, `most` of them. A word
/// longer than a line is cut where the line ends, and a name that runs past the last line ends
/// it in three dots.
fn wrap(text: &str, width: usize, most: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        while !word.is_empty() {
            let len = line.chars().count();
            let room = match len {
                0 => width,
                _ => width.saturating_sub(len + 1),
            };
            if word.len() <= room {
                if len > 0 {
                    line.push(' ');
                }
                line.extend(word.drain(..));
            } else if len == 0 {
                line.extend(word.drain(..width));
                lines.push(std::mem::take(&mut line));
            } else {
                lines.push(std::mem::take(&mut line));
            }
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    if lines.len() > most {
        lines.truncate(most);
        let last = &mut lines[most - 1];
        let kept: String = last.chars().take(width - 3).collect();
        *last = format!("{}...", kept.trim_end());
    }
    lines
}

/// The notice while it is on the screen: its rows, how far in it is and what has been
/// uploaded of it.
struct Up {
    rows: Vec<Row>,
    sounds: Vec<(f32, Sfx)>,
    /// How long the black takes to come over the shelf before the terminal fades in: none on
    /// the opening's black.
    lead: f32,
    /// ms since its first frame, once there has been one.
    elapsed: Option<f32>,
    /// When a press put it away, and how bright the terminal stood then.
    gone: Option<(f32, f32)>,
    /// Each row's face, as the frontend uploaded it for the row's text now.
    faces: Vec<Option<(TexId, u32, u32)>>,
}

impl Up {
    fn new(fell: &Fell, model: &str, lead: f32) -> Up {
        let (rows, sounds) = script(fell, model);
        Up {
            rows,
            sounds,
            lead,
            elapsed: None,
            gone: None,
            faces: vec![None; LINES as usize],
        }
    }

    fn elapsed(&self) -> f32 {
        self.elapsed.unwrap_or(0.0)
    }

    /// How far into the terminal it is, in ms from the start of its fade in: still once a
    /// press has put it away.
    fn script_ms(&self) -> f32 {
        self.gone.map_or_else(|| self.elapsed(), |(at, _)| at) - self.lead
    }

    /// How brightly the terminal is drawn over the black.
    fn ink(&self) -> f32 {
        if let Some((at, level)) = self.gone {
            return level * (1.0 - ease(((self.elapsed() - at) / OUT_MS).clamp(0.0, 1.0)));
        }
        let ms = self.script_ms();
        if ms < 0.0 {
            0.0
        } else {
            ease((ms / IN_MS).min(1.0))
        }
    }

    /// How black the screen is under the terminal.
    fn veil(&self) -> f32 {
        match self.gone {
            Some((at, _)) => {
                1.0 - ease(((self.elapsed() - at - OUT_MS) / REVEAL_MS).clamp(0.0, 1.0))
            }
            None if self.lead > 0.0 => ease((self.elapsed() / self.lead).clamp(0.0, 1.0)),
            None => 1.0,
        }
    }

    /// Whether the shelf has started to show through.
    fn revealing(&self) -> bool {
        self.gone
            .is_some_and(|(at, _)| self.elapsed() > at + OUT_MS)
    }

    fn over(&self) -> bool {
        self.gone.is_some() && self.veil() <= 0.0
    }

    /// Whether the prompt is up, and a press may put the notice away.
    fn prompted(&self) -> bool {
        self.script_ms() >= PROMPT_MS
    }

    /// The terminal's rows and what each says now.
    fn lines(&self) -> Vec<(usize, String)> {
        let ms = self.script_ms();
        self.rows
            .iter()
            .filter(|r| ms >= r.at)
            .map(|r| {
                let text = match &r.line {
                    Line::Shown(text) => text.clone(),
                    Line::Prompt(text) => match ((ms - r.at) / BLINK_MS) as u32 % 2 {
                        0 => format!("{text}_"),
                        _ => text.clone(),
                    },
                };
                (r.row, text)
            })
            .collect()
    }

    /// The last sound due after `from` ms and by `to`.
    fn sound(&self, from: f32, to: f32) -> Option<Sfx> {
        self.sounds
            .iter()
            .filter(|(at, _)| *at > from && *at <= to)
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, s)| *s)
    }
}

/// The notice from the boot that found the note until it has been put away: what it tells,
/// and its terminal once it has come up.
pub(crate) struct StartNotice {
    fell: Fell,
    /// The handheld's name, as the opening tells it.
    model: String,
    up: Option<Up>,
}

impl StartNotice {
    /// The notice for `key`, the note of a game starting found on the card at `root`, when it
    /// names a game.
    pub(crate) fn of(root: &Path, key: &str) -> Option<StartNotice> {
        let (platform, stem) = persist::starting_game(key)?;
        let core = omega_store::core_for_platform(root, &stem, platform);
        Some(StartNotice {
            fell: Fell {
                platform,
                stem,
                core,
            },
            model: Facts::default().model,
            up: None,
        })
    }
}

impl App {
    /// The game the start notice names, while it is due or shown.
    pub fn start_notice_names(&self) -> Option<&Fell> {
        self.start_notice.as_ref().map(|n| &n.fell)
    }

    /// Whether the start notice is still to come up, or on the screen.
    pub fn start_notice_due(&self) -> bool {
        self.start_notice.is_some()
    }

    /// Whether the start notice is on the screen.
    pub fn start_notice_shown(&self) -> bool {
        self.notice_up().is_some()
    }

    /// Whether the start notice's prompt is up, so a press puts it away.
    pub fn start_notice_prompted(&self) -> bool {
        self.notice_up().is_some_and(Up::prompted)
    }

    /// How black the start notice has the screen, and how brightly its terminal is drawn,
    /// while it is shown.
    pub fn start_notice_levels(&self) -> Option<(f32, f32)> {
        self.notice_up().map(|u| (u.veil(), u.ink()))
    }

    /// The notice's rows and what each says now, while it can be seen.
    pub fn start_notice_lines(&self) -> Vec<(usize, String)> {
        match self.notice_up() {
            Some(u) if u.ink() > 0.0 => u.lines(),
            _ => Vec::new(),
        }
    }

    /// Hands the notice `tex`, `w` by `h`, as row `row`'s face for what it says now.
    pub fn set_start_notice_face(&mut self, row: usize, tex: TexId, w: u32, h: u32) {
        if let Some(face) = self.notice_up_mut().and_then(|u| u.faces.get_mut(row)) {
            *face = Some((tex, w, h));
        }
    }

    fn notice_up(&self) -> Option<&Up> {
        self.start_notice.as_ref()?.up.as_ref()
    }

    fn notice_up_mut(&mut self) -> Option<&mut Up> {
        self.start_notice.as_mut()?.up.as_mut()
    }

    /// Names the handheld in the notice as `facts` do.
    pub(crate) fn tell_start_notice(&mut self, facts: &Facts) {
        if let Some(n) = &mut self.start_notice {
            n.model.clone_from(&facts.model);
        }
    }

    /// Whether the notice keeps the screen black, so nothing under it need be drawn.
    pub(crate) fn start_notice_covers(&self) -> bool {
        self.notice_up().is_some_and(|u| u.veil() >= 1.0)
    }

    /// Whether the notice keeps the shelf out of sight: still to come up, or on the black.
    pub(crate) fn start_notice_hides_shelf(&self) -> bool {
        self.start_notice.is_some() && (self.notice_up().is_none() || self.start_notice_covers())
    }

    /// Whether the notice takes `action`. Until its prompt is up a press is ignored, and after
    /// it A, B or START puts the notice away; either way the press goes no further. Once the
    /// shelf shows through, a press ends it there and is the shelf's. The lid and POWER end it
    /// at once and do what they do; brightness and volume work through it, as through the
    /// opening.
    pub(crate) fn start_notice_takes(&mut self, action: Action) -> bool {
        let Some(up) = self.notice_up_mut() else {
            return false;
        };
        match action {
            Action::BrightnessUp
            | Action::BrightnessDown
            | Action::BlueLightUp
            | Action::BlueLightDown
            | Action::VolumeUp
            | Action::VolumeDown
            | Action::MuteToggle
            | Action::LidOpen => false,
            Action::LidClose
            | Action::PowerPress
            | Action::PowerTap
            | Action::PowerHold
            | Action::PowerRestart
            | Action::PowerOff => {
                self.start_notice = None;
                false
            }
            Action::GbaUp(_) | Action::RewindStop | Action::FfStop => !up.revealing(),
            _ if up.revealing() => {
                self.start_notice = None;
                false
            }
            Action::GbaDown(Btn::A | Btn::B | Btn::Start) if up.prompted() => {
                if up.gone.is_none() {
                    up.gone = Some((up.elapsed(), up.ink()));
                }
                true
            }
            _ => true,
        }
    }

    /// Brings the notice up where the shelf would first be seen: on the opening's black as
    /// the opening would let the shelf in, or over the shelf once it is shown with nothing
    /// moving on it. Then moves it on by `dt` seconds, sounding what falls due, and ends it
    /// once the shelf has come in or gone.
    pub(crate) fn step_start_notice(&mut self, dt: f32) {
        if self.start_notice.as_ref().is_some_and(|n| n.up.is_none()) {
            if self.opening_lifting() {
                self.opening = None;
                self.bring_up_notice(0.0);
            } else if self.opening.is_none()
                && self.boot_fades()
                && !self.power_shown()
                && self.core_picker.is_none()
            {
                self.bring_up_notice(BLACK_MS);
            }
        }
        let on_shelf = matches!(self.phase, Phase::Shelf);
        let Some(up) = self.notice_up_mut() else {
            return;
        };
        let before = up.elapsed.map_or(f32::NEG_INFINITY, |_| up.script_ms());
        up.elapsed = Some(match up.elapsed {
            None => 0.0,
            Some(e) => e + (dt * 1000.0).min(STEP_MS),
        });
        let sfx = match up.gone {
            None => up.sound(before, up.script_ms()),
            Some(_) => None,
        };
        let over = up.over();
        if let Some(sfx) = sfx {
            self.sfx = Some(sfx);
        }
        if over || !on_shelf {
            self.start_notice = None;
        }
    }

    /// Puts the notice's terminal on the screen, the black taking `lead` ms to come over the
    /// shelf. A hold begun on the shelf before it came goes no further.
    fn bring_up_notice(&mut self, lead: f32) {
        self.play_held = None;
        self.shelf_mut().release_hold();
        if let Some(n) = &mut self.start_notice {
            n.up = Some(Up::new(&n.fell, &n.model, lead));
        }
    }

    /// The black over everything and the notice's terminal on it, as far as each has faded.
    pub(crate) fn draw_start_notice(&self, out: &mut Vec<Draw>) {
        if let Some(u) = self.notice_up() {
            draw_terminal(u.veil(), u.ink(), &u.faces, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::wrap;

    #[test]
    fn a_name_breaks_at_its_spaces_and_a_long_one_ends_in_dots() {
        assert_eq!(wrap("Mega Man X", 25, 2), ["Mega Man X"]);
        assert_eq!(
            wrap("The Legend of Zelda A Link to the Past", 25, 2),
            ["The Legend of Zelda A", "Link to the Past"]
        );
        assert_eq!(
            wrap("Super Mario World 2 Yoshis Island Special Edition", 25, 2),
            ["Super Mario World 2", "Yoshis Island Special..."]
        );
        assert_eq!(
            wrap("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123", 25, 2),
            ["ABCDEFGHIJKLMNOPQRSTUVWXY", "Z0123"]
        );
        assert_eq!(wrap("", 25, 2), [""]);
    }
}
