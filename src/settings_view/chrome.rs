//! Settings window helpers split out of `settings_view/mod.rs`:
//! the small buttons the Settings pages reuse (`action_button`,
//! `trash_button`) plus `dismiss_notification_after`,
//! `encode_file_path` and `resolve_config_dir_display`. The window
//! layout itself (page sidebar, search, groups) is GPUI Kit's
//! `Settings`.

use gpui::{App, DismissEvent, ElementId, Entity, IntoElement, SharedString, Window};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    notification::Notification,
    IconName, Sizable,
};

use crate::data::appdata;

/// Schedule a Notification to dismiss itself ~1.5 s after the user
/// clicks its action button. Without this the toast stays up for
/// the full autohide window (5 s) even after the user already
/// acted on it — feels like the click had no effect. The brief
/// linger gives time to read the follow-up "Restored …" toast
/// before the original "Removed …" one fades.
pub(super) fn dismiss_notification_after(notif: Entity<Notification>, cx: &mut gpui::App) {
    // Spawn from the Notification's own Context, not via
    // `App::spawn`. The latter route gets dropped when the user
    // switches tabs while the timer is pending — the SettingsView
    // re-renders, the click-handler closure is rebuilt, and the
    // detached App-spawn task evidently doesn't survive the
    // turnover. Entity-scoped spawns stay alive as long as the
    // notification entity does.
    notif.update(cx, |_, ctx| {
        ctx.spawn(async move |weak, cx_async| {
            cx_async
                .background_executor()
                .timer(std::time::Duration::from_millis(1500))
                .await;
            let _ = weak.update(cx_async, |_, ctx| {
                ctx.emit(DismissEvent);
            });
        })
        .detach();
    });
}

/// Percent-encode a filesystem path for use in a `file://` URL.
/// Keeps the RFC 3986 unreserved set plus `/` (path separator)
/// verbatim and `%xx`-encodes everything else. Handles the common
/// case of macOS paths containing spaces
/// (`~/Library/Application Support/...`) — without this, the URL
/// constructor rejects the unencoded space and the open is a
/// silent no-op.
pub(super) fn encode_file_path(p: &std::path::Path) -> String {
    let s = p.to_string_lossy();
    let mut out = String::with_capacity(s.len());
    for &byte in s.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Resolve the current Baudrun support directory for display in
/// Settings → Advanced → Config Directory. Falls back to a sentinel
/// string when the resolver errors out so the UI still has
/// something to show.
pub(super) fn resolve_config_dir_display() -> SharedString {
    match appdata::support_dir() {
        Ok(path) => SharedString::from(path.display().to_string()),
        Err(err) => SharedString::from(t!("settings_chrome.config_dir_unavailable", err = err)),
    }
}

/// Small outline button for the pages' secondary actions (Import…,
/// Preview, Choose…, Reset, Reveal). `id` is explicit because the
/// Advanced page carries two Choose… / Reset pairs.
pub(super) fn action_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    Button::new(id)
        .outline()
        .small()
        .label(label)
        .on_click(move |_, window, cx| on_click(window, cx))
}

/// Trash icon button that removes a user-imported skin, theme or
/// highlight pack. Danger-styled so the intent is clear before the
/// click; every removal offers Undo in a toast.
pub(super) fn trash_button(
    id: impl Into<ElementId>,
    tip: impl Into<SharedString>,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    Button::new(id)
        .danger()
        .outline()
        .small()
        .icon(IconName::Delete)
        .tooltip(tip)
        .on_click(move |_, window, cx| on_click(window, cx))
}
