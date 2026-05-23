//! Shortcut tables + keystroke spec marshalling, split out of
//! `settings_view/mod.rs`.
//!
//! Public surface (still reachable as `settings_view::*`):
//!
//!   * `SHORTCUT_ACTIONS` — the canonical id list, in the order
//!     Settings → Shortcuts displays the rows.
//!   * `effective_shortcut(action, overrides)` — resolves a per-
//!     action keybinding string by falling back to the per-OS
//!     default when the user hasn't overridden.
//!   * `spec_to_gpui_binding(spec)` — converts a Tauri-style spec
//!     string (`"Meta+Shift+K"`) to gpui's keybinding syntax
//!     (`"secondary-shift-k"`).
//!
//! Pub(super) helpers used by `SettingsView` itself:
//!
//!   * `shortcut_label` — display label for a `SHORTCUT_ACTIONS` id.
//!   * `format_spec(&Keystroke)` — render a captured keystroke as
//!     a Tauri-style spec for persistence.
//!   * `parse_spec(spec)` — reverse of `format_spec`, used to
//!     render the live binding into the row's pill display.
//!   * `any_modifier(&Modifiers)` — checks whether at least one
//!     modifier was held during a keystroke capture (used to gate
//!     the recording-state Escape).
//!
//! Module-private:
//!
//!   * `default_for_action` — per-OS default spec.
//!   * `canonical_key_for_storage` / `canonical_key_for_display` —
//!     normalise gpui key names against the Tauri shortcut storage
//!     format.

use std::collections::HashMap;

use gpui::{Keystroke, Modifiers};

// -- Shortcut tables (mirror Tauri's `src/lib/shortcuts.ts`) ----------

/// Display order in Settings → Shortcuts. Same grouping as Tauri:
/// session control first, then transfer / window management, then
/// view actions.
pub(crate) const SHORTCUT_ACTIONS: &[&str] = &[
    "connect",
    "disconnect",
    "suspend",
    "resume",
    "clear",
    "break",
    "send-file",
    "new-profile",
    "open-window",
    "font-increase",
    "font-decrease",
    "font-reset",
    // Terminal-pane clipboard actions. Context-scoped to the
    // terminal in `key_binding_for_action`, so customising them
    // here won't accidentally hijack Cmd+C inside a profile-form
    // text input (gpui-component's Input ships its own Copy
    // binding against the "Input" context that the terminal
    // binding can't shadow).
    "copy",
    "paste",
    "select-all",
];

pub(super) fn shortcut_label(action: &'static str) -> &'static str {
    match action {
        "connect" => "Connect",
        "disconnect" => "Disconnect",
        "suspend" => "Suspend session",
        "resume" => "Resume session",
        "clear" => "Clear terminal",
        "break" => "Send Break",
        "send-file" => "Send file (X/YMODEM)",
        "new-profile" => "New profile",
        "open-window" => "Open profile in new window",
        "font-increase" => "Increase font size",
        "font-decrease" => "Decrease font size",
        "font-reset" => "Reset font size",
        "copy" => "Copy",
        "paste" => "Paste",
        "select-all" => "Select all",
        _ => action,
    }
}

// Defaults differ from the Tauri shipping app in one spot:
// "new-profile" was Meta+N there because Tauri ran single-window,
// so Cmd+N had no conflicting "New Window" candidate. Now that
// every menubar gets a real File → New Window (statically bound to
// Cmd+N to match macOS convention), New Profile shifts to
// Cmd+Shift+P so the bindings don't collide. Users who imported a
// settings.json with the old Meta+N override still get their
// preference — `effective_shortcut` honours the override before
// falling back here.
#[cfg(target_os = "macos")]
fn default_for_action(action: &str) -> &'static str {
    match action {
        "connect" => "Meta+Enter",
        "disconnect" => "Meta+Shift+D",
        "suspend" => "Meta+Shift+S",
        "resume" => "Meta+Shift+R",
        "clear" => "Meta+K",
        "break" => "Meta+Shift+B",
        "send-file" => "Meta+Shift+T",
        "new-profile" => "Meta+Shift+P",
        "open-window" => "Meta+Shift+Enter",
        "font-increase" => "Meta+=",
        "font-decrease" => "Meta+-",
        "font-reset" => "Meta+0",
        "copy" => "Meta+C",
        "paste" => "Meta+V",
        "select-all" => "Meta+A",
        _ => "",
    }
}

