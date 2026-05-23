//! Resolved chrome colours for the active skin. Replaces the
//! per-file hardcoded constants `app_view.rs` and `settings_view.rs`
//! used to carry — both files now read from the same `SkinTokens`
//! installed as a gpui `Global`.
//!
//! Resolution path: `data::skins::Store` keeps the parsed `Skin`
//! JSON; `SkinTokens::from_skin(&skin, dark)` walks the skin's
//! `vars` map (with light/dark overlays) and parses each colour
//! string into a packed `0xRRGGBBAA` `u32` suitable for
//! `gpui::rgba(...)`. Slots that fail to parse fall back per-field
//! to `SkinTokens::baudrun_default()` so a partially-bad skin
//! still renders most of the chrome instead of nothing.

use gpui::{linear_color_stop, linear_gradient, rgba, Background, Global, SharedString};

use crate::data::skins::Skin;

/// Per-skin font + font-size hints. Lifted out of `SkinTokens` so
/// the latter can stay `Copy` (chrome render code reads it on
/// every paint and passes it by value to helpers). Fonts are only
/// consumed by `AppView::apply_skin` to push into the gpui-
/// component `Theme` — chrome render never touches them, so
/// keeping them on a separate non-Copy bag avoids the SharedString
/// from poisoning the Copy bound.
#[derive(Debug, Clone)]
pub struct SkinFonts {
    /// First UI font family from the skin's `--font-ui` stack.
    /// Empty string means "use the platform default" (gpui-
    /// component's `.SystemUIFont`).
    pub font_ui: SharedString,
    pub font_mono: SharedString,
    /// Base UI font size in pixels. Drives widget text via the
    /// gpui-component Theme.
    pub font_size_base: f32,
}

impl SkinFonts {
    pub fn defaults() -> Self {
        Self {
            font_ui: SharedString::default(),
            font_mono: SharedString::default(),
            font_size_base: 13.0,
        }
    }

    pub fn from_skin(skin: &Skin, dark: bool) -> Self {
        let raw_var = |key: &str| -> Option<&String> {
            if dark {
                skin.dark_vars.get(key).or_else(|| skin.vars.get(key))
            } else {
                skin.light_vars.get(key).or_else(|| skin.vars.get(key))
            }
        };
        Self {
            font_ui: raw_var("--font-ui")
                .map(|s| first_font_family(s))
                .unwrap_or_default(),
            font_mono: raw_var("--font-mono")
                .map(|s| first_font_family(s))
                .unwrap_or_default(),
            font_size_base: raw_var("--font-size-base")
                .and_then(|s| parse_px(s))
                .unwrap_or(13.0),
        }
    }
}

impl SkinShadows {
    /// Resolve the active skin into the three shadow lists.
    /// `parse_box_shadow_list` skips inset entries silently —
    /// gpui has no inset-shadow primitive.
    pub fn from_skin(skin: &Skin, dark: bool) -> Self {
        let raw_var = |key: &str| -> Option<&String> {
            if dark {
                skin.dark_vars.get(key).or_else(|| skin.vars.get(key))
            } else {
                skin.light_vars.get(key).or_else(|| skin.vars.get(key))
            }
        };
        Self {
            panel: raw_var("--panel-shadow")
                .map(|s| parse_box_shadow_list(s))
                .unwrap_or_default(),
            floating: raw_var("--shadow-floating")
                .map(|s| parse_box_shadow_list(s))
                .unwrap_or_default(),
            panel_overlay: raw_var("--shadow-panel")
                .map(|s| parse_box_shadow_list(s))
                .unwrap_or_default(),
        }
    }
}

/// Packed-RGBA chrome tokens. Layout matches `gpui::rgba`'s
/// expected format (MSB → R, then G, B, A in the LSB) so callers
/// can always do `rgba(tokens.fg_secondary)` without conversion.
/// Fully-opaque colours have alpha `0xFF`; translucent skin vars
/// keep their declared alpha.
#[derive(Debug, Clone, Copy)]
pub struct SkinTokens {
    /// Outermost shell background — painted on the outermost
    /// AppView / SettingsView div so the translucent overlays
    /// (`bg_main`, `bg_panel`, `bg_sidebar`) have an opaque base
    /// to composite against. Without this, skins like macOS-26
    /// whose `--bg-main` is `rgba(255,255,255,0.7)` end up looking
    /// dark when the underlying transparent window shows the OS
    /// desktop instead of the frosted-glass effect Tauri produces.
    /// Resolves from the skin's `--bg-window`, falling back to
    /// `--shell-bg` (gradients are flattened to their first solid
    /// stop), then to a hardcoded opaque value.
    ///
    /// Pair with `bg_window_gradient` when the skin ships a multi-
    /// stop `--shell-bg`: the solid value here is the safe fallback
    /// (for popup / dialog bg layering) and the gradient applies to
    /// the outermost shell div only via `window_background()`.
    pub bg_window: u32,

