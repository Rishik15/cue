//! Purpose: The native windows' colours (recording pill, notices, tray menu, dictionary card, Grab frame), read from `src/themes.json`, the same file the settings window
//! uses, so every surface matches in every family and mode. Derived colours use the file's ink percentages, the same maths as `src/styles.css`.
//! Contents: Rgb — a colour as 0..1 floats; Palette — every role plus derived hover, line and border; resolve — pick the family's dark or light variant (unknown family -> default).

use std::sync::OnceLock;

use serde::Deserialize;

use crate::platform::Theme;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub f32, pub f32, pub f32);

impl Rgb {
    const fn hex(v: u32) -> Rgb {
        Rgb(((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0)
    }

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

#[derive(Deserialize)]
struct Derive {
    hover: f32,
    line: f32,
    #[serde(rename = "lineStrong")]
    line_strong: f32,
}

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

#[derive(Deserialize)]
struct Family {
    id: String,
    dark: Role,
    light: Role,
}

#[derive(Deserialize)]
struct Themes {
    derive: Derive,
    families: Vec<Family>,
}

fn themes() -> &'static Themes {
    static THEMES: OnceLock<Themes> = OnceLock::new();
    // The file is embedded and checked by the tests below, so a parse failure is a build defect.
    THEMES.get_or_init(|| serde_json::from_str(include_str!("../../../../src/themes.json")).expect("src/themes.json parses"))
}

impl Role {
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

    pub fn line(&self) -> Rgb {
        self.card.mix(self.ink, themes().derive.line)
    }

    pub fn border(&self) -> Rgb {
        self.card.mix(self.ink, themes().derive.line_strong)
    }
}

/// System mode follows Windows (`system_dark`); an unknown family falls back to the first one, `default`.
pub fn resolve(theme: &Theme, system_dark: bool) -> Palette {
    let dark = match theme.mode.as_str() {
        "Light" => false,
        "Dark" => true,
        _ => system_dark,
    };
    let families = &themes().families;
    // An earlier version saved the display name ("Tokyo Night"); it names the same family as the id.
    let wanted = theme.family.to_lowercase().replace(' ', "-");
    let family = families.iter().find(|f| f.id == wanted).unwrap_or(&families[0]);
    (if dark { &family.dark } else { &family.light }).palette()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme(family: &str, mode: &str) -> Theme {
        Theme { family: family.into(), mode: mode.into() }
    }

    #[test]
    fn a_name_saved_by_an_earlier_version_finds_its_family() {
        assert_eq!(resolve(&theme("Tokyo Night", "Dark"), false), resolve(&theme("tokyo-night", "Dark"), false));
    }

    #[test]
    fn every_family_and_mode_resolves_with_every_role() {
        // resolve() panics on a role that is not #rrggbb, so this parses all 8 roles of both modes
        for f in &themes().families {
            for mode in ["Dark", "Light"] {
                resolve(&theme(&f.id, mode), false);
            }
        }
        assert_eq!(themes().families[0].id, "default");
    }

    #[test]
    fn picks_the_variant_for_the_mode() {
        let (dark, light) = (resolve(&theme("github", "Dark"), false), resolve(&theme("github", "Light"), true));
        assert_eq!(dark.card, Rgb::parse("#161b22").unwrap());
        assert_eq!(light.card, Rgb::parse("#ffffff").unwrap());
        assert_eq!(resolve(&theme("github", "System"), true), dark); // follows Windows
    }

    #[test]
    fn unknown_family_falls_back_to_default() {
        assert_eq!(resolve(&theme("rose-pine", "Dark"), false), resolve(&theme("default", "Dark"), false));
        assert_eq!(resolve(&theme("", "Light"), false), resolve(&theme("default", "Light"), false));
    }

    #[test]
    fn derives_by_mixing_ink_into_the_card() {
        // default light: card #ffffff, ink #1d1d1f; line = 8 % ink -> red 255 + (29 - 255) * 0.08 = 236.92 -> 237
        let p = resolve(&theme("default", "Light"), false);
        assert_eq!(p.line().bytes().0, 237);
        assert_eq!(p.border().bytes().0, 210); // 255 - 226 * 0.20 = 209.8
    }

    #[test]
    fn mixes_and_converts() {
        assert_eq!(Rgb(0.0, 0.0, 0.0).mix(Rgb(1.0, 1.0, 1.0), 0.5), Rgb(0.5, 0.5, 0.5));
        assert_eq!(Rgb(1.0, 0.0, 0.0).colorref(), 0x0000FF); // GDI wants blue in the high byte
    }
}
