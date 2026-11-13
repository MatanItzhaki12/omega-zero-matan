//! The clock: its words on the shelf and in Settings, and the picker that sets it, at first
//! boot and from Settings' Date & Time.
//!
//! The picker is one line of Pixel Code on the kit's grid, as flow.html's Date & Time draws it:
//! `2026 - 10 - 10   16 : 00   UTC+00:00`, each field a box of its own so the one under the
//! cursor is underlined, a one-pixel bar in the frame's bright colour two rows below its
//! capitals. The line is a face at art resolution in the kit's palette, for the Date & Time
//! panel to hold.

use omega_store::{
    civil_from_days, days_from_civil, days_in_month, parse_stamp, UTC_OFFSET_MAX, UTC_OFFSET_MIN,
};

use crate::kit::palette::Palette;
use crate::kit::pixel::{blank, fill, ink, letters, ADVANCE};
use crate::plate::UndoFace;

const DAY: i64 = 86_400;

const YEAR_MIN: i64 = 2000;
const YEAR_MAX: i64 = 2099;

const OFFSET_STEP: i64 = 30;

/// What stands between one field of the line and the next: a dash between the date's, a colon
/// between the time's, and three spaces either side of the time.
const BETWEEN: [&str; 5] = [" - ", " - ", "   ", " : ", "   "];

/// How many letters the line is, whatever it says: every field is always as wide.
pub const LINE_LETTERS: u32 = 36;

/// How far under the line's top the underline is: its row 11, a row clear of the line's own 10
/// (whose capitals are rows 2 to 8).
pub const UNDERLINE_Y: u32 = 11;

/// The line's face: 6 art pixels a letter across, and down to the underline.
pub const LINE_W: u32 = LINE_LETTERS * ADVANCE as u32;
pub const LINE_H: u32 = UNDERLINE_Y + 1;

pub fn clock_label(stamp: &str) -> String {
    parse_stamp(stamp).map(hhmm).unwrap_or_default()
}

pub fn hhmm(secs: i64) -> String {
    let rem = secs.rem_euclid(DAY);
    format!("{:02}:{:02}", rem / 3600, rem / 60 % 60)
}

/// The time as the top bar reads it: 24-hour as `hhmm`, or 12-hour as `3:07 PM`, as Slot's
/// Clock setting picks.
pub fn clock_text(secs: i64, h24: bool) -> String {
    if h24 {
        return hhmm(secs);
    }
    let rem = secs.rem_euclid(DAY);
    let (hour, minute) = (rem / 3600, rem / 60 % 60);
    let twelve = match hour % 12 {
        0 => 12,
        h => h,
    };
    let half = if hour < 12 { "AM" } else { "PM" };
    format!("{twelve}:{minute:02} {half}")
}

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// The date and the time as Settings' Date & Time row reads them, in 24 or 12-hour time.
pub fn date_time_text_in(secs: i64, h24: bool) -> String {
    let (date, _) = date_time_text(secs)
        .rsplit_once(' ')
        .map(|(d, t)| (d.to_string(), t.to_string()))
        .unwrap_or_default();
    format!("{date} {}", clock_text(secs, h24))
}

