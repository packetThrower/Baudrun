//! TerminalView — checkpoints #4 + #5.
//!
//! Wraps `Term` + `Processor` + `TerminalGrid` into a single gpui
//! entity that captures keyboard events, encodes them into device-
//! shaped bytes, and either pushes them to a serial port (if one is
//! attached) or loops them back through our own VT parser (if none
//! is). The branch lives in one Option field — `serial_tx` — so the
//! same key handler covers both modes.
//!
//! Encoder responsibility split:
//!   * `encode_for_serial` produces the bytes that go on the wire to
//!     a real device (e.g. `\r` for Enter, `\x08` for Backspace).
//!   * `loopback_translate` post-processes those wire bytes into the
//!     local-echo equivalent (`\r\n`, `\x08 \x08`) so the no-device
//!     mode looks right on screen — bytes that would normally come
//!     back from the device's echo are synthesized here instead.
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
    div, prelude::*, px, rgb, App, Context, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    Keystroke, Render, Window,
};

use crate::serial_io::ts_us;
use crate::term_bridge::{make_term, mirror_to_grid};
use crate::terminal_grid::{pack, TerminalGrid};

pub struct TerminalView {
    term: Term<VoidListener>,
    processor: Processor,
    grid: TerminalGrid,
    focus_handle: FocusHandle,
    default_fg: Rgb,
    default_bg: Rgb,
    /// `Some` once a serial port has been attached via `set_serial_tx`.
    /// In that mode key bytes go on the wire and the device's echo
    /// drives the grid via the read channel. `None` means loopback —
    /// keystrokes feed the local Term directly.
    serial_tx: Option<flume::Sender<Vec<u8>>>,
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
            serial_tx: None,
        }
    }

    /// Attach a serial-port write channel. After this call, typed
    /// keystrokes go to the device (no local echo) and the device's
    /// echo is what updates the grid via `feed_bytes`.
    pub fn set_serial_tx(&mut self, tx: flume::Sender<Vec<u8>>) {
        self.serial_tx = Some(tx);
    }

    /// Feed a chunk of bytes through the VT parser, then re-mirror
    /// the resulting grid into the render-side cells. Used both for
    /// the boot-time sample stream and for typed-input loopback.
    /// `cx.notify()` triggers a re-render.
    pub fn feed_bytes(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        eprintln!(
            "[t={}us] feed_bytes: {} bytes",
            ts_us(),
            bytes.len()
        );
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
        let Some(serial_bytes) = encode_for_serial(&event.keystroke) else {
            return;
        };
        eprintln!(
            "[t={}us] handle_key_down: key={:?} -> {} bytes",
            ts_us(),
            event.keystroke.key,
            serial_bytes.len()
        );
        if let Some(tx) = &self.serial_tx {
            // Serial mode: bytes go on the wire. The device's echo
            // (or lack thereof — a passwd prompt won't echo) is what
            // updates the grid. `send` on an unbounded channel
            // doesn't block.
            let _ = tx.send(serial_bytes);
        } else {
            // Loopback: synthesize what a device's echo would look
            // like and feed it directly into our own Term.
            let echoed = loopback_translate(&serial_bytes);
            self.feed_bytes(&echoed, cx);
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
        // `size_full` + the background colour means the entire
        // window fills with the terminal background even when the
        // 24×80 grid doesn't reach the window edges (which it
        // doesn't on a 1100×720 window). Without this the unfilled
        // region renders transparent on Windows — a known gpui
        // default — and you can see whatever's behind the window.
        div()
            .size_full()
            .bg(rgb(pack(self.default_bg)))
            .p(px(8.0))
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(self.grid.element())
    }
}

/// Encode a keystroke as the wire bytes that should go to a serial
/// device. No echo affordances baked in: Enter is `\r`, Backspace is
/// `\x08`, etc. When a real device is attached the device's own echo
/// is what makes typed characters appear on screen. For the no-
/// device path, see `loopback_translate`.
fn encode_for_serial(k: &Keystroke) -> Option<Vec<u8>> {
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
        "enter" => return Some(b"\r".to_vec()),
        "tab" => return Some(b"\t".to_vec()),
        "backspace" => return Some(b"\x08".to_vec()),
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

/// Synthesize the bytes a real device would echo back, given the
/// wire bytes we just "sent." Used in loopback mode to make the
/// no-device path interactive.
///
///   * `\r` → `\r\n` so Enter advances visually instead of just
///     parking the cursor at column 0.
///   * `\x08` → `\x08 \x08` (BS, space, BS) — the canonical
///     "erase last char on screen" echo. Without the space-and-rewind
///     the cursor moves back but the char is still visible.
///
/// Pass-through for everything else: in real serial sessions, most
/// printable keystrokes are echoed verbatim by the device, which is
/// exactly what writing the same byte into our local Term produces.
fn loopback_translate(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            b'\r' => out.extend_from_slice(b"\r\n"),
            0x08 => out.extend_from_slice(b"\x08 \x08"),
            other => out.push(other),
        }
    }
    out
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
