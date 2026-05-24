//! Generic pill/button styled helpers used by the session header
//! and the profile form. All three return a bare `gpui::Div` so the
//! call site can attach its own `.on_mouse_up` / `.tooltip` etc. —
//! these helpers just own the visual styling.

use gpui::{div, prelude::*, px, rgba};

use super::STATUS_DOT_PX;
use crate::skin_tokens::SkinTokens;

/// Toggle pill for the session header's mid-session DTR / RTS
/// indicators. Visually a `pill_button` with a small status dot in
/// front of the label — green (`--success`, same colour as the
/// session-header connection dot at the left of the row) when the
/// line is asserted, muted (`--fg-tertiary`) when it's deasserted.
/// The pill background itself stays the same muted shape in both
/// states so the eye reads the dot as the state, not the pill fill.
/// Tooltip + click handler live on the wrapping div in
/// `session_header` rather than here so the helper stays a pure
/// styled child.
pub(super) fn line_pill(s: SkinTokens, label: &'static str, active: bool) -> gpui::Div {
    let dot_color = if active { s.success } else { s.fg_tertiary };
    let hover_bg = s.bg_input_hover;
    div()
        .px_3()
        .py_1()
        .bg(rgba(s.bg_input))
        .text_color(rgba(s.fg_primary))
        .text_size(px(13.0))
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .rounded_md()
        .cursor_pointer()
        .hover(move |st| st.bg(rgba(hover_bg)))
        .child(
            div()
                .w(px(STATUS_DOT_PX))
                .h(px(STATUS_DOT_PX))
                .rounded_full()
                .bg(rgba(dot_color)),
        )
        .child(label)
}

/// Pill button styled per the Baudrun skin. Neutral translucent
/// fill by default; `danger=true` swaps the foreground to system
/// red for destructive actions like Delete. Returns a bare `Div`
/// so the call site can attach `.on_mouse_up` etc. — the helper
/// just owns the visual styling.
pub(super) fn pill_button(s: SkinTokens, label: &'static str, danger: bool) -> gpui::Div {
    let fg = if danger {
        rgba(s.danger)
    } else {
        rgba(s.fg_primary)
    };
    let hover_bg = s.bg_input_hover;
    div()
        .px_3()
        .py_1()
        .bg(rgba(s.bg_input))
        .text_color(fg)
        .text_size(px(13.0))
        // Flex + items_center centers the glyph line-box within
        // the pill's padding box. Without this the text sits at
        // the top of the box because gpui's default line-height
        // is taller than the glyph itself and the leading
        // accumulates above rather than around the cap height.
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .cursor_pointer()
        .hover(move |st| st.bg(rgba(hover_bg)))
        .child(label)
}

/// Primary action button — solid `--accent` blue, white text. Used
/// for the form's Connect button (the call-to-action). Same shape
/// as `pill_button` so they line up flush in a button row.
pub(super) fn primary_button(s: SkinTokens, label: &'static str) -> gpui::Div {
    div()
        .px_3()
        .py_1()
        .bg(rgba(s.accent))
        .text_color(rgba(s.accent_fg))
        .text_size(px(13.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .cursor_pointer()
        .child(label)
}