    /// Optional 2-stop linear gradient sourced from a skin's
    /// `--shell-bg`. `Some((from, to, angle_deg))` paints a
    /// gradient on the outermost shell div via
    /// `window_background()`; `None` falls back to the solid
    /// `bg_window` colour. CSS multi-stop gradients are flattened
    /// to first + last; the middle stops disappear (gpui's
    /// `linear_gradient` only takes two stops). On macOS 26 dark
    /// the source `linear-gradient(135deg, #1d1d1d, #292929 45%,
    /// #1f1f1f)` becomes a subtle `#1d1d1d → #1f1f1f` diagonal —
    /// less visual punch than the Tauri 3-stop bow-tie but enough
    /// to lift the shell off a totally-flat dark plane.
    pub bg_window_gradient: Option<(u32, u32, f32)>,
    /// Translucent main pane bg. Layered ON TOP of `bg_window`.
    pub bg_main: u32,
    pub bg_sidebar: u32,
    pub bg_panel: u32,
    pub bg_input: u32,
    /// Slightly brighter input background for hover / focus
    /// states. `--bg-input-focus` in the skin vars; falls back to
    /// `bg_input` (no visible hover effect) when absent.
    pub bg_input_hover: u32,
    /// Hover / selected-row background — `--bg-hover` in the skin
    /// vars; doubles as the sidebar's "selected profile row" tint.
    pub bg_hover: u32,
    /// Translucent accent fill behind the active editor tab and
    /// Settings rail row. `--bg-active` in the skin vars.
    pub bg_active: u32,

    pub fg_primary: u32,
    pub fg_secondary: u32,
    pub fg_tertiary: u32,

    pub border_subtle: u32,
    pub border_strong: u32,

    pub accent: u32,
    /// Foreground colour rendered on top of `accent` (e.g. text on
    /// the primary Connect button). No skin var — defaults to
    /// pure white since every shipped skin uses a saturated accent
    /// where white reads cleanest.
    pub accent_fg: u32,
    pub danger: u32,
    pub success: u32,
    pub warn: u32,

    /// Inline error text under a sidebar profile row + the failed-
    /// connect status dot. No direct skin var; falls back to
    /// `danger` in `from_skin` when the skin doesn't declare
    /// `--sidebar-error`.
    pub sidebar_error: u32,

    /// Dropdown / popover background. Skins ship `--option-bg` as
    /// an explicitly opaque colour (the popover floats free over
    /// the window so a translucent value would let whatever's
    /// behind the window bleed through). Drives gpui-component's
    /// `Theme.popover` so Select menus stay readable.
    pub option_bg: u32,
    /// Dropdown / popover text colour. Pairs with `option_bg`.
    pub option_fg: u32,

    // -- Sizing tokens -------------------------------------------
    // Component-shape vars from the skin. `--radius-*` drives card
    // corners + button/input rounding; `--font-size-base` carries
    // through to widget text. macOS-26 Liquid Glass uses much
    // larger radii than Baudrun, which is the visible difference
    // between the two skins beyond colour.
    pub radius_lg: f32,
    pub radius_md: f32,
    pub radius_sm: f32,

    /// Padding from the window edge to the sidebar + main pane
    /// pair. macOS-26 Liquid Glass ships 10px to float both panes
    /// as rounded cards inside the shell; every other skin ships
    /// 0 to leave the panes flush with the window. AppView applies
    /// this as horizontal + bottom padding on the pane-row
    /// container; the title bar and status bar stay flush.
    /// `--shell-padding` in the skin vars.
    pub shell_padding_px: f32,
    /// Gap between the sidebar and the main pane. macOS-26 ships
    /// 10px so the two rounded cards have visible breathing room;
    /// every other skin ships 0 (the sidebar's right border draws
    /// the separator instead). `--shell-gap` in the skin vars.
    pub shell_gap_px: f32,
    /// Corner radius for the sidebar + main pane when they render
    /// as floating cards. macOS-26 ships 18px; flush-edged skins
    /// ship 0. AppView uses this to round the panes whenever
    /// `shell_padding_px` is non-zero. `--panel-radius` in the
    /// skin vars.
    pub panel_radius_px: f32,

    /// Visible title-bar strip height for flush-edged skins
    /// (gpui-component's `TitleBar` widget is overridden via
    /// `.h(...)` to this value when it sits in the flex_col).
    /// Floating-card skins use the absolute-overlay title bar
    /// instead and the height has no visible effect — only the
    /// hit-test / drag region is sized by it. `--titlebar-height`
    /// in the skin vars; default 44px (close to the macOS
    /// "Big Sur+" / Liquid Glass standard). macOS Classic sets a
    /// shorter 24px strip; flush-edged Baudrun / Windows 11 /
    /// Adwaita / … inherit the 44px default for breathing room
    /// above the sidebar header.
    pub titlebar_height_px: f32,
    /// Extra top inset on the sidebar's first content row when
    /// panes extend up under the title bar (floating-card skins).
    /// Pushes the "PROFILES" header + + / ⊟ / ⚙ icons below the
    /// macOS traffic lights at (16, 16) so they don't overlap.
    /// macOS-26 ships 24px; every other bundled skin ships 0.
    /// `--titlebar-content-inset` in the skin vars.
    pub titlebar_content_inset_px: f32,

    // -- Colour tokens added in the second skin-coverage pass ----
    /// Accent hover state — `--accent-hover` in the skin vars.
    /// Drives the brighter accent used on hovered buttons / pills.
    /// Currently pushed into gpui-component's `theme.primary_hover`
    /// so the primary Connect button picks it up. Falls back to
    /// `accent` when the skin doesn't declare it.
    pub accent_hover: u32,
    /// Terminal viewport background — `--bg-terminal` in the
    /// skins. Applied to the terminal grid's frame so the
    /// chrome can paint a different shade than the active
    /// terminal theme's `background` (typical pattern: dark
    /// chrome shell wraps a near-black terminal viewport).
    /// Falls back to `bg_window` when the skin doesn't declare
    /// it.
    pub bg_terminal: u32,
    /// Input border colour in the idle (non-focused) state —
    /// `--input-border-idle` in the skin vars. Default is
    /// `border_subtle`; some skins ship `"transparent"` for a
    /// borderless look.
    pub input_border_idle: u32,
    /// Foreground colour for group labels inside a dropdown
    /// popover — `--option-group-fg` in the skin vars. Drives
    /// gpui-component's `theme.muted_foreground` slot.
    pub option_group_fg: u32,
    /// Scrollbar thumb colour — `--scrollbar-thumb` in the skin
    /// vars. Pushed into gpui-component's
    /// `theme.scrollbar_thumb`.
    pub scrollbar_thumb: u32,
    /// Scrollbar thumb hover state — `--scrollbar-thumb-hover`.
    /// Pushed into `theme.scrollbar_thumb_hover`.
    pub scrollbar_thumb_hover: u32,
    /// Modal scrim colour — `--overlay` in the skin vars.
    /// Pushed into gpui-component's `theme.overlay` so dialogs
    /// dim the window behind them in the skin's preferred tint.
    pub overlay: u32,

