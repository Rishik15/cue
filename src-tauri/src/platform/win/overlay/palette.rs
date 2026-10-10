//! Purpose: The overlay's colours, taken from the user's Cue theme so the pill matches the settings window in every theme and mode.
//! Contents: Rgb / Palette — colours as 0..1 floats; resolve — pick the dark or light variant of the saved theme; the Cue theme is the
//! fallback when nothing is saved yet. The settings window saves the four roles it needs as JSON (`theme_overlay`, see `src/theme.ts`).

use serde_json::Value;

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

    /// As a GDI COLORREF (0x00BBGGRR).
    pub fn colorref(self) -> u32 {
        let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u32;
        byte(self.0) | byte(self.1) << 8 | byte(self.2) << 16
    }
}

/// `card` is the surface, `ink` the text, `muted` secondary text and idle bars, `accent` live bars.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub card: Rgb,
    pub ink: Rgb,
    pub muted: Rgb,
    pub accent: Rgb,
}

const CUE_DARK: Palette =
    Palette { card: Rgb::hex(0x232327), ink: Rgb::hex(0xededef), muted: Rgb::hex(0x8e8e96), accent: Rgb::hex(0x6f8bff) };
const CUE_LIGHT: Palette =
    Palette { card: Rgb::hex(0xffffff), ink: Rgb::hex(0x1a1a1d), muted: Rgb::hex(0x62626c), accent: Rgb::hex(0x4c6ef5) };

fn variant(colors: &Value, name: &str) -> Option<Palette> {
    let role = |key: &str| colors.get(name)?.get(key)?.as_str().and_then(Rgb::parse);
    Some(Palette { card: role("card")?, ink: role("ink")?, muted: role("muted")?, accent: role("accent")? })
}

/// System mode follows Windows (`system_dark`); a missing or damaged saved theme falls back to the Cue theme.
pub fn resolve(theme: &Theme, system_dark: bool) -> Palette {
    let dark = match theme.mode.as_str() {
        "Light" => false,
        "Dark" => true,
        _ => system_dark,
    };
    let saved = serde_json::from_str::<Value>(&theme.colors).ok();
    saved.and_then(|c| variant(&c, if dark { "dark" } else { "light" })).unwrap_or(if dark { CUE_DARK } else { CUE_LIGHT })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme(mode: &str, colors: &str) -> Theme {
        Theme { mode: mode.into(), colors: colors.into() }
    }

    const SAVED: &str = r##"{"dark":{"card":"#101010","ink":"#ffffff","muted":"#808080","accent":"#ff0000"},
                           "light":{"card":"#f0f0f0","ink":"#000000","muted":"#707070","accent":"#0000ff"}}"##;

    #[test]
    fn picks_the_variant_for_the_mode() {
        assert_eq!(resolve(&theme("Dark", SAVED), false).accent, Rgb(1.0, 0.0, 0.0));
        assert_eq!(resolve(&theme("Light", SAVED), true).accent, Rgb(0.0, 0.0, 1.0));
        assert_eq!(resolve(&theme("System", SAVED), true).accent, Rgb(1.0, 0.0, 0.0));
        // follows Windows
    }

    #[test]
    fn falls_back_to_the_cue_theme() {
        assert_eq!(resolve(&theme("System", ""), true), CUE_DARK);
        assert_eq!(resolve(&theme("Light", "not json"), true), CUE_LIGHT);
        assert_eq!(resolve(&theme("Dark", r#"{"dark":{"card":"red"}}"#), false), CUE_DARK);
        // incomplete or malformed colours
    }

    #[test]
    fn mixes_and_converts() {
        assert_eq!(Rgb(0.0, 0.0, 0.0).mix(Rgb(1.0, 1.0, 1.0), 0.5), Rgb(0.5, 0.5, 0.5));
        assert_eq!(Rgb(1.0, 0.0, 0.0).colorref(), 0x0000FF); // GDI wants blue in the high byte
    }
}
