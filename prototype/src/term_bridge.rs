//! Bridge between `alacritty_terminal` and our `TerminalGrid`.
//!
//! The split:
//!   * `alacritty_terminal::Term` owns the source-of-truth grid
//!     and the VT escape state (cursor pos, modes, charsets, etc.).
//!   * `vte::ansi::Processor` is the byte-stream parser. It calls
//!     into `Term` (via its `Handler` impl) for every glyph, escape
//!     sequence, and mode change.
//!   * Our `TerminalGrid` is the render-side view. Only carries
//!     what the renderer needs (char + concrete RGB colors); does
//!     not hold cursor state or scrollback.
//!
//! Data flow: bytes → `Processor::advance(&mut term, bytes)` →
//! `term.grid()` is the new ground truth → `mirror_to_grid` walks
//! the grid and copies cells into `TerminalGrid`, resolving each
//! cell's abstract `Color` to a concrete `Rgb` via the palette
//! resolver below.
//!
//! The resolver currently hardcodes Baudrun's default ANSI palette.
//! `Term::colors` is private with only an `&Colors` getter — we
//! can't pre-populate the live palette without going through
//! `Handler::set_color` via parsed OSC sequences, which is more
//! ceremony than the prototype warrants. Once the real theme
//! system is wired, this becomes "look up the active theme's
//! palette" without changing the function signature.
//!
//! Performance note: for the prototype we mirror the entire grid
//! after every `advance`. The proper implementation would track
//! `term.damage()` and only mirror dirty rows. Defer until we
//! have something to measure.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use alacritty_terminal::{
    event::{Event, EventListener},
    grid::Dimensions,
    term::{
        cell::{Cell as TermCell, Flags as TermFlags},
        Config, Term,
    },
    vte::ansi::{Color, CursorShape, NamedColor, Processor, Rgb},
};

use crate::terminal_grid::{Cell as GridCell, CellFlags, TerminalGrid, SELECTION_BG};

/// Shared state mutated by `TerminalListener` and read from
/// `TerminalView`. Lives behind an `Rc` so the listener (held
/// inside `Term`) and the view (which owns the `Term`) share
/// the same instance — alacritty's `EventListener::send_event`
/// takes `&self`, so any mutable state has to go through
/// interior mutability. `Cell<Option<Instant>>` is enough for
/// our current event surface (just `Bell` so far); add fields
/// here as more event variants get wired up.
#[derive(Default)]
pub struct ListenerState {
    /// Set to `Some(Instant::now())` whenever alacritty processes
    /// a BEL byte. The view drains this with `take()` after each
    /// `feed_bytes` and uses it to start a brief visual flash.
    pub bell: Cell<Option<Instant>>,
}

/// Implements alacritty's `EventListener` by stashing relevant
/// events into a shared `ListenerState`. Cheap to clone (just an
/// `Rc` bump); cloned by `make_term` so both `Term` and the view
/// get a handle to the same state.
#[derive(Clone)]
pub struct TerminalListener {
    state: Rc<ListenerState>,
}

impl EventListener for TerminalListener {
    fn send_event(&self, event: Event) {
        if matches!(event, Event::Bell) {
            self.state.bell.set(Some(Instant::now()));
        }
    }
}

/// Trivial `Dimensions` impl. `Term::new` requires this so it
/// knows the initial buffer shape; we need our own type because
/// the trait is foreign and we can't blanket-impl it on a tuple.
pub struct Dims {
    pub rows: usize,
    pub cols: usize,
}

