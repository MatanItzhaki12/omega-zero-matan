//! The opening: Mega Man X5's NOM ENGINEER WORK SYSTEM boot screen, told about this handheld.
//! A boot that lands on the shelf fades the bootloader's logo out to black, fades the terminal
//! in, types its way through ten seconds of it, fades it back out and fades the shelf in. A
//! press cuts it short: the terminal fades out at once, then the shelf comes in, and the press
//! goes no further. Once the shelf shows through, a press ends it there and is the shelf's.
//! When the boot left a game on the shelf because its start took omega-zero down, the black
//! stays and the start notice comes up on it in place of the shelf (`start_notice`).

use omega_gfx::{out_h, out_w};
use omega_input::Action;
use omega_power::{baseos_release, BaseosRelease};
use omega_ui::{ease, Draw, TexId, OPENING_ADVANCE, OPENING_PITCH, OPENING_RISE};

use crate::app::{App, Phase};
use crate::audio::Sfx;
use crate::boot_fade::{FADE_MS, STEP_MS};

/// How long the bootloader's logo takes to fade to black before the terminal comes in, in
/// ms: the logo's own fade.
pub const LEAD_MS: f32 = FADE_MS;

/// How long the terminal takes to fade in, and out at the end, in ms.
pub const IN_MS: f32 = 500.0;
pub const OUT_MS: f32 = 500.0;

/// How long the terminal runs from the start of its fade in to the start of its fade out.
pub const SCRIPT_MS: f32 = 10_000.0;

/// How long the black takes to lift off the shelf, in ms.
pub const REVEAL_MS: f32 = 500.0;

/// How long a press takes to fade the terminal out, and the shelf in after it.
pub const CUT_OUT_MS: f32 = 200.0;
pub const CUT_REVEAL_MS: f32 = 300.0;

/// The terminal's size, in letters and lines: the screen it was written for.
pub(crate) const COLS: u32 = 32;
pub(crate) const LINES: u32 = 22;

/// How long a count of memory takes to run up, and in how many steps.
const COUNT_MS: f32 = 800.0;
const COUNT_STEPS: u32 = 16;

/// The gaps between keys, taken in turn: a hand types unevenly.
const KEY_GAPS: [f32; 7] = [120.0, 95.0, 150.0, 110.0, 165.0, 100.0, 135.0];

/// How long after its prompt a line's first key is typed.
const FIRST_KEY_MS: f32 = 300.0;

/// What the opening tells about the handheld.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Facts {
    /// Its name, as BaseOS gives it.
    pub model: String,
    /// The memory fitted, in MB: what the kernel was given, rounded up to the chips' size.
    pub mem_mb: u32,
    /// The memory free to use as it boots, in MB.
    pub avail_mb: u32,
    /// Its battery's size, where it is known.
    pub battery_mah: Option<u32>,
}

impl Default for Facts {
    /// The RG35XX SP's, the handheld omega-zero is made on, for a computer standing in for one.
    fn default() -> Self {
        Facts {
            model: "RG35XX SP".into(),
            mem_mb: 1024,
            avail_mb: 872,
            battery_mah: Some(3300),
        }
    }
}

impl Facts {
    /// This handheld's, from BaseOS and the kernel, each the RG35XX SP's where it cannot be
    /// read.
    pub fn read() -> Facts {
        let meminfo = std::fs::read_to_string("/proc/meminfo").ok();
        Facts::from_parts(baseos_release(), meminfo.as_deref())
    }

    /// The facts in `release`, BaseOS's description of the handheld, and `meminfo`, the
    /// kernel's /proc/meminfo.
    pub fn from_parts(release: Option<BaseosRelease>, meminfo: Option<&str>) -> Facts {
        let mut facts = Facts::default();
        if let Some(r) = release {
            if let Some(model) = r
                .model
                .clone()
                .or_else(|| r.target.as_ref().map(|t| t.to_uppercase()))
            {
                facts.model = model;
            }
            facts.battery_mah = r.target.as_deref().and_then(battery_mah);
        }
        let kb = |key: &str| {
            meminfo?
                .lines()
                .find_map(|l| l.strip_prefix(key))
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|n| n.parse::<u64>().ok())
        };
        if let Some(total) = kb("MemTotal:") {
            facts.mem_mb = fitted_mb(total);
        }
        if let Some(avail) = kb("MemAvailable:") {
            facts.avail_mb = (avail / 1024) as u32;
        }
        facts
    }
}

