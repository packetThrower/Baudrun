//! Theme rendering helpers split out of `settings_view/mod.rs`.
//! Two entry points that mod.rs renders:
//!
//!   * `theme_swatches(&Theme)` — compact 8-color strip used in
//!     the installed-themes list.
//!   * `theme_preview_block(&Theme)` — full-fidelity preview
//!     dialog body, with sample lines + paired fg/bg blocks.
//!
//! Module-private helpers:
//!
//!   * `preview_sample_lines()` — the canned sample-output rows
//!     the preview block renders against each theme's palette.
//!   * `parse_theme_color(&str)` — `#RRGGBB` / `#RGB` / `#RRGGBBAA`
//!     → `0xRRGGBBAA` u32 used by gpui's `rgba`.

use gpui::{div, prelude::*, px, rgba, IntoElement, SharedString};

use crate::data::themes;

/// Compact swatch strip — 8 normal-ANSI colors as small rounded tiles
/// fused into a single horizontal pill. Sits at the left of each
/// installed-themes row so the user can compare palettes at a glance
/// without opening the full preview dialog.
pub(super) fn theme_swatches(theme: &themes::Theme) -> impl IntoElement {
    let palette = [
        &theme.black,
        &theme.red,
        &theme.green,
        &theme.yellow,
        &theme.blue,
        &theme.magenta,
        &theme.cyan,
        &theme.white,
    ];
    div()
        .flex()
        .flex_row()
        .overflow_hidden()
        .rounded(px(6.0))
        .children(palette.into_iter().map(|hex| {
            let color = parse_theme_color(hex).unwrap_or(0x808080FFu32);
            div().w(px(16.0)).h(px(28.0)).bg(rgba(color))
        }))
}

/// Sample lines for the theme-preview dialog. Each entry is one line
/// rendered as a horizontal flex of (palette-slot, text) spans. Slots
/// resolve against the theme being previewed so the snippet shows
/// every major colour the palette declares — `magenta` for keywords,
/// `green`/`red`/`yellow` for status, `cyan` for identifiers, plain
/// `fg` for prose.
fn preview_sample_lines() -> Vec<Vec<(&'static str, &'static str)>> {
    vec![
        vec![("fg", "RUGGEDCOM RS900G login: "), ("magenta", "admin")],
        vec![("fg", "Password: "), ("yellow", "********")],
        vec![],
        vec![
            ("fg", "Last login: Fri Apr 17 "),
            ("cyan", "11:47:05"),
            ("fg", " 2026 from "),
            ("cyan", "192.168.1.47"),
        ],
        vec![("fg", "RUGGEDCOM RS900G # show interfaces status")],
        vec![],
        vec![
            ("magenta", "Interface"),
            ("fg", "          Status  VLAN  "),
            ("magenta", "Duplex Speed"),
        ],
        vec![
            ("green", "GigabitEthernet0/1"),
            ("fg", "  "),
            ("green", "up"),
            ("fg", "      1     "),
            ("green", "full"),
            ("fg", "    1000"),
        ],
        vec![
            ("green", "GigabitEthernet0/2"),
            ("fg", "  "),
            ("red", "down"),
            ("fg", "    -     -       -"),
        ],
        vec![
            ("green", "Gi1/0/3"),
            ("fg", "             "),
            ("green", "up"),
            ("fg", "      10    "),
            ("green", "full"),
            ("fg", "    1000"),
        ],
        vec![
            ("green", "FastEthernet0/24"),
            ("fg", "    "),
            ("red", "err-disabled"),
            ("fg", "  -     -       -"),
        ],
        vec![
            ("green", "ge-0/0/1"),
            ("fg", "            "),
            ("green", "up"),
            ("fg", "      -     "),
            ("green", "full"),
            ("fg", "    10000"),
        ],
        vec![],
        vec![
            ("cyan", "11:47:12.120"),
            ("fg", " MAC "),
            ("magenta", "aa:bb:cc:dd:ee:ff"),
            ("fg", " learned on "),
            ("magenta", "Gi1/0/1"),
        ],
        vec![
            ("cyan", "11:47:13.440"),
            ("fg", " "),
            ("yellow", "WARNING"),
            ("fg", ": "),
            ("magenta", "STP"),
            ("fg", " "),
            ("yellow", "learning"),
            ("fg", " on "),
            ("magenta", "Port-channel1"),
        ],
        vec![
            ("cyan", "11:47:15.002"),
            ("fg", " Link "),
            ("red", "DOWN"),
            ("fg", " on "),
            ("magenta", "GigabitEthernet0/2"),
        ],
        vec![
            ("cyan", "11:47:18.881"),
            ("fg", " Route "),
            ("magenta", "192.168.1.47/24"),
            ("fg", " via "),
            ("green", "ge-0/0/1"),
            ("fg", " "),
            ("green", "established"),
        ],
        vec![],
        vec![
            ("cyan", "2026-04-17 11:48:00"),
            ("fg", " Session idle "),
            ("red", "timeout"),
            ("fg", ": "),
            ("cyan", "00:10:00"),
        ],
        vec![("fg", "RUGGEDCOM RS900G #")],
    ]
}

