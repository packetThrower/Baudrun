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

use gpui::{div, prelude::*, px, rgba, ClickEvent, SharedString, Window};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    Disableable,
};

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
pub(super) fn send_file_field_label(label: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(px(11.0))
        .text_color(rgba(0x808080CCu32))
        .child(label.into())
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

/// "Choose…" — outline button next to the path display.
pub(super) fn send_file_choose_button(
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> Button {
    Button::new("send-file-choose")
        .outline()
        .label(t!("settings.choose"))
        .on_click(on_click)
}

/// Cancel-style button at the bottom of the dialog. Outline so it
/// doesn't compete with the primary Send button.
pub(super) fn send_file_secondary_button(
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> Button {
    Button::new("send-file-secondary")
        .outline()
        .label(label)
        .on_click(on_click)
}

/// Send-style primary button. Disabled (dimmed, ignores clicks)
/// until a file is picked, matching Tauri's "pick a file first"
/// affordance.
pub(super) fn send_file_primary_button(
    label: impl Into<SharedString>,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> Button {
    Button::new("send-file-primary")
        .primary()
        .label(label)
        .disabled(!enabled)
        .on_click(on_click)
}
