//! Resolved chrome colours for the active skin. Replaces the
//! per-file hardcoded constants `app_view.rs` and `settings_view.rs`
//! used to carry — both files now read from the same `SkinTokens`
//! installed as a gpui `Global`.
//!
//! Resolution path: `data::skins::Store` keeps the parsed `Skin`
//! JSON; `SkinTokens::from_skin(&skin, dark)` walks the skin's
//! `vars` map (with light/dark overlays) and parses each colour
//! string into a packed `0xRRGGBBAA` `u32` suitable for
//! `gpui::rgba(...)`. Slots that fail to parse fall back per-field
//! to `SkinTokens::baudrun_default()` so a partially-bad skin
//! still renders most of the chrome instead of nothing.

use gpui::{Global, SharedString};

use crate::data::skins::Skin;

/// Per-skin font + font-size hints. Lifted out of `SkinTokens` so
/// the latter can stay `Copy` (chrome render code reads it on
/// every paint and passes it by value to helpers). Fonts are only
/// consumed by `AppView::apply_skin` to push into the gpui-
/// component `Theme` — chrome render never touches them, so
/// keeping them on a separate non-Copy bag avoids the SharedString
/// from poisoning the Copy bound.
#[derive(Debug, Clone)]
pub struct SkinFonts {
    /// First UI font family from the skin's `--font-ui` stack.
    /// Empty string means "use the platform default" (gpui-
    /// component's `.SystemUIFont`).
    pub font_ui: SharedString,
    pub font_mono: SharedString,
    /// Base UI font size in pixels. Drives widget text via the
    /// gpui-component Theme.
    pub font_size_base: f32,
}

impl SkinFonts {
    pub fn defaults() -> Self {
        Self {
            font_ui: SharedString::default(),
            font_mono: SharedString::default(),
            font_size_base: 13.0,
        }
    }

    pub fn from_skin(skin: &Skin, dark: bool) -> Self {
        let raw_var = |key: &str| -> Option<&String> {
            if dark {
                skin.dark_vars.get(key).or_else(|| skin.vars.get(key))
            } else {
                skin.light_vars.get(key).or_else(|| skin.vars.get(key))
            }
        };
        Self {
            font_ui: raw_var("--font-ui")
                .map(|s| first_font_family(s))
                .unwrap_or_default(),
            font_mono: raw_var("--font-mono")
                .map(|s| first_font_family(s))
                .unwrap_or_default(),
            font_size_base: raw_var("--font-size-base")
                .and_then(|s| parse_px(s))
                .unwrap_or(13.0),
        }
    }
}

/// Packed-RGBA chrome tokens. Layout matches `gpui::rgba`'s
/// expected format (MSB → R, then G, B, A in the LSB) so callers
/// can always do `rgba(tokens.fg_secondary)` without conversion.
/// Fully-opaque colours have alpha `0xFF`; translucent skin vars
/// keep their declared alpha.
#[derive(Debug, Clone, Copy)]
pub struct SkinTokens {
    /// Outermost shell background — painted on the outermost
    /// AppView / SettingsView div so the translucent overlays
    /// (`bg_main`, `bg_panel`, `bg_sidebar`) have an opaque base
    /// to composite against. Without this, skins like macOS-26
    /// whose `--bg-main` is `rgba(255,255,255,0.7)` end up looking
    /// dark when the underlying transparent window shows the OS
    /// desktop instead of the frosted-glass effect Tauri produces.
    /// Resolves from the skin's `--bg-window`, falling back to
    /// `--shell-bg` (gradients are flattened to their first solid
    /// stop), then to a hardcoded opaque value.
    pub bg_window: u32,
    /// Translucent main pane bg. Layered ON TOP of `bg_window`.
    pub bg_main: u32,
    pub bg_sidebar: u32,
    pub bg_panel: u32,
    pub bg_input: u32,
    /// Slightly brighter input background for hover / focus
    /// states. `--bg-input-focus` in the skin vars; falls back to
    /// `bg_input` (no visible hover effect) when absent.
    pub bg_input_hover: u32,
    /// Hover / selected-row background — `--bg-hover` in the skin
    /// vars; doubles as the sidebar's "selected profile row" tint.
    pub bg_hover: u32,
    /// Translucent accent fill behind the active editor tab and
    /// Settings rail row. `--bg-active` in the skin vars.
    pub bg_active: u32,