    // -- Typography tokens ---------------------------------------
    /// `--font-size-h1` — large heading (Settings page header,
    /// welcome pane title). Default 24px.
    pub font_size_h1_px: f32,
    /// `--font-size-section` — section headings inside forms /
    /// Settings cards. Default 15px.
    pub font_size_section_px: f32,
    /// `--font-size-label` — UPPERCASE-style small field labels
    /// (e.g. "TERMINAL FONT SIZE" in Settings, "PROFILES" in the
    /// sidebar header). Default 11px.
    pub font_size_label_px: f32,
    /// `--label-letter-spacing` (in `em`) — letter-spacing on
    /// the small UPPERCASE labels. Default 0.04em ≈ 0.44px on a
    /// 11px label. We store as em so the value scales with the
    /// applied font size at render time.
    pub label_letter_spacing_em: f32,
    /// `--label-weight` — font weight on the small labels.
    /// Default 500 (Medium); skins with chunkier UI typography
    /// (High Contrast, Cyberpunk) ship 600 (SemiBold).
    pub label_weight: u16,
    /// `--label-transform` — text transform applied to small
    /// labels. Default `Uppercase`. macOS-26's Liquid Glass
    /// design language ships `None` (sentence case).
    pub label_transform: LabelTransform,
    /// `--sidebar-divider` — controls the 1px right-edge
    /// separator on the sidebar (between sidebar and main pane
    /// on flush-edged skins). `None` hides it entirely (macOS-26
    /// uses the shell-gap instead); `Solid` paints a 1px line
    /// in the declared colour. The CSS shorthand
    /// `"1px solid var(--border-subtle)"` resolves to the
    /// `border_subtle` colour at parse time; bare colour names
    /// `"none"` short-circuit to the `None` variant.
    pub sidebar_divider: SidebarDivider,

    /// `--panel-border` — CSS-style border shorthand applied to
    /// raised cards inside the chrome (form cards, Settings
    /// section cards). `None` paints no border; `Solid` paints
    /// a `w`-wide border in colour `c`. Parsed from strings like
    /// `"1px solid rgba(0, 0, 0, 0.14)"` or `"none"`;
    /// `var(--border-subtle)` references fall back to
    /// `border_subtle`. Default is `Solid(1.0, border_subtle)`
    /// so skins that don't declare a `--panel-border` (notably
    /// macOS-26) still get an outline on their cards — Tauri
    /// drew that line via `--panel-shadow`'s inset entry which
    /// gpui can't render.
    pub panel_border: PanelBorder,
}

/// Card border resolution. `None` = `--panel-border: none` or a
/// skin that explicitly disables borders; `Solid(width, colour)`
/// = a 1px (or whatever the skin declares) line. We don't bother
/// with a separate `Inherit` variant — the parser returns the
/// skin's declared value or falls back to the `Solid` default,
/// so by the time render touches it the choice is concrete.
#[derive(Debug, Clone, Copy)]
pub enum PanelBorder {
    None,
    Solid(f32, u32),
}

/// Parsed `box-shadow` lists pulled from a skin's `--panel-shadow`,
/// `--shadow-floating`, and `--shadow-panel` vars. Lives separately
/// from `SkinTokens` because `Vec<BoxShadow>` isn't `Copy` — but the
/// render path needs the chrome tokens on every paint, so we keep
/// `SkinTokens` `Copy` and store shadows here as a parallel non-Copy
/// gpui `Global`.
///
/// **Inset shadows dropped at parse time.** gpui's `BoxShadow` is
/// outer-shadow only; CSS `inset` entries in the source string are
/// silently skipped. The most visible loss is macOS-26's
/// `--panel-shadow` which stacks an outer drop + two inset
/// highlights (1px top "glass shine" + 1px inset border); the
/// drop survives, the shines don't.
#[derive(Debug, Clone, Default)]
pub struct SkinShadows {
    /// `--panel-shadow` — drop shadow on raised cards
    /// (Settings sections, form rule cards, the highlight-pack
    /// list rows). Parsed + populated but not yet threaded
    /// through the card render path — most bundled skins ship
    /// `"none"` here and macOS-26's value is dominated by inset
    /// entries gpui can't render, so the visible payoff vs the
    /// ~8 caller signatures we'd need to change isn't there
    /// yet. Reserved for a future pass.
    #[allow(dead_code)]
    pub panel: Vec<gpui::BoxShadow>,
    /// `--shadow-floating` — drop shadow on floating popups
    /// (right-click profile menu, session-overflow `⋯` menu).
    /// gpui-component dialogs paint their own internal shadow
    /// via `.shadow_md()`; this one only reaches our hand-rolled
    /// popups in `app_view.rs`.
    pub floating: Vec<gpui::BoxShadow>,
    /// `--shadow-panel` — drop shadow on the main shell panes
    /// (sidebar wrapper + right-pane wrapper on floating-card
    /// skins). On flush-edged skins these wrappers don't have a
    /// distinct shape so the shadow has nowhere to draw and the
    /// var is effectively no-op.
    pub panel_overlay: Vec<gpui::BoxShadow>,
}

