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

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use alacritty_terminal::{
    grid::Scroll,
    index::{Column, Line, Point as GridPoint, Side},
    selection::{Selection, SelectionType},
    term::Term,
    vte::ansi::{Processor, Rgb},
};
use gpui::{
    black, div, font, prelude::*, px, rgb, App, Bounds, ClipboardItem, Context, FocusHandle,
    Focusable, IntoElement, KeyDownEvent, Keystroke, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point as PixelPoint, Render, ScrollDelta, ScrollWheelEvent, Task,
    TextRun, Window,
};
use gpui_component::WindowExt;

use crate::term_bridge::{make_term, mirror_to_grid, Dims, ListenerState, TerminalListener};
use crate::terminal_grid::{pack, TerminalGrid, CELL_HEIGHT_PX, FONT_SIZE_PX};

/// Cursor blink half-period. The cursor toggles visible/invisible
/// every `BLINK_INTERVAL`. ~530ms matches xterm's historical
/// default and the macOS system caret rate; long enough to be
/// readable, short enough to feel alive.
const BLINK_INTERVAL: Duration = Duration::from_millis(530);

/// How long the bell flash overlay stays painted after a BEL byte
/// is processed. Short enough to read as a flash rather than a
/// solid colour; long enough to be unmistakable across one frame
/// at 60fps (>= 16ms).
const BELL_FLASH_DURATION: Duration = Duration::from_millis(120);

/// Padding between the window edge and the grid. Mirrors the `.p()`
/// in `TerminalView::render` so the resize math knows how much of
/// the viewport is unavailable for cells.
const GRID_PADDING_PX: f32 = 8.0;

/// Minimum grid dimensions. Smaller than this and `Term::resize`
/// gets unhappy and TUIs render garbage. 4×10 is below every
/// realistic terminal but above the breaking point.
const MIN_ROWS: usize = 4;
const MIN_COLS: usize = 10;

/// Profile-driven keystroke + echo + paste behaviour. Stays
/// separate from the larger `Profile` struct because TerminalView
/// only needs the runtime bytes-on-the-wire knobs — theme, hex
/// view, logging etc. live elsewhere or are wired via separate
/// paths. Defaults match `Profile::defaults`: CR line ending, DEL
/// on backspace, no local echo, paste safety on, slow-paste off
/// with a 10ms-per-char default delay.
#[derive(Debug, Clone)]
pub struct ProfileSettings {
    /// Bytes sent on Enter. `"cr"` → `\r` (default; what serial
    /// consoles for Cisco/Juniper/Aruba expect), `"lf"` → `\n`
    /// (Linux consoles, embedded), `"crlf"` → `\r\n`
    /// (legacy / Windows).
    pub line_ending: String,
    /// Bytes sent on Backspace. `"del"` → `\x7F` (default; VT100,
    /// xterm, modern devices), `"bs"` → `\x08` (some older
    /// Cisco / Foundry gear).
    pub backspace_key: String,
    /// When true, typed bytes are also fed into the local Term as
    /// if the device had echoed them — useful when talking to a
    /// device that doesn't echo (some bootloaders, custom firmware).
    pub local_echo: bool,
    /// When true, prompt before pasting clipboard text that
    /// contains line breaks — catches "I pasted into the wrong
    /// terminal" mistakes before a routing config goes onto the
    /// wrong device.
    pub paste_warn_multiline: bool,
    /// When true, send pasted bytes one character at a time with
    /// `paste_char_delay_ms` between each. Lets slow UARTs (typical
    /// on industrial gear) keep up without dropping bytes.
    pub paste_slow: bool,
    /// Per-character delay for `paste_slow`, in milliseconds.
    /// Clamped to a non-negative value; 0 effectively disables the
    /// delay even when `paste_slow` is on.
    pub paste_char_delay_ms: u32,
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            line_ending: "cr".into(),
            backspace_key: "del".into(),
            local_echo: false,
            paste_warn_multiline: true,
            paste_slow: true,
            paste_char_delay_ms: 10,
        }
    }
}