    pub fg_primary: u32,
    pub fg_secondary: u32,
    pub fg_tertiary: u32,

    pub border_subtle: u32,
    pub border_strong: u32,

    pub accent: u32,
    /// Foreground colour rendered on top of `accent` (e.g. text on
    /// the primary Connect button). No skin var — defaults to
    /// pure white since every shipped skin uses a saturated accent
    /// where white reads cleanest.
    pub accent_fg: u32,
    pub danger: u32,
    pub success: u32,
    pub warn: u32,

    /// Inline error text under a sidebar profile row + the failed-
    /// connect status dot. No direct skin var; falls back to
    /// `danger` in `from_skin` when the skin doesn't declare
    /// `--sidebar-error`.
    pub sidebar_error: u32,

    /// Dropdown / popover background. Skins ship `--option-bg` as
    /// an explicitly opaque colour (the popover floats free over
    /// the window so a translucent value would let whatever's
    /// behind the window bleed through). Drives gpui-component's
    /// `Theme.popover` so Select menus stay readable.
    pub option_bg: u32,
    /// Dropdown / popover text colour. Pairs with `option_bg`.
    pub option_fg: u32,

    // -- Sizing tokens -------------------------------------------
    // Component-shape vars from the skin. `--radius-*` drives card
    // corners + button/input rounding; `--font-size-base` carries
    // through to widget text. macOS-26 Liquid Glass uses much
    // larger radii than Baudrun, which is the visible difference
    // between the two skins beyond colour.
    pub radius_lg: f32,
    pub radius_md: f32,
    pub radius_sm: f32,
}

impl Global for SkinTokens {}

impl SkinTokens {
    /// Hardcoded values matching the previously-inline constants
    /// in `app_view.rs` / `settings_view.rs` — keeps the chrome
    /// looking the same when no skin is loaded (boot-time
    /// fallback) or when individual slots can't parse.
    pub const fn baudrun_default() -> Self {
        Self {
            bg_window: 0x18181AFF,
            // Opaque base for the window. Translucent overlays
            // (panel / sidebar) composite onto this.
            bg_main: 0x18181AFF,
            bg_sidebar: 0x262627FF,
            bg_panel: 0xFFFFFF0F,
            bg_input: 0xFFFFFF14,
            bg_input_hover: 0xFFFFFF1F,
            bg_hover: 0x2D3548FF,
            bg_active: 0x007AFF40,

            fg_primary: 0xFFFFFFF2,
            fg_secondary: 0xFFFFFFA6,
            fg_tertiary: 0xFFFFFF66,

            border_subtle: 0xFFFFFF14,
            border_strong: 0xFFFFFF24,

            accent: 0x0A84FFFF,
            accent_fg: 0xFFFFFFFF,
            danger: 0xFF453AFF,
            success: 0x32D74BFF,
            warn: 0xF5D76EFF,

            sidebar_error: 0xFF7A8AFF,

            // Opaque dark dropdown bg + light text — same values
            // the Baudrun skin's `--option-bg` / `--option-fg`
            // declare for dark mode.
            option_bg: 0x1E1E22FF,
            option_fg: 0xE4E4E7FF,

            radius_lg: 10.0,
            radius_md: 6.0,
            radius_sm: 4.0,
        }
    }

