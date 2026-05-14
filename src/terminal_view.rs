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
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point as GridPoint, Side},
    selection::{Selection, SelectionType},
    term::Term,
    vte::ansi::Processor,
};
use gpui::{
    anchored, black, deferred, div, font, prelude::*, px, rgb, rgba, relative, App,
    AnyElement, Bounds, ClipboardItem, Context, FocusHandle, Focusable, IntoElement,
    KeyDownEvent, Keystroke, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Point as PixelPoint, Render, ScrollDelta, ScrollWheelEvent, SharedString,
    Task, TextRun, Window,
};
use gpui_component::WindowExt;

use crate::term_bridge::{make_term, mirror_to_grid, Dims, ListenerState, Palette, TerminalListener};
use crate::terminal_grid::{pack, TerminalGrid};

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
    /// When true, incoming bytes are formatted as a `xxd`-style
    /// hex dump before being fed to the VT parser — useful for
    /// reverse-engineering binary protocols where the raw byte
    /// stream matters more than the rendered text.
    pub hex_view: bool,
    /// When true, every newline-started line is prefixed with a
    /// dim-grey wall-clock timestamp (`[HH:MM:SS.mmm] `). Helps
    /// when grepping a session for "what happened around 14:30".
    /// Applied AFTER `hex_view`, so each hex row gets its own
    /// timestamp when both flags are on.
    pub timestamps: bool,
    /// When true, every newline-started line is prefixed with a
    /// dim-grey sequential line number (`   42  `). Applied
    /// AFTER `timestamps`, so a line with both flags reads
    /// `   42  [14:30:01.123] content`. Resets to 1 on every
    /// disconnect / reconnect cycle — line numbers are a
    /// session-local counter, not a cumulative log line index.
    pub line_numbers: bool,
}

/// Computed geometry for the right-edge scroll indicator. Both
/// fields are fractions of the available track height — the
/// renderer multiplies them by the painted track size.
#[derive(Debug, Clone, Copy)]
struct ScrollIndicator {
    thumb_top_pct: f32,
    thumb_height_pct: f32,
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
            hex_view: false,
            timestamps: false,
            line_numbers: false,
        }
    }
}

pub struct TerminalView {
    term: Term<TerminalListener>,
    processor: Processor,
    grid: TerminalGrid,
    focus_handle: FocusHandle,
    /// Active 16-ANSI palette + foreground / background / cursor.
    /// Replaces the previous `default_fg` / `default_bg` pair —
    /// `palette.fg` and `palette.bg` are the same values, with the
    /// rest of the slots driving cell-color resolution. Swapped
    /// via `set_palette` when the user picks a different theme in
    /// the Settings window.
    palette: Palette,
    /// Effective scrollback line count. Snapshotted at construction
    /// and updated by `set_scrollback_lines`; mirrored back to the
    /// underlying Term via `set_options`. Held on the view so the
    /// highlight-replay path can rebuild the Term with the same
    /// buffer size instead of falling back to alacritty's default.
    scrollback_lines: usize,
    /// Total newlines received this session, capped at
    /// `scrollback_lines`. Drives the status bar's `X/Y` line
    /// counter — more intuitive than alacritty's `history_size()`
    /// which only counts rows that have scrolled OUT of the
    /// visible viewport (so it sits at 0 on a fresh session
    /// even with 20+ rows of content on screen). Reset by
    /// `clear_serial_tx` so the counter starts fresh on each
    /// connect.
    lines_received: usize,
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
    /// `true` when the active selection came from a triple-click
    /// (line-content select). That selection is a `Simple` range
    /// pinned to the line's printed extent — character-granular,
    /// so unlike the word-sticky `Semantic` double-click it would
    /// collapse on the slightest mouse jiggle. `handle_mouse_move`
    /// checks this and skips drag-extension for triple-clicks;
    /// `is_dragging` still stays `true` so copy-on-select fires
    /// on release.
    triple_click_selection: bool,
    /// PuTTY-style auto-copy on mouse-selection release. Reads
    /// the global `Settings::copy_on_select` flag, mirrored here
    /// by `AppView::apply_copy_on_select` so each settings change
    /// reflects on every live terminal without threading the bus
    /// in. Honoured by `handle_mouse_up` — on release with a
    /// non-empty selection, writes the text to the clipboard.
    copy_on_select: bool,
    /// Window-coords anchor for the right-click context menu.
    /// `Some(pos)` while the menu is open, `None` otherwise.
    /// The menu offers Copy / Paste / Select All / Clear — same
    /// rows the keybindings cover, surfaced for users who prefer
    /// mousing. Dismissed by clicking an item, clicking outside
    /// (AppView's root catch-all calls our `close_context_menu`),
    /// or pressing Escape.
    context_menu_pos: Option<PixelPoint<Pixels>>,
    /// `Some(grab_offset_within_thumb)` while the user is dragging
    /// the scrollback thumb. The grab offset (Y distance from the
    /// thumb's top to the initial click) is preserved across the
    /// drag so the thumb tracks the cursor without snapping to its
    /// midpoint. `None` outside of a scrollbar drag.
    scrollbar_drag: Option<Pixels>,
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
    /// `Some` while the active profile has hex view enabled. The
    /// formatter accumulates bytes and emits `xxd`-style lines
    /// that we feed into the VT parser instead of the raw stream;
    /// `None` means raw passthrough (the normal terminal mode).
    hex_formatter: Option<HexFormatter>,
    /// `Some` while the active profile has line-timestamps on.
    /// Tracks "are we at the start of a line" across `feed_bytes`
    /// calls so a chunk that arrives mid-line doesn't get a
    /// stamp inserted partway through.
    timestamps_state: Option<TimestampInjector>,
    /// `Some` while the active profile has line numbers on. Same
    /// at-line-start tracking as the timestamp injector, plus a
    /// monotonic counter that resets on every reconnect (see
    /// `apply_profile_settings`). Applied after the timestamp
    /// injector so the line number is the leftmost column in
    /// the rendered output.
    line_numbers_state: Option<LineNumberInjector>,
    /// Idle flush for the hex formatter's partial-line buffer.
    /// Re-armed on every `feed_bytes` call so a streaming run
    /// stays buffered (proper 16-per-row layout); after
    /// `HEX_PARTIAL_FLUSH_DELAY` of quiet, the partial line is
    /// emitted so single-byte echoes (Enter, prompt) eventually
    /// appear on screen. Dropping the field cancels the timer.
    hex_flush_task: Option<Task<()>>,
    /// `Some` while the active profile has highlighting on AND
    /// at least one rule pack is enabled. Stateless — runs the
    /// compiled regex set against each `\n`-terminated segment in
    /// the chunk and wraps matches in ANSI colour escapes. The
    /// trailing partial (no newline yet) passes through raw so
    /// single-byte echoes from typing appear instantly.
    highlight: Option<crate::highlight_runtime::HighlightBuffer>,
    /// Idle-flush timer for the highlight buffer's partial tail
    /// (a chunk that hasn't seen its `\n` yet — typing echo,
    /// prompts). 30ms is short enough that typing feels
    /// responsive but long enough that bytes arriving close
    /// together coalesce into a complete line for the regex set.
    highlight_flush_task: Option<Task<()>>,
    /// Raw device bytes captured this session, used to replay the
    /// scrollback through a fresh highlight rule set when the
    /// user toggles packs in Settings. Capped at
    /// `SESSION_REPLAY_MAX_BYTES` so an hour-long firehose run
    /// doesn't grow without bound.
    session_bytes: Vec<u8>,
    /// True while replaying `session_bytes` after a rule change —
    /// guards `feed_bytes` against double-recording (so the
    /// replayed bytes don't append a second copy to
    /// `session_bytes`).
    replaying: bool,
}