impl Dimensions for Dims {
    /// `total_lines` for `Dimensions` is "screen + scrollback,"
    /// but `Term::new` reads `screen_lines` and `columns` only —
    /// scrollback comes from the `Config`. Returning just `rows`
    /// is fine for construction; the live grid's `Dimensions`
    /// impl reports the real total once the grid is built.
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

/// Resolve an abstract `Color` to a concrete `Rgb`. `Color::Spec`
/// passes through; `Named` and `Indexed` go through the helpers
/// below, which encode Baudrun's default ANSI palette inline.
pub fn resolve(color: Color, default_fg: Rgb, default_bg: Rgb) -> Rgb {
    match color {
        Color::Spec(rgb) => rgb,
        Color::Named(named) => named_to_rgb(named, default_fg, default_bg),
        Color::Indexed(idx) => indexed_to_rgb(idx, default_fg),
    }
}

/// Baudrun's default 16-color ANSI palette + foreground / background
/// special slots. Values copied from `builtin_themes.json`'s
/// "baudrun" theme. The match is exhaustive on the variants we
/// care about and falls through to `default_fg` for the dim*
/// slots — alacritty exposes `DimRed`, `DimGreen`, etc. that we
/// don't have distinct values for, and using the regular fg is
/// strictly better than fabricating darker shades inline.
fn named_to_rgb(named: NamedColor, default_fg: Rgb, default_bg: Rgb) -> Rgb {
    use NamedColor::*;
    const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
        Rgb { r, g, b }
    }
    match named {
        Black => rgb(0x1e, 0x1e, 0x22),
        Red => rgb(0xff, 0x69, 0x61),
        Green => rgb(0x7c, 0xd9, 0x92),
        Yellow => rgb(0xf5, 0xd7, 0x6e),
        Blue => rgb(0x6c, 0xb6, 0xff),
        Magenta => rgb(0xd7, 0x94, 0xff),
        Cyan => rgb(0x7c, 0xe0, 0xe0),
        White => rgb(0xd4, 0xd4, 0xd8),
        BrightBlack => rgb(0x4a, 0x4a, 0x52),
        BrightRed => rgb(0xff, 0x8a, 0x80),
        BrightGreen => rgb(0xa2, 0xe5, 0xb3),
        BrightYellow => rgb(0xfc, 0xe4, 0x88),
        BrightBlue => rgb(0x94, 0xcc, 0xff),
        BrightMagenta => rgb(0xe5, 0xb6, 0xff),
        BrightCyan => rgb(0xa6, 0xec, 0xec),
        BrightWhite => rgb(0xff, 0xff, 0xff),
        Foreground => default_fg,
        Background => default_bg,
        // Dim*, Bright Foreground, Cursor — fall back to default
        // fg until the real theme system carries explicit values.
        _ => default_fg,
    }
}

/// 256-color resolution: 0..16 = the named ANSI colors above,
/// 16..232 = the 6×6×6 cube, 232..256 = the 24-step grayscale ramp.
fn indexed_to_rgb(idx: u8, default_fg: Rgb) -> Rgb {
    if idx < 16 {
        // Indices 0..16 align 1:1 with NamedColor's first 16 variants.
        // `NamedColor` isn't `#[repr(u8)]` (its enum has `Foreground =
        // 256`, so the discriminant has to be at least u16), which
        // rules out a `transmute`. A small explicit match is the
        // straightforward alternative.
        use NamedColor::*;
        let named = match idx {
            0 => Black,
            1 => Red,
            2 => Green,
            3 => Yellow,
            4 => Blue,
            5 => Magenta,
            6 => Cyan,
            7 => White,
            8 => BrightBlack,
            9 => BrightRed,
            10 => BrightGreen,
            11 => BrightYellow,
            12 => BrightBlue,
            13 => BrightMagenta,
            14 => BrightCyan,
            _ => BrightWhite,
        };
        named_to_rgb(named, default_fg, default_fg)
    } else if idx < 232 {
        // 6×6×6 RGB cube. Channel ramp: [0, 95, 135, 175, 215, 255].
        let i = (idx - 16) as u32;
        let r = i / 36;
        let g = (i % 36) / 6;
        let b = i % 6;
        Rgb {
            r: cube_step(r as u8),
            g: cube_step(g as u8),
            b: cube_step(b as u8),
        }
    } else {
        // 24-step grayscale ramp: 232 = #080808, 255 = #eeeeee.
        let v = 8 + (idx - 232) * 10;
        Rgb { r: v, g: v, b: v }
    }
}

#[inline]
fn cube_step(v: u8) -> u8 {
    match v {
        0 => 0,
        1 => 95,
        2 => 135,
        3 => 175,
        4 => 215,
        _ => 255,
    }
}