#[cfg(not(target_os = "macos"))]
fn default_for_action(action: &str) -> &'static str {
    match action {
        "connect" => "Control+Enter",
        "disconnect" => "Control+Shift+D",
        "suspend" => "Control+Shift+S",
        "resume" => "Control+Shift+R",
        "clear" => "Control+Shift+K",
        "break" => "Control+Shift+B",
        "send-file" => "Control+Shift+T",
        "new-profile" => "Control+Shift+P",
        "open-window" => "Control+Shift+Enter",
        "font-increase" => "Control+=",
        "font-decrease" => "Control+-",
        "font-reset" => "Control+0",
        // Defaults to plain Ctrl+C / Ctrl+V on Windows / Linux per
        // the user's preference. Trades the wire's traditional
        // 0x03 (ETX / SIGINT) -- a network engineer reaches that
        // via Send Break or by overriding this binding to e.g.
        // Control+Shift+C and freeing Control+C back to the wire.
        // A "Ctrl+C is selection-aware (copies when there's a
        // selection, sends interrupt otherwise)" toggle lives on
        // the follow-up list.
        "copy" => "Control+C",
        "paste" => "Control+V",
        "select-all" => "Control+A",
        _ => "",
    }
}

/// Effective spec — user override if present + non-empty, else
/// platform default. Matches Tauri's `effectiveShortcut`: empty
/// string in the override map is treated as "unset" so the reset
/// affordance can clear without having to delete the key.
pub(crate) fn effective_shortcut(action: &str, overrides: &HashMap<String, String>) -> String {
    if let Some(s) = overrides.get(action) {
        if !s.trim().is_empty() {
            return s.clone();
        }
    }
    default_for_action(action).to_string()
}

// -- Spec ↔ Keystroke conversion --------------------------------------

pub(super) fn any_modifier(m: &Modifiers) -> bool {
    m.control || m.alt || m.shift || m.platform || m.function
}

/// Format a captured `Keystroke` into the W3C aria-keyshortcuts
/// shape Tauri persists (`Meta+Shift+K`). Only the modifiers we
/// surface in the UI are emitted — `function` is captured but
/// dropped from the spec since the persisted format has no slot
/// for it (Fn-key bindings on macOS aren't useful as terminal
/// shortcuts anyway).
pub(super) fn format_spec(keystroke: &Keystroke) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(5);
    let m = &keystroke.modifiers;
    // Order matches Tauri's parser tolerance: any order parses,
    // but we standardise on Control → Meta → Shift → Alt for
    // round-trip stability.
    if m.control {
        parts.push("Control");
    }
    if m.platform {
        parts.push("Meta");
    }
    if m.shift {
        parts.push("Shift");
    }
    if m.alt {
        parts.push("Alt");
    }
    let key_str = canonical_key_for_storage(keystroke.key.as_str());
    let mut out = parts.join("+");
    if !out.is_empty() {
        out.push('+');
    }
    out.push_str(&key_str);
    out
}

/// Map gpui's lowercase key names to the W3C key value names Tauri
/// stores. Letter / digit / punctuation keys round-trip as their
/// raw form (uppercase letters); arrow / page / function names
/// are normalized to the W3C names so the shipping app can read
/// them back.
fn canonical_key_for_storage(key: &str) -> String {
    match key {
        "up" => "ArrowUp".into(),
        "down" => "ArrowDown".into(),
        "left" => "ArrowLeft".into(),
        "right" => "ArrowRight".into(),
        "pageup" => "PageUp".into(),
        "pagedown" => "PageDown".into(),
        "enter" => "Enter".into(),
        "escape" => "Escape".into(),
        "tab" => "Tab".into(),
        "space" => " ".into(),
        "backspace" => "Backspace".into(),
        "delete" => "Delete".into(),
        "home" => "Home".into(),
        "end" => "End".into(),
        // Single ASCII letter → uppercase. Digits / punctuation
        // / multi-char already-named keys (F1, etc.) pass through
        // unchanged.
        other if other.len() == 1 && other.chars().next().unwrap().is_ascii_alphabetic() => {
            other.to_ascii_uppercase()
        }
        other => other.to_string(),
    }
}

