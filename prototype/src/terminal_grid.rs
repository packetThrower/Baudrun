//! TerminalGrid — checkpoint #2's deliverable.
//!
//! A fixed-size grid of (rows × cols) cells. Each cell holds one
//! character plus its own foreground / background RGB colors.
//! Renders as a nested flex layout (one row per row, one cell-sized
//! div per cell) using a monospace font.
//!
//! This is intentionally the dumbest possible implementation — no
//! VT parsing, no scroll buffer, no input handling, no batching of
//! adjacent same-color runs. We're proving the rendering primitive
//! is correct and reasonably visible; perf optimization is a
//! separate concern after the typing-latency measurement on
//! Windows tells us whether the broader rewrite is worth doing.
//!
//! Things deferred:
//!   * Cell sizing is hardcoded — proper handling means asking the
//!     font for advance width / line height. gpui 0.2 has the API
//!     for that (Window's text-system service) but it adds wiring
//!     we don't need until we care about font-size changes.
//!   * No bold / italic / underline / dim attributes. Add when the
//!     `alacritty_terminal::Term` integration starts emitting them.
//!   * Per-cell divs allocate one String per cell per render. For
//!     a 24×80 grid that's 1920 allocations; for typical interactive
//!     use that's fine, but for `show tech-support`-scale output
//!     we'll want to batch same-style runs into single text spans.
//!     Defer until we have measurements.

use gpui::{div, prelude::*, px, rgb, Context, IntoElement, Render, Window};

/// One terminal cell. RGB colors as packed `u32` (0xRRGGBB) to
/// match gpui's `rgb()` constructor and keep the integration with
/// `alacritty_terminal`'s color types straightforward later.
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub fg: u32,
    pub bg: u32,
}

impl Cell {
    pub const fn blank(fg: u32, bg: u32) -> Self {
        Self { ch: ' ', fg, bg }
    }
}

/// Fixed-size grid of cells, rendered top-to-bottom. The view
/// owns its cells; mutations go through `set_cell` / `write_str`
/// and trigger a re-render via gpui's normal entity-update path.
pub struct TerminalGrid {
    /// Row-major: `cells[row][col]`. `rows` × `cols` is enforced
    /// at construction; the helpers below clamp out-of-range
    /// writes silently rather than panicking.
    cells: Vec<Vec<Cell>>,
    rows: usize,
    cols: usize,

    /// Cell metrics in pixels. Derived empirically from SF Mono /
    /// Menlo at 13 pt; close enough that test output looks like a
    /// terminal. Replace with a proper font-metrics lookup when we
    /// stop hardcoding the font size.
    cell_w_px: f32,
    cell_h_px: f32,

    /// Background painted under the entire grid (visible in the
    /// padding around content) and also used as the default
    /// "transparent cell bg." Stored on the grid (not the cell)
    /// because per-cell `bg` of 0x0b0b0d would otherwise be
    /// indistinguishable from "no bg," and we want to be able
    /// to draw an explicit black highlight over the grid bg later.
    grid_bg: u32,
    /// Default foreground for blank cells — used when the
    /// alacritty_terminal pipeline doesn't yet have a color for
    /// a cell. Roughly `theme.foreground` from Baudrun's existing
    /// theme schema.
    default_fg: u32,
}

impl TerminalGrid {
    pub fn new(rows: usize, cols: usize, default_fg: u32, grid_bg: u32) -> Self {
        let blank = Cell::blank(default_fg, grid_bg);
        Self {
            cells: vec![vec![blank; cols]; rows],
            rows,
            cols,
            cell_w_px: 8.4,
            cell_h_px: 18.0,
            grid_bg,
            default_fg,
        }
    }

    /// Update a single cell. Out-of-range coords are silently
    /// dropped — saves the call sites from having to bounds-check
    /// when they're driven by a VT parser that may overshoot.
    pub fn set_cell(&mut self, row: usize, col: usize, cell: Cell) {
        if row < self.rows && col < self.cols {
            self.cells[row][col] = cell;
        }
    }

    /// Write `s` starting at `(row, col)`, with a single fg/bg
    /// applied to every cell. Truncates at the right edge of the
    /// row — does NOT wrap, since wrap is the VT parser's job.
    pub fn write_str(&mut self, row: usize, col: usize, s: &str, fg: u32, bg: u32) {
        if row >= self.rows {
            return;
        }
        for (i, ch) in s.chars().enumerate() {
            let c = col + i;
            if c >= self.cols {
                break;
            }
            self.cells[row][c] = Cell { ch, fg, bg };
        }
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Total grid size in pixels — handy for sizing the parent
    /// container before the gpui layout pass runs.
    pub fn dimensions_px(&self) -> (f32, f32) {
        (self.cols as f32 * self.cell_w_px, self.rows as f32 * self.cell_h_px)
    }
}

impl Render for TerminalGrid {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // First attempt explicitly sized each cell at `cell_w_px`,
        // but the picked width was slightly wider than the actual
        // glyph advance width and characters showed up with
        // visible gaps between them. Trying to predict the advance
        // width across font fallbacks is fragile — every time the
        // font picks a different fallback (`SF Mono` not present →
        // Menlo → system monospace) the advance changes.
        //
        // Cleaner: drop the explicit width entirely. For a
        // monospace font, each cell's content (one character) has
        // a natural width equal to the font's advance, so cells
        // auto-size to exactly one column. `flex_shrink_0` keeps
        // them from being compressed if the row is narrower than
        // the viewport, which preserves the monospace invariant.
        // Cell HEIGHT stays explicit because the line-box height
        // includes leading we don't want bleeding through.
        let cell_h = px(self.cell_h_px);

        div()
            .flex()
            .flex_col()
            .bg(rgb(self.grid_bg))
            .font_family("Menlo, SF Mono, monospace")
            .text_size(px(13.0))
            .text_color(rgb(self.default_fg))
            .children(self.cells.iter().map(|row| {
                div().flex().flex_row().h(cell_h).children(row.iter().map(|cell| {
                    div()
                        .flex_shrink_0()
                        .h(cell_h)
                        .bg(rgb(cell.bg))
                        .text_color(rgb(cell.fg))
                        .child(cell.ch.to_string())
                }))
            }))
    }
}
