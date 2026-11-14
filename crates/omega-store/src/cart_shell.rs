use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;

pub const CART_SHELL_FILE: &str = "Config/cart_shell.ini";
pub const LABELS_SHELL_FILE: &str = "Labels/cart_shell.ini";
/// The same lines under the name Slot reads them from, which Cart Studio writes and a card moved
/// over from Slot already has. Labels/cart_shell.ini wins over it for a game both name.
pub const SLOT_SHELL_FILE: &str = "Labels/cart_shell.txt";

/// The shape a cart is drawn in, where its system has more than one: `auto` leaves it to the
/// ROM. The Game Boy pak comes notched (the grey and black paks) or rounded (the Game Boy
/// Color's); the SNES cart comes as the American cart (`ntsc`) or the Super Famicom's (`sfc`),
/// which Europe's carts share. A shape of another system's is no choice and draws as `auto`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Outline {
    Auto,
    Notched,
    Rounded,
    Ntsc,
    Sfc,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ShellFinish {
    Solid,
    Clear,
    Glitter,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ShellChoice {
    pub outline: Outline,
    pub colour: [u8; 3],
    pub finish: ShellFinish,
}

impl ShellChoice {
    pub fn parse(value: &str) -> Option<ShellChoice> {
        let mut words = value.split_whitespace();
        let outline = match words.next()? {
            "auto" => Outline::Auto,
            "notched" => Outline::Notched,
            "rounded" => Outline::Rounded,
            "ntsc" => Outline::Ntsc,
            "sfc" => Outline::Sfc,
            _ => return None,
        };
        let hex = words.next()?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let colour = [0, 2, 4].map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0));
        let finish = match words.next()? {
            "solid" => ShellFinish::Solid,
            "clear" => ShellFinish::Clear,
            "glitter" => ShellFinish::Glitter,
            _ => return None,
        };
        words.next().is_none().then_some(ShellChoice {
            outline,
            colour,
            finish,
        })
    }

    pub fn to_value(&self) -> String {
        let outline = match self.outline {
            Outline::Auto => "auto",
            Outline::Notched => "notched",
            Outline::Rounded => "rounded",
            Outline::Ntsc => "ntsc",
            Outline::Sfc => "sfc",
        };
        let finish = match self.finish {
            ShellFinish::Solid => "solid",
            ShellFinish::Clear => "clear",
            ShellFinish::Glitter => "glitter",
        };
        let [r, g, b] = self.colour;
        format!("{outline} {r:02x}{g:02x}{b:02x} {finish}")
    }
}

/// Keeps `choice` as the look of the game `stem`, in `Labels/cart_shell.ini`, the file whose
/// choices win over the others: none writes `auto`, the cart's own look whatever the others
/// say. The cart editor's Save.
pub fn write_choice(
    root: &std::path::Path,
    stem: &str,
    choice: Option<ShellChoice>,
) -> std::io::Result<()> {
    let path = root.join(LABELS_SHELL_FILE);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let value = choice.map_or_else(|| "auto".to_string(), |c| c.to_value());
    let text = crate::ini::set(&text, stem, Some(&value))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::atomic_write(&path, text.as_bytes())
}

pub fn key(stem: &str) -> String {
    stem.nfc().collect()
}

pub fn choices(text: &str) -> HashMap<String, ShellChoice> {
    crate::ini::parse(text)
        .into_iter()
        .filter_map(|(stem, value)| Some((key(&stem), ShellChoice::parse(&value)?)))
        .collect()
}

pub fn layered(system: &str, labels: &str) -> HashMap<String, ShellChoice> {
    let mut out = choices(system);
    for (stem, value) in crate::ini::parse(labels) {
        if value.trim() == "auto" {
            out.remove(&key(&stem));
        } else if let Some(choice) = ShellChoice::parse(&value) {
            out.insert(key(&stem), choice);
        }
    }
    out
}