pub struct TerminalView {
    term: Term<TerminalListener>,
    processor: Processor,
    grid: TerminalGrid,
    focus_handle: FocusHandle,
    default_fg: Rgb,
    default_bg: Rgb,
    /// Shared with the `TerminalListener` inside `term`. Polled
    /// after each `feed_bytes` to pick up bell events that fired
    /// during the parser advance.
    listener_state: Rc<ListenerState>,
    /// `Some` once a serial port has been attached via `set_serial_tx`.
    /// In that mode key bytes go on the wire and the device's echo
    /// drives the grid via the read channel. `None` means loopback —
    /// keystrokes feed the local Term directly.
    serial_tx: Option<flume::Sender<Vec<u8>>>,
    /// Profile-driven keystroke encoding settings (line_ending,
    /// backspace_key, local_echo). Updated by `AppView` when a new
    /// profile connects; defaults to `Profile::defaults` equivalents
    /// otherwise so the no-profile loopback path stays sensible.
    profile_settings: ProfileSettings,
    /// Cached cell-width measurement from gpui's text-system. Lazy
    /// because the text-system isn't queryable until after the
    /// platform window is up — we resolve on the first render and
    /// then reuse, since neither the font nor the size changes
    /// during a session yet.
    cell_width_px: Option<f32>,
    /// Window-coords bounds of the painted grid, written by
    /// `GridElement::paint` after each frame and read by
    /// `pixel_to_point` to translate mouse-event positions into
    /// cell coords. Without this the click math hard-codes the
    /// grid as starting at `(GRID_PADDING_PX, GRID_PADDING_PX)`,
    /// which is only true when the TerminalView fills the whole
    /// window — drag-selection breaks the moment a sidebar (or
    /// any other layout) shifts the grid right.
    grid_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
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
    /// Cursor blink phase: `true` = visible, `false` = hidden.
    /// Flipped by the blink task every `BLINK_INTERVAL`. Reset to
    /// `true` on user input so the cursor doesn't disappear in the
    /// middle of typing — the next blink-off lands one full
    /// interval after the keystroke instead of mid-stroke.
    cursor_blink_phase: bool,
    /// `Some` if a `Bell` event fired recently and the flash
    /// overlay is still being painted. Holds the timestamp at
    /// which the flash ends; `paint` checks `Instant::now()` against
    /// it. Cleared lazily on the next render after the timestamp
    /// passes.
    bell_flash_until: Option<Instant>,
    /// Held to keep the periodic blink task alive. Dropping this
    /// cancels the task; we never explicitly do so because the
    /// view itself outlives the task's relevance.
    _blink_task: Task<()>,
    /// In-flight slow-paste task (one-byte-at-a-time write loop).
    /// Held in a field so disconnect can drop it — otherwise a
    /// previously-spawned slow paste keeps writing to the old
    /// (dropped) sender after the user switches profiles. `None`
    /// when no slow paste is in progress.
    paste_task: Option<Task<()>>,
}

