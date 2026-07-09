//! Profile-form select-widget primitives split out of
//! `app_view/mod.rs`. Owns the `Opt` value type, the
//! `make_select` / `read_select` ergonomic wrappers around
//! `gpui-component`'s `SelectState`, and every per-field option-
//! list builder (baud / data-bits / parity / stop-bits / flow-
//! control / line-ending / line-policy / backspace / port).
//!
//! The port-list builder lives here too because it returns the
//! same `Vec<Opt>` shape and the editor form composes it alongside
//! the static lists, even though its data comes from a runtime
//! `serial::ports::list_ports()` call rather than a hardcoded
//! table.

use gpui::{AppContext, Context, Entity, SharedString, Window};
use gpui_component::{
    select::{SelectItem, SelectState},
    IndexPath,
};

use super::AppView;
use crate::data::serial::ports;

/// One choice in a select widget. `id` is the canonical value
/// stored on the `Profile` (e.g. `"none"`, `"9600"`, `"crlf"`);
/// `title` is the human-readable label shown in the menu and as
/// the closed-state value (e.g. `"None"`, `"9600 (default)"`,
/// `"CRLF (\\r\\n) — modems"`). Cheap to clone (two `String`s) —
/// the option lists are tiny and built once per editor open.
#[derive(Clone)]
pub(super) struct Opt {
    id: String,
    title: SharedString,
}

impl Opt {
    pub(super) fn new(id: &str, title: &str) -> Self {
        Self {
            id: id.to_string(),
            title: SharedString::from(title.to_string()),
        }
    }
}

impl SelectItem for Opt {
    type Value = String;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }
}

/// Build a `SelectState<Vec<Opt>>` pre-selected to whichever option
/// in `opts` has `id == selected`. If `selected` doesn't match
/// anything, no option is pre-selected (the closed-state shows the
/// placeholder, if any). Wraps the gpui-component constructor so
/// caller sites don't have to deal with `IndexPath` directly.
pub(super) fn make_select(
    opts: Vec<Opt>,
    selected: &str,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> Entity<SelectState<Vec<Opt>>> {
    let idx = opts
        .iter()
        .position(|o| o.id == selected)
        .map(IndexPath::new);
    cx.new(|cx| SelectState::new(opts, idx, window, cx))
}

/// Read the currently-selected id from a `SelectState<Vec<Opt>>`.
/// Falls back to an empty string if nothing is selected — the
/// `Profile` validator rejects empty strings for these fields, so
/// the user gets a clear error rather than a silent bad save.
pub(super) fn read_select(state: &Entity<SelectState<Vec<Opt>>>, cx: &Context<AppView>) -> String {
    state.read(cx).selected_value().cloned().unwrap_or_default()
}

// --- Option lists --------------------------------------------------
//
// Mirrors the Tauri ProfileForm.svelte option arrays. Labels are
// hand-written to match: short id + parenthetical hint where the
// raw id alone (e.g. "cr", "del") would be opaque to a user who
// hasn't shipped serial-console code before. Wrapping each in a
// fn keeps the borrowed-vec ergonomics simple — gpui-component
// takes the `Vec<Opt>` by value into the SelectState.

pub(super) fn baud_opts() -> Vec<Opt> {
    let mut opts = vec![Opt::new("9600", &t!("opts.baud.9600"))];
    for rate in [
        "19200", "38400", "57600", "115200", "230400", "460800", "921600",
    ] {
        opts.push(Opt::new(rate, rate));
    }
    opts
}

pub(super) fn data_bits_opts() -> Vec<Opt> {
    ["5", "6", "7", "8"]
        .into_iter()
        .map(|s| Opt::new(s, s))
        .collect()
}

pub(super) fn parity_opts() -> Vec<Opt> {
    [
        ("none", t!("opts.parity.none")),
        ("odd", t!("opts.parity.odd")),
        ("even", t!("opts.parity.even")),
        ("mark", t!("opts.parity.mark")),
        ("space", t!("opts.parity.space")),
    ]
    .into_iter()
    .map(|(id, title)| Opt::new(id, &title))
    .collect()
}

pub(super) fn stop_bits_opts() -> Vec<Opt> {
    ["1", "1.5", "2"]
        .into_iter()
        .map(|s| Opt::new(s, s))
        .collect()
}

pub(super) fn flow_control_opts() -> Vec<Opt> {
    [
        ("none", t!("opts.flow_control.none")),
        ("rtscts", t!("opts.flow_control.rtscts")),
        ("xonxoff", t!("opts.flow_control.xonxoff")),
    ]
    .into_iter()
    .map(|(id, title)| Opt::new(id, &title))
    .collect()
}

pub(super) fn line_ending_opts() -> Vec<Opt> {
    [
        ("cr", t!("opts.line_ending.cr")),
        ("lf", t!("opts.line_ending.lf")),
        ("crlf", t!("opts.line_ending.crlf")),
    ]
    .into_iter()
    .map(|(id, title)| Opt::new(id, &title))
    .collect()
}

/// Line-policy options for DTR/RTS on connect/disconnect — pulled
/// verbatim from `src/lib/api.ts` (LINE_POLICIES). Empty-string id
/// is allowed by `Profile::validate` but isn't useful in the UI;
/// "default" carries the same semantics ("leave as-is").
pub(super) fn line_policy_opts() -> Vec<Opt> {
    [
        ("default", t!("opts.line_policy.default")),
        ("assert", t!("opts.line_policy.assert")),
        ("deassert", t!("opts.line_policy.deassert")),
    ]
    .into_iter()
    .map(|(id, title)| Opt::new(id, &title))
    .collect()
}

pub(super) fn backspace_opts() -> Vec<Opt> {
    [
        ("del", t!("opts.backspace.del")),
        ("bs", t!("opts.backspace.bs")),
    ]
    .into_iter()
    .map(|(id, title)| Opt::new(id, &title))
    .collect()
}

/// Build the Serial Port select options from the current OS port
/// list. Each detected port becomes one option; the title bundles
/// the device path with whatever the enumerator found
/// (`/dev/cu.usbserial-XYZ — FT232R USB UART · FTDI`) — same shape
/// the Tauri form uses, so the user gets enough info to identify
/// the right adapter without opening System Settings.
///
/// If `keep_selected` is non-empty and isn't in the detected list,
/// it's prepended as an "(not connected)" option so the saved
/// profile still shows its port even when the device is unplugged.
/// On port enumeration failure we still want a usable form, so we
/// fall back to the keep_selected (if any) and otherwise an empty
/// list — the user can rescan later.
pub(super) fn port_opts(keep_selected: &str) -> Vec<Opt> {
    let detected = ports::list_ports().unwrap_or_default();
    let mut opts: Vec<Opt> = detected
        .iter()
        .map(|p| {
            let mut title = p.name.clone();
            if !p.product.is_empty() {
                title.push_str(" — ");
                title.push_str(&p.product);
            }
            if !p.chipset.is_empty() {
                title.push_str(" · ");
                title.push_str(&p.chipset);
            }
            Opt::new(&p.name, &title)
        })
        .collect();
    if !keep_selected.is_empty() && !detected.iter().any(|p| p.name == keep_selected) {
        // Prepend so the user's saved port shows up first when
        // it isn't currently detected (cable unplugged, etc.).
        let title = t!("opts.port_not_connected", port = keep_selected);
        opts.insert(0, Opt::new(keep_selected, &title));
    }
    opts
}
