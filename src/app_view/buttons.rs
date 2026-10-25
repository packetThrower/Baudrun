//! The main window's buttons: GPUI Kit `Button`s in Baudrun's three
//! shapes. Colours come from the skin through the Theme slots
//! `AppView::apply_skin` sets (`button_primary*` = `--accent`,
//! `button_secondary*` = `--bg-input`). Call sites attach `.on_click`
//! / `.tooltip` themselves.

use gpui::{div, prelude::*, px, rgba, ElementId, SharedString};
use gpui_kit::component::button::{Button, ButtonVariants};

use super::STATUS_DOT_PX;
use crate::skin_tokens::SkinTokens;

/// Toggle pill for the session header's mid-session DTR / RTS
/// indicators: a `pill_button` with a status dot in front of the
/// label — green (`--success`) when the line is asserted, muted
/// (`--fg-tertiary`) when deasserted. The pill itself stays the same
/// in both states so the eye reads the dot as the state.
pub(super) fn line_pill(
    s: SkinTokens,
    id: impl Into<ElementId>,
    label: &'static str,
    active: bool,
) -> Button {
    let dot_color = if active { s.success } else { s.fg_tertiary };
    Button::new(id)
        .secondary()
        .child(
            div()
                .size(px(STATUS_DOT_PX))
                .rounded_full()
                .bg(rgba(dot_color)),
        )
        .child(label)
}

/// Neutral pill button (`--bg-input` fill). `danger=true` swaps the
/// label to `--danger` for destructive actions like Delete while the
/// pill stays neutral.
pub(super) fn pill_button(
    s: SkinTokens,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    danger: bool,
) -> Button {
    Button::new(id)
        .secondary()
        .label(label)
        .when(danger, |b| b.text_color(rgba(s.danger)))
}

/// Primary action button — solid `--accent`, `--accent-fg` text. The
/// form's Connect, the header's Disconnect, Resume.
pub(super) fn primary_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).primary().label(label)
}