/// Build the body of the theme-preview dialog — a styled block of
/// fake network-gear output painted in the chosen theme's palette.
/// Each line is rendered as a horizontal flex so per-span colouring
/// just works without HTML-style markup.
pub(super) fn theme_preview_block(theme: &themes::Theme) -> impl IntoElement {
    let bg = parse_theme_color(&theme.background).unwrap_or(0x000000FFu32);
    let fg = parse_theme_color(&theme.foreground).unwrap_or(0xE4E4E7FFu32);
    let pick = |slot: &str| -> u32 {
        let raw = match slot {
            "fg" => &theme.foreground,
            "bg" => &theme.background,
            "black" => &theme.black,
            "red" => &theme.red,
            "green" => &theme.green,
            "yellow" => &theme.yellow,
            "blue" => &theme.blue,
            "magenta" => &theme.magenta,
            "cyan" => &theme.cyan,
            "white" => &theme.white,
            _ => &theme.foreground,
        };
        parse_theme_color(raw).unwrap_or(0x808080FFu32)
    };
    let line_views: Vec<gpui::Div> = preview_sample_lines()
        .into_iter()
        .map(|spans| {
            // Empty line → render a thin spacer so blank lines in the
            // sample read as actual blank lines, not collapsed gaps.
            if spans.is_empty() {
                return div().h(px(8.0));
            }
            div()
                .flex()
                .flex_row()
                .children(spans.into_iter().map(|(slot, text)| {
                    div()
                        .text_color(rgba(pick(slot)))
                        .child(SharedString::from(text))
                }))
        })
        .collect();
    // Selection demo — one representative line painted with the
    // theme's `selection` background so the picker shows what a
    // drag- or triple-click selection will actually look like.
    // Honours `selectionForeground` exactly like the live
    // renderer (`term_bridge::mirror_to_grid`): when the theme
    // declares one, selected text takes it; otherwise each span
    // keeps its own palette colour and only the backdrop changes.
    // The row shrink-wraps its text rather than stretching full
    // width — matching the triple-click line-content selection,
    // which stops at the last printed character.
    let selection_bg = parse_theme_color(&theme.selection).unwrap_or(0x4A5A80FFu32);
    let selection_fg = parse_theme_color(&theme.selection_foreground);
    let selected_line = div().flex().flex_row().bg(rgba(selection_bg)).children(
        [
            ("green", "GigabitEthernet0/1"),
            ("fg", "  "),
            ("green", "up"),
            ("fg", "      1     "),
            ("green", "full"),
            ("fg", "    1000"),
        ]
        .into_iter()
        .map(|(slot, text)| {
            let color = selection_fg.unwrap_or_else(|| pick(slot));
            div()
                .text_color(rgba(color))
                .child(SharedString::from(text))
        }),
    );
    div()
        .px_4()
        .py_3()
        .rounded_md()
        .bg(rgba(bg))
        .text_color(rgba(fg))
        .text_size(px(13.0))
        .font_family("Menlo")
        .flex()
        .flex_col()
        .children(line_views)
        // A blank gap, then the selected line — set apart from the
        // sample above so it reads as a deliberate "this is the
        // selection" swatch rather than another output line.
        .child(div().h(px(12.0)))
        .child(selected_line)
}

/// Compact `#rrggbb` parser specialised for the theme JSON shape.
/// Themes never carry alpha, so we always pack `0xFF` and let the
/// caller treat it as `0xRRGGBBAA`. Returns `None` on any malformed
/// input — the caller falls back to a neutral grey so the preview
/// doesn't disappear on a single bad slot.
fn parse_theme_color(s: &str) -> Option<u32> {
    let hex = s.trim().strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | 0xFF)
}
