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

use crate::data::themes::Theme;
use crate::terminal_grid::{Cell as GridCell, CellFlags, TerminalGrid};

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

/// 16 ANSI colors + foreground/background/cursor slots. Plays
/// the role the old hardcoded `named_to_rgb` match used to play —
/// but parameterised, so swapping themes is "build a new Palette
/// and hand it to the next mirror call."
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub fg: Rgb,
    pub bg: Rgb,
    pub cursor: Rgb,
    /// Background fill painted behind selected cells. Sourced from
    /// the theme's `selection` field (see `from_theme`).
    pub selection: Rgb,
    /// Foreground override for text inside the selection. `Some`
    /// when the theme declares `selectionForeground`; `None` —
    /// the common case — leaves selected text in its own colour,
    /// recoloured only by the `selection` background behind it.
    pub selection_fg: Option<Rgb>,
    pub black: Rgb,
    pub red: Rgb,
    pub green: Rgb,
    pub yellow: Rgb,
    pub blue: Rgb,
    pub magenta: Rgb,
    pub cyan: Rgb,
    pub white: Rgb,
    pub bright_black: Rgb,
    pub bright_red: Rgb,
    pub bright_green: Rgb,
    pub bright_yellow: Rgb,
    pub bright_blue: Rgb,
    pub bright_magenta: Rgb,
    pub bright_cyan: Rgb,
    pub bright_white: Rgb,
}

impl Palette {
    /// Hardcoded Baudrun palette — the same values the old
    /// `named_to_rgb` returned. Used as the boot-time default
    /// before any theme has been applied, and as the fallback
    /// when a theme JSON contains a malformed hex that we can't
    /// parse (better to ship the wrong-but-readable colour than
    /// black-on-black).
    pub const fn baudrun() -> Self {
        const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
            Rgb { r, g, b }
        }
        Palette {
            fg: rgb(0xe4, 0xe4, 0xe7),
            bg: rgb(0x0b, 0x0b, 0x0d),
            cursor: rgb(0xe4, 0xe4, 0xe7),
            // The pre-theming default — the muted blue-grey that
            // used to live in `terminal_grid::SELECTION_BG`.
            selection: rgb(0x4a, 0x5a, 0x80),
            selection_fg: None,
            black: rgb(0x1e, 0x1e, 0x22),
            red: rgb(0xff, 0x69, 0x61),
            green: rgb(0x7c, 0xd9, 0x92),
            yellow: rgb(0xf5, 0xd7, 0x6e),
            blue: rgb(0x6c, 0xb6, 0xff),
            magenta: rgb(0xd7, 0x94, 0xff),
            cyan: rgb(0x7c, 0xe0, 0xe0),
            white: rgb(0xd4, 0xd4, 0xd8),
            bright_black: rgb(0x4a, 0x4a, 0x52),
            bright_red: rgb(0xff, 0x8a, 0x80),
            bright_green: rgb(0xa2, 0xe5, 0xb3),
            bright_yellow: rgb(0xfc, 0xe4, 0x88),
            bright_blue: rgb(0x94, 0xcc, 0xff),
            bright_magenta: rgb(0xe5, 0xb6, 0xff),
            bright_cyan: rgb(0xa6, 0xec, 0xec),
            bright_white: rgb(0xff, 0xff, 0xff),
        }
    }

    /// Build from a `data::themes::Theme`. Per-slot hex strings are
    /// parsed via `parse_hex_rgb`; any slot that fails to parse
    /// (malformed `#xx` value, missing field, …) falls back to
    /// the corresponding `baudrun()` slot so a partially-bad theme
    /// still renders most of the screen instead of nothing.
    pub fn from_theme(theme: &Theme) -> Self {
        let fb = Self::baudrun();
        let pick = |s: &str, default: Rgb| parse_hex_rgb(s).unwrap_or(default);
        Palette {
            fg: pick(&theme.foreground, fb.fg),
            bg: pick(&theme.background, fb.bg),
            cursor: pick(&theme.cursor, fb.cursor),
            selection: pick(&theme.selection, fb.selection),
            // `selectionForeground` is optional in the theme
            // schema — an empty / unparseable value means "leave
            // selected text its own colour," so this stays `None`
            // rather than falling back to a slot.
            selection_fg: parse_hex_rgb(&theme.selection_foreground),
            black: pick(&theme.black, fb.black),
            red: pick(&theme.red, fb.red),
            green: pick(&theme.green, fb.green),
            yellow: pick(&theme.yellow, fb.yellow),
            blue: pick(&theme.blue, fb.blue),
            magenta: pick(&theme.magenta, fb.magenta),
            cyan: pick(&theme.cyan, fb.cyan),
            white: pick(&theme.white, fb.white),
            bright_black: pick(&theme.bright_black, fb.bright_black),
            bright_red: pick(&theme.bright_red, fb.bright_red),
            bright_green: pick(&theme.bright_green, fb.bright_green),
            bright_yellow: pick(&theme.bright_yellow, fb.bright_yellow),
            bright_blue: pick(&theme.bright_blue, fb.bright_blue),
            bright_magenta: pick(&theme.bright_magenta, fb.bright_magenta),
            bright_cyan: pick(&theme.bright_cyan, fb.bright_cyan),
            bright_white: pick(&theme.bright_white, fb.bright_white),
        }
    }
}

