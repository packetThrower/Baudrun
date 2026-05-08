//! TerminalGrid — checkpoint #2's deliverable.
//!
//! A fixed-size grid of (rows × cols) cells. Each cell holds one
//! character plus its own foreground / background RGB colors.
//! Renders as a nested flex layout (one row per row, one
//! auto-sized div per cell) using a monospace font.
//!
//! Color values are `alacritty_terminal::vte::ansi::Rgb` rather
//! than a homegrown type — `Rgb` is just `{ r, g, b }` of `u8`s
//! and reusing it means the bridge code (checkpoint #3+) can copy
//! resolved colors directly from `alacritty_terminal::Term`'s
//! grid into ours, with `palette.resolve(abstract_color)` as the
//! only translation step. We keep our own `Cell` struct so we
//! can attach Baudrun-specific per-cell metadata (selection,
//! syntax-highlight-pack tags, search matches) alongside the
//! terminal-level fields without conflicting with alacritty's
//! cell type.
//!
//! Things still deferred:
//!   * Bold / italic / underline / dim attributes — add a
//!     `flags: CellFlags` field when alacritty integration
//!     starts emitting them.
//!   * Per-cell `String` allocation per render. For a 24×80 grid
//!     that's 1920 allocations; fine for interactive use, worth
//!     batching same-style runs into single text spans before
//!     `show tech-support`-scale output.

use alacritty_terminal::vte::ansi::Rgb;
use gpui::{div, prelude::*, px, rgb, IntoElement};

/// Font family stack. macOS has Menlo / SF Mono natively; Windows
/// ships Cascadia Mono (Windows Terminal's default since 2019) and
/// Consolas; Linux distros usually have DejaVu Sans Mono. Without
/// Windows entries the gpui DirectWrite backend falls back to Segoe
/// UI, which is proportional — every character renders at a
/// different advance and the grid alignment collapses.
///
/// `pub` so `terminal_view` can hand the same family string to
/// gpui's text-system metric APIs when sizing the grid to the
/// window. Mismatched font here vs. there would mean we'd render
/// at one cell width and pack cells using another.
pub const FONT_FAMILY: &str =
    "Cascadia Mono, Menlo, SF Mono, Consolas, DejaVu Sans Mono, Courier New, monospace";

/// Glyph point size — must match `FONT_SIZE_PX` for the same
/// reason `FONT_FAMILY` is shared.
pub const FONT_SIZE_PX: f32 = 13.0;

/// Per-cell line height in pixels. Hand-tuned bigger than the
/// font's natural bounding box to give breathing room.
pub const CELL_HEIGHT_PX: f32 = 18.0;

/// Background colour for selected cells. Hardcoded for now;
/// becomes a theme field when the theme system lands.
pub const SELECTION_BG: Rgb = Rgb { r: 0x4a, g: 0x5a, b: 0x80 };

/// One terminal cell. RGB values are concrete (already resolved
/// through whatever palette / theme is active) — the bridge does
/// the abstract-to-concrete conversion at copy time, so the
/// renderer stays oblivious to palettes.
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
}

impl Cell {
    pub const fn blank(fg: Rgb, bg: Rgb) -> Self {
        Self { ch: ' ', fg, bg }
    }
}

/// Pack an `Rgb { r, g, b }` into the `0xRRGGBB` `u32` shape
/// gpui's `rgb()` constructor expects. Inlined into the render
/// loop; lifted to a function for clarity. `pub(crate)` so the
/// outer `TerminalView` wrapper can colour its own background to
/// match the grid without re-implementing the shift.
#[inline]
pub(crate) fn pack(c: Rgb) -> u32 {
    ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32)
}

/// Fixed-size grid of cells, rendered top-to-bottom. The view
/// owns its cells; mutations go through `set_cell` / `write_str`
/// and trigger a re-render via gpui's normal entity-update path.
pub struct TerminalGrid {
    cells: Vec<Vec<Cell>>,
    rows: usize,
    cols: usize,
    cell_h_px: f32,
    grid_bg: Rgb,
    default_fg: Rgb,
}

impl TerminalGrid {
    pub fn new(rows: usize, cols: usize, default_fg: Rgb, grid_bg: Rgb) -> Self {
        let blank = Cell::blank(default_fg, grid_bg);
        Self {
            cells: vec![vec![blank; cols]; rows],
            rows,
            cols,
            cell_h_px: CELL_HEIGHT_PX,
            grid_bg,
            default_fg,
        }
    }