    /// Resolve a skin into chrome tokens. `dark` selects which
    /// overlay (`darkVars` vs `lightVars`) to apply on top of the
    /// base `vars`; `false` here means light mode. Skins flagged
    /// `supportsLight = false` should be passed `true` regardless
    /// — the caller (AppView::apply_settings) handles that gate.
    pub fn from_skin(skin: &Skin, dark: bool) -> Self {
        let fb = Self::baudrun_default();
        let raw_var = |key: &str| -> Option<&String> {
            // Overlay layer wins over base. Same precedence the
            // Tauri `applySkin` uses in src/lib/skinApplier.ts.
            if dark {
                skin.dark_vars.get(key).or_else(|| skin.vars.get(key))
            } else {
                skin.light_vars.get(key).or_else(|| skin.vars.get(key))
            }
        };
        let pick = |key: &str, default: u32| {
            raw_var(key).and_then(|s| parse_color(s)).unwrap_or(default)
        };
        let pick_px = |key: &str, default: f32| {
            raw_var(key).and_then(|s| parse_px(s)).unwrap_or(default)
        };
        Self {
            // `--bg-window` first (most skins ship a solid here),
            // then fall back to `--shell-bg` (macOS-26 + Baudrun
            // ship a gradient there — flattened to its first stop
            // by `parse_shell_color`). Final fallback is the
            // baudrun_default opaque dark.
            bg_window: raw_var("--bg-window")
                .and_then(|s| parse_color(s))
                .or_else(|| {
                    raw_var("--shell-bg").and_then(|s| parse_shell_color(s))
                })
                .unwrap_or(fb.bg_window),
            bg_main: pick("--bg-main", fb.bg_main),
            bg_sidebar: pick("--bg-sidebar", fb.bg_sidebar),
            bg_panel: pick("--bg-panel", fb.bg_panel),
            bg_input: pick("--bg-input", fb.bg_input),
            bg_input_hover: pick("--bg-input-focus", fb.bg_input_hover),
            bg_hover: pick("--bg-hover", fb.bg_hover),
            bg_active: pick("--bg-active", fb.bg_active),
            fg_primary: pick("--fg-primary", fb.fg_primary),
            fg_secondary: pick("--fg-secondary", fb.fg_secondary),
            fg_tertiary: pick("--fg-tertiary", fb.fg_tertiary),
            border_subtle: pick("--border-subtle", fb.border_subtle),
            border_strong: pick("--border-strong", fb.border_strong),
            accent: pick("--accent", fb.accent),
            // Accent foreground isn't declared as a skin var, so
            // derive it from the accent's luminance: dark accents
            // (Baudrun blue, macOS-26 blue) get white-on-accent
            // text; bright/light accents (CRT phosphor green,
            // Synthwave magenta) get black-on-accent so the
            // hovered dropdown row stays readable.
            accent_fg: contrast_fg_for(pick("--accent", fb.accent)),
            danger: pick("--danger", fb.danger),
            success: pick("--success", fb.success),
            warn: pick("--warn", fb.warn),
            // Skins don't declare a sidebar-error slot today —
            // when one does, the lookup picks it up automatically.
            // Falls back through the same chain `--danger` uses
            // so the row-error tint stays in family with the rest
            // of the destructive palette.
            sidebar_error: pick("--sidebar-error", pick("--danger", fb.sidebar_error)),
            option_bg: pick("--option-bg", fb.option_bg),
            option_fg: pick("--option-fg", fb.option_fg),
            radius_lg: pick_px("--radius-lg", fb.radius_lg),
            radius_md: pick_px("--radius-md", fb.radius_md),
            radius_sm: pick_px("--radius-sm", fb.radius_sm),
        }
    }
}

/// Parse a CSS-style colour string into packed `0xRRGGBBAA`. Handles
/// `#rrggbb`, `#rrggbbaa`, `rgb(r, g, b)`, and `rgba(r, g, b, a)`
/// — the four forms the bundled skins actually use. Any other form
/// (named colours, `hsl()`, `var()`, …) returns `None` so the
/// caller falls back to its baseline value.
fn parse_color(raw: &str) -> Option<u32> {
    let s = raw.trim();
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(inner) = s.strip_prefix("rgba(").and_then(|t| t.strip_suffix(')')) {
        return parse_rgba_args(inner);
    }
    if let Some(inner) = s.strip_prefix("rgb(").and_then(|t| t.strip_suffix(')')) {
        return parse_rgb_args(inner);
    }
    None
}

fn parse_hex(s: &str) -> Option<u32> {
    match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            Some(pack(r, g, b, 0xFF))
        }
        8 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            let a = u8::from_str_radix(&s[6..8], 16).ok()?;
            Some(pack(r, g, b, a))
        }
        _ => None,
    }
}