impl gpui::Global for SkinShadows {}

/// Skin's `--label-transform` declaration. Drives whether
/// UPPERCASE / lowercase / sentence-case labels in the chrome
/// (sidebar PROFILES header, settings field labels) get
/// text-transformed at render time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelTransform {
    /// CSS `text-transform: uppercase` — convert label strings
    /// to `to_uppercase()` at render.
    Uppercase,
    /// CSS `text-transform: lowercase`.
    Lowercase,
    /// CSS `text-transform: none` — render the original string.
    /// Used by macOS-26 / Liquid Glass for sentence-case labels.
    None,
}

impl LabelTransform {
    /// Apply the transform to a label string. Helper so render
    /// sites can chain `.child(s.label_transform.apply("PROFILES"))`
    /// instead of branching on the enum at every call.
    pub fn apply(self, s: &str) -> SharedString {
        match self {
            LabelTransform::Uppercase => s.to_uppercase().into(),
            LabelTransform::Lowercase => s.to_lowercase().into(),
            LabelTransform::None => s.to_string().into(),
        }
    }
}

/// Resolved `--sidebar-divider` value. Either `None` (no
/// separator drawn) or `Solid` with the line colour. We always
/// render at 1px wide; per-skin width tuning isn't a pattern
/// any bundled skin uses (the Tauri version had a few skins
/// declaring `1px solid X` and the rest declaring `none`).
#[derive(Debug, Clone, Copy)]
pub enum SidebarDivider {
    None,
    Solid(u32),
}

impl Global for SkinTokens {}

impl SkinTokens {
    /// Background for the outermost shell div on AppView and
    /// SettingsView. Returns a 2-stop linear gradient when the
    /// active skin shipped one in `--shell-bg`; falls back to the
    /// solid `bg_window` colour otherwise. Popups / dialogs /
    /// session-overflow panels keep using `rgba(s.bg_window)`
    /// directly — the gradient would look out of place on a
    /// 220px floating panel.
    pub fn window_background(&self) -> Background {
        if let Some((from, to, angle)) = self.bg_window_gradient {
            linear_gradient(
                angle,
                linear_color_stop(rgba(from), 0.0),
                linear_color_stop(rgba(to), 1.0),
            )
        } else {
            rgba(self.bg_window).into()
        }
    }

    /// Hardcoded values matching the previously-inline constants
    /// in `app_view.rs` / `settings_view.rs` — keeps the chrome
    /// looking the same when no skin is loaded (boot-time
    /// fallback) or when individual slots can't parse.
    pub const fn baudrun_default() -> Self {
        Self {
            bg_window: 0x18181AFF,
            bg_window_gradient: None,
            // Opaque base for the window. Translucent overlays
            // (panel / sidebar) composite onto this.
            bg_main: 0x18181AFF,
            bg_sidebar: 0x262627FF,
            bg_panel: 0xFFFFFF0F,
            bg_input: 0xFFFFFF14,
            bg_input_hover: 0xFFFFFF1F,
            bg_hover: 0x2D3548FF,
            bg_active: 0x007AFF40,

            fg_primary: 0xFFFFFFF2,
            fg_secondary: 0xFFFFFFA6,
            fg_tertiary: 0xFFFFFF66,

            border_subtle: 0xFFFFFF14,
            border_strong: 0xFFFFFF24,

            accent: 0x0A84FFFF,
            accent_fg: 0xFFFFFFFF,
            danger: 0xFF453AFF,
            success: 0x32D74BFF,
            warn: 0xF5D76EFF,

            sidebar_error: 0xFF7A8AFF,

            // Opaque dark dropdown bg + light text — same values
            // the Baudrun skin's `--option-bg` / `--option-fg`
            // declare for dark mode.
            option_bg: 0x1E1E22FF,
            option_fg: 0xE4E4E7FF,

            radius_lg: 10.0,
            radius_md: 6.0,
            radius_sm: 4.0,
            // Default Baudrun shell is flush-edged (panes meet the
            // window directly). macOS-26 overrides via from_skin.
            shell_padding_px: 0.0,
            shell_gap_px: 0.0,
            panel_radius_px: 0.0,
            // 44px title-bar strip is the modern-macOS standard
            // and gives the sidebar's "PROFILES" header
            // breathing room above it. macOS Classic overrides
            // to 24px; macOS-26 explicitly declares 44px (no-op
            // vs default).
            titlebar_height_px: 44.0,
            // Flush-edged skins don't need extra inset — the
            // visible title bar already separates traffic
            // lights from sidebar content. macOS-26 overrides.
            titlebar_content_inset_px: 0.0,

            // Fallback chrome tokens — used when the active
            // skin doesn't declare these vars. Most fall back
            // to one of the colour slots above so a
            // partially-authored custom skin still reads
            // cohesive (e.g. `accent_hover` reuses `accent`,
            // `bg_terminal` reuses `bg_window`).
            accent_hover: 0x0A84FFFF,
            bg_terminal: 0x18181AFF,
            input_border_idle: 0xFFFFFF14,
            option_group_fg: 0x9AA0A6FF,
            scrollbar_thumb: 0xFFFFFF2E,
            scrollbar_thumb_hover: 0xFFFFFF47,
            overlay: 0x000000A0,

            // Typography defaults.
            font_size_h1_px: 24.0,
            font_size_section_px: 15.0,
            font_size_label_px: 11.0,
            label_letter_spacing_em: 0.04,
            label_weight: 500,
            label_transform: LabelTransform::Uppercase,

            // Default flush-edged skins draw a 1px right-edge
            // line on the sidebar; floating-card skins
            // (macOS-26) override to `None`.
            sidebar_divider: SidebarDivider::Solid(0xFFFFFF14),
            // 1px subtle outline by default on every card.
            // Skins overriding to `"none"` (Baudrun, Adwaita,
            // Elementary, macOS-26 implicitly) get no border;
            // skins declaring an explicit colour get that.
            panel_border: PanelBorder::Solid(1.0, 0xFFFFFF14),
        }
    }