impl TerminalView {
    pub fn new(
        rows: usize,
        cols: usize,
        palette: Palette,
        scrolling_history: usize,
        cx: &mut Context<Self>,
    ) -> Self {
        let (term, processor, listener_state) = make_term(rows, cols, scrolling_history);
        let mut grid = TerminalGrid::new(rows, cols, palette.fg, palette.bg);
        // Paint the initial Term state into the grid so the cursor
        // is visible at startup. Without this the grid stays blank
        // (and cursor-less) until the first `feed_bytes` call —
        // i.e. you don't see where you're about to type until you
        // type something, which defeats the cursor's purpose.
        mirror_to_grid(&term, &mut grid, &palette, true);

        // Periodic blink task: every `BLINK_INTERVAL`, flip the
        // cursor's visible phase and notify so the renderer
        // re-paints. Detached doesn't apply here — we hold the
        // task in `_blink_task` so it lives exactly as long as
        // the view, no longer.
        //
        // Under prefers-reduced-motion the toggle inside the
        // update closure becomes a no-op — `cursor_blink_phase`
        // stays at its initial `true`, the grid keeps painting
        // the cursor solid, and the task ticks as a cheap timer
        // that re-checks the global on every interval. That lets
        // a runtime toggle of the reduce-motion preference start
        // / stop the blink without restarting the app (relaunch
        // still updates the `ReduceMotion` global, but the task
        // itself doesn't require it).
        let blink_task = cx.spawn(async move |weak, cx| {
            loop {
                cx.background_executor().timer(BLINK_INTERVAL).await;
                if weak
                    .update(cx, |this, cx| {
                        if cx.global::<crate::ReduceMotion>().0 {
                            // Keep cursor solid + leave phase
                            // alone; renderer keeps showing the
                            // cursor.
                            return;
                        }
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
                            &this.palette,
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
            palette,
            scrollback_lines: scrolling_history,
            lines_received: 0,
            listener_state,
            serial_tx: None,
            profile_settings: ProfileSettings::default(),
            cell_width_px: None,
            grid_bounds: Rc::new(Cell::new(None)),
            scroll_accum: 0.0,
            is_dragging: false,
            triple_click_selection: false,
            copy_on_select: false,
            context_menu_pos: None,
            scrollbar_drag: None,
            cursor_blink_phase: true,
            bell_flash_until: None,
            _blink_task: blink_task,
            paste_task: None,
            hex_formatter: None,
            timestamps_state: None,
            line_numbers_state: None,
            hex_flush_task: None,
            highlight: None,
            highlight_flush_task: None,
            session_bytes: Vec::new(),
            replaying: false,
        }
    }

    /// Attach a serial-port write channel. After this call, typed
    /// keystrokes go to the device (no local echo) and the device's
    /// echo is what updates the grid via `feed_bytes`.
    pub fn set_serial_tx(&mut self, tx: flume::Sender<Vec<u8>>) {
        self.serial_tx = Some(tx);
        // Each new serial session starts with an empty replay
        // buffer — otherwise toggling highlight packs after a
        // reconnect would replay the previous device's bytes too.
        self.session_bytes.clear();
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
        // Reset session counters so the status bar starts fresh
        // on the next connect.
        self.lines_received = 0;
    }

    /// Replace the active highlight rule set. Called by
    /// `AppView::connect_to` after resolving the effective list
    /// (per-profile override → global setting → built-in default).
    /// `None` disables highlighting entirely; `Some(rules)` with
    /// an empty list also disables it (matches Tauri's "highlight
    /// off" semantics — no rules to compile).
    ///
    /// On rule change with non-empty session history, replays the
    /// raw byte stream through the new rule set so existing
    /// scrollback re-colours too. Same UX shape as a theme swap:
    /// you change the setting, the screen agrees with it.
    pub fn set_highlight_rules(
        &mut self,
        rules: Option<Vec<crate::data::highlight::HighlightRule>>,
        cx: &mut Context<Self>,
    ) {
        match rules {
            Some(rules) if !rules.is_empty() => {
                let engine = crate::highlight_runtime::HighlightEngine::from_rules(&rules);
                if engine.is_empty() {
                    // Every rule failed to compile — drop the
                    // buffer entirely; running through it would
                    // be a pure overhead.
                    self.highlight = None;
                } else {
                    self.highlight =
                        Some(crate::highlight_runtime::HighlightBuffer::new(engine));
                }
            }
            _ => {
                self.highlight = None;
            }
        }
        // Cancel any pending partial flush — its closure captured
        // a now-stale buffer reference, and the replay below will
        // either emit the partial freshly or drop it entirely.
        self.highlight_flush_task = None;
        self.replay_session(cx);
    }

    /// Reset the alacritty `Term` and re-feed the captured raw
    /// bytes through the (now-current) transform pipeline. Called
    /// after highlight rules change so the existing scrollback
    /// picks up the new colouring; no-op when there's no captured
    /// history yet.
    fn replay_session(&mut self, cx: &mut Context<Self>) {
        if self.session_bytes.is_empty() {
            return;
        }
        // Reset alacritty `Term` + parser state so the replay
        // doesn't pile a second copy on top of the existing grid.
        let rows = self.grid.rows();
        let cols = self.grid.cols();
        let (term, processor, listener_state) =
            make_term(rows, cols, self.scrollback_lines);
        self.term = term;
        self.processor = processor;
        self.listener_state = listener_state;
        // Replace the render grid with a fresh blank one so any
        // cells the previous replay/state left behind get
        // overwritten — `mirror_to_grid` only writes cells the
        // new Term reports in `display_iter`, which can be fewer
        // than the previous frame.
        self.grid = TerminalGrid::new(rows, cols, self.palette.fg, self.palette.bg);
        // Reset transform state so timestamps / hex / line-numbers
        // re-emit from a clean slate. Highlight is already stateless.
        if let Some(ts) = self.timestamps_state.as_mut() {
            *ts = TimestampInjector::new();
        }
        if let Some(ln) = self.line_numbers_state.as_mut() {
            *ln = LineNumberInjector::new();
        }
        if let Some(hex) = self.hex_formatter.as_mut() {
            *hex = HexFormatter::new();
        }
        // Pull bytes out under a flag so the replay's feed_bytes
        // doesn't append a second copy back into session_bytes.
        let bytes = std::mem::take(&mut self.session_bytes);
        self.replaying = true;
        self.feed_bytes(&bytes, cx);
        self.replaying = false;
        self.session_bytes = bytes;
    }

    /// Replace the active profile-keystroke settings. Called by
    /// `AppView::connect_to` after opening a profile's port so the
    /// keystroke encoder picks up the right line ending / backspace
    /// byte / echo behaviour. Profiles that share these defaults
    /// (most do) get a no-op assignment.
    /// Swap the active palette and re-mirror so the new colours
    /// take effect on the next paint without waiting for a fresh
    /// `feed_bytes`. Called from `AppView` when the user picks a
    /// different theme in the Settings window. Cells whose colours
    /// resolved to palette slots (the common case) re-resolve
    /// against the new palette; cells with literal `Color::Spec`
    /// values keep their hardcoded colour, same as in xterm.
    pub fn set_palette(&mut self, palette: Palette, cx: &mut Context<Self>) {
        self.palette = palette;
        // Update the grid's blank-cell + background colours so any
        // newly-allocated rows (resize, scroll-back-into-history)
        // pick up the new theme too.
        self.grid.set_default_colors(palette.fg, palette.bg);
        mirror_to_grid(
            &self.term,
            &mut self.grid,
            &self.palette,
            self.cursor_blink_phase,
        );
        cx.notify();
    }

    /// Swap the active terminal-pane font size. Updates the grid's
    /// glyph-size + cell-height fields, drops the cached cell-width
    /// (needs re-measuring at the new font), and re-mirrors so the
    /// existing screen content reflows. The next `maybe_resize`
    /// pass (driven by the per-frame paint) will recompute the
    /// row/col count from the pane bounds against the new cell
    /// metrics — no need to do it eagerly here.
    pub fn set_font_size(&mut self, font_size_px: f32, cx: &mut Context<Self>) {
        if (self.grid.font_size_px() - font_size_px).abs() < f32::EPSILON {
            return;
        }
        self.grid.set_font_size(font_size_px);
        self.cell_width_px = None;
        mirror_to_grid(
            &self.term,
            &mut self.grid,
            &self.palette,
            self.cursor_blink_phase,
        );
        cx.notify();
    }

    /// Resize the underlying alacritty Term's scrollback to `lines`.
    /// Pushes a fresh `term::Config` through `set_options` — the
    /// only field that actually changes is `scrolling_history`, but
    /// alacritty's API takes the whole Config so we build it from
    /// the default each call. Shrinking drops the oldest scrollback
    /// rows immediately; growing leaves new room at the top of the
    /// buffer for the next inbound chunk.
    /// Current (filled, max) scrollback line count. The status bar
    /// uses it to render the `X/Y` indicator. Filled comes from
    /// alacritty's grid (rows currently held above the visible
    /// area); max is the configured `scrolling_history`.
    pub fn scrollback_state(&self) -> (usize, usize) {
        (self.lines_received, self.scrollback_lines)
    }

    pub fn set_scrollback_lines(&mut self, lines: usize, cx: &mut Context<Self>) {
        if self.scrollback_lines == lines {
            return;
        }
        self.scrollback_lines = lines;
        // Re-cap the live session counter against the new
        // ceiling so a shrink immediately reflects in the bar.
        self.lines_received = self.lines_received.min(lines);
        let cfg = alacritty_terminal::term::Config {
            scrolling_history: lines,
            ..alacritty_terminal::term::Config::default()
        };
        self.term.set_options(cfg);
        cx.notify();
    }

    /// Mirror the global `Settings::copy_on_select` flag into the
    /// view. `AppView::apply_copy_on_select` calls this on settings
    /// changes (and on construction) so each live terminal in any
    /// window matches the latest toggle without restart.
    pub fn set_copy_on_select(&mut self, flag: bool) {
        self.copy_on_select = flag;
    }

    pub fn set_profile_settings(&mut self, settings: ProfileSettings) {
        // Sync the hex-view formatter with the new setting. Toggling
        // ON resets to a fresh formatter (offset 0, empty buffer);
        // toggling OFF drops the formatter so subsequent bytes feed
        // raw. Mid-session toggles via Save would land here too if
        // we ever reconnect from inside the editor.
        match (settings.hex_view, self.hex_formatter.is_some()) {
            (true, false) => self.hex_formatter = Some(HexFormatter::new()),
            (false, true) => self.hex_formatter = None,
            _ => {}
        }
        // Same toggle pattern for the timestamp injector.
        match (settings.timestamps, self.timestamps_state.is_some()) {
            (true, false) => self.timestamps_state = Some(TimestampInjector::new()),
            (false, true) => self.timestamps_state = None,
            _ => {}
        }
        // Same toggle pattern for the line-number injector. Toggling
        // ON resets the counter to 1 (a fresh `LineNumberInjector`);
        // toggling OFF drops the state so re-enabling later restarts
        // from 1 — matches what the user expects from a session-
        // local counter.
        match (settings.line_numbers, self.line_numbers_state.is_some()) {
            (true, false) => self.line_numbers_state = Some(LineNumberInjector::new()),
            (false, true) => self.line_numbers_state = None,
            _ => {}
        }
        self.profile_settings = settings;
    }

    /// Window-coords bounds of the terminal pane (the outer div in
    /// `render`). Computed by inflating the painted grid bounds by
    /// `GRID_PADDING_PX` since the grid sits inside that padding.
    /// `None` until the first paint populates `grid_bounds`.
    fn pane_bounds(&self) -> Option<Bounds<Pixels>> {
        let grid = self.grid_bounds.get()?;
        Some(Bounds {
            origin: PixelPoint {
                x: grid.origin.x - px(GRID_PADDING_PX),
                y: grid.origin.y - px(GRID_PADDING_PX),
            },
            size: gpui::Size {
                width: grid.size.width + px(GRID_PADDING_PX * 2.0),
                height: grid.size.height + px(GRID_PADDING_PX * 2.0),
            },
        })
    }

    /// Translate a window-Y mouse coord into a target alacritty
    /// `display_offset` for the current pane height + scrollback
    /// state. Mirrors the inverse of `scroll_indicator`'s thumb
    /// math. Returns the delta (positive = scroll up, negative =
    /// scroll down) to apply via `Scroll::Delta`.
    fn scrollbar_drag_delta(&self, mouse_y_window: Pixels, grab_offset: Pixels) -> Option<i32> {
        let pane = self.pane_bounds()?;
        let g = self.term.grid();
        let history = g.history_size();
        if history == 0 {
            return None;
        }
        let screen = g.screen_lines();
        let total = (history + screen) as f32;
        let pane_h = f32::from(pane.size.height);
        let pane_y = f32::from(pane.origin.y);
        let thumb_h_px = pane_h * (screen as f32 / total).max(0.08);
        let max_top_px = (pane_h - thumb_h_px).max(0.0);
        if max_top_px <= 0.0 {
            return None;
        }
        let rel_y = f32::from(mouse_y_window) - pane_y;
        let target_top_px = (rel_y - f32::from(grab_offset)).clamp(0.0, max_top_px);
        let normalized = target_top_px / max_top_px;
        // 0 = top of track ⇒ oldest history; 1 = bottom ⇒ live screen.
        // alacritty's display_offset runs the OPPOSITE direction
        // (0 = live, history = oldest), so flip.
        let target_offset = ((1.0 - normalized) * history as f32).round() as i32;
        let delta = target_offset - g.display_offset() as i32;
        if delta == 0 {
            None
        } else {
            Some(delta)
        }
    }

    fn handle_scrollbar_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left {
            return;
        }
        // Stop propagation so the terminal beneath doesn't treat
        // this as a selection-start click.
        cx.stop_propagation();
        let Some(pane) = self.pane_bounds() else {
            return;
        };
        let Some(geom) = self.scroll_indicator() else {
            return;
        };
        let pane_h = f32::from(pane.size.height);
        let thumb_h_px = pane_h * geom.thumb_height_pct;
        let thumb_top_px = pane_h * geom.thumb_top_pct;
        let rel_y = f32::from(event.position.y) - f32::from(pane.origin.y);
        // If the click landed on the thumb, preserve the grab
        // offset so the thumb tracks the cursor without jumping.
        // Otherwise (track click), centre the thumb on the click
        // point — matches platform-default behaviour for "click
        // empty track to jump there".
        let grab_offset = if rel_y >= thumb_top_px && rel_y <= thumb_top_px + thumb_h_px {
            px(rel_y - thumb_top_px)
        } else {
            px(thumb_h_px / 2.0)
        };
        self.scrollbar_drag = Some(grab_offset);
        self.apply_scrollbar_drag(event.position.y, cx);
    }

    fn apply_scrollbar_drag(&mut self, mouse_y_window: Pixels, cx: &mut Context<Self>) {
        let Some(grab) = self.scrollbar_drag else {
            return;
        };
        let Some(delta) = self.scrollbar_drag_delta(mouse_y_window, grab) else {
            return;
        };
        self.term.scroll_display(Scroll::Delta(delta));
        mirror_to_grid(
            &self.term,
            &mut self.grid,
            &self.palette,
            self.cursor_blink_phase,
        );
        cx.notify();
    }

    /// Compute the on-screen scroll-indicator geometry from the
    /// alacritty grid's current state. `None` when there's no
    /// scrollback yet (the indicator stays hidden until the user
    /// has actually scrolled past the live screen). Geometry is
    /// returned as fractions (0.0 – 1.0) so the renderer can
    /// scale to whatever track height it wants.
    fn scroll_indicator(&self) -> Option<ScrollIndicator> {
        let g = self.term.grid();
        let history = g.history_size();
        if history == 0 {
            return None;
        }
        let screen = g.screen_lines();
        let offset = g.display_offset();
        let total = (history + screen) as f32;
        // Thumb height = visible fraction of the total scrollable
        // content. Floor at 8% so the thumb stays grabbable on very
        // long sessions (a 10000-line `show tech-support` would
        // otherwise produce a sub-pixel thumb).
        let thumb_h = ((screen as f32) / total).max(0.08);
        let max_top = 1.0 - thumb_h;
        // Alacritty's `display_offset` runs 0 (live screen, viewport
        // at the BOTTOM of history) to `history` (viewport at the
        // TOP of history). Map onto thumb position so 0 ⇒ thumb at
        // bottom, history ⇒ thumb at top.
        let top = (1.0 - (offset as f32 / history as f32)) * max_top;
        Some(ScrollIndicator {
            thumb_top_pct: top,
            thumb_height_pct: thumb_h,
        })
    }

    /// Wipe the visible grid + scrollback. Implemented by feeding
    /// the standard "clear + cursor home" VT sequence through the
    /// parser — purely local, doesn't touch the wire (so a
    /// connected device's shell isn't affected). The screen is
    /// blank afterwards but the device is still feeding bytes,
    /// so a long `show running-config` mid-clear keeps streaming.
    pub fn clear_screen(&mut self, cx: &mut Context<Self>) {
        // Reset the hex formatter's offset / partial buffer so the
        // next chunk starts from `00000000` again — clearing the
        // visible grid without resetting offset would leave a
        // confusing gap.
        if let Some(f) = self.hex_formatter.as_mut() {
            f.reset();
        }
        // `\x1b[3J` — clear scrollback (xterm extension; alacritty
        // honours it). `\x1b[2J` — clear visible grid. `\x1b[H` —
        // cursor home. Order matters: clear scrollback first so
        // the live grid lines aren't pushed into a freshly-emptied
        // history. Bypass `feed_bytes` so the sequence isn't itself
        // rendered as a hex dump when hex view is on.
        self.feed_terminal_raw(b"\x1b[3J\x1b[2J\x1b[H", cx);
    }

    /// Feed a chunk of bytes through the VT parser, then re-mirror
    /// the resulting grid into the render-side cells. Used both for
    /// the boot-time sample stream and for typed-input loopback.
    /// `cx.notify()` triggers a re-render.
    pub fn feed_bytes(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        // Count newlines for the status bar's session-line count.
        // alacritty's `history_size()` (the old `scrollback_state`
        // numerator) reports rows above the visible viewport —
        // not "lines received" — so it stays at 0 until the user
        // scrolls past the viewport's row capacity. Tracking the
        // raw `\n` byte tally here gives the bar an
        // incrementing-from-0 counter even on a fresh session,
        // capped at `scrollback_lines` to match the buffer's
        // own retention. Don't double-count during replay (the
        // highlight-rule replay path feeds the same bytes back
        // through this function after edits).
        if !self.replaying {
            let new_lines = bytes.iter().filter(|&&b| b == b'\n').count();
            self.lines_received = self
                .lines_received
                .saturating_add(new_lines)
                .min(self.scrollback_lines);
        }
        // Three optional transforms run in series before the bytes
        // hit the VT parser:
        //   1. Hex view: 16-byte-per-row hex dump (own line layout).
        //   2. Highlight: regex-driven ANSI colour wrappers per
        //      complete line. Skipped when hex_view is on.
        //   3. Timestamps: prepend `[HH:MM:SS.mmm] ` to each line
        //      that's about to start.
        // Order matters — timestamps AFTER hex+highlight means each
        // emitted line gets one stamp, and highlight runs on the
        // device's own line content (without our timestamp prefix
        // confusing the regex set).
        // All three off → raw passthrough.
        // Record raw device bytes for the highlight-rule-replay
        // path. Skipped while replaying so we don't grow the
        // buffer in a feedback loop. Bounded so a firehose
        // (`debug all` on a chatty router) doesn't grow without
        // bound — when we exceed the cap, drop the oldest bytes.
        if !self.replaying {
            self.session_bytes.extend_from_slice(bytes);
            if self.session_bytes.len() > SESSION_REPLAY_MAX_BYTES {
                let drop_n = self.session_bytes.len() - SESSION_REPLAY_MAX_BYTES;
                self.session_bytes.drain(..drop_n);
            }
        }
        let (hex_bytes, hex_str_held) = if let Some(f) = self.hex_formatter.as_mut() {
            let s = f.feed(bytes);
            // Borrow-checker dance: hold the String so the slice
            // returned alongside lives long enough.
            let owned = s;
            (owned.as_bytes().to_vec(), Some(owned))
        } else {
            (bytes.to_vec(), None)
        };
        let highlighted_bytes = if self.hex_formatter.is_some() {
            // Hex view: skip highlight to keep the dump clean.
            hex_bytes
        } else if let Some(h) = self.highlight.as_mut() {
            h.feed(&hex_bytes)
        } else {
            hex_bytes
        };
        let stamped_bytes = if let Some(ts) = self.timestamps_state.as_mut() {
            ts.feed(&highlighted_bytes)
        } else {
            highlighted_bytes
        };
        // Line numbers run AFTER timestamps so a stamped line has
        // its number on the very left: `   42  [HH:MM:SS] line`.
        let final_bytes = if let Some(ln) = self.line_numbers_state.as_mut() {
            ln.feed(&stamped_bytes)
        } else {
            stamped_bytes
        };
        drop(hex_str_held);
        self.feed_terminal_raw(&final_bytes, cx);
        if self.hex_formatter.is_some() {
            self.schedule_hex_flush(cx);
        }
        if self.highlight.is_some() && self.hex_formatter.is_none() {
            self.schedule_highlight_flush(cx);
        }
    }

    /// Re-arm the hex formatter's partial-line idle flush. Cancels
    /// any prior pending flush by replacing the task field.
    fn schedule_hex_flush(&mut self, cx: &mut Context<Self>) {
        let task = cx.spawn(async move |weak, cx| {
            cx.background_executor()
                .timer(HEX_PARTIAL_FLUSH_DELAY)
                .await;
            weak.update(cx, |this, cx| {
                let hex_line = this
                    .hex_formatter
                    .as_mut()
                    .map(|f| f.flush_partial())
                    .unwrap_or_default();
                if !hex_line.is_empty() {
                    // Same chain as `feed_bytes` — keep the
                    // timestamp pass in sync with the hex pass so
                    // a partial-line flush gets stamped too.
                    let stamped = if let Some(ts) = this.timestamps_state.as_mut() {
                        ts.feed(hex_line.as_bytes())
                    } else {
                        hex_line.into_bytes()
                    };
                    let numbered = if let Some(ln) = this.line_numbers_state.as_mut() {
                        ln.feed(&stamped)
                    } else {
                        stamped
                    };
                    this.feed_terminal_raw(&numbered, cx);
                }
                this.hex_flush_task = None;
            })
            .ok();
        });
        self.hex_flush_task = Some(task);
    }

    /// Re-arm the highlight buffer's partial-line idle flush.
    /// 30ms is short enough that single-byte typing echoes feel
    /// instant and long enough that bytes arriving in close
    /// succession coalesce into a complete line before the
    /// regex set runs against them. Without this the partial
    /// after the last `\n` would never appear (prompts hang).
    fn schedule_highlight_flush(&mut self, cx: &mut Context<Self>) {
        let task = cx.spawn(async move |weak, cx| {
            cx.background_executor()
                .timer(HIGHLIGHT_PARTIAL_FLUSH_DELAY)
                .await;
            weak.update(cx, |this, cx| {
                let partial = this
                    .highlight
                    .as_mut()
                    .map(|h| h.flush_partial())
                    .unwrap_or_default();
                if !partial.is_empty() {
                    let stamped = if let Some(ts) = this.timestamps_state.as_mut() {
                        ts.feed(&partial)
                    } else {
                        partial
                    };
                    let numbered = if let Some(ln) = this.line_numbers_state.as_mut() {
                        ln.feed(&stamped)
                    } else {
                        stamped
                    };
                    this.feed_terminal_raw(&numbered, cx);
                }
                this.highlight_flush_task = None;
            })
            .ok();
        });
        self.highlight_flush_task = Some(task);
    }

    /// Feed bytes directly to the alacritty VT parser, bypassing
    /// the hex formatter. Used by `clear_screen` (so the
    /// `\x1b[3J\x1b[2J\x1b[H` sequence isn't itself rendered as
    /// hex) and as the final stage of `feed_bytes` after the
    /// hex transform.
    fn feed_terminal_raw(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
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
            &self.palette,
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
            px(self.grid.font_size_px()),
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
            ScrollDelta::Pixels(p) => f32::from(p.y) / self.grid.cell_h_px(),
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
            &self.palette,
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
        // Click count picks the selection granularity, matching the
        // universal terminal convention:
        //   1 click  → character-level (drag to extend)
        //   2 clicks → the word under the cursor — alacritty's
        //              `Semantic` mode, bounded by its semantic-
        //              escape character set
        //   3 clicks → the line's printed content
        // gpui bumps `click_count` for rapid repeats in the same
        // spot; 4+ falls through to `Simple` so a stray extra
        // click drops back to a fresh character selection.
        //
        // Triple-click does NOT use alacritty's `SelectionType::Lines`.
        // That type spans the whole row including every trailing
        // blank cell, and `selection_to_string` tacks a `\n` onto
        // the result — on a serial terminal that newline submits
        // the line the moment it's pasted. Instead we scan the row
        // for its last printed (non-space) column and build a plain
        // `Simple` range from column 0 to there: the highlight
        // stops at the visible text and the copied string carries
        // no trailing newline.
        self.triple_click_selection = event.click_count >= 3;
        if self.triple_click_selection {
            let line = point.line;
            let cols = self.term.columns();
            let row = &self.term.grid()[line];
            // Highest column on the line that holds a non-space
            // glyph. `None` ⇒ the line is entirely blank, in which
            // case the selection collapses to a single cell at
            // column 0 (empty — `copy_selection` filters it out).
            let last_content = (0..cols).rev().find(|&i| row[Column(i)].c != ' ');
            let mut sel =
                Selection::new(SelectionType::Simple, GridPoint::new(line, Column(0)), Side::Left);
            if let Some(last) = last_content {
                sel.update(GridPoint::new(line, Column(last)), Side::Right);
            }
            self.term.selection = Some(sel);
        } else {
            // `Semantic` (double-click) covers its word the moment
            // it's created — no drag needed — and a subsequent drag
            // extends word-by-word through `sel.update()`. `Simple`
            // (single-click) starts empty and grows character-wise.
            let sel_type = match event.click_count {
                2 => SelectionType::Semantic,
                _ => SelectionType::Simple,
            };
            self.term.selection = Some(Selection::new(sel_type, point, Side::Left));
        }
        // `is_dragging` stays true for every click count so the
        // mouse-up handler treats it as a finished selection for
        // copy-on-select. `handle_mouse_move` separately checks
        // `triple_click_selection` and skips drag-extension for
        // the triple-click case.
        self.is_dragging = true;
        mirror_to_grid(
            &self.term,
            &mut self.grid,
            &self.palette,
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
        // Scrollbar drag wins over selection drag — they're
        // mutually exclusive (mouse_down sets one or the other).
        if self.scrollbar_drag.is_some() {
            self.apply_scrollbar_drag(event.position.y, cx);
            return;
        }
        if !self.is_dragging {
            return;
        }
        // A triple-click line selection is pinned to the line's
        // printed extent; dragging must not extend it (a `Simple`
        // range would just collapse toward the cursor). The button
        // is still "down" so `is_dragging` stays set for the
        // mouse-up / copy-on-select path — we only skip the
        // extend.
        if self.triple_click_selection {
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
            &self.palette,
            self.cursor_blink_phase,
        );
        cx.notify();
    }

    /// Right-mouse-down handler. Opens the terminal context menu
    /// anchored at the cursor. Selection state is preserved — a
    /// right-click that lands inside an active drag-selection keeps
    /// the selection alive so the user can right-click → Copy. The
    /// `stop_propagation` keeps the AppView root's mouse-up
    /// catch-all (which dismisses any open popup) from firing on
    /// the same gesture and immediately closing the menu we just
    /// opened.
    fn handle_right_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Right {
            return;
        }
        cx.stop_propagation();
        self.context_menu_pos = Some(event.position);
        cx.notify();
    }

    /// Tear down the right-click context menu. Called by the menu
    /// rows themselves after running their action, and by
    /// `AppView`'s outer click-outside catch-all (via
    /// `shortcut_dismiss_terminal_context_menu`). Safe to call
    /// when the menu is already closed.
    pub fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        if self.context_menu_pos.take().is_some() {
            cx.notify();
        }
    }

    /// Build the right-click context menu overlay. Returns `None`
    /// when the menu isn't open. The panel renders as a
    /// `deferred(anchored(pos))` overlay so it sits above the
    /// terminal grid + scrollbar thumb and pins to the cursor
    /// coordinate captured by `handle_right_mouse_down`.
    fn render_context_menu(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let pos = self.context_menu_pos?;
        let s = *cx.global::<crate::skin_tokens::SkinTokens>();
        // Disable Copy when there's nothing selected — the action
        // would no-op but a greyed entry communicates "you'd need
        // to drag-select first" without the user having to click.
        let has_selection = self
            .term
            .selection_to_string()
            .filter(|t| !t.is_empty())
            .is_some();
        // Rows are uniform — single label, single click handler,
        // optional dim state. The closure shape matches what
        // `gpui::div().child(...)` expects, so each row is a
        // standalone Div.
        type ClickHandler =
            Box<dyn Fn(&MouseUpEvent, &mut Window, &mut gpui::App) + 'static>;
        let row = move |label: &'static str,
                        enabled: bool,
                        on_click: ClickHandler|
              -> AnyElement {
            let hover_bg = s.bg_hover;
            let d = div()
                .id(SharedString::from(label))
                .px_3()
                .py(px(6.0))
                .text_color(rgba(s.fg_primary))
                .text_size(px(13.0))
                .child(label);
            if enabled {
                d.cursor_pointer()
                    .hover(move |st| st.bg(rgba(hover_bg)))
                    .on_mouse_up(MouseButton::Left, on_click)
                    .into_any_element()
            } else {
                d.opacity(0.45).into_any_element()
            }
        };
        // Each row: stop_propagation to keep the AppView root
        // catch-all from re-firing dismiss after we've already
        // run the action; then dispatch through the same path
        // the keybindings use (entity.update on the terminal)
        // so the action + menu surfaces stay equivalent.
        let copy_row = {
            let entity = cx.entity();
            row(
                "Copy",
                has_selection,
                Box::new(move |_, _window, app| {
                    entity.update(app, |this, cx| {
                        cx.stop_propagation();
                        this.copy_selection(cx);
                        this.close_context_menu(cx);
                    });
                }),
            )
        };
        let paste_row = {
            let entity = cx.entity();
            row(
                "Paste",
                true,
                Box::new(move |_, window, app| {
                    entity.update(app, |this, cx| {
                        cx.stop_propagation();
                        this.paste_clipboard(window, cx);
                        this.close_context_menu(cx);
                    });
                }),
            )
        };
        let select_all_row = {
            let entity = cx.entity();
            row(
                "Select All",
                true,
                Box::new(move |_, _window, app| {
                    entity.update(app, |this, cx| {
                        cx.stop_propagation();
                        this.select_all(cx);
                        this.close_context_menu(cx);
                    });
                }),
            )
        };
        let clear_row = {
            let entity = cx.entity();
            row(
                "Clear",
                true,
                Box::new(move |_, _window, app| {
                    entity.update(app, |this, cx| {
                        cx.stop_propagation();
                        this.clear_screen(cx);
                        this.close_context_menu(cx);
                    });
                }),
            )
        };
        // Match `app_view::profile_context_menu_overlay`'s
        // two-layer paint: opaque `bg_window` base for legibility
        // over whatever sits behind, then the translucent
        // `bg_panel` skin overlay on top to match the rest of the
        // chrome's frosted look. `--shadow-floating` from the skin
        // wins over the default `shadow_md` when authored.
        let shadow_floating = cx
            .global::<crate::skin_tokens::SkinShadows>()
            .floating
            .clone();
        let panel = div()
            .min_w(px(160.0))
            .bg(rgba(s.bg_window))
            .border_1()
            .border_color(rgba(s.border_subtle))
            .rounded(px(s.radius_md))
            .map(|this| {
                if shadow_floating.is_empty() {
                    this.shadow_md()
                } else {
                    this.shadow(shadow_floating.clone())
                }
            })
            .child(
                div()
                    .w_full()
                    .bg(rgba(s.bg_panel))
                    .rounded(px(s.radius_md))
                    .py_1()
                    .child(copy_row)
                    .child(paste_row)
                    .child(select_all_row)
                    .child(clear_row),
            );
        Some(deferred(anchored().position(pos).child(panel)).into_any_element())
    }

    fn handle_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left {
            return;
        }
        // Did this release end a selection drag (vs. just a stray
        // mouse-up on the scrollbar)? Capture before clearing so
        // the copy-on-select branch below knows whether to fire.
        let drag_ended = self.is_dragging;
        // Clear scrollbar drag first (no-op if it wasn't set).
        // Selection state survives the release so the user can
        // copy it with Cmd-C / Ctrl-Shift-C; cleared on the next
        // mouse_down (a fresh click drops the prior selection).
        self.scrollbar_drag = None;
        self.is_dragging = false;
        // PuTTY-style copy-on-select. When the global setting is
        // on, releasing the mouse after a drag copies the new
        // selection straight to the clipboard — no Cmd+C needed.
        // Skipped when the release wasn't ending a drag (e.g. a
        // stray button-up without a matching button-down on the
        // grid). `copy_selection` itself no-ops when the selection
        // is empty (single-click without drag), so the check
        // collapses to "did the user just finish dragging some
        // text?" without us having to inspect selection extents.
        if drag_ended && self.copy_on_select {
            self.copy_selection(cx);
        }
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
        let display_row = (local_y / self.grid.cell_h_px()).floor() as i32;
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

    /// Copy the current alacritty selection to the system
    /// clipboard. No-op when no selection is active or the
    /// selection text is empty. Called from:
    ///   * the `TerminalCopy` action handler in `app_view`
    ///     (dispatched by Cmd+C / Ctrl+C / Ctrl+Shift+C);
    ///   * the right-click context menu's Copy row; and
    ///   * `handle_mouse_up` when copy-on-select is enabled.
    pub fn copy_selection(&mut self, cx: &mut App) {
        if let Some(text) = self.term.selection_to_string().filter(|s| !s.is_empty()) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    /// Build a selection that spans every cell from the top of
    /// the scrollback buffer to the bottom-right of the visible
    /// viewport. Mirrors the standard "Select All" semantics in
    /// xterm / iTerm2 / Windows Terminal: a single Cmd+A / Ctrl+A
    /// is meant to capture the entire scrollback session, not just
    /// what fits on screen.
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        let cols = self.term.columns();
        let total_lines = self.term.screen_lines();
        let history = self.term.grid().history_size() as i32;
        if cols == 0 || total_lines == 0 {
            return;
        }
        let start = GridPoint::new(Line(-history), Column(0));
        let end = GridPoint::new(
            Line(total_lines as i32 - 1),
            Column(cols.saturating_sub(1)),
        );
        let mut sel = Selection::new(SelectionType::Simple, start, Side::Left);
        sel.update(end, Side::Right);
        self.term.selection = Some(sel);
        mirror_to_grid(
            &self.term,
            &mut self.grid,
            &self.palette,
            self.cursor_blink_phase,
        );
        cx.notify();
    }

    pub fn paste_clipboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        // Chrome overhead: bytes the window doesn't give to the
        // terminal grid. Hardcoded against the current AppView
        // layout — sidebar takes horizontal width, title bar +
        // session header + status bar take vertical height. When
        // the layout shifts (e.g. multi-window, removable header)
        // this needs to come from real measured pane bounds
        // instead. Treat these numbers as "what gpui actually
        // paints those rows at" — counted from running the app,
        // not from padding spec, so they include text-line-height
        // fudge.
        const SIDEBAR_PX: f32 = 220.0;
        const SESSION_HEADER_PX: f32 = 50.0;
        // Status bar measures ~24px, but giving the grid an extra
        // ~16px of room above it keeps the prompt from sitting
        // flush against the footer (Tauri version does the same).
        const STATUS_BAR_PX: f32 = 40.0;
        // Title bar height is skin-driven now (38-46px depending
        // on `--titlebar-height`). On floating-card skins
        // (`--panel-radius > 0`) the title bar is an absolute
        // overlay that takes zero vertical space — the pane row
        // pads itself by `--shell-padding` top + bottom instead.
        // Subtracting the right value here keeps the bottom row
        // of the terminal viewport visible regardless of skin.
        let s = *cx.global::<crate::skin_tokens::SkinTokens>();
        let title_bar_h = if s.panel_radius_px > 0.0 {
            s.shell_padding_px * 2.0
        } else {
            s.titlebar_height_px
        };
        let chrome_w = SIDEBAR_PX;
        let chrome_h = title_bar_h + SESSION_HEADER_PX + STATUS_BAR_PX;
        let content_w =
            (f32::from(viewport.width) - chrome_w - GRID_PADDING_PX * 2.0).max(0.0);
        let content_h =
            (f32::from(viewport.height) - chrome_h - GRID_PADDING_PX * 2.0).max(0.0);
        let new_cols = ((content_w / cell_w).floor() as usize).max(MIN_COLS);
        let new_rows = ((content_h / self.grid.cell_h_px()).floor() as usize).max(MIN_ROWS);
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
            &self.palette,
            self.cursor_blink_phase,
        );
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Clipboard + select-all shortcuts no longer live here.
        // They're registered as gpui actions (TerminalCopy /
        // TerminalPaste / TerminalSelectAll) with `Some("Terminal")`
        // context in `apply_shortcut_bindings`, and the terminal
        // div sets `.key_context("Terminal")` so gpui's keymap
        // dispatches the action before this raw key handler ever
        // runs. The earlier inline `m.platform && key == "c"`
        // check worked on macOS when the focus chain held its
        // ground, but it was fragile — any change to focus
        // routing or event ordering could silently drop the
        // shortcut. Action dispatch is the idiomatic path and is
        // shared with the right-click context menu so both UX
        // surfaces invoke exactly the same code.

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
            &self.palette,
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
        let indicator = self.scroll_indicator();
        div()
            .size_full()
            // The grid's intrinsic size (rows × cell_h) is computed
            // from the full window viewport, but our actual render
            // area is smaller once chrome (sidebar, session header,
            // status bar) takes its share. Without `overflow_hidden`
            // the bottom row(s) of the grid leak past the terminal
            // pane and bleed into the status bar. Real fix is to
            // teach `maybe_resize` about the actual pane bounds; for
            // now this is the cheap visual containment.
            .overflow_hidden()
            // `relative` so the scroll-indicator overlay below can
            // anchor to this div with absolute positioning.
            .relative()
            .bg(rgb(pack(self.palette.bg)))
            .p(px(GRID_PADDING_PX))
            // Key context — gates the `TerminalCopy`, `TerminalPaste`,
            // and `TerminalSelectAll` KeyBindings (registered with
            // `Some("Terminal")` in `apply_shortcut_bindings`) so
            // they fire only when this div is in the focus chain.
            // Stops Cmd+C from grabbing a profile-form Input's text
            // when the user happens to have the terminal pane open
            // in a different window.
            .key_context("Terminal")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .on_scroll_wheel(cx.listener(Self::handle_scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
            // Right-click opens the Copy / Paste / Select All /
            // Clear context menu anchored at the cursor — see
            // `handle_right_mouse_down` for why this lives on
            // mouse_down rather than mouse_up (matches the macOS
            // / Windows / Linux convention of "menu appears on
            // press, not release").
            .on_mouse_down(MouseButton::Right, cx.listener(Self::handle_right_mouse_down))
            .on_mouse_move(cx.listener(Self::handle_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
            .child(self.grid.element(
                cell_w,
                self.bell_flash_active(),
                self.grid_bounds.clone(),
            ))
            // Right-click context menu overlay. Painted only when
            // `context_menu_pos` is `Some`; `deferred` so it draws
            // above sibling chrome (scrollbar thumb, etc.) and
            // `anchored` so it pins to the cursor coordinate.
            .children(self.render_context_menu(cx))
            // Scroll-indicator overlay on the right edge. Mirrors
            // alacritty's display_offset / history_size so the
            // thumb position reflects the user's place in the
            // scrollback. Hidden when there's no scrollback yet.
            // The overlay carries a mouse_down handler so the user
            // can drag the thumb (or click the empty track) to
            // jump in the scrollback; mouse_move + mouse_up fall
            // through to the terminal's existing handlers, which
            // short-circuit on `scrollbar_drag = Some`.
            .children(indicator.map(|ind| {
                div()
                    .absolute()
                    .top(relative(ind.thumb_top_pct))
                    .right_1()
                    .w(px(SCROLLBAR_WIDTH_PX))
                    .h(relative(ind.thumb_height_pct))
                    .bg(rgba(SCROLLBAR_THUMB))
                    .rounded_sm()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(Self::handle_scrollbar_mouse_down),
                    )
            }))
    }
}

/// Pixel width of the scrollback indicator on the right edge.
/// Matches gpui-component's `THUMB_ACTIVE_WIDTH` (8px) so the
/// terminal's scrollbar has the same visual weight as the form
/// pane's. Wide enough to be a comfortable drag target without
/// obscuring more than ~1 cell-column of grid content beneath.
const SCROLLBAR_WIDTH_PX: f32 = 8.0;
/// Thumb colour. White at ~30% alpha — visible against the
/// terminal's dark bg without competing with cell text.
const SCROLLBAR_THUMB: u32 = 0xFFFFFF4D;

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

/// How long the hex-view partial-line buffer waits for more
/// bytes before flushing what's there. 100ms matches the Tauri
/// `hexdump.ts` setting; long enough that a chunky `show
/// running-config` stays aligned at 16 per row, short enough that
/// a single-byte prompt echo (Enter, then idle) becomes visible
/// before the user gets impatient.
const HEX_PARTIAL_FLUSH_DELAY: Duration = Duration::from_millis(100);

/// Idle window before the highlight buffer's partial tail (a
/// chunk that hasn't seen its `\n` yet — typing echo, prompts)
/// gets emitted. 30ms ≈ 2 frames at 60Hz: short enough that the
/// keystroke echo feels immediate, long enough that a flurry of
/// bytes arriving close together (the device emitting a long
/// reply) coalesces into a complete line for the regex set.
const HIGHLIGHT_PARTIAL_FLUSH_DELAY: Duration = Duration::from_millis(30);

/// Cap on the per-session raw-byte buffer used to replay history
/// through a fresh highlight rule set when the user toggles packs
/// in Settings. 1 MiB ≈ 12-15k typical lines — enough to cover
/// the visible scrollback (default 10k lines) plus a margin,
/// with a hard ceiling so an open-ended `debug all` session
/// can't grow without bound.
const SESSION_REPLAY_MAX_BYTES: usize = 1024 * 1024;

/// Streaming `xxd`-style hex dump formatter. Mirrors the Tauri
/// version's `src/lib/hexdump.ts`: 16 bytes per line with a gap
/// after byte 8, 8-digit hex offset, ASCII gutter on the right
/// (printable bytes 0x20..=0x7e, others as `.`).
///
/// Partial lines (anything less than 16 bytes at the tail of the
/// buffer) stay in the buffer across `feed` calls. The owner is
/// expected to schedule a `flush_partial` call after a quiet
/// period (typically ~100ms) so single-byte chunks like keyboard
/// echo eventually surface, but contiguous streaming output still
/// gets the proper 16-byte-per-row layout.
pub(crate) struct HexFormatter {
    offset: usize,
    /// Bytes accumulated for the current line (carried across
    /// `feed` calls so a chunk that doesn't end on a 16-byte
    /// boundary can be continued by the next chunk).
    buffer: Vec<u8>,
}

impl HexFormatter {
    const BYTES_PER_LINE: usize = 16;

    pub(crate) fn new() -> Self {
        Self {
            offset: 0,
            buffer: Vec::with_capacity(Self::BYTES_PER_LINE),
        }
    }

    /// Reset offset + drop partial buffer. Called by `clear_screen`
    /// so the next chunk starts from `00000000`.
    pub(crate) fn reset(&mut self) {
        self.offset = 0;
        self.buffer.clear();
    }

    /// Append `bytes` and return the formatted lines to feed to the
    /// VT parser. Emits ONLY complete 16-byte rows; whatever's left
    /// stays in the buffer for the next call (or for
    /// `flush_partial`).
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> String {
        let mut out = String::new();
        for &b in bytes {
            self.buffer.push(b);
            if self.buffer.len() >= Self::BYTES_PER_LINE {
                let line: Vec<u8> = self.buffer.drain(..Self::BYTES_PER_LINE).collect();
                out.push_str(&self.emit_line(&line));
            }
        }
        out
    }

    /// Emit whatever's in the partial buffer as a (short) line, or
    /// an empty string if the buffer is empty. Used by the
    /// idle-flush timer in `TerminalView` so a short chunk that
    /// doesn't fill a row eventually shows up.
    pub(crate) fn flush_partial(&mut self) -> String {
        if self.buffer.is_empty() {
            return String::new();
        }
        let line = std::mem::take(&mut self.buffer);
        self.emit_line(&line)
    }

    fn emit_line(&mut self, bytes: &[u8]) -> String {
        let mut hex = String::new();
        for i in 0..Self::BYTES_PER_LINE {
            // Extra space at the half-line boundary, matching xxd
            // and the Tauri formatter — gives the eye a chunking
            // anchor inside a 16-byte row.
            if i == 8 {
                hex.push(' ');
            }
            if i < bytes.len() {
                hex.push_str(&format!(" {:02x}", bytes[i]));
            } else {
                hex.push_str("   ");
            }
        }
        let ascii: String = bytes
            .iter()
            .map(|&b| {
                if (0x20..0x7f).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        let line = format!("{:08x} {}  |{}|\r\n", self.offset, hex, ascii);
        self.offset += bytes.len();
        line
    }
}

/// Inserts `[HH:MM:SS.mmm] ` (dim grey ANSI) at the start of every
/// line in the byte stream. Tracks "are we at the start of a line"
/// across `feed` calls so a chunk that arrives mid-line doesn't get
/// a stamp inserted partway through, and so a fresh `\n` arriving
/// in a later chunk arms the next stamp.
///
/// `\r` is also treated as a line break — Cisco-style `\r\r\n`
/// (where the second CR resets the cursor) and lone-CR overwrite
/// streams (progress bars) both end up with the timestamp at the
/// visible start of the next paint, which matches what the user
/// expects from the on-screen view.
pub(crate) struct TimestampInjector {
    at_line_start: bool,
}

impl TimestampInjector {
    pub(crate) fn new() -> Self {
        Self {
            at_line_start: true,
        }
    }

    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(bytes.len() + 32);
        for &b in bytes {
            if self.at_line_start && b != b'\n' && b != b'\r' {
                out.extend_from_slice(format_timestamp().as_bytes());
                self.at_line_start = false;
            }
            out.push(b);
            if b == b'\n' || b == b'\r' {
                self.at_line_start = true;
            }
        }
        out
    }
}

/// Line-number injector. Same line-state machine as
/// `TimestampInjector` — flips an `at_line_start` flag on `\n` /
/// `\r` and prepends a dim-grey 5-column-padded number on the next
/// non-newline byte — but with a counter that bumps per prepended
/// line. Counter is session-local: `apply_profile_settings` and
/// `replay_session_into_grid` each construct a fresh instance, so
/// every reconnect / hex-toggle / highlight-toggle re-numbers
/// from 1, which matches the "what's the third line in this
/// session" mental model better than a cumulative counter would.
pub(crate) struct LineNumberInjector {
    at_line_start: bool,
    next_line: u32,
}

impl LineNumberInjector {
    pub(crate) fn new() -> Self {
        Self {
            at_line_start: true,
            next_line: 1,
        }
    }

    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(bytes.len() + 16);
        for &b in bytes {
            if self.at_line_start && b != b'\n' && b != b'\r' {
                out.extend_from_slice(format_line_number(self.next_line).as_bytes());
                self.next_line = self.next_line.saturating_add(1);
                self.at_line_start = false;
            }
            out.push(b);
            if b == b'\n' || b == b'\r' {
                self.at_line_start = true;
            }
        }
        out
    }
}

/// Format a line number as a dim-grey 5-column-padded prefix with
/// two trailing spaces. 5 columns handles up to 99999 lines without
/// jitter — beyond that the column grows but the alignment within
/// each "decade" stays stable, which matches `less -N`'s behaviour.
fn format_line_number(n: u32) -> String {
    format!("\x1b[90m{:>5}\x1b[0m  ", n)
}

/// Format the current wall-clock time as a dim-grey bracketed
/// prefix. Same shape the Tauri build's `formatTimestamp` uses
/// (`src/lib/highlight.ts`), so a session log captured here looks
/// the same as one captured there.
fn format_timestamp() -> String {
    use chrono::{Local, Timelike};
    let now = Local::now();
    format!(
        "\x1b[90m[{:02}:{:02}:{:02}.{:03}]\x1b[0m ",
        now.hour(),
        now.minute(),
        now.second(),
        now.timestamp_subsec_millis()
    )
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