/// The memory chips' size, in MB, for the `kb` the kernel was given of them: it keeps some for
/// itself and the GPU, so it rounds up to the power of two.
fn fitted_mb(kb: u64) -> u32 {
    (kb.div_ceil(1024).max(1) as u32).next_power_of_two()
}

/// The battery fitted to each BaseOS build target, where its maker's figure has been checked.
fn battery_mah(target: &str) -> Option<u32> {
    match target {
        "rg35xxsp" => Some(3300),
        _ => None,
    }
}

/// How one line of the terminal comes up.
enum Line {
    /// Printed whole.
    Shown(String),
    /// `prompt` printed, then `text` typed a key at a time, at `keys` ms, starred out when
    /// `masked`.
    Typed {
        prompt: &'static str,
        text: &'static str,
        keys: Vec<f32>,
        masked: bool,
    },
    /// `label` and a count of MB run up to `to`.
    Counted { label: &'static str, to: u32 },
    /// The battery: its size where it is known, and its charge as it stands.
    Battery(Option<u32>),
}

/// A line, the terminal's row it is printed on, and when it comes up, in ms from the start of
/// the terminal's fade in.
struct Row {
    row: usize,
    at: f32,
    line: Line,
}

/// The terminal's lines and the sounds they make, after X5's.
struct Script {
    rows: Vec<Row>,
    sounds: Vec<(f32, Sfx)>,
    gaps: std::iter::Cycle<std::slice::Iter<'static, f32>>,
}

impl Script {
    fn new(facts: &Facts) -> Script {
        let mut s = Script {
            rows: Vec::new(),
            sounds: vec![(0.0, Sfx::Blip)],
            gaps: KEY_GAPS.iter().cycle(),
        };
        s.shown(0, 0.0, "OMEGA ZERO MATAN HANDHELD");
        let model: String = facts.model.chars().take(COLS as usize - 6).collect();
        s.shown(1, 0.0, &format!("Model {model}"));
        s.shown(3, 450.0, "(c) 2026 OAURAMOS & MATAN");
        s.shown(4, 450.0, "omega-zero-matan on BaseOS");
        s.shown(5, 450.0, "GPL-3.0 Free Software");
        s.counted(7, 850.0, "real  mem =", facts.mem_mb);
        s.counted(8, 850.0 + COUNT_MS + 150.0, "avail mem =", facts.avail_mb);
        s.shown(10, 2750.0, "cpu     : H700 1.5GHz x4");
        s.shown(11, 3000.0, "gpu     : Mali-G31 MP2");
        s.push(12, 3250.0, Line::Battery(facts.battery_mah));
        let done = s.typed(14, 3650.0, "login: ", "root", false);
        let done = s.typed(15, done + 300.0, "code : ", "root", true);
        let done = s.typed(17, done + 350.0, ">", "omega -boot -a", false);
        let read = done + 400.0;
        for (i, (file, sound)) in [
            ("GBA.BIOS", Sfx::Reading),
            ("CARTS", Sfx::Reading),
            ("OMEGA", Sfx::Omega),
        ]
        .into_iter()
        .enumerate()
        {
            let at = read + 550.0 * i as f32;
            s.shown(19 + i, at, &format!("reading \"{file}\""));
            s.sounds.push((at, sound));
        }
        s
    }

    fn push(&mut self, row: usize, at: f32, line: Line) {
        self.rows.push(Row { row, at, line });
    }

    fn shown(&mut self, row: usize, at: f32, text: &str) {
        self.push(row, at, Line::Shown(text.into()));
    }

    fn counted(&mut self, row: usize, at: f32, label: &'static str, to: u32) {
        self.push(row, at, Line::Counted { label, to });
    }

    /// Types `text` after `prompt`, from `at`, a sound to each key that is not a space, and
    /// says when the last key falls.
    fn typed(
        &mut self,
        row: usize,
        at: f32,
        prompt: &'static str,
        text: &'static str,
        masked: bool,
    ) -> f32 {
        let mut keys = Vec::new();
        let mut t = at + FIRST_KEY_MS;
        for ch in text.chars() {
            keys.push(t);
            if ch != ' ' {
                self.sounds.push((t, Sfx::Key));
            }
            t += self.gaps.next().copied().unwrap_or(120.0);
        }
        let last = keys.last().copied().unwrap_or(at);
        self.push(
            row,
            at,
            Line::Typed {
                prompt,
                text,
                keys,
                masked,
            },
        );
        last
    }