    /// Resolve a skin into chrome tokens. `dark` selects which
    /// overlay (`darkVars` vs `lightVars`) to apply on top of the
    /// base `vars`; `false` here means light mode. Skins flagged
    /// `supportsLight = false` should be passed `true` regardless
    /// — the caller (AppView::apply_settings) handles that gate.
    pub fn from_skin(skin: &Skin, dark: bool) -> Self {
        let fb = Self::baudrun_default();
        let raw_var = |key: &str| -> Option<&String> {
            // Overlay layer wins over base. Same precedence the
            // Tauri `applySkin` uses in src/lib/skinApplier.ts.
            if dark {
                skin.dark_vars.get(key).or_else(|| skin.vars.get(key))
            } else {
                skin.light_vars.get(key).or_else(|| skin.vars.get(key))
            }
        };
        let pick =
            |key: &str, default: u32| raw_var(key).and_then(|s| parse_color(s)).unwrap_or(default);
        let pick_px =
            |key: &str, default: f32| raw_var(key).and_then(|s| parse_px(s)).unwrap_or(default);
        // `--shell-bg` may declare a CSS `linear-gradient(…)` —
        // capture both the solid fallback (first stop) and the
        // optional gradient (first + last stops with the declared
        // angle). The solid feeds `bg_window`; the gradient feeds
        // `bg_window_gradient`, painted on the outermost shell
        // div only via `window_background()`.
        let shell_bg_raw = raw_var("--shell-bg");
        let shell_solid = shell_bg_raw.and_then(|s| parse_shell_color(s));
        let shell_gradient = shell_bg_raw.and_then(|s| parse_shell_gradient(s));
        Self {
            // `--bg-window` first (most skins ship a solid here),
            // then fall back to `--shell-bg` (macOS-26 + Baudrun
            // ship a gradient there — flattened to its first stop
            // by `parse_shell_color`). Final fallback is the
            // baudrun_default opaque dark.
            bg_window: raw_var("--bg-window")
                .and_then(|s| parse_color(s))
                .or(shell_solid)
                .unwrap_or(fb.bg_window),
            bg_window_gradient: shell_gradient,
            bg_main: pick("--bg-main", fb.bg_main),
            bg_sidebar: pick("--bg-sidebar", fb.bg_sidebar),
            bg_panel: pick("--bg-panel", fb.bg_panel),
            bg_input: pick("--bg-input", fb.bg_input),
            bg_input_hover: pick("--bg-input-focus", fb.bg_input_hover),
            bg_hover: pick("--bg-hover", fb.bg_hover),
            bg_active: pick("--bg-active", fb.bg_active),
            fg_primary: pick("--fg-primary", fb.fg_primary),
            fg_secondary: pick("--fg-secondary", fb.fg_secondary),
            fg_tertiary: pick("--fg-tertiary", fb.fg_tertiary),
            border_subtle: pick("--border-subtle", fb.border_subtle),
            border_strong: pick("--border-strong", fb.border_strong),
            accent: pick("--accent", fb.accent),
            // Accent foreground isn't declared as a skin var, so
            // derive it from the accent's luminance: dark accents
            // (Baudrun blue, macOS-26 blue) get white-on-accent
            // text; bright/light accents (CRT phosphor green,
            // Synthwave magenta) get black-on-accent so the
            // hovered dropdown row stays readable.
            accent_fg: contrast_fg_for(pick("--accent", fb.accent)),
            danger: pick("--danger", fb.danger),
            success: pick("--success", fb.success),
            warn: pick("--warn", fb.warn),
            // Skins don't declare a sidebar-error slot today —
            // when one does, the lookup picks it up automatically.
            // Falls back through the same chain `--danger` uses
            // so the row-error tint stays in family with the rest
            // of the destructive palette.
            sidebar_error: pick("--sidebar-error", pick("--danger", fb.sidebar_error)),
            option_bg: pick("--option-bg", fb.option_bg),
            option_fg: pick("--option-fg", fb.option_fg),
            radius_lg: pick_px("--radius-lg", fb.radius_lg),
            radius_md: pick_px("--radius-md", fb.radius_md),
            radius_sm: pick_px("--radius-sm", fb.radius_sm),
            shell_padding_px: pick_px("--shell-padding", fb.shell_padding_px),
            shell_gap_px: pick_px("--shell-gap", fb.shell_gap_px),
            panel_radius_px: pick_px("--panel-radius", fb.panel_radius_px),
            titlebar_height_px: pick_px("--titlebar-height", fb.titlebar_height_px),
            titlebar_content_inset_px: pick_px(
                "--titlebar-content-inset",
                fb.titlebar_content_inset_px,
            ),

            // -- new colour tokens ---------------------------
            accent_hover: pick("--accent-hover", pick("--accent", fb.accent_hover)),
            // No `--bg-terminal` → fall back through `bg_window`
            // so the chrome shell colour does double duty.
            bg_terminal: raw_var("--bg-terminal")
                .and_then(|s| parse_color(s))
                .unwrap_or_else(|| {
                    raw_var("--bg-window")
                        .and_then(|s| parse_color(s))
                        .unwrap_or(fb.bg_terminal)
                }),
            input_border_idle: pick(
                "--input-border-idle",
                pick("--border-subtle", fb.input_border_idle),
            ),
            option_group_fg: pick("--option-group-fg", fb.option_group_fg),
            scrollbar_thumb: pick("--scrollbar-thumb", fb.scrollbar_thumb),
            scrollbar_thumb_hover: pick("--scrollbar-thumb-hover", fb.scrollbar_thumb_hover),
            overlay: pick("--overlay", fb.overlay),

            // -- typography tokens ----------------------------
            font_size_h1_px: pick_px("--font-size-h1", fb.font_size_h1_px),
            font_size_section_px: pick_px("--font-size-section", fb.font_size_section_px),
            font_size_label_px: pick_px("--font-size-label", fb.font_size_label_px),
            label_letter_spacing_em: raw_var("--label-letter-spacing")
                .and_then(|s| parse_em(s))
                .unwrap_or(fb.label_letter_spacing_em),
            label_weight: raw_var("--label-weight")
                .and_then(|s| s.trim().parse::<u16>().ok())
                .unwrap_or(fb.label_weight),
            label_transform: raw_var("--label-transform")
                .map(|s| parse_label_transform(s))
                .unwrap_or(fb.label_transform),
            sidebar_divider: raw_var("--sidebar-divider")
                .map(|s| parse_sidebar_divider(s, fb.sidebar_divider))
                .unwrap_or(fb.sidebar_divider),
            panel_border: raw_var("--panel-border")
                .map(|s| parse_border_shorthand(s, fb.border_subtle))
                // No declaration → keep the default from
                // `baudrun_default()` (Solid border_subtle); the
                // parser only fires when the skin explicitly
                // declares the var, which lets us distinguish
                // "not set" (preserve default) from "set to none".
                .unwrap_or(PanelBorder::Solid(1.0, fb.border_subtle)),
        }
    }
}