fn parse_rgba_args(inner: &str) -> Option<u32> {
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if parts.len() != 4 {
        return None;
    }
    let r: u8 = parts[0].parse().ok()?;
    let g: u8 = parts[1].parse().ok()?;
    let b: u8 = parts[2].parse().ok()?;
    let af: f32 = parts[3].parse().ok()?;
    let a = (af.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(pack(r, g, b, a))
}

/// Parse `--shell-bg` values that may carry a CSS `linear-gradient`
/// wrapper. Extracts the first colour stop and returns it; for
/// non-gradient values (`#1d1d1e` for the Baudrun shell) falls
/// back to the regular `parse_color`. gpui can't render multi-stop
/// gradients on a div bg, so the visual loses the gradient
/// shading — the first stop is a close-enough opaque base for the
/// translucent panels above to composite against.
fn parse_shell_color(raw: &str) -> Option<u32> {
    let s = raw.trim();
    let inner = s
        .strip_prefix("linear-gradient(")
        .and_then(|t| t.strip_suffix(')'));
    if let Some(args) = inner {
        // First arg is the angle / direction; subsequent args are
        // colour stops like "#1d1d1d 0%". Find the first stop and
        // strip the trailing percentage if present.
        for part in args.split(',').skip(1) {
            let stop = part.trim();
            // Stop syntax: `<color> <position?>`. Take everything
            // before the first space as the colour.
            let color = stop.split_whitespace().next().unwrap_or(stop);
            if let Some(parsed) = parse_color(color) {
                return Some(parsed);
            }
        }
        return None;
    }
    parse_color(s)
}

fn parse_rgb_args(inner: &str) -> Option<u32> {
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if parts.len() != 3 {
        return None;
    }
    let r: u8 = parts[0].parse().ok()?;
    let g: u8 = parts[1].parse().ok()?;
    let b: u8 = parts[2].parse().ok()?;
    Some(pack(r, g, b, 0xFF))
}

const fn pack(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32)
}

/// Pick black or white text for a given background colour by its
/// perceptual luminance. Uses the WCAG-style coefficients
/// (0.299R + 0.587G + 0.114B). Threshold is 0.55 — slightly past
/// the midpoint to bias toward white text, since most accents in
/// the bundled skins are deeply saturated dark blues + already
/// look right with white. The handful of bright-phosphor / pastel
/// accents (CRT, Synthwave magenta) cross the threshold and flip
/// to black.
fn contrast_fg_for(packed: u32) -> u32 {
    let r = ((packed >> 24) & 0xFF) as f32 / 255.0;
    let g = ((packed >> 16) & 0xFF) as f32 / 255.0;
    let b = ((packed >> 8) & 0xFF) as f32 / 255.0;
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    if luma > 0.55 { 0x000000FF } else { 0xFFFFFFFF }
}

/// Parse a CSS pixel value (`"10px"`, `"24px"`, `"0"`) into a `f32`.
/// Bare numbers are accepted as already-pixels. Returns `None` for
/// anything else (`"calc(...)"`, `"var(...)"`, percentages, …) so
/// the caller falls back to the baseline.
fn parse_px(raw: &str) -> Option<f32> {
    let s = raw.trim();
    if let Some(num) = s.strip_suffix("px") {
        return num.trim().parse().ok();
    }
    s.parse().ok()
}

/// Return the first font family from a CSS font stack, with quotes
/// stripped. Skips `system-ui` / `-apple-system` and similar
/// keywords because gpui doesn't resolve those — the caller will
/// fall back to the platform default when this returns an empty
/// string. Quoted families (`"SF Pro Text"`) keep their internal
/// spaces.
fn first_font_family(stack: &str) -> SharedString {
    for raw in stack.split(',') {
        let mut t = raw.trim();
        // Strip surrounding quotes if present (CSS allows `"..."`
        // and `'...'`).
        if (t.starts_with('"') && t.ends_with('"'))
            || (t.starts_with('\'') && t.ends_with('\''))
        {
            t = &t[1..t.len() - 1];
        }
        if t.is_empty() {
            continue;
        }
        // CSS keyword fallbacks gpui can't resolve. Skip and let
        // the platform default take over.
        if matches!(
            t,
            "system-ui"
                | "-apple-system"
                | "BlinkMacSystemFont"
                | "ui-monospace"
                | "ui-sans-serif"
                | "ui-serif"
                | "monospace"
                | "sans-serif"
                | "serif"
        ) {
            continue;
        }
        return SharedString::from(t.to_string());
    }
    SharedString::default()
}