/// Parse `#rrggbb` / `#rrggbbaa` (alpha discarded) → `Rgb`. Returns
/// `None` for anything that doesn't start with `#` or whose hex
/// digits don't parse — caller picks the fallback colour.
fn parse_hex_rgb(s: &str) -> Option<Rgb> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 && s.len() != 8 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(Rgb { r, g, b })
}

/// Resolve an abstract `Color` to a concrete `Rgb` via the
/// supplied palette. `Color::Spec` passes through; `Named` and
/// `Indexed` go through the helpers below.
pub fn resolve(color: Color, palette: &Palette) -> Rgb {
    match color {
        Color::Spec(rgb) => rgb,
        Color::Named(named) => named_to_rgb(named, palette),
        Color::Indexed(idx) => indexed_to_rgb(idx, palette),
    }
}

/// Map a `NamedColor` slot to its palette entry. Falls back to
/// `palette.fg` for slots we don't carry distinct values for
/// (Dim*, Bright Foreground), since "use the foreground" is
/// strictly better than fabricating darker shades inline.
fn named_to_rgb(named: NamedColor, palette: &Palette) -> Rgb {
    use NamedColor::*;
    match named {
        Black => palette.black,
        Red => palette.red,
        Green => palette.green,
        Yellow => palette.yellow,
        Blue => palette.blue,
        Magenta => palette.magenta,
        Cyan => palette.cyan,
        White => palette.white,
        BrightBlack => palette.bright_black,
        BrightRed => palette.bright_red,
        BrightGreen => palette.bright_green,
        BrightYellow => palette.bright_yellow,
        BrightBlue => palette.bright_blue,
        BrightMagenta => palette.bright_magenta,
        BrightCyan => palette.bright_cyan,
        BrightWhite => palette.bright_white,
        Foreground => palette.fg,
        Background => palette.bg,
        Cursor => palette.cursor,
        // Dim*, Bright Foreground, Bright Cursor — fall back to
        // foreground until we carry explicit values for them.
        _ => palette.fg,
    }
}