/// Reverse of `format_spec` — parse a stored Tauri spec back into
/// a gpui `Keystroke` so the `Kbd` widget can render it. Returns
/// `None` for malformed specs (no key, only modifiers, …) so the
/// caller can fall back to a placeholder.
pub(super) fn parse_spec(spec: &str) -> Option<Keystroke> {
    if spec.is_empty() {
        return None;
    }
    let mut modifiers = Modifiers::default();
    let mut key: Option<String> = None;
    for raw in spec.split('+') {
        let tok = raw.trim();
        if tok.is_empty() {
            continue;
        }
        match tok.to_ascii_lowercase().as_str() {
            "control" | "ctrl" => modifiers.control = true,
            "meta" | "cmd" | "command" | "super" | "win" => modifiers.platform = true,
            "shift" => modifiers.shift = true,
            "alt" | "option" => modifiers.alt = true,
            // Last non-modifier token wins, like Tauri's parser.
            _ => key = Some(canonical_key_for_display(tok)),
        }
    }
    let key = key?;
    Some(Keystroke {
        modifiers,
        key,
        key_char: None,
    })
}

/// Convert a stored W3C shortcut spec (`"Meta+Shift+K"`) into the
/// hyphen-joined lowercase form gpui's `KeyBinding::new` parser
/// expects (`"cmd-shift-k"`). Returns `None` for specs with no key
/// or only modifiers (gpui would reject those at parse time).
///
/// Mirrors `parse_spec` for the modifier-token vocabulary; the
/// difference is the output encoding — gpui uses `-` between
/// parts. Crucially, gpui's `cmd` / `super` / `win` tokens all set
/// `modifiers.platform`, which is the Cmd key on macOS but the
/// Windows / Super key on Windows / Linux — there is no
/// auto-translation. The portable token is `secondary-`, which
/// resolves to Cmd on macOS and Ctrl elsewhere. We can still emit
/// `cmd-` here because the upstream `default_for_action` is
/// `#[cfg]`-gated to only use `Meta+...` on macOS; on Windows /
/// Linux every shortcut feeds in as `Control+...` and we encode
/// that as `ctrl-...` (which IS portable to the real Control key
/// on every OS).
pub(crate) fn spec_to_gpui_binding(spec: &str) -> Option<String> {
    if spec.is_empty() {
        return None;
    }
    let mut parts: Vec<&str> = Vec::with_capacity(5);
    let mut control = false;
    let mut platform = false;
    let mut shift = false;
    let mut alt = false;
    let mut key: Option<String> = None;
    for raw in spec.split('+') {
        let tok = raw.trim();
        if tok.is_empty() {
            continue;
        }
        match tok.to_ascii_lowercase().as_str() {
            "control" | "ctrl" => control = true,
            "meta" | "cmd" | "command" | "super" | "win" => platform = true,
            "shift" => shift = true,
            "alt" | "option" => alt = true,
            _ => key = Some(canonical_key_for_display(tok)),
        }
    }
    let key = key?;
    // Order matches gpui's parser tolerance but we standardise on
    // ctrl → cmd → alt → shift → key for round-trip stability with
    // the rest of the codebase's KeyBinding strings.
    if control {
        parts.push("ctrl");
    }
    if platform {
        parts.push("cmd");
    }
    if alt {
        parts.push("alt");
    }
    if shift {
        parts.push("shift");
    }
    let mut out = parts.join("-");
    if !out.is_empty() {
        out.push('-');
    }
    out.push_str(&key);
    Some(out)
}

/// W3C → gpui key name. Inverse of `canonical_key_for_storage`.
/// Unknown keys lowercase by default (matches gpui's convention
/// for letters / punctuation).
fn canonical_key_for_display(key: &str) -> String {
    match key {
        "ArrowUp" => "up".into(),
        "ArrowDown" => "down".into(),
        "ArrowLeft" => "left".into(),
        "ArrowRight" => "right".into(),
        "PageUp" => "pageup".into(),
        "PageDown" => "pagedown".into(),
        "Enter" => "enter".into(),
        "Escape" => "escape".into(),
        "Tab" => "tab".into(),
        " " | "Space" => "space".into(),
        "Backspace" => "backspace".into(),
        "Delete" => "delete".into(),
        "Home" => "home".into(),
        "End" => "end".into(),
        other if other.len() == 1 && other.chars().next().unwrap().is_ascii_alphabetic() => {
            other.to_ascii_lowercase()
        }
        other => other.to_string(),
    }
}
