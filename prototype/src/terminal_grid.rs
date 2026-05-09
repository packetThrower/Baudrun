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

use std::panic;

use alacritty_terminal::vte::ansi::Rgb;
use gpui::{
    fill, font, point, px, rgb, rgba, size, App, Bounds, Element, ElementId, FontWeight,
    GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, Size, StrikethroughStyle,
    Style, TextAlign, TextRun, UnderlineStyle, Window,
};

/// Single-family monospace per platform. The new gpui's
/// `force_width` snap math (which gives us correct cell-aligned
/// glyph positioning) misclassifies glyphs as combining marks
/// when shape_line emits multi-font runs from a comma-separated
/// family list — letters end up missing or duplicated. So we
/// pick one known-installed family per OS and let the platform
/// text system shape against just that. Cross-platform fallback
/// gets re-added via `Font::with_fallbacks` once we figure out
/// which API path doesn't trip the snap.
///
/// `pub` so `terminal_view::cell_width` can use the same family
/// string for layout-line measurement.
#[cfg(target_os = "macos")]
pub const FONT_FAMILY: &str = "Menlo";
#[cfg(target_os = "windows")]
pub const FONT_FAMILY: &str = "Cascadia Mono";
#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
pub const FONT_FAMILY: &str = "DejaVu Sans Mono";

/// Glyph point size — must match `FONT_SIZE_PX` for the same
/// reason `FONT_FAMILY` is shared.
pub const FONT_SIZE_PX: f32 = 13.0;

/// Per-cell line height in pixels. Hand-tuned bigger than the
/// font's natural bounding box to give breathing room.
pub const CELL_HEIGHT_PX: f32 = 18.0;

/// Background colour for selected cells. Hardcoded for now;
/// becomes a theme field when the theme system lands.
pub const SELECTION_BG: Rgb = Rgb { r: 0x4a, g: 0x5a, b: 0x80 };

/// Visible text-style attributes for a cell. Subset of alacritty's
/// `Flags` covering only what affects how a glyph is *painted* —
/// inverse and hidden are handled in `term_bridge::mirror_to_grid`
/// by mutating fg/bg before the cell lands here, and wide-char
/// related flags are deferred until CJK / emoji support lands.
/// Hashable + `Eq` so it slots into the run-coalescer's style key
/// without ceremony.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CellFlags {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub dim: bool,
}

/// One terminal cell. RGB values are concrete (already resolved
/// through whatever palette / theme is active) — the bridge does
/// the abstract-to-concrete conversion at copy time, so the
/// renderer stays oblivious to palettes.
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
    pub flags: CellFlags,
}

impl Cell {
    pub const fn blank(fg: Rgb, bg: Rgb) -> Self {
        Self { ch: ' ', fg, bg, flags: CellFlags { bold: false, italic: false, underline: false, strikethrough: false, dim: false } }
    }
}

