//! Send File dialog primitives split out of `app_view/mod.rs`:
//!
//!   * `YMODEM_PROTOCOL_ID` — the stable id used for the default
//!     pick in the protocol Select.
//!   * `transfer_protocol_opts` — the option list for that Select.
//!   * `send_file_field_label`, `send_file_path_pill`,
//!     `send_file_choose_button`, `send_file_secondary_button`,
//!     `send_file_primary_button` — the Send-File-dialog-specific
//!     pill/button helpers, named for their position in the dialog
//!     layout rather than collapsed into the generic `buttons.rs`
//!     primitives (they have distinct sizing + spacing).
//!
//! `TransferIo` / `TransferState` / `TransferResult` /
//! `SendHexState` / `SendFileState` stay in mod.rs because the
//! transfer state-machine driving them lives on `impl AppView`
//! and the methods do direct field access on every field —
//! moving the types here would force ~13 `pub(super)` widenings
//! for no functional gain.

use gpui::{div, prelude::*, px, rgba, MouseButton, MouseUpEvent, SharedString, Window};

use super::opts::Opt;

/// Stable id used as the YMODEM Select option (the default pick).
pub(super) const YMODEM_PROTOCOL_ID: &str = "ymodem";

/// Option list for the Send file dialog's Protocol select. Order +
/// labels mirror the Tauri dialog so the muscle memory transfers.
pub(super) fn transfer_protocol_opts() -> Vec<Opt> {
    vec![
        Opt::new(
            YMODEM_PROTOCOL_ID,
            "YMODEM \u{2014} 1024-byte blocks with filename + size",
        ),
        Opt::new(
            "xmodem-crc",
            "XMODEM-CRC \u{2014} 128-byte blocks with CRC-16",
        ),
        Opt::new(
            "xmodem-1k",
            "XMODEM-1K \u{2014} 1024-byte blocks with CRC-16",
        ),
        Opt::new(
            "xmodem-classic",
            "XMODEM-Classic \u{2014} 128-byte blocks with checksum",
        ),
    ]
}

/// Small uppercase-ish field label above an input row.
pub(super) fn send_file_field_label(label: &'static str) -> gpui::Div {
    div()
        .text_size(px(11.0))
        .text_color(rgba(0x808080CCu32))
        .child(label)
}

/// Read-only path display — input-styled pill that shows the chosen
/// filename (or a muted "No file selected" placeholder).
pub(super) fn send_file_path_pill(label: String) -> gpui::Div {
    let placeholder = label.is_empty();
    let text: SharedString = if placeholder {
        SharedString::from("No file selected")
    } else {
        SharedString::from(label)
    };
    div()
        .flex_1()
        .px_3()
        .py(px(6.0))
        .rounded_md()
        .border_1()
        .border_color(rgba(0x80808055u32))
        .bg(rgba(0x80808014u32))
        .text_size(px(13.0))
        .text_color(rgba(if placeholder {
            0x808080AAu32
        } else {
            0xE4E4E7FFu32
        }))
        .child(text)
}

/// "Choose…" — secondary pill button next to the path display.
pub(super) fn send_file_choose_button<F>(on_click: F) -> gpui::Div
where
    F: Fn(&MouseUpEvent, &mut Window, &mut gpui::App) + 'static,
{
    div()
        .px_3()
        .py(px(6.0))
        .rounded_md()
        .border_1()
        .border_color(rgba(0x80808055u32))
        .bg(rgba(0x80808022u32))
        .text_size(px(13.0))
        .cursor_pointer()
        .hover(|st| st.bg(rgba(0x4DA6FF22u32)))
        .child("Choose\u{2026}")
        .on_mouse_up(MouseButton::Left, on_click)
}

/// Cancel-style button at the bottom of the dialog. Quiet styling
/// (matches Tauri's secondary button) so it doesn't compete with
/// the primary Send button.
pub(super) fn send_file_secondary_button<F>(label: &'static str, on_click: F) -> gpui::Div
where
    F: Fn(&MouseUpEvent, &mut Window, &mut gpui::App) + 'static,
{
    div()
        .px_4()
        .py(px(6.0))
        .rounded_md()
        .border_1()
        .border_color(rgba(0x80808055u32))
        .text_size(px(13.0))
        .cursor_pointer()
        .hover(|st| st.bg(rgba(0x80808033u32)))
        .child(label)
        .on_mouse_up(MouseButton::Left, on_click)
}

/// Send-style primary button. Disabled state (when `enabled` is
/// false) still renders but ignores clicks and dims out, matching
/// Tauri's "you must pick a file first" affordance.
pub(super) fn send_file_primary_button<F>(
    label: &'static str,
    enabled: bool,
    on_click: F,
) -> gpui::Div
where
    F: Fn(&MouseUpEvent, &mut Window, &mut gpui::App) + 'static,
{
    let text_color = if enabled {
        0xFFFFFFFFu32
    } else {
        0xFFFFFFAAu32
    };
    let bg = if enabled {
        0x4DA6FFFFu32
    } else {
        0x4DA6FF66u32
    };
    let mut btn = div()
        .px_4()
        .py(px(6.0))
        .rounded_md()
        .bg(rgba(bg))
        .text_size(px(13.0))
        .text_color(rgba(text_color))
        .child(label);
    if enabled {
        btn = btn
            .cursor_pointer()
            .on_mouse_up(MouseButton::Left, on_click);
    }
    btn
}
