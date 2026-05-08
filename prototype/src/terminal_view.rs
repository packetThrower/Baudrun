//! TerminalView — checkpoint #4.
//!
//! Wraps `Term` + `Processor` + `TerminalGrid` into a single gpui
//! entity that captures keyboard events, encodes them as bytes, and
//! feeds them back through the same VT parser the rendering path
//! reads from. After checkpoint #5 lands and a real serial port is
//! wired up, the byte sink will become "write to serial port" and
//! the device's echo will drive `processor.advance`. Until then we
//! short-circuit: typed bytes go straight back into our own Term so
//! the prototype is interactive without a device.
//!
//! Why an entity wraps the grid instead of the grid being the entity
//! directly: `TerminalGrid` is "render-side state only" — cells +
//! palette + display geometry. Mixing the parser state (Term) and
//! input plumbing (FocusHandle, key listeners) into the grid would
//! conflate concerns we want separable when the real chrome lands
//! (multiple grids per window, settings panes that don't need a
//! parser, etc.).

use alacritty_terminal::{
    event::VoidListener,
    term::Term,
    vte::ansi::{Processor, Rgb},
};
use gpui::{
    div, prelude::*, App, Context, FocusHandle, Focusable, IntoElement, KeyDownEvent, Keystroke,
    Render, Window,
};

use crate::term_bridge::{make_term, mirror_to_grid};
use crate::terminal_grid::TerminalGrid;

pub struct TerminalView {
    term: Term<VoidListener>,
    processor: Processor,
    grid: TerminalGrid,
    focus_handle: FocusHandle,
    default_fg: Rgb,
    default_bg: Rgb,
}

impl TerminalView {
    pub fn new(
        rows: usize,
        cols: usize,
        default_fg: Rgb,
        default_bg: Rgb,
        cx: &mut Context<Self>,
    ) -> Self {
        let (term, processor) = make_term(rows, cols);
        let grid = TerminalGrid::new(rows, cols, default_fg, default_bg);
        Self {
            term,
            processor,
            grid,
            focus_handle: cx.focus_handle(),
            default_fg,
            default_bg,
        }
    }

    /// Feed a chunk of bytes through the VT parser, then re-mirror
    /// the resulting grid into the render-side cells. Used both for
    /// the boot-time sample stream and for typed-input loopback.
    /// `cx.notify()` triggers a re-render.
    pub fn feed_bytes(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        self.processor.advance(&mut self.term, bytes);
        mirror_to_grid(&self.term, &mut self.grid, self.default_fg, self.default_bg);
        cx.notify();
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(bytes) = loopback_encode(&event.keystroke) {
            self.feed_bytes(&bytes, cx);
        }
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(self.grid.element())
    }
}

/// Encode a keystroke as bytes that — when fed back through our own
/// `Processor::advance` — look right on screen. This is the loopback
/// shape: it bakes in tiny "device-side" affordances we'd normally
/// expect a real device to provide:
///
///   * `Enter` → `\r\n`. A real device would echo `\r\n` after we
///     send `\r`; the loopback collapses that round-trip.
///   * `Backspace` → `\x08 \x08`. The standard "erase last visible
///     char" sequence. Without the trailing space-and-rewind, BS
///     alone moves the cursor without erasing, which on a local-echo
///     prototype reads as a stuck cursor.
///
/// When checkpoint #5 wires a real serial port, this function gets
/// replaced by a pair: a keystroke-to-serial-bytes encoder (no echo
/// affordances; just `\r` and `\x08`) and a serial-bytes channel
/// that feeds `feed_bytes` directly. The visual correctness then
/// comes from the device's echo, not from our loopback.
fn loopback_encode(k: &Keystroke) -> Option<Vec<u8>> {
    let m = &k.modifiers;

    // Cmd / Win / Super: leave for the OS / future keybindings.
    // Handing these to the parser would steal Cmd-Q, Cmd-Tab, etc.
    if m.platform {
        return None;
    }

    if m.control && !m.alt {
        if let Some(b) = ctrl_byte(&k.key) {
            return Some(vec![b]);
        }
    }

    match k.key.as_str() {
        "enter" => return Some(b"\r\n".to_vec()),
        "tab" => return Some(b"\t".to_vec()),
        "backspace" => return Some(b"\x08 \x08".to_vec()),
        "escape" => return Some(b"\x1b".to_vec()),
        "left" => return Some(b"\x1b[D".to_vec()),
        "right" => return Some(b"\x1b[C".to_vec()),
        "up" => return Some(b"\x1b[A".to_vec()),
        "down" => return Some(b"\x1b[B".to_vec()),
        "home" => return Some(b"\x1b[H".to_vec()),
        "end" => return Some(b"\x1b[F".to_vec()),
        "delete" => return Some(b"\x1b[3~".to_vec()),
        "pageup" => return Some(b"\x1b[5~".to_vec()),
        "pagedown" => return Some(b"\x1b[6~".to_vec()),
        "space" => return Some(b" ".to_vec()),
        _ => {}
    }

    // `key_char` is gpui's IME-aware "what would be typed" value
    // (e.g. shift-resolved capitals, option-typed `ß`). Prefer it
    // over `key` when present.
    if let Some(s) = k.key_char.as_deref() {
        if !s.is_empty() {
            return Some(s.as_bytes().to_vec());
        }
    }

    // Fall back to `key` if it's a single printable character.
    let mut chars = k.key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if !c.is_control() {
            let mut buf = [0u8; 4];
            return Some(c.encode_utf8(&mut buf).as_bytes().to_vec());
        }
    }

    None
}

/// Translate a single-character key under Ctrl into its control
/// byte. Mirrors xterm: Ctrl-A..Ctrl-Z → 0x01..0x1A, plus the
/// non-letter Ctrl bindings that map to ASCII control codes.
fn ctrl_byte(key: &str) -> Option<u8> {
    let mut chars = key.chars();
    let c = match (chars.next(), chars.next()) {
        (Some(c), None) => c,
        _ => return None,
    };
    match c {
        'a'..='z' => Some((c as u8) - b'a' + 1),
        'A'..='Z' => Some((c as u8) - b'A' + 1),
        '@' | ' ' => Some(0x00),
        '[' => Some(0x1b),
        '\\' => Some(0x1c),
        ']' => Some(0x1d),
        '^' => Some(0x1e),
        '_' | '?' => Some(0x1f),
        _ => None,
    }
}