    /// Update a single cell. Out-of-range coords are silently
    /// dropped — saves call sites driven by a VT parser from
    /// having to bounds-check on every overshoot.
    pub fn set_cell(&mut self, row: usize, col: usize, cell: Cell) {
        if row < self.rows && col < self.cols {
            self.cells[row][col] = cell;
        }
    }

    /// Write `s` starting at `(row, col)`, with a single fg/bg
    /// applied to every cell. Truncates at the right edge of the
    /// row — does NOT wrap, since wrap is the VT parser's job.
    pub fn write_str(&mut self, row: usize, col: usize, s: &str, fg: Rgb, bg: Rgb) {
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

    /// Reshape the grid to `rows × cols`. Cells in the overlap
    /// region are kept verbatim; new cells (when growing) are
    /// blanks; existing cells outside the new bounds (when
    /// shrinking) are dropped. Called from `TerminalView` after
    /// the window resizes so the next mirror has somewhere to
    /// land. Pairs with `Term::resize` on the parser side, which
    /// reflows scrollback / cursor itself.
    pub fn resize(&mut self, rows: usize, cols: usize) {
        if rows == self.rows && cols == self.cols {
            return;
        }
        let blank = Cell::blank(self.default_fg, self.grid_bg);
        // Resize each existing row's column count first.
        for row in self.cells.iter_mut() {
            row.resize(cols, blank);
        }
        // Then resize the row count, padding with all-blank rows.
        self.cells.resize_with(rows, || vec![blank; cols]);
        self.rows = rows;
        self.cols = cols;
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }
}

impl TerminalGrid {
    /// Build the grid's element tree. Called from `TerminalView::render`,
    /// which wraps it with focus tracking and key event handlers. We
    /// dropped the `Render` impl in checkpoint #4 because the entity that
    /// owns this grid is `TerminalView` — `TerminalGrid` is now plain
    /// data, not a gpui entity, so it doesn't need to be `Render`.
    ///
    /// Cells with matching fg + bg in the same row are coalesced into
    /// a single text span (see `row_runs`). This is the difference
    /// between ~1920 nested flex children per render and a typical
    /// ~30–100 — taffy's layout cost scales with element count, and
    /// we measured the per-cell version as visibly slower than
    /// `screen` against the same device. Most lines from a serial
    /// console are mostly default-color, so coalescing collapses
    /// them aggressively.
    pub fn element(&self, cell_width_px: f32) -> impl IntoElement {
        // Each run div gets an *explicit* pixel width of
        // `run_cells * cell_width_px`. This is the only way the
        // mouse-pixel → grid-cell math can be guaranteed to match
        // what's painted on screen: gpui's actual font rendering
        // applies scaling that neither `ch_advance` nor
        // `layout_line` exposes (measured 7.82 px/cell on macOS
        // Menlo @ 13pt, but the painted cells were ~5.86 px wide,
        // breaking drag-selection). Forcing the box width pins the
        // layout to our model regardless of what the text shaper
        // does inside the box.
        let cell_h = px(self.cell_h_px);

        div()
            .flex()
            .flex_col()
            .bg(rgb(pack(self.grid_bg)))
            .font_family(FONT_FAMILY)
            .text_size(px(FONT_SIZE_PX))
            .text_color(rgb(pack(self.default_fg)))
            .children(self.cells.iter().map(move |row| {
                div()
                    .flex()
                    .flex_row()
                    .h(cell_h)
                    .children(row_runs(row).into_iter().map(move |run| {
                        let run_cells = run.text.chars().count() as f32;
                        div()
                            .flex_shrink_0()
                            .w(px(run_cells * cell_width_px))
                            .h(cell_h)
                            .bg(rgb(pack(run.bg)))
                            .text_color(rgb(pack(run.fg)))
                            .child(run.text)
                    }))
            }))
    }
}

/// A maximal run of cells in one row that share fg + bg colors,
/// rendered as a single styled span.
struct Run {
    fg: Rgb,
    bg: Rgb,
    text: String,
}

fn row_runs(row: &[Cell]) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::with_capacity(8);
    for cell in row {
        match runs.last_mut() {
            Some(last) if last.fg == cell.fg && last.bg == cell.bg => {
                last.text.push(cell.ch);
            }
            _ => runs.push(Run {
                fg: cell.fg,
                bg: cell.bg,
                text: cell.ch.to_string(),
            }),
        }
    }
    runs
}