impl TerminalView {
    pub fn new(
        rows: usize,
        cols: usize,
        default_fg: Rgb,
        default_bg: Rgb,
        cx: &mut Context<Self>,
    ) -> Self {
        let (term, processor, listener_state) = make_term(rows, cols);
        let mut grid = TerminalGrid::new(rows, cols, default_fg, default_bg);
        // Paint the initial Term state into the grid so the cursor
        // is visible at startup. Without this the grid stays blank
        // (and cursor-less) until the first `feed_bytes` call —
        // i.e. you don't see where you're about to type until you
        // type something, which defeats the cursor's purpose.
        mirror_to_grid(&term, &mut grid, default_fg, default_bg, true);

        // Periodic blink task: every `BLINK_INTERVAL`, flip the
        // cursor's visible phase and notify so the renderer
        // re-paints. Detached doesn't apply here — we hold the
        // task in `_blink_task` so it lives exactly as long as
        // the view, no longer.
        let blink_task = cx.spawn(async move |weak, cx| {
            loop {
                cx.background_executor().timer(BLINK_INTERVAL).await;
                if weak
                    .update(cx, |this, cx| {
                        this.cursor_blink_phase = !this.cursor_blink_phase;
                        // Re-mirror so the cursor cell's fg/bg
                        // swap (or absence of it) actually shows
                        // up in the next paint. Without this the
                        // phase flips internally but the grid
                        // bytes the renderer reads from never
                        // change.
                        mirror_to_grid(
                            &this.term,
                            &mut this.grid,
                            this.default_fg,
                            this.default_bg,
                            this.cursor_blink_phase,
                        );
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });

        Self {
            term,
            processor,
            grid,
            focus_handle: cx.focus_handle(),
            default_fg,
            default_bg,
            listener_state,
            serial_tx: None,
            profile_settings: ProfileSettings::default(),
            cell_width_px: None,
            grid_bounds: Rc::new(Cell::new(None)),
            scroll_accum: 0.0,
            is_dragging: false,
            cursor_blink_phase: true,
            bell_flash_until: None,
            _blink_task: blink_task,
            paste_task: None,
        }
    }

    /// Attach a serial-port write channel. After this call, typed
    /// keystrokes go to the device (no local echo) and the device's
    /// echo is what updates the grid via `feed_bytes`.
    pub fn set_serial_tx(&mut self, tx: flume::Sender<Vec<u8>>) {
        self.serial_tx = Some(tx);
    }

    /// Drop the active serial sender, putting the view back into
    /// loopback mode. Called by `AppView` before opening a
    /// different profile's port — releases the OS write thread
    /// in `serial_io` because its receiver returns an error.
    /// Also cancels any in-flight slow paste so its remaining
    /// bytes don't keep flowing into the now-detached port.
    pub fn clear_serial_tx(&mut self) {
        self.serial_tx = None;
        self.paste_task = None;
    }

    /// Replace the active profile-keystroke settings. Called by
    /// `AppView::connect_to` after opening a profile's port so the
    /// keystroke encoder picks up the right line ending / backspace
    /// byte / echo behaviour. Profiles that share these defaults
    /// (most do) get a no-op assignment.
    pub fn set_profile_settings(&mut self, settings: ProfileSettings) {
        self.profile_settings = settings;
    }

    /// Feed a chunk of bytes through the VT parser, then re-mirror
    /// the resulting grid into the render-side cells. Used both for
    /// the boot-time sample stream and for typed-input loopback.
    /// `cx.notify()` triggers a re-render.
    pub fn feed_bytes(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        self.processor.advance(&mut self.term, bytes);
        // Drain any bell that fired during the parser advance.
        // Latches the flash window and schedules a one-shot notify
        // after the flash duration so the overlay clears even
        // when no other event triggers a re-render.
        if let Some(rang_at) = self.listener_state.bell.take() {
            self.bell_flash_until = Some(rang_at + BELL_FLASH_DURATION);
            cx.spawn(async move |weak, cx| {
                cx.background_executor().timer(BELL_FLASH_DURATION).await;
                weak.update(cx, |_, cx| cx.notify()).ok();
            })
            .detach();
        }
        mirror_to_grid(
            &self.term,
            &mut self.grid,
            self.default_fg,
            self.default_bg,
            self.cursor_blink_phase,
        );
        cx.notify();
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    /// Whether the bell flash overlay should still be painted.
    /// `true` from the moment a BEL byte was processed until
    /// `BELL_FLASH_DURATION` after, then permanently `false`
    /// (until the next bell). Computed against `Instant::now()`
    /// at render time rather than stored as state, so the same
    /// `bell_flash_until` instant naturally lapses without us
    /// having to clear it.
    fn bell_flash_active(&self) -> bool {
        self.bell_flash_until
            .is_some_and(|deadline| Instant::now() < deadline)
    }

    /// Resolve cell width via gpui's window-scoped text system.
    /// Lays out a 100-character string of `'0'`s, divides, then
    /// applies a 0.75 calibration. The calibration survives the
    /// move from div-rendering to a custom `Element` because
    /// `shape_line` reports advance in the same unit as
    /// `ch_advance` (both at ~7.82 px for Menlo @ 13pt), but
    /// gpui's paint pipeline renders glyphs at 0.75× that
    /// (~5.87 px) — looks like a typography point-vs-CSS-pixel
    /// confusion deep in gpui that neither the layout APIs nor
    /// the `force_width` per-glyph positioning escape. Without
    /// the calibration, glyphs sit at the right *position* but
    /// occupy less than the cell, producing visible extra
    /// spacing between chars. Drop this when gpui exposes the
    /// effective render-pixel size directly.
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
        // Use the layout-line reported advance directly — no
        // calibration. Earlier iterations applied 0.75x to match
        // an apparent paint-pipeline width difference, but that
        // was only needed when text runs SKIPPED blank cells (so
        // mid-line gaps came out wrong). With blanks now included
        // in the runs, the natural advance matches the cell
        // positioning end-to-end: glyph N at start + N*advance,
        // cursor at (col+1)*advance = right after glyph N.
        let advance = f32::from(layout.width) / SAMPLE_LEN as f32;
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
        mirror_to_grid(
            &self.term,
            &mut self.grid,
            self.default_fg,
            self.default_bg,
            self.cursor_blink_phase,
        );
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
        mirror_to_grid(
            &self.term,
            &mut self.grid,
            self.default_fg,
            self.default_bg,
            self.cursor_blink_phase,
        );
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
        mirror_to_grid(
            &self.term,
            &mut self.grid,
            self.default_fg,
            self.default_bg,
            self.cursor_blink_phase,
        );
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
        // The grid's actual window-coords origin, written by
        // `GridElement::paint`. Falls back to the padding-only
        // assumption for the first frame (before the first paint
        // populates the cell), so a very-early click degrades
        // gracefully instead of returning None.
        let grid_origin = self
            .grid_bounds
            .get()
            .map(|b| b.origin)
            .unwrap_or_else(|| PixelPoint {
                x: px(GRID_PADDING_PX),
                y: px(GRID_PADDING_PX),
            });
        let local_x = (f32::from(pos.x) - f32::from(grid_origin.x)).max(0.0);
        let local_y = (f32::from(pos.y) - f32::from(grid_origin.y)).max(0.0);
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

    fn paste_clipboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        let normalised: String = text.replace("\r\n", "\r").replace('\n', "\r");
        let line_count = normalised.matches('\r').count() + 1;
        let bytes = normalised.into_bytes();

        // Multi-line paste warning: prompt before sending text that
        // contains line breaks. Catches "I pasted a routing config
        // into the wrong device" mistakes. The dialog's `on_ok`
        // callback fires asynchronously and only has `&mut App`, so
        // we capture a weak handle to ourselves and re-enter via
        // `weak.update`.
        if self.profile_settings.paste_warn_multiline && line_count > 1 {
            let weak = cx.entity().downgrade();
            window.open_alert_dialog(cx, move |alert, _, _| {
                let weak = weak.clone();
                let bytes = bytes.clone();
                alert
                    .confirm()
                    .title("Paste multiple lines?")
                    .description(format!(
                        "About to paste {line_count} lines into the terminal."
                    ))
                    .on_ok(move |_, window, cx| {
                        let bytes = bytes.clone();
                        if let Some(this) = weak.upgrade() {
                            this.update(cx, |this, cx| {
                                this.send_paste(bytes, window, cx);
                            });
                        }
                        true
                    })
            });
            return;
        }

        self.send_paste(bytes, window, cx);
    }

    /// Push paste bytes onto the wire (or into local Term in
    /// loopback). Honors `paste_slow` by spawning a per-byte writer
    /// task with `paste_char_delay_ms` between sends; otherwise
    /// fires the whole buffer in one channel send.
    fn send_paste(&mut self, bytes: Vec<u8>, _window: &mut Window, cx: &mut Context<Self>) {
        if self.profile_settings.paste_slow {
            self.spawn_slow_paste(bytes, cx);
            return;
        }
        if let Some(tx) = &self.serial_tx {
            let _ = tx.send(bytes);
        } else {
            let echoed = loopback_translate(&bytes);
            self.feed_bytes(&echoed, cx);
        }
    }

    fn spawn_slow_paste(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        let delay = Duration::from_millis(self.profile_settings.paste_char_delay_ms as u64);
        // Held in `paste_task` so disconnect can drop it; otherwise
        // an in-flight slow paste keeps writing to the old sender
        // after `clear_serial_tx`.
        match self.serial_tx.clone() {
            Some(tx) => {
                let task = cx.spawn(async move |_, cx| {
                    for b in bytes {
                        if tx.send(vec![b]).is_err() {
                            break;
                        }
                        if !delay.is_zero() {
                            cx.background_executor().timer(delay).await;
                        }
                    }
                });
                self.paste_task = Some(task);
            }
            None => {
                // Loopback slow paste: echo bytes one-by-one
                // through `feed_bytes` with the same delay so the
                // visual pacing matches the wire pacing.
                let weak = cx.entity().downgrade();
                let task = cx.spawn(async move |_, cx| {
                    for b in bytes {
                        let chunk = loopback_translate(&[b]);
                        if weak
                            .update(cx, |this, cx| this.feed_bytes(&chunk, cx))
                            .is_err()
                        {
                            break;
                        }
                        if !delay.is_zero() {
                            cx.background_executor().timer(delay).await;
                        }
                    }
                });
                self.paste_task = Some(task);
            }
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
        mirror_to_grid(
            &self.term,
            &mut self.grid,
            self.default_fg,
            self.default_bg,
            self.cursor_blink_phase,
        );
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
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
            self.paste_clipboard(window, cx);
            return;
        }

        let Some(serial_bytes) = encode_for_serial(&event.keystroke, &self.profile_settings)
        else {
            return;
        };
        // Reset the blink phase so the cursor is visible during
        // typing — without this the cursor can disappear mid-stroke
        // if a key arrives just as the blink-off frame paints.
        self.cursor_blink_phase = true;
        // Typing implies the user wants to see the response, so
        // snap the view back to the live screen if they were
        // scrolled into history. Standard convention (xterm,
        // iTerm2, screen, tmux). Output bytes from the device do
        // NOT snap — letting users read scrollback while a chatty
        // device keeps sending is the whole reason scrollback
        // exists.
        if self.term.grid().display_offset() > 0 {
            self.term.scroll_display(Scroll::Bottom);
            mirror_to_grid(
            &self.term,
            &mut self.grid,
            self.default_fg,
            self.default_bg,
            self.cursor_blink_phase,
        );
            cx.notify();
        }
        if let Some(tx) = &self.serial_tx {
            // Serial mode: bytes go on the wire. The device's echo
            // (or lack thereof — a passwd prompt won't echo) is what
            // updates the grid. `send` on an unbounded channel
            // doesn't block.
            let local_echo = self.profile_settings.local_echo;
            let _ = tx.send(serial_bytes.clone());
            // With local echo on, also synthesize the echo locally
            // — useful when the device doesn't echo (some
            // bootloaders / custom firmware). Goes through the same
            // loopback translator so Enter renders as a CRLF and
            // Backspace as `BS SP BS`, matching what a real echo
            // would look like.
            if local_echo {
                let echoed = loopback_translate(&serial_bytes);
                self.feed_bytes(&echoed, cx);
            }
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
            .child(self.grid.element(
                cell_w,
                self.bell_flash_active(),
                self.grid_bounds.clone(),
            ))
    }
}

/// Encode a keystroke as the wire bytes that should go to a serial
/// device. The profile-configurable bytes (Enter / Backspace) come
/// from `settings`; everything else is fixed by the VT100 / xterm
/// keyboard convention. When a real device is attached the device's
/// own echo is what makes typed characters appear on screen — for
/// the no-device path see `loopback_translate`.
fn encode_for_serial(k: &Keystroke, settings: &ProfileSettings) -> Option<Vec<u8>> {
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
        "enter" => return Some(line_ending_bytes(&settings.line_ending)),
        "tab" => return Some(b"\t".to_vec()),
        "backspace" => return Some(vec![backspace_byte(&settings.backspace_key)]),
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

/// Bytes sent on Enter, per profile setting. Anything unrecognised
/// (an empty string from a freshly-loaded profile, etc.) falls back
/// to CR — that's the safest default for serial network gear,
/// matches `Profile::defaults`.
fn line_ending_bytes(line_ending: &str) -> Vec<u8> {
    match line_ending {
        "lf" => b"\n".to_vec(),
        "crlf" => b"\r\n".to_vec(),
        _ => b"\r".to_vec(),
    }
}

/// Byte sent on Backspace, per profile setting. Defaults to DEL
/// (0x7F) — VT100 / xterm / modern. `"bs"` selects BS (0x08) for
/// older Cisco / Foundry gear that misinterprets DEL.
fn backspace_byte(backspace_key: &str) -> u8 {
    match backspace_key {
        "bs" => 0x08,
        _ => 0x7F,
    }
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
