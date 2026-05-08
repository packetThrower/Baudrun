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
    grid::Scroll,
    index::{Column, Line, Point as GridPoint, Side},
    selection::{Selection, SelectionType},
    term::Term,
    vte::ansi::{Processor, Rgb},
};
use gpui::{
    black, div, font, prelude::*, px, rgb, App, ClipboardItem, Context, FocusHandle, Focusable,
    IntoElement, KeyDownEvent, Keystroke, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point as PixelPoint, Render, ScrollDelta, ScrollWheelEvent, TextRun,
    Window,
};

use crate::term_bridge::{make_term, mirror_to_grid, Dims};
use crate::terminal_grid::{pack, TerminalGrid, CELL_HEIGHT_PX, FONT_SIZE_PX};

/// Padding between the window edge and the grid. Mirrors the `.p()`
/// in `TerminalView::render` so the resize math knows how much of
/// the viewport is unavailable for cells.
const GRID_PADDING_PX: f32 = 8.0;

/// Minimum grid dimensions. Smaller than this and `Term::resize`
/// gets unhappy and TUIs render garbage. 4×10 is below every
/// realistic terminal but above the breaking point.
const MIN_ROWS: usize = 4;
const MIN_COLS: usize = 10;

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
    /// Cached cell-width measurement from gpui's text-system. Lazy
    /// because the text-system isn't queryable until after the
    /// platform window is up — we resolve on the first render and
    /// then reuse, since neither the font nor the size changes
    /// during a session yet.
    cell_width_px: Option<f32>,
    /// Sub-line accumulator for trackpad scrolling. macOS trackpads
    /// emit Pixels deltas one frame at a time; without buffering
    /// the fractional remainder, slow scrolls under one line per
    /// frame would never trigger.
    scroll_accum: f32,
    /// `true` while the left mouse button is held and we're
    /// extending an active selection. Set on mouse_down, cleared
    /// on mouse_up. Without this gate, mouse_move events while the
    /// button is up would still extend selection.
    is_dragging: bool,
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
        let mut grid = TerminalGrid::new(rows, cols, default_fg, default_bg);
        // Paint the initial Term state into the grid so the cursor
        // is visible at startup. Without this the grid stays blank
        // (and cursor-less) until the first `feed_bytes` call —
        // i.e. you don't see where you're about to type until you
        // type something, which defeats the cursor's purpose.
        mirror_to_grid(&term, &mut grid, default_fg, default_bg);
        Self {
            term,
            processor,
            grid,
            focus_handle: cx.focus_handle(),
            default_fg,
            default_bg,
            serial_tx: None,
            cell_width_px: None,
            scroll_accum: 0.0,
            is_dragging: false,
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
        self.processor.advance(&mut self.term, bytes);
        mirror_to_grid(&self.term, &mut self.grid, self.default_fg, self.default_bg);
        cx.notify();
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    /// Resolve cell width by asking gpui's window-scoped text
    /// system to lay out a 100-character string of `'0'`s, then
    /// applying a 0.75 calibration factor.
    ///
    /// The factor is empirical: gpui's `layout_line` reports cell
    /// advance assuming `font_size` is in CSS pixels (Menlo @ 13
    /// → 7.82 px/cell), but the actual paint pipeline appears to
    /// treat the same value as typography points and renders at
    /// 13 / 1.333 ≈ 9.75 effective pixels of font (yielding
    /// ~5.86 px/cell). 0.75 = 1/1.333 collapses the two views
    /// into one number that's correct for both:
    ///   * pixel-to-cell math (so drag-selection tracks the mouse)
    ///   * the explicit `.w(cells × cell_w)` set on each run div
    ///     (so the box sizes match the painted text — without the
    ///     calibration the boxes were oversized and the rendered
    ///     text inside them showed gaps where the next run began).
    /// If/when gpui's text-system gets a more direct
    /// "what-will-actually-paint" API, drop the constant.
    fn cell_width(&mut self, window: &Window, _cx: &mut App) -> f32 {
        if let Some(w) = self.cell_width_px {
            return w;
        }
        #[cfg(target_os = "macos")]
        let family = "Menlo";
        #[cfg(target_os = "windows")]
        let family = "Cascadia Mono";
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        let family = "DejaVu Sans Mono";

        const SAMPLE_LEN: usize = 100;
        let sample = "0".repeat(SAMPLE_LEN);
        let runs = [TextRun {
            len: sample.len(),
            font: font(family),
            color: black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }];
        let layout = window.text_system().layout_line(
            &sample,
            px(FONT_SIZE_PX),
            &runs,
            None,
        );
        let measured = f32::from(layout.width) / SAMPLE_LEN as f32;
        // See doc comment above for the source of 0.75.
        const PAINT_CALIBRATION: f32 = 0.75;
        let advance = measured * PAINT_CALIBRATION;
        self.cell_width_px = Some(advance);
        advance
    }

    fn handle_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Convert the platform delta into whole lines. macOS
        // trackpads emit fine-grained Pixels deltas; mice usually
        // emit coarse Lines.
        let lines = match event.delta {
            ScrollDelta::Lines(p) => p.y,
            ScrollDelta::Pixels(p) => f32::from(p.y) / CELL_HEIGHT_PX,
        };
        // Accumulate sub-line motion so slow trackpad scrolling
        // doesn't drop every event below 1.0. Round-to-zero rather
        // than floor so flick gestures still register on the first
        // frame.
        self.scroll_accum += lines;
        let whole = self.scroll_accum.trunc() as i32;
        if whole == 0 {
            return;
        }
        self.scroll_accum -= whole as f32;

        // Convention: positive `delta.y` means "user wants to see
        // older content" (scroll up), which in alacritty is a
        // positive `Scroll::Delta`. Both directions get clamped
        // internally to `[0, history_size]`, so we don't have to
        // bound-check.
        self.term.scroll_display(Scroll::Delta(whole));
        mirror_to_grid(&self.term, &mut self.grid, self.default_fg, self.default_bg);
        cx.notify();
    }

    fn handle_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left {
            return;
        }
        // gpui delivers a focus-grabbing click separately; if this
        // is the very first click into the window, also accept it
        // as the focus event but don't start a selection drag.
        if event.first_mouse {
            return;
        }
        let Some(point) = self.pixel_to_point(event.position) else {
            return;
        };
        // Fresh selection on every click. Drag-update extends; a
        // single click without movement leaves an empty selection
        // which `mirror_to_grid` ignores.
        self.term.selection = Some(Selection::new(SelectionType::Simple, point, Side::Left));
        self.is_dragging = true;
        mirror_to_grid(&self.term, &mut self.grid, self.default_fg, self.default_bg);
        cx.notify();
    }

    fn handle_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_dragging {
            return;
        }
        let Some(point) = self.pixel_to_point(event.position) else {
            return;
        };
        if let Some(sel) = self.term.selection.as_mut() {
            sel.update(point, Side::Left);
        }
        mirror_to_grid(&self.term, &mut self.grid, self.default_fg, self.default_bg);
        cx.notify();
    }

    fn handle_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left {
            return;
        }
        // Selection state survives the release so the user can
        // copy it with Cmd-C / Ctrl-Shift-C; cleared on the next
        // mouse_down (a fresh click drops the prior selection).
        self.is_dragging = false;
    }

    /// Translate a window-coords pixel position into an alacritty
    /// grid Point. Returns None if the point falls outside the grid
    /// (clicks in the padding around the rendered cells, etc.).
    /// The `Line` returned is in alacritty's screen-with-scrollback
    /// coordinate system: positive = visible live screen, negative
    /// = above the screen (scrollback).
    fn pixel_to_point(&self, pos: PixelPoint<Pixels>) -> Option<GridPoint> {
        let cell_w = self.cell_width_px?;
        if cell_w <= 0.0 {
            return None;
        }
        let local_x = (f32::from(pos.x) - GRID_PADDING_PX).max(0.0);
        let local_y = (f32::from(pos.y) - GRID_PADDING_PX).max(0.0);
        let col = (local_x / cell_w).floor() as usize;
        let display_row = (local_y / CELL_HEIGHT_PX).floor() as i32;
        let cols = self.grid.cols();
        let rows = self.grid.rows();
        if cols == 0 || rows == 0 {
            return None;
        }
        let col = col.min(cols - 1);
        let display_row = display_row.min(rows as i32 - 1);
        let display_offset = self.term.grid().display_offset() as i32;
        let line = display_row - display_offset;
        Some(GridPoint::new(Line(line), Column(col)))
    }

    fn copy_selection(&mut self, cx: &mut App) {
        if let Some(text) = self.term.selection_to_string().filter(|s| !s.is_empty()) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn paste_clipboard(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx
            .read_from_clipboard()
            .as_ref()
            .and_then(ClipboardItem::text)
        else {
            return;
        };
        if text.is_empty() {
            return;
        }
        // Pasted CR/LF normalisation: incoming clipboards from
        // Windows arrive as `\r\n`, from old macOS as `\r`, from
        // Unix as `\n`. Serial devices universally want `\r` for
        // line submission, so normalise everything to that. Skip
        // the loopback echo translation (the device-side echo —
        // or our `loopback_translate` for the no-device path —
        // handles visualisation as if these had been typed).
        let normalised: String = text
            .replace("\r\n", "\r")
            .replace('\n', "\r");
        let bytes = normalised.into_bytes();
        if let Some(tx) = &self.serial_tx {
            let _ = tx.send(bytes);
        } else {
            let echoed = loopback_translate(&bytes);
            self.feed_bytes(&echoed, cx);
        }
    }

    /// Adjust the grid + Term to match the current window size,
    /// if they don't already. Called on every render — idempotent
    /// when dimensions haven't changed, so cheap when nothing
    /// resized; only mirrors when the size actually moved.
    fn maybe_resize(&mut self, window: &Window, cx: &mut App) {
        let viewport = window.viewport_size();
        let cell_w = self.cell_width(window, cx);
        if cell_w <= 0.0 {
            return;
        }
        let content_w = (f32::from(viewport.width) - GRID_PADDING_PX * 2.0).max(0.0);
        let content_h = (f32::from(viewport.height) - GRID_PADDING_PX * 2.0).max(0.0);
        let new_cols = ((content_w / cell_w).floor() as usize).max(MIN_COLS);
        let new_rows = ((content_h / CELL_HEIGHT_PX).floor() as usize).max(MIN_ROWS);
        if new_rows == self.grid.rows() && new_cols == self.grid.cols() {
            return;
        }
        self.term.resize(Dims { rows: new_rows, cols: new_cols });
        self.grid.resize(new_rows, new_cols);
        // Re-mirror so the freshly-resized grid reflects whatever
        // alacritty did to its own cells (cursor reposition,
        // scrollback rotation). No `cx.notify()` — we're called
        // from `render`; gpui will paint the up-to-date grid in
        // the very next step.
        mirror_to_grid(&self.term, &mut self.grid, self.default_fg, self.default_bg);
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Clipboard shortcuts come before the wire encoder. macOS
        // uses Cmd-C / Cmd-V (which the encoder already drops to
        // None because of `m.platform`). Linux / Windows use
        // Ctrl-Shift-C / Ctrl-Shift-V — the bare Ctrl-C / Ctrl-V
        // on those platforms keeps its terminal-control meaning
        // (XOFF... actually 0x03 SIGINT and 0x16 SYN — both real
        // serial-device codes), which is why network terminals
        // moved to the Shift-modified variants.
        let m = &event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        let copy_combo = (m.platform && !m.control && !m.alt && key == "c")
            || (m.control && m.shift && !m.platform && !m.alt && key == "c");
        let paste_combo = (m.platform && !m.control && !m.alt && key == "v")
            || (m.control && m.shift && !m.platform && !m.alt && key == "v");
        if copy_combo {
            self.copy_selection(cx);
            return;
        }
        if paste_combo {
            self.paste_clipboard(cx);
            return;
        }

        let Some(serial_bytes) = encode_for_serial(&event.keystroke) else {
            return;
        };
        // Typing implies the user wants to see the response, so
        // snap the view back to the live screen if they were
        // scrolled into history. Standard convention (xterm,
        // iTerm2, screen, tmux). Output bytes from the device do
        // NOT snap — letting users read scrollback while a chatty
        // device keeps sending is the whole reason scrollback
        // exists.
        if self.term.grid().display_offset() > 0 {
            self.term.scroll_display(Scroll::Bottom);
            mirror_to_grid(&self.term, &mut self.grid, self.default_fg, self.default_bg);
            cx.notify();
        }
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Reshape the grid + Term to fit the current window before
        // rendering. Idempotent when nothing resized, so it costs
        // nothing on steady-state frames; on a resize this is what
        // makes the new column / row count take effect immediately.
        self.maybe_resize(window, cx);
        let cell_w = self.cell_width(window, cx);

        // `size_full` + the background colour means the entire
        // window fills with the terminal background even when the
        // grid doesn't reach the window edges. Without this the
        // unfilled region renders transparent on Windows — a known
        // gpui default — and you can see whatever's behind the
        // window.
        div()
            .size_full()
            .bg(rgb(pack(self.default_bg)))
            .p(px(GRID_PADDING_PX))
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .on_scroll_wheel(cx.listener(Self::handle_scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
            .on_mouse_move(cx.listener(Self::handle_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
            .child(self.grid.element(cell_w))
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
