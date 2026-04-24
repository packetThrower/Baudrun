//! iTerm2 `.itermcolors` parser. Input is an Apple XML plist with
//! `Red Component`, `Green Component`, `Blue Component`, `Alpha
//! Component` fields on each color. Component values are normalized
//! [0.0, 1.0] floats; we clamp-round to 8-bit `#rrggbb`.

use serde::Deserialize;

use super::{slugify, Theme};

#[derive(Debug, Deserialize, Default)]
struct ItermColor {
    #[serde(rename = "Red Component", default)]
    red: f64,
    #[serde(rename = "Green Component", default)]
    green: f64,
    #[serde(rename = "Blue Component", default)]
    blue: f64,
    #[serde(rename = "Alpha Component", default)]
    alpha: f64,
}

impl ItermColor {
    fn hex(&self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            clamp_255(self.red),
            clamp_255(self.green),
            clamp_255(self.blue),
        )
    }

    /// Distinguishes "explicit black" (all components 0, alpha 0)
    /// from "color set by the user" — iTerm writes all four
    /// components even for defaults, so checking for any non-zero
    /// field is a decent heuristic.
    fn is_set(&self) -> bool {
        self.alpha > 0.0 || self.red > 0.0 || self.green > 0.0 || self.blue > 0.0
    }
}

fn clamp_255(v: f64) -> u8 {
    let scaled = (v * 255.0 + 0.5) as i32;
    scaled.clamp(0, 255) as u8
}

#[derive(Debug, Deserialize, Default)]
struct ItermTheme {
    #[serde(rename = "Ansi 0 Color", default)]
    ansi0: ItermColor,
    #[serde(rename = "Ansi 1 Color", default)]
    ansi1: ItermColor,
    #[serde(rename = "Ansi 2 Color", default)]
    ansi2: ItermColor,
    #[serde(rename = "Ansi 3 Color", default)]
    ansi3: ItermColor,
    #[serde(rename = "Ansi 4 Color", default)]
    ansi4: ItermColor,
    #[serde(rename = "Ansi 5 Color", default)]
    ansi5: ItermColor,
    #[serde(rename = "Ansi 6 Color", default)]
    ansi6: ItermColor,
    #[serde(rename = "Ansi 7 Color", default)]
    ansi7: ItermColor,
    #[serde(rename = "Ansi 8 Color", default)]
    ansi8: ItermColor,
    #[serde(rename = "Ansi 9 Color", default)]
    ansi9: ItermColor,
    #[serde(rename = "Ansi 10 Color", default)]
    ansi10: ItermColor,
    #[serde(rename = "Ansi 11 Color", default)]
    ansi11: ItermColor,
    #[serde(rename = "Ansi 12 Color", default)]
    ansi12: ItermColor,
    #[serde(rename = "Ansi 13 Color", default)]
    ansi13: ItermColor,
    #[serde(rename = "Ansi 14 Color", default)]
    ansi14: ItermColor,
    #[serde(rename = "Ansi 15 Color", default)]
    ansi15: ItermColor,

    #[serde(rename = "Background Color", default)]
    background: ItermColor,
    #[serde(rename = "Foreground Color", default)]
    foreground: ItermColor,
    #[serde(rename = "Cursor Color", default)]
    cursor: ItermColor,
    #[serde(rename = "Cursor Text Color", default)]
    cursor_text: ItermColor,
    #[serde(rename = "Selection Color", default)]
    selection: ItermColor,
    #[serde(rename = "Selected Text Color", default)]
    selected_text: ItermColor,
}

/// Parse an `.itermcolors` XML plist into a [`Theme`]. `name` is
/// used as the display name and slug source for the id. Returns a
/// human-readable message on decode failure.
pub fn parse_iterm_colors(data: &[u8], name: &str) -> Result<Theme, String> {
    let parsed: ItermTheme =
        plist::from_bytes(data).map_err(|err| format!("decode plist: {}", err))?;

    let mut theme = Theme {
        id: slugify(name, "theme"),
        name: name.to_string(),
        source: "user".into(),

        background: parsed.background.hex(),
        foreground: parsed.foreground.hex(),
        cursor: parsed.cursor.hex(),
        cursor_accent: String::new(),
        selection: parsed.selection.hex(),
        selection_foreground: String::new(),

        black: parsed.ansi0.hex(),
        red: parsed.ansi1.hex(),
        green: parsed.ansi2.hex(),
        yellow: parsed.ansi3.hex(),
        blue: parsed.ansi4.hex(),
        magenta: parsed.ansi5.hex(),
        cyan: parsed.ansi6.hex(),
        white: parsed.ansi7.hex(),
        bright_black: parsed.ansi8.hex(),
        bright_red: parsed.ansi9.hex(),
        bright_green: parsed.ansi10.hex(),
        bright_yellow: parsed.ansi11.hex(),
        bright_blue: parsed.ansi12.hex(),
        bright_magenta: parsed.ansi13.hex(),
        bright_cyan: parsed.ansi14.hex(),
        bright_white: parsed.ansi15.hex(),
    };

    if parsed.cursor_text.is_set() {
        theme.cursor_accent = parsed.cursor_text.hex();
    }
    if parsed.selected_text.is_set() {
        theme.selection_foreground = parsed.selected_text.hex();
    }

    Ok(theme)
}