/// 50/50 blend of two RGB colours. Used to render alacritty's
/// `DIM` flag — SGR 2 means "halve foreground intensity," and the
/// classic implementation is to lerp the resolved fg toward the
/// resolved bg.
#[inline]
fn blend_50(a: Rgb, b: Rgb) -> Rgb {
    Rgb {
        r: ((a.r as u16 + b.r as u16) / 2) as u8,
        g: ((a.g as u16 + b.g as u16) / 2) as u8,
        b: ((a.b as u16 + b.b as u16) / 2) as u8,
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
            self.cells[row][c] = Cell { ch, fg, bg, flags: CellFlags::default() };
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
    /// Build a `GridElement` that paints the grid via gpui's
    /// `Element` trait, bypassing taffy's flex layout entirely.
    ///
    /// Backgrounds are emitted as rectangles via `paint_quad` and
    /// text per row is handed to `WindowTextSystem::shape_line`
    /// with `force_width = run_cells × cell_w`, which forces every
    /// glyph (regular, bold, italic, …) to fit exactly into one
    /// cell width. Same approach Zed's terminal pane uses, and
    /// the same shape Alacritty's OpenGL renderer uses; cell
    /// alignment is preserved no matter what font cut gpui loads
    /// for the styled run.
    ///
    /// Was a styled-div tree before. The div approach broke under
    /// bold and italic because gpui's flex layout sized each div
    /// to natural shaped width, and bold cuts in particular have
    /// slightly different advances — drag-selection drifted, and
    /// forcing `.w()` per div had its own clipping issues. With
    /// per-cell paint this is no longer a problem to design
    /// around.
    pub fn element(&self, cell_w_px: f32, bell_flash: bool) -> GridElement {
        GridElement::new(self, cell_w_px, bell_flash)
    }
}

/// gpui `Element` that paints a `TerminalGrid` directly: bg rects
/// via `paint_quad`, text per row via `shape_line` + `paint`. Owns
/// pre-built bg-rect and text-run lists so `paint` itself walks
/// flat vectors without touching the source grid (which gives gpui
/// the lifetime story it wants — Element implementors must be
/// `'static`).
pub struct GridElement {
    bg_rects: Vec<BgRect>,
    text_runs: Vec<RowTextRun>,
    rows: usize,
    cols: usize,
    cell_w_px: f32,
    cell_h_px: f32,
    default_fg: Rgb,
    default_bg: Rgb,
    /// When `true`, paint a brief translucent overlay across the
    /// whole grid bounds — visual bell. Caller (TerminalView)
    /// computes this against a wall-clock deadline and passes the
    /// boolean state per render.
    bell_flash: bool,
}

#[derive(Debug, Clone, Copy)]
struct BgRect {
    row: usize,
    start_col: usize,
    cells: usize,
    color: Rgb,
}

struct RowTextRun {
    row: usize,
    start_col: usize,
    text: String,
    fg: Rgb,
    flags: CellFlags,
}

impl GridElement {
    fn new(grid: &TerminalGrid, cell_w_px: f32, bell_flash: bool) -> Self {
        let mut bg_rects = Vec::with_capacity(grid.rows * 2);
        let mut text_runs: Vec<RowTextRun> = Vec::with_capacity(grid.rows * 4);

        for (row_idx, row) in grid.cells.iter().enumerate() {
            // Background pass: contiguous same-colour cells whose
            // colour differs from the grid bg become rectangles.
            // Cells matching grid_bg are skipped — the grid bg is
            // painted once for the whole element below.
            let mut current_bg: Option<BgRect> = None;
            for (col_idx, cell) in row.iter().enumerate() {
                if cell.bg == grid.grid_bg {
                    if let Some(r) = current_bg.take() {
                        bg_rects.push(r);
                    }
                    continue;
                }
                match &mut current_bg {
                    Some(r) if r.color == cell.bg && r.start_col + r.cells == col_idx => {
                        r.cells += 1;
                    }
                    _ => {
                        if let Some(r) = current_bg.take() {
                            bg_rects.push(r);
                        }
                        current_bg = Some(BgRect {
                            row: row_idx,
                            start_col: col_idx,
                            cells: 1,
                            color: cell.bg,
                        });
                    }
                }
            }
            if let Some(r) = current_bg {
                bg_rects.push(r);
            }

            // Text pass: contiguous same-(fg,flags) cells become
            // one shape_line run. We deliberately do NOT skip
            // blank/space cells — including them in the run lets
            // the shaper place subsequent glyphs at the spacing
            // alacritty intended. Skipping them splits the run
            // and the next sub-run starts at its own cell-aligned
            // pixel position, but the previous sub-run's natural
            // advance keeps painting glyphs into that gap, causing
            // visible squishing (spaces disappear in typed input).
            let mut current_run: Option<RowTextRun> = None;
            for (col_idx, cell) in row.iter().enumerate() {
                // Apply dim's fg-blend at extraction time — dim
                // changes fg colour but not the underlying glyph,
                // so doing it here keeps the run-coalescer's key
                // the resolved colour rather than (colour, dim?).
                let effective_fg = if cell.flags.dim {
                    blend_50(cell.fg, cell.bg)
                } else {
                    cell.fg
                };
                let style_match = current_run
                    .as_ref()
                    .is_some_and(|r| r.fg == effective_fg && r.flags == cell.flags);
                let contiguous = current_run
                    .as_ref()
                    .is_some_and(|r| r.start_col + r.text.chars().count() == col_idx);
                if style_match && contiguous {
                    if let Some(r) = current_run.as_mut() {
                        r.text.push(cell.ch);
                    }
                } else {
                    if let Some(r) = current_run.take() {
                        text_runs.push(r);
                    }
                    current_run = Some(RowTextRun {
                        row: row_idx,
                        start_col: col_idx,
                        text: cell.ch.to_string(),
                        fg: effective_fg,
                        flags: cell.flags,
                    });
                }
            }
            if let Some(r) = current_run {
                text_runs.push(r);
            }
        }

        Self {
            bg_rects,
            text_runs,
            rows: grid.rows,
            cols: grid.cols,
            cell_w_px,
            cell_h_px: grid.cell_h_px,
            default_fg: grid.default_fg,
            default_bg: grid.grid_bg,
            bell_flash,
        }
    }
}

impl IntoElement for GridElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for GridElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size = Size {
            width: px(self.cols as f32 * self.cell_w_px).into(),
            height: px(self.rows as f32 * self.cell_h_px).into(),
        };
        let layout_id = window.request_layout(style, [], cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let origin = bounds.origin;
        let cell_w = px(self.cell_w_px);
        let cell_h = px(self.cell_h_px);

        // 1. Whole-grid background. One quad covering the layout
        //    bounds — same as wrapping the element in `.bg(...)`,
        //    but we own the paint order so we know the bg goes in
        //    *before* per-cell rects + text.
        window.paint_quad(fill(bounds, rgb(pack(self.default_bg))));

        // 2. Per-cell bg rectangles. Only emitted for cells whose
        //    bg differs from the grid bg (selection highlight,
        //    inverse cells, themed bg runs).
        for r in &self.bg_rects {
            let rect_origin = point(
                origin.x + cell_w * r.start_col as f32,
                origin.y + cell_h * r.row as f32,
            );
            let rect_size = size(cell_w * r.cells as f32, cell_h);
            window.paint_quad(fill(Bounds::new(rect_origin, rect_size), rgb(pack(r.color))));
        }

        // 3. Text. `shape_line` per run with `force_width =
        //    Some(cell_w)` — the per-glyph cell-snap path. On the
        //    Zed-git gpui pin this snaps each base glyph to a
        //    cell-aligned position while keeping combining marks
        //    at their natural relative offset (see
        //    `apply_force_width_to_layout` in gpui's
        //    `text_system::line_layout`). Result: every cell ends
        //    up exactly `cell_w` wide on screen regardless of
        //    bold/italic cut metrics, so the cursor cell at
        //    `col × cell_w` lines up flush with the visual end of
        //    the previous run. The crates.io `gpui = "0.2.2"`
        //    line had an older snap that compressed multi-glyph
        //    runs visibly — that's the reason for the git pin.
        for run in &self.text_runs {
            let text_len_bytes = run.text.len();
            let pos = point(
                origin.x + cell_w * run.start_col as f32,
                origin.y + cell_h * run.row as f32,
            );
            let force_width = cell_w;

            let mut run_font = font(FONT_FAMILY);
            if run.flags.bold {
                run_font.weight = FontWeight::BOLD;
            }
            if run.flags.italic {
                run_font.style = gpui::FontStyle::Italic;
            }

            let underline_style = run.flags.underline.then(|| UnderlineStyle {
                thickness: px(1.0),
                ..Default::default()
            });
            let strikethrough_style = run.flags.strikethrough.then(|| StrikethroughStyle {
                thickness: px(1.0),
                ..Default::default()
            });

            let text_run = TextRun {
                len: text_len_bytes,
                font: run_font,
                color: rgb(pack(run.fg)).into(),
                background_color: None,
                underline: underline_style,
                strikethrough: strikethrough_style,
            };

            let shaped = window.text_system().shape_line(
                run.text.clone().into(),
                px(FONT_SIZE_PX),
                &[text_run],
                Some(force_width),
            );
            let _ = shaped.paint(pos, cell_h, TextAlign::Left, None, window, cx);
        }

        // 4. Bell flash. Painted last so it sits over everything
        //    else. Translucent white at ~30% alpha — visible
        //    against any reasonable terminal palette without
        //    obliterating the underlying content. The TerminalView
        //    keeps `bell_flash = true` for ~120ms after a BEL byte
        //    is processed, then schedules a re-render to clear it.
        if self.bell_flash {
            window.paint_quad(fill(bounds, rgba(0xffffff4d)));
        }

        // Suppress the `default_fg` / `default_bg` "never read"
        // warnings until we wire them into per-row default fg
        // resolution (currently each text run carries its own).
        let _ = self.default_fg;
    }
}