/// Parse a CSS-style colour string into packed `0xRRGGBBAA`. Handles
/// `#rrggbb`, `#rrggbbaa`, `rgb(r, g, b)`, and `rgba(r, g, b, a)`
/// — the four forms the bundled skins actually use. Any other form
/// (named colours, `hsl()`, `var()`, …) returns `None` so the
/// caller falls back to its baseline value.
fn parse_color(raw: &str) -> Option<u32> {
    let s = raw.trim();
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(inner) = s.strip_prefix("rgba(").and_then(|t| t.strip_suffix(')')) {
        return parse_rgba_args(inner);
    }
    if let Some(inner) = s.strip_prefix("rgb(").and_then(|t| t.strip_suffix(')')) {
        return parse_rgb_args(inner);
    }
    None
}

fn parse_hex(s: &str) -> Option<u32> {
    match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            Some(pack(r, g, b, 0xFF))
        }
        8 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            let a = u8::from_str_radix(&s[6..8], 16).ok()?;
            Some(pack(r, g, b, a))
        }
        _ => None,
    }
}

fn parse_rgba_args(inner: &str) -> Option<u32> {
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if parts.len() != 4 {
        return None;
    }
    let r: u8 = parts[0].parse().ok()?;
    let g: u8 = parts[1].parse().ok()?;
    let b: u8 = parts[2].parse().ok()?;
    let af: f32 = parts[3].parse().ok()?;
    let a = (af.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(pack(r, g, b, a))
}

/// Parse `--shell-bg` values that may carry a CSS `linear-gradient`
/// wrapper. Extracts the first colour stop and returns it; for
/// non-gradient values (`#1d1d1e` for the Baudrun shell) falls
/// back to the regular `parse_color`. Single-value fallback so
/// `bg_window` always has SOMETHING to compose against when the
/// gradient parse fails or the skin ships a flat shell.
fn parse_shell_color(raw: &str) -> Option<u32> {
    let s = raw.trim();
    let inner = s
        .strip_prefix("linear-gradient(")
        .and_then(|t| t.strip_suffix(')'));
    if let Some(args) = inner {
        // First arg is the angle / direction; subsequent args are
        // colour stops like "#1d1d1d 0%". Find the first stop and
        // strip the trailing percentage if present.
        for part in args.split(',').skip(1) {
            let stop = part.trim();
            // Stop syntax: `<color> <position?>`. Take everything
            // before the first space as the colour.
            let color = stop.split_whitespace().next().unwrap_or(stop);
            if let Some(parsed) = parse_color(color) {
                return Some(parsed);
            }
        }
        return None;
    }
    parse_color(s)
}

/// Parse `--shell-bg` for the 2-stop approximation gpui's
/// `linear_gradient` API accepts: `(from_color, to_color, angle_deg)`.
/// CSS may declare 2+ stops (macOS-26 dark is 3); we keep the
/// first and last colour stops and pass the angle through unchanged
/// (CSS and gpui both use degrees-clockwise-from-top so no
/// conversion is needed).
///
/// Returns `None` when the value isn't a `linear-gradient(...)` or
/// when we can't extract at least two valid colour stops — in that
/// case the caller falls back to the solid `bg_window` colour.
/// The middle stops of a CSS multi-stop gradient are discarded;
/// for the macOS-26 bow-tie pattern (`dark → light → dark`) this
/// flattens to a near-imperceptible `dark → dark` since the first
/// and last stops are nearly identical. Acceptable for the alpha;
/// the lift comes from `--bg-main` translucency picking up the
/// gradient anyway.
fn parse_shell_gradient(raw: &str) -> Option<(u32, u32, f32)> {
    let s = raw.trim();
    let inner = s
        .strip_prefix("linear-gradient(")
        .and_then(|t| t.strip_suffix(')'))?;
    let mut parts = inner.split(',').map(str::trim);
    // First arg is the angle (e.g. "135deg"). Strip the unit and
    // parse as f32. `to top` / `to right` / etc. directional
    // keywords aren't supported — fall through and let the caller
    // use the solid fallback.
    let angle_raw = parts.next()?;
    let angle = angle_raw
        .strip_suffix("deg")
        .and_then(|t| t.trim().parse::<f32>().ok())?;
    // Subsequent args are colour stops; collect their colours
    // (drop any trailing `<position>` token).
    let mut stop_colours: Vec<u32> = Vec::new();
    for stop in parts {
        let colour_token = stop.split_whitespace().next().unwrap_or(stop);
        if let Some(parsed) = parse_color(colour_token) {
            stop_colours.push(parsed);
        }
    }
    if stop_colours.len() < 2 {
        return None;
    }
    let from = *stop_colours.first()?;
    let to = *stop_colours.last()?;
    Some((from, to, angle))
}

fn parse_rgb_args(inner: &str) -> Option<u32> {
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if parts.len() != 3 {
        return None;
    }
    let r: u8 = parts[0].parse().ok()?;
    let g: u8 = parts[1].parse().ok()?;
    let b: u8 = parts[2].parse().ok()?;
    Some(pack(r, g, b, 0xFF))
}

const fn pack(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32)
}