pub fn date_time_text(secs: i64) -> String {
    let (_, month, day) = civil_from_days(secs.div_euclid(DAY));
    let name = MONTHS
        .get((month - 1) as usize)
        .copied()
        .unwrap_or_default();
    format!("{name} {day} {}", hhmm(secs))
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Field {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Offset,
}

impl Field {
    pub const ALL: [Field; 6] = [
        Field::Year,
        Field::Month,
        Field::Day,
        Field::Hour,
        Field::Minute,
        Field::Offset,
    ];
}

#[derive(Clone, Debug)]
pub struct ClockPicker {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    offset_min: i64,
    cursor: usize,
}

impl ClockPicker {
    pub fn from_secs(secs: i64) -> Self {
        let (year, month, day) = civil_from_days(secs.div_euclid(DAY));
        let rem = secs.rem_euclid(DAY);
        ClockPicker {
            year: year.clamp(YEAR_MIN, YEAR_MAX),
            month,
            day,
            hour: rem / 3600,
            minute: rem / 60 % 60,
            offset_min: 0,
            cursor: 0,
        }
    }

    pub fn local(utc: i64, offset_min: i16) -> Self {
        let offset = i64::from(offset_min);
        ClockPicker {
            offset_min: offset,
            ..ClockPicker::from_secs(utc + offset * 60)
        }
    }

    pub fn from_ymd(year: i64, month: i64, day: i64) -> Self {
        ClockPicker {
            year: year.clamp(YEAR_MIN, YEAR_MAX),
            month,
            day,
            hour: 0,
            minute: 0,
            offset_min: 0,
            cursor: 0,
        }
    }

    pub fn secs(&self) -> i64 {
        let local = days_from_civil(self.year, self.month, self.day) * DAY
            + self.hour * 3600
            + self.minute * 60;
        local - self.offset_min * 60
    }

    pub fn offset_min(&self) -> i64 {
        self.offset_min
    }

    pub fn month(&self) -> i64 {
        self.month
    }

    pub fn day(&self) -> i64 {
        self.day
    }

    pub fn cursor(&self) -> Field {
        Field::ALL[self.cursor]
    }

    pub fn field(&mut self, field: Field) {
        self.cursor = Field::ALL.iter().position(|f| *f == field).unwrap_or(0);
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(Field::ALL.len() - 1);
    }

    pub fn up(&mut self) {
        self.step(1);
    }

    pub fn down(&mut self) {
        self.step(-1);
    }

    fn step(&mut self, by: i64) {
        match self.cursor() {
            Field::Year => self.year = wrap(self.year + by, YEAR_MIN, YEAR_MAX),
            Field::Month => self.month = wrap(self.month + by, 1, 12),
            Field::Day => self.day = wrap(self.day + by, 1, days_in_month(self.year, self.month)),
            Field::Hour => self.hour = wrap(self.hour + by, 0, 23),
            Field::Minute => self.minute = wrap(self.minute + by, 0, 59),
            Field::Offset => {
                self.offset_min = (self.offset_min + by * OFFSET_STEP)
                    .clamp(i64::from(UTC_OFFSET_MIN), i64::from(UTC_OFFSET_MAX));
            }
        }
        self.day = self.day.min(days_in_month(self.year, self.month));
    }

    pub fn text(&self) -> String {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02} {}",
            self.year,
            self.month,
            self.day,
            self.hour,
            self.minute,
            offset_text(self.offset_min)
        )
    }

    /// The line as it reads, field by field.
    pub fn line(&self) -> String {
        self.parts().into_iter().map(|(text, _)| text).collect()
    }

    /// The line's parts, left to right: each field's text with the field, and what stands
    /// between two fields with none.
    fn parts(&self) -> Vec<(String, Option<Field>)> {
        let mut parts = Vec::new();
        for (i, field) in Field::ALL.iter().enumerate() {
            parts.push((self.field_text(*field), Some(*field)));
            if let Some(between) = BETWEEN.get(i) {
                parts.push((between.to_string(), None));
            }
        }
        parts
    }

    /// The line in Pixel Code in the panel's cream, the field under the cursor underlined in
    /// the frame's bright colour, on a clear face `LINE_W` by `LINE_H`: put its top at the
    /// line's top.
    ///
    /// Each part is set on its own, as flow.html sets each in a box of its own. So the colon
    /// between the hour and the minute, alone in its box, stands where a colon does, and only
    /// the offset's, between figures, is raised a pixel (see `kit::pixel::letters`).
    pub fn face(&self, pal: &Palette) -> UndoFace {
        let mut face = blank(LINE_W, LINE_H);
        let mut x = 0;
        for (text, _) in self.parts() {
            ink(&mut face, &letters(&text, ADVANCE), x, 0, pal.cream);
            x += (text.chars().count() * ADVANCE) as i32;
        }
        let [x, y, w, h] = self.underline();
        fill(
            &mut face,
            [x as i32, y as i32, w as i32, h as i32],
            pal.frame_hi,
        );
        face
    }

    /// Where the underline is, `[x, y, w, h]` in the line face's art pixels: under the field
    /// the cursor is on, as wide as its letters but for the last one's blank column.
    pub fn underline(&self) -> [u32; 4] {
        let mut x = 0;
        for (text, field) in self.parts() {
            let w = (text.chars().count() * ADVANCE) as u32;
            if field == Some(self.cursor()) {
                return [x, UNDERLINE_Y, w - 1, 1];
            }
            x += w;
        }
        [0, UNDERLINE_Y, 0, 1]
    }

    fn field_text(&self, field: Field) -> String {
        match field {
            Field::Year => format!("{:04}", self.year),
            Field::Month => format!("{:02}", self.month),
            Field::Day => format!("{:02}", self.day),
            Field::Hour => format!("{:02}", self.hour),
            Field::Minute => format!("{:02}", self.minute),
            Field::Offset => zone_text(self.offset_min),
        }
    }
}

/// A time zone `offset_min` minutes from UTC as the picker shows it: `UTC+05:30`.
pub fn zone_text(offset_min: i64) -> String {
    format!("UTC{}", offset_text(offset_min))
}

fn offset_text(offset_min: i64) -> String {
    let sign = if offset_min < 0 { '-' } else { '+' };
    let mins = offset_min.abs();
    format!("{sign}{:02}:{:02}", mins / 60, mins % 60)
}

fn wrap(v: i64, lo: i64, hi: i64) -> i64 {
    (v - lo).rem_euclid(hi - lo + 1) + lo
}