/// 256-color resolution: 0..16 = the named ANSI colors above,
/// 16..232 = the 6×6×6 cube, 232..256 = the 24-step grayscale ramp.
fn indexed_to_rgb(idx: u8, palette: &Palette) -> Rgb {
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
        named_to_rgb(named, palette)
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
    scrolling_history: usize,
) -> (Term<TerminalListener>, Processor, Rc<ListenerState>) {
    let state = Rc::new(ListenerState::default());
    let listener = TerminalListener {
        state: state.clone(),
    };
    let config = Config {
        scrolling_history,
        ..Config::default()
    };
    let term = Term::new(config, &Dims { rows, cols }, listener);
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
    palette: &Palette,
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

        let mut fg = resolve(term_cell.fg, palette);
        let mut bg = resolve(term_cell.bg, palette);
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
            // Selection background is theme-driven (`palette.selection`).
            // `selection_fg` recolours the text only when the theme
            // declared `selectionForeground`; otherwise selected text
            // keeps its own colour and just gets the new backdrop.
            bg = palette.selection;
            if let Some(sfg) = palette.selection_fg {
                fg = sfg;
            }
        }

        let flags = CellFlags {
            bold: term_flags.contains(TermFlags::BOLD),
            italic: term_flags.contains(TermFlags::ITALIC),
            underline: term_flags.intersects(TermFlags::ALL_UNDERLINES),
            strikethrough: term_flags.contains(TermFlags::STRIKEOUT),
            dim: term_flags.contains(TermFlags::DIM),
        };

        out.set_cell(
            row,
            col,
            GridCell {
                ch: term_cell.c,
                fg,
                bg,
                flags,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `indexed_to_rgb` is the 256-colour resolver: 0..16 are the
    // named ANSI slots (theme-driven), 16..232 the 6×6×6 cube,
    // 232..256 the 24-step grayscale ramp. The cube + grayscale
    // halves are pure arithmetic with three transition points
    // (15→16 named→cube, 231→232 cube→grayscale, the 255
    // endpoint) — exactly the kind of boundary math worth pinning
    // so a future "cleanup" of the channel ramp can't silently
    // shift every 256-colour value.

    #[test]
    fn indexed_named_range_maps_through_palette() {
        let p = Palette::baudrun();
        // 0..16 mirror NamedColor's first 16 variants 1:1.
        assert_eq!(indexed_to_rgb(0, &p), p.black);
        assert_eq!(indexed_to_rgb(1, &p), p.red);
        assert_eq!(indexed_to_rgb(7, &p), p.white);
        assert_eq!(indexed_to_rgb(8, &p), p.bright_black);
        assert_eq!(indexed_to_rgb(15, &p), p.bright_white);
    }

    #[test]
    fn indexed_cube_boundaries() {
        let p = Palette::baudrun();
        // 16 = first cube cell = (0,0,0) → pure black.
        assert_eq!(indexed_to_rgb(16, &p), Rgb { r: 0, g: 0, b: 0 });
        // 231 = last cube cell = (5,5,5) → pure white.
        assert_eq!(
            indexed_to_rgb(231, &p),
            Rgb {
                r: 255,
                g: 255,
                b: 255
            }
        );
        // A mid-cube index proves the r/g/b decomposition:
        // 113 = 16 + 36·2 + 6·4 + 1 → (r=2, g=4, b=1)
        //     → ramp steps (135, 215, 95).
        assert_eq!(
            indexed_to_rgb(113, &p),
            Rgb {
                r: 135,
                g: 215,
                b: 95
            }
        );
    }

    #[test]
    fn indexed_grayscale_boundaries() {
        let p = Palette::baudrun();
        // 232 = darkest gray = 8 + 0·10 = #080808.
        assert_eq!(indexed_to_rgb(232, &p), Rgb { r: 8, g: 8, b: 8 });
        // 255 = lightest gray = 8 + 23·10 = 238 = #eeeeee.
        assert_eq!(
            indexed_to_rgb(255, &p),
            Rgb {
                r: 238,
                g: 238,
                b: 238
            }
        );
    }

    #[test]
    fn cube_step_is_the_canonical_xterm_ramp() {
        // The 6-level per-channel ramp every xterm-256 terminal
        // shares: 0, 95, 135, 175, 215, 255.
        let ramp: [u8; 6] = std::array::from_fn(|i| cube_step(i as u8));
        assert_eq!(ramp, [0, 95, 135, 175, 215, 255]);
    }
}