/// Pick black or white text for a given background colour by its
/// perceptual luminance. Uses the WCAG-style coefficients
/// (0.299R + 0.587G + 0.114B). Threshold is 0.55 — slightly past
/// the midpoint to bias toward white text, since most accents in
/// the bundled skins are deeply saturated dark blues + already
/// look right with white. The handful of bright-phosphor / pastel
/// accents (CRT, Synthwave magenta) cross the threshold and flip
/// to black.
fn contrast_fg_for(packed: u32) -> u32 {
    let r = ((packed >> 24) & 0xFF) as f32 / 255.0;
    let g = ((packed >> 16) & 0xFF) as f32 / 255.0;
    let b = ((packed >> 8) & 0xFF) as f32 / 255.0;
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    if luma > 0.55 {
        0x000000FF
    } else {
        0xFFFFFFFF
    }
}

/// Parse a CSS pixel value (`"10px"`, `"24px"`, `"0"`) into a `f32`.
/// Bare numbers are accepted as already-pixels. Returns `None` for
/// anything else (`"calc(...)"`, `"var(...)"`, percentages, …) so
/// the caller falls back to the baseline.
fn parse_px(raw: &str) -> Option<f32> {
    let s = raw.trim();
    if let Some(num) = s.strip_suffix("px") {
        return num.trim().parse().ok();
    }
    s.parse().ok()
}

/// Parse a CSS `em` value (e.g. `"0.04em"`) into the bare f32
/// multiplier. Returns `None` for non-em strings so the caller
/// falls back to the baseline. We store the multiplier rather
/// than computing px at parse time so the value scales with the
/// label's applied font size at render.
fn parse_em(raw: &str) -> Option<f32> {
    let s = raw.trim();
    if let Some(num) = s.strip_suffix("em") {
        return num.trim().parse().ok();
    }
    // Bare number is treated as em (CSS `letter-spacing: 0.04`
    // is unit-less and inherits-from-font-size, same effect).
    s.parse().ok()
}

/// Parse a CSS `text-transform` value into our `LabelTransform`
/// enum. Unknown values fall through to `Uppercase` (the
/// Baudrun + every-other-flush-skin default) since that's the
/// historical behaviour for vars that fail to parse.
fn parse_label_transform(raw: &str) -> LabelTransform {
    match raw.trim().to_ascii_lowercase().as_str() {
        "uppercase" => LabelTransform::Uppercase,
        "lowercase" => LabelTransform::Lowercase,
        "none" | "" => LabelTransform::None,
        _ => LabelTransform::Uppercase,
    }
}

/// Parse a CSS border shorthand into `(width_px, color)`. Accepts
/// `"none"` (returns `None`) and `"<width> solid <colour>"`. The
/// `solid` style is the only one any bundled skin uses; other
/// styles (`dashed`, `dotted`, `double`) parse the same way (we
/// just ignore the style keyword) which means custom skins can't
/// get a dashed border but at least won't fail-fall-back to the
/// default. `var(--token)` references fall back to the caller's
/// `default_colour` (`border_subtle` in practice).
fn parse_border_shorthand(raw: &str, default_colour: u32) -> PanelBorder {
    let s = raw.trim();
    if s.eq_ignore_ascii_case("none") || s.is_empty() {
        return PanelBorder::None;
    }
    // Tokenize while respecting parentheses (rgba(r, g, b, a) is
    // one token, not four).
    let tokens = split_top_level(s, ' ');
    let width = tokens.first().and_then(|t| parse_px(t)).unwrap_or(1.0);
    // Find the colour token: the last one that isn't a recognised
    // style keyword.
    let colour = tokens
        .iter()
        .rev()
        .find(|t| {
            let lower = t.to_ascii_lowercase();
            !matches!(
                lower.as_str(),
                "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge"
            ) && !t.is_empty()
        })
        .and_then(|t| {
            if t.starts_with("var(") {
                Some(default_colour)
            } else {
                parse_color(t)
            }
        })
        .unwrap_or(default_colour);
    PanelBorder::Solid(width, colour)
}