    /// Row `row`'s text `ms` into the terminal, with the battery at `battery` percent, if it
    /// has come up.
    fn text(&self, row: &Row, ms: f32, battery: Option<u8>) -> Option<String> {
        if ms < row.at {
            return None;
        }
        Some(match &row.line {
            Line::Shown(text) => text.clone(),
            Line::Typed {
                prompt,
                text,
                keys,
                masked,
            } => {
                let n = keys.iter().filter(|&&k| ms >= k).count();
                let typed: String = if *masked {
                    "*".repeat(n)
                } else {
                    text.chars().take(n).collect()
                };
                format!("{prompt}{typed}")
            }
            Line::Counted { label, to } => {
                let step =
                    (((ms - row.at) / COUNT_MS * COUNT_STEPS as f32) as u32 + 1).min(COUNT_STEPS);
                let n = (*to as u64 * step as u64 / COUNT_STEPS as u64) as u32;
                format!("{label}{n:5} MB")
            }
            Line::Battery(mah) => {
                let size = mah.map(|m| format!("{m}mAh  ")).unwrap_or_default();
                let charge = battery.map(|p| format!("{p}%")).unwrap_or_default();
                format!("battery : {size}{charge}").trim_end().to_string()
            }
        })
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

/// The opening while it runs: its script, how far in it is and what has been uploaded of it.
pub(crate) struct Opening {
    script: Script,
    /// ms since its first frame, once there has been one.
    elapsed: Option<f32>,
    /// When a press cut it short, and how bright the terminal stood then.
    cut: Option<(f32, f32)>,
    /// Each row's face, as the frontend uploaded it for the row's text now.
    faces: Vec<Option<(TexId, u32, u32)>>,
}

impl Opening {
    fn new(facts: &Facts) -> Opening {
        Opening {
            script: Script::new(facts),
            elapsed: None,
            cut: None,
            faces: vec![None; LINES as usize],
        }
    }

    fn elapsed(&self) -> f32 {
        self.elapsed.unwrap_or(0.0)
    }

    /// How far into the terminal it is, in ms from the start of its fade in: still once a
    /// press has cut it short.
    fn script_ms(&self) -> f32 {
        self.cut.map_or_else(|| self.elapsed(), |(at, _)| at) - LEAD_MS
    }

    /// How brightly the terminal is drawn over the black.
    fn ink(&self) -> f32 {
        if let Some((at, level)) = self.cut {
            return level * (1.0 - ease(((self.elapsed() - at) / CUT_OUT_MS).clamp(0.0, 1.0)));
        }
        let ms = self.script_ms();
        if ms < 0.0 {
            0.0
        } else if ms < IN_MS {
            ease(ms / IN_MS)
        } else if ms < SCRIPT_MS {
            1.0
        } else {
            1.0 - ease(((ms - SCRIPT_MS) / OUT_MS).min(1.0))
        }
    }

    /// When the black starts to lift off the shelf, in ms from the first frame, and how long
    /// it takes.
    fn reveal(&self) -> (f32, f32) {
        match self.cut {
            Some((at, _)) => (at + CUT_OUT_MS, CUT_REVEAL_MS),
            None => (LEAD_MS + SCRIPT_MS + OUT_MS, REVEAL_MS),
        }
    }

    /// How black the screen is under the terminal.
    fn veil(&self) -> f32 {
        let (from, takes) = self.reveal();
        1.0 - ease(((self.elapsed() - from) / takes).clamp(0.0, 1.0))
    }

    /// Whether the shelf has started to show through.
    fn revealing(&self) -> bool {
        self.elapsed() > self.reveal().0
    }

    fn over(&self) -> bool {
        self.veil() <= 0.0
    }

    /// The terminal's rows and what each says now.
    fn lines(&self, battery: Option<u8>) -> Vec<(usize, String)> {
        let ms = self.script_ms();
        self.script
            .rows
            .iter()
            .filter_map(|r| self.script.text(r, ms, battery).map(|t| (r.row, t)))
            .collect()
    }
}

/// Where the terminal's first letter stands on a `w` by `h` screen: the terminal in the middle
/// of it, on the screen pixels' even grid. The top of its capitals, not of the line's face.
pub fn terminal_origin(w: u32, h: u32) -> (u32, u32) {
    let x = w.saturating_sub(COLS * OPENING_ADVANCE) / 2;
    let y = h.saturating_sub(LINES * OPENING_PITCH) / 2;
    (x & !1, y & !1)
}

fn origin() -> (f32, f32) {
    let (x, y) = terminal_origin(out_w(), out_h());
    (x as f32, y as f32)
}

impl App {
    /// Plays the opening over the shelf from the next frame, telling `facts`, where SHELF's
    /// Boot Animation has it played. It is only played while the boot is still on the shelf it
    /// landed on, as the boot logo's fade is. The start notice names the handheld as it does,
    /// whether it plays or not.
    pub fn start_opening(&mut self, facts: &Facts) {
        self.tell_start_notice(facts);
        if self.state.boot_animation && self.boot_fades() {
            self.opening = Some(Opening::new(facts));
        }
    }

    /// Whether the opening is playing.
    pub fn opening_shown(&self) -> bool {
        self.opening.is_some()
    }

    /// How black the opening has the screen, and how brightly its terminal is drawn, while it
    /// plays.
    pub fn opening_levels(&self) -> Option<(f32, f32)> {
        self.opening.as_ref().map(|o| (o.veil(), o.ink()))
    }

    /// The terminal's rows and what each says now, while it can be seen.
    pub fn opening_lines(&self) -> Vec<(usize, String)> {
        match &self.opening {
            Some(o) if o.ink() > 0.0 => o.lines(self.battery().map(|b| b.percent)),
            _ => Vec::new(),
        }
    }

    /// Hands the opening `tex`, `w` by `h`, as row `row`'s face for what it says now.
    pub fn set_opening_face(&mut self, row: usize, tex: TexId, w: u32, h: u32) {
        if let Some(face) = self.opening.as_mut().and_then(|o| o.faces.get_mut(row)) {
            *face = Some((tex, w, h));
        }
    }

    /// Whether the opening keeps the screen black, so nothing under it need be drawn.
    pub(crate) fn opening_covers(&self) -> bool {
        self.opening.as_ref().is_some_and(|o| o.veil() >= 1.0)
    }

    /// Whether the opening has put its terminal away and its black would start to lift off the
    /// shelf now.
    pub(crate) fn opening_lifting(&self) -> bool {
        self.opening.as_ref().is_some_and(Opening::revealing)
    }

    /// Whether the opening takes `action`. A press cuts the terminal short and goes no
    /// further; once the shelf shows through it ends the opening and is the shelf's. The lid
    /// and POWER end it at once and do what they do; brightness and volume work through it.
    pub(crate) fn opening_takes(&mut self, action: Action) -> bool {
        let Some(o) = &mut self.opening else {
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
                self.opening = None;
                false
            }
            Action::GbaUp(_) | Action::RewindStop | Action::FfStop => !o.revealing(),
            _ if o.revealing() => {
                self.opening = None;
                false
            }
            _ => {
                if o.cut.is_none() {
                    o.cut = Some((o.elapsed(), o.ink()));
                }
                true
            }
        }
    }

    /// Moves the opening on by `dt` seconds, from its first frame, sounding what falls due,
    /// and ends it once the shelf has come in or gone.
    pub(crate) fn step_opening(&mut self, dt: f32) {
        let on_shelf = matches!(self.phase, Phase::Shelf);
        let Some(o) = &mut self.opening else {
            return;
        };
        let before = o.elapsed.map_or(f32::NEG_INFINITY, |_| o.script_ms());
        o.elapsed = Some(match o.elapsed {
            None => 0.0,
            Some(e) => e + (dt * 1000.0).min(STEP_MS),
        });
        if o.cut.is_none() {
            if let Some(sfx) = o.script.sound(before, o.script_ms()) {
                self.sfx = Some(sfx);
            }
        }
        if o.over() || !on_shelf {
            self.opening = None;
        }
    }

    /// The black over everything and the terminal on it, as far as each has faded.
    pub(crate) fn draw_opening(&self, out: &mut Vec<Draw>) {
        if let Some(o) = &self.opening {
            draw_terminal(o.veil(), o.ink(), &o.faces, out);
        }
    }
}

/// A terminal over everything: the black under it, `veil` opaque, and its rows' `faces` on the
/// terminal's grid, as bright as `ink`.
pub(crate) fn draw_terminal(
    veil: f32,
    ink: f32,
    faces: &[Option<(TexId, u32, u32)>],
    out: &mut Vec<Draw>,
) {
    if veil <= 0.0 {
        return;
    }
    out.push(Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: out_w() as f32,
        h: out_h() as f32,
        colour: [0.0, 0.0, 0.0, veil],
    });
    if ink <= 0.0 {
        return;
    }
    let (x, y) = origin();
    for (row, face) in faces.iter().enumerate() {
        let Some((tex, w, h)) = *face else {
            continue;
        };
        out.push(Draw::Tex {
            x,
            y: y + (row as u32 * OPENING_PITCH) as f32 - OPENING_RISE as f32,
            w: w as f32,
            h: h as f32,
            tex,
            alpha: ink,
        });
    }
}