/// Convenience: build a `Term` plus matching `Processor` for a
/// given grid size, plus the shared `ListenerState` the caller
/// reads bell events from. The Term's color palette starts empty;
/// resolution happens through our `resolve()` above.
pub fn make_term(
    rows: usize,
    cols: usize,
) -> (Term<TerminalListener>, Processor, Rc<ListenerState>) {
    let state = Rc::new(ListenerState::default());
    let listener = TerminalListener { state: state.clone() };
    let term = Term::new(Config::default(), &Dims { rows, cols }, listener);
    (term, Processor::new(), state)
}

/// Walk the Term's display grid and write every cell into the
/// render-side `TerminalGrid`. Resolves each cell's abstract
/// foreground / background colors via `resolve()`. The cursor's
/// cell is rendered as a block by swapping fg/bg, gated on the
/// `show_cursor` parameter so callers can drive blink (passing
/// `false` during the off-phase suppresses the inversion for
/// that frame).
pub fn mirror_to_grid(
    term: &Term<TerminalListener>,
    out: &mut TerminalGrid,
    default_fg: Rgb,
    default_bg: Rgb,
    show_cursor: bool,
) {
    let content = term.renderable_content();
    // When the user has scrolled into history, `display_iter()`
    // yields lines starting at `Line(-display_offset)` and counting
    // up. The grid's `Line` is `i32` and goes negative for
    // scrollback rows; the cursor's `Line` is in live-screen
    // coordinates and stays >= 0. To paint either into our 0-
    // indexed render grid, we add `display_offset` and treat the
    // result as the display row. Without that addition, the old
    // `as usize` cast wrap-converts negative line numbers into huge
    // values, `set_cell` silently bounds-rejects them, and
    // scrolling produces no visible change.
    let display_offset = content.display_offset as i32;
    let cursor_visible = show_cursor && !matches!(content.cursor.shape, CursorShape::Hidden);
    let cursor_display_row = content.cursor.point.line.0 + display_offset;
    let cursor_col = content.cursor.point.column.0;
    let selection = content.selection;

    for indexed in content.display_iter {
        let row_signed = indexed.point.line.0 + display_offset;
        // Defensive: `display_iter` is supposed to stay within
        // `[0, screen_lines)` after the offset adjustment, but
        // skipping a stray out-of-range row beats panicking.
        if row_signed < 0 {
            continue;
        }
        let row = row_signed as usize;
        let col = indexed.point.column.0;
        let term_cell: &TermCell = indexed.cell;
        let term_flags = term_cell.flags;

        let mut fg = resolve(term_cell.fg, default_fg, default_bg);
        let mut bg = resolve(term_cell.bg, default_fg, default_bg);
        // INVERSE / HIDDEN are handled here rather than passed
        // through as flags because they're really fg/bg mutations,
        // not paint-style attributes — a renderer that treated
        // them as flags would have to know how to "undo" them
        // around cursor / selection overrides below, which is
        // exactly the dance we're avoiding by collapsing them
        // into the resolved colours up front.
        if term_flags.contains(TermFlags::INVERSE) {
            std::mem::swap(&mut fg, &mut bg);
        }
        if term_flags.contains(TermFlags::HIDDEN) {
            fg = bg;
        }

        let is_cursor = cursor_visible
            && cursor_display_row >= 0
            && cursor_display_row as usize == row
            && col == cursor_col;
        let is_selected = selection.is_some_and(|r| r.contains(indexed.point));

        // Cursor wins over selection on its own cell — easier to
        // see where you're typing even when the cursor cell happens
        // to fall inside an active selection.
        if is_cursor {
            std::mem::swap(&mut fg, &mut bg);
        } else if is_selected {
            bg = SELECTION_BG;
        }

        let flags = CellFlags {
            bold: term_flags.contains(TermFlags::BOLD),
            italic: term_flags.contains(TermFlags::ITALIC),
            underline: term_flags
                .intersects(TermFlags::ALL_UNDERLINES),
            strikethrough: term_flags.contains(TermFlags::STRIKEOUT),
            dim: term_flags.contains(TermFlags::DIM),
        };

        out.set_cell(row, col, GridCell { ch: term_cell.c, fg, bg, flags });
    }
}