/// Parse a CSS `box-shadow` list (one or more comma-separated
/// shadows) into a `Vec<BoxShadow>`. Each shadow is
/// `<x> <y> <blur> [<spread>] <colour> [inset]`. Entries flagged
/// `inset` are dropped — gpui has no inset-shadow primitive.
/// `"none"` returns an empty `Vec`.
///
/// The top-level comma split respects parentheses so the commas
/// inside `rgba(r, g, b, a)` don't get treated as shadow
/// separators.
fn parse_box_shadow_list(raw: &str) -> Vec<gpui::BoxShadow> {
    let s = raw.trim();
    if s.eq_ignore_ascii_case("none") || s.is_empty() {
        return Vec::new();
    }
    split_top_level(s, ',')
        .into_iter()
        .filter_map(|entry| parse_single_box_shadow(&entry))
        .collect()
}

/// Parse one shadow declaration from a `box-shadow` list. Returns
/// `None` on `inset` shadows (dropped — gpui limitation) or on
/// values we can't make sense of (logged once in dev builds,
/// silent in release).
fn parse_single_box_shadow(raw: &str) -> Option<gpui::BoxShadow> {
    let s = raw.trim();
    let tokens = split_top_level(s, ' ');
    // Look for the `inset` keyword and skip the whole shadow.
    if tokens.iter().any(|t| t.eq_ignore_ascii_case("inset")) {
        return None;
    }

    // Walk tokens; the FIRST 3-4 length tokens are the offsets +
    // blur (+ optional spread), the LAST non-length token is the
    // colour. Two-pass parse keeps the colour token (which may
    // contain `rgba(...)` with internal spaces — already handled
    // by `split_top_level`) easy to identify.
    let mut lengths: Vec<f32> = Vec::with_capacity(4);
    let mut colour: Option<u32> = None;
    for t in &tokens {
        if let Some(px) = parse_px(t) {
            lengths.push(px);
        } else if colour.is_none() {
            if let Some(packed) = parse_color(t) {
                colour = Some(packed);
            }
        }
    }
    if lengths.len() < 2 || colour.is_none() {
        return None;
    }
    let x = lengths.first().copied().unwrap_or(0.0);
    let y = lengths.get(1).copied().unwrap_or(0.0);
    let blur = lengths.get(2).copied().unwrap_or(0.0);
    let spread = lengths.get(3).copied().unwrap_or(0.0);
    // gpui's `Rgba::from(packed_u32) → Hsla` conversion is what
    // every other site in this file uses for colour into shadow.
    let colour_hsla: gpui::Hsla = gpui::rgba(colour?).into();
    Some(gpui::BoxShadow {
        color: colour_hsla,
        offset: gpui::point(gpui::px(x), gpui::px(y)),
        blur_radius: gpui::px(blur),
        spread_radius: gpui::px(spread),
    })
}

/// Split `s` on the separator `sep` at top level only — commas
/// or spaces inside `(...)` groups stay attached to their token.
/// Used by the border + box-shadow parsers so `rgba(0, 0, 0, 0.3)`
/// doesn't shred into four pieces. Empty fragments are skipped.
fn split_top_level(s: &str, sep: char) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut depth: i32 = 0;
    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            ch if ch == sep && depth == 0 => {
                let trimmed = cur.trim();
                if !trimmed.is_empty() {
                    out.push(trimmed.to_string());
                }
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    let trimmed = cur.trim();
    if !trimmed.is_empty() {
        out.push(trimmed.to_string());
    }
    out
}

/// Parse a `--sidebar-divider` declaration into the
/// `SidebarDivider` enum. Accepts:
///   - `"none"` → `None` (no separator drawn)
///   - `"1px solid <colour>"` → `Solid(<colour>)`. We pin the
///     width at 1px for every skin since that's the only width
///     any bundled skin authored; per-skin width tuning is a
///     YAGNI extension if it ever comes up.
///   - `"1px solid var(--border-subtle)"` is the most common
///     authored form. The CSS `var(...)` reference isn't resolved
///     here — the parser strips it and falls back to the
///     baseline (passed as `default`), which is itself sourced
///     from `border_subtle`. So the effective colour is correct
///     even though the `var(...)` chain isn't traversed.
fn parse_sidebar_divider(raw: &str, default: SidebarDivider) -> SidebarDivider {
    let s = raw.trim().to_ascii_lowercase();
    if s == "none" {
        return SidebarDivider::None;
    }
    // Try to extract a colour from the shorthand. Split on
    // whitespace; the last token is typically the colour.
    let last = s.split_whitespace().last().unwrap_or(&s);
    if last.starts_with("var(") {
        // `var(--border-subtle)` — caller's default already
        // resolves to the right colour, so keep it.
        return default;
    }
    if let Some(packed) = parse_color(last) {
        return SidebarDivider::Solid(packed);
    }
    default
}

/// Return the first font family from a CSS font stack, with quotes
/// stripped. Skips `system-ui` / `-apple-system` and similar
/// keywords because gpui doesn't resolve those — the caller will
/// fall back to the platform default when this returns an empty
/// string. Quoted families (`"SF Pro Text"`) keep their internal
/// spaces.
fn first_font_family(stack: &str) -> SharedString {
    for raw in stack.split(',') {
        let mut t = raw.trim();
        // Strip surrounding quotes if present (CSS allows `"..."`
        // and `'...'`).
        if (t.starts_with('"') && t.ends_with('"')) || (t.starts_with('\'') && t.ends_with('\'')) {
            t = &t[1..t.len() - 1];
        }
        if t.is_empty() {
            continue;
        }
        // CSS keyword fallbacks gpui can't resolve. Skip and let
        // the platform default take over.
        if matches!(
            t,
            "system-ui"
                | "-apple-system"
                | "BlinkMacSystemFont"
                | "ui-monospace"
                | "ui-sans-serif"
                | "ui-serif"
                | "monospace"
                | "sans-serif"
                | "serif"
        ) {
            continue;
        }
        return SharedString::from(t.to_string());
    }
    SharedString::default()
}
