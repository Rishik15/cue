//! File: palette.rs
//! Purpose: The native windows' colours (recording pill, notices, tray menu, dictionary card, Grab frame), read from `src/themes.json`, the same file the settings window
//! uses, so every surface matches in every family and mode. Derived colours use the file's ink percentages, the same maths as `src/styles.css`.
//! Contents: Rgb — a colour as 0..1 floats; Palette — every role plus derived hover, line and border; resolve — pick the family's dark or light variant (unknown family -> default).

use std::sync::OnceLock;

use serde::Deserialize;

use crate::platform::Theme;

/// A colour as red, green and blue floats from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub f32, pub f32, pub f32);

impl Rgb {
    /// A colour from 0xRRGGBB.
    const fn hex(v: u32) -> Rgb {
        Rgb(((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0)
    }

    /// Parses "#rrggbb". Return: None for any other text.
    fn parse(text: &str) -> Option<Rgb> {
        let digits = text.strip_prefix('#').filter(|d| d.len() == 6)?;
        u32::from_str_radix(digits, 16).ok().map(Rgb::hex)
    }

    /// `t` = 0 is `self`, 1 is `other`.
    pub fn mix(self, other: Rgb, t: f32) -> Rgb {
        let k = t.clamp(0.0, 1.0);
        Rgb(self.0 + (other.0 - self.0) * k, self.1 + (other.1 - self.1) * k, self.2 + (other.2 - self.2) * k)
    }

    /// As 0..255 bytes.
    pub fn bytes(self) -> (u8, u8, u8) {
        let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
        (byte(self.0), byte(self.1), byte(self.2))
    }

    /// As a GDI COLORREF (0x00BBGGRR).
    pub fn colorref(self) -> u32 {
        let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u32;
        byte(self.0) | byte(self.1) << 8 | byte(self.2) << 16
    }
}

/// Every role of one family and mode. `card` is the surface, `ink` the text, `muted` secondary text and idle bars, `accent` live bars and frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub bg: Rgb,
    pub sidebar: Rgb,
    pub card: Rgb,
    pub control: Rgb,
    pub ink: Rgb,
    pub muted: Rgb,
    pub accent: Rgb,
    pub danger: Rgb,
}

/// How strongly ink is mixed over the card for the derived hover, line and strong-line colours, as in `styles.css`.
#[derive(Deserialize)]
struct Derive {
    hover: f32,
    line: f32,
    #[serde(rename = "lineStrong")]
    line_strong: f32,
}

/// One theme role (light or dark) of `themes.json`: its colours as "#rrggbb" text.
#[derive(Deserialize)]
struct Role {
    bg: String,
    sidebar: String,
    card: String,
    control: String,
    ink: String,
    muted: String,
    accent: String,
    danger: String,
}

/// One theme family: its id, its display name and its light and dark roles.
#[derive(Deserialize)]
struct Family {
    id: String,
    dark: Role,
    light: Role,
}

/// The whole `themes.json`: the families and the derived-colour strengths.
#[derive(Deserialize)]
struct Themes {
    derive: Derive,
    families: Vec<Family>,
}

/// The embedded theme table, parsed once. A parse failure is a build defect (a test checks the file). The file is embedded and checked by the tests below, so a parse failure is a build defect.
fn themes() -> &'static Themes {
    static THEMES: OnceLock<Themes> = OnceLock::new();
    THEMES.get_or_init(|| serde_json::from_str(include_str!("../../../../src/themes.json")).expect("src/themes.json parses"))
}

impl Role {
    /// Turns one theme role of `themes.json` into colours.
    fn palette(&self) -> Palette {
        let c = |t: &str| Rgb::parse(t).expect("theme colours are #rrggbb");
        Palette {
            bg: c(&self.bg),
            sidebar: c(&self.sidebar),
            card: c(&self.card),
            control: c(&self.control),
            ink: c(&self.ink),
            muted: c(&self.muted),
            accent: c(&self.accent),
            danger: c(&self.danger),
        }
    }
}

impl Palette {
    /// Ink over the card at the strengths of `styles.css`: `--hover` (a highlighted row), `--line` (a divider), `--line-strong` (an outline).
    pub fn hover(&self) -> Rgb {
        self.card.mix(self.ink, themes().derive.hover)
    }

    /// The divider colour: ink over the card at the `--line` strength.
    pub fn line(&self) -> Rgb {
        self.card.mix(self.ink, themes().derive.line)
    }

    /// The outline colour: ink over the card at the `--line-strong` strength.
    pub fn border(&self) -> Rgb {
        self.card.mix(self.ink, themes().derive.line_strong)
    }
}

/// System mode follows Windows (`system_dark`); an unknown family falls back to the first one, `default`. An earlier version saved the display name ("Tokyo Night"); it names the same family as the id.
pub fn resolve(theme: &Theme, system_dark: bool) -> Palette {
    let dark = match theme.mode.as_str() {
        "Light" => false,
        "Dark" => true,
        _ => system_dark,
    };
    let families = &themes().families;
    let wanted = theme.family.to_lowercase().replace(' ', "-");
    let family = families.iter().find(|f| f.id == wanted).unwrap_or(&families[0]);
    (if dark { &family.dark } else { &family.light }).palette()
}
