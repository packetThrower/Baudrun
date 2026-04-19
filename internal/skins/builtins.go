package skins

// Built-in skins. Every variable present in style.css :root should have an
// entry in seriesly (the default), so swapping to seriesly resets everything
// to known good values. Other skins only need to override what differs —
// unset variables fall back to the seriesly defaults via the cascade.
var builtins = []Skin{
	{
		ID:            "seriesly",
		Name:          "Seriesly",
		Source:        "builtin",
		Description:   "The default look with translucent panels and compact iOS-style labels. Adapts to system light/dark.",
		SupportsLight: true,
		LightVars: map[string]string{
			"--bg-window":             "rgba(245, 245, 247, 0)",
			"--bg-sidebar":            "rgba(0, 0, 0, 0.04)",
			"--bg-main":                "rgba(245, 245, 247, 0.85)",
			"--bg-panel":               "rgba(0, 0, 0, 0.05)",
			"--bg-hover":               "rgba(0, 0, 0, 0.06)",
			"--bg-active":              "rgba(0, 113, 227, 0.18)",
			"--bg-input":               "rgba(0, 0, 0, 0.06)",
			"--bg-input-focus":         "rgba(0, 0, 0, 0.10)",
			"--option-bg":              "#ffffff",
			"--option-fg":              "#1d1d1f",
			"--option-group-fg":        "rgba(0, 0, 0, 0.55)",
			"--fg-primary":             "rgba(0, 0, 0, 0.88)",
			"--fg-secondary":           "rgba(0, 0, 0, 0.6)",
			"--fg-tertiary":            "rgba(0, 0, 0, 0.4)",
			"--border-subtle":          "rgba(0, 0, 0, 0.08)",
			"--border-strong":          "rgba(0, 0, 0, 0.16)",
			"--sidebar-divider":        "1px solid rgba(0, 0, 0, 0.1)",
			"--accent":                 "#0071e3",
			"--accent-hover":           "#0a7aee",
			"--danger":                 "#d70015",
			"--success":                "#248a3d",
			"--warn":                   "#b25000",
			"--scrollbar-thumb":        "rgba(0, 0, 0, 0.22)",
			"--scrollbar-thumb-hover":  "rgba(0, 0, 0, 0.35)",
		},
		Vars: map[string]string{
			// Typography
			"--font-ui":           `-apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro", system-ui, sans-serif`,
			"--font-mono":         `"SF Mono", Menlo, Monaco, "Roboto Mono", ui-monospace, monospace`,
			"--font-size-base":    "13px",
			"--font-size-label":   "11px",
			"--font-size-section": "15px",
			"--font-size-h1":      "24px",

			// Label styling
			"--label-transform":      "uppercase",
			"--label-letter-spacing": "0.04em",
			"--label-weight":         "500",

			// Surfaces
			"--bg-window":      "rgba(30, 30, 34, 0)",
			"--bg-sidebar":     "rgba(255, 255, 255, 0.04)",
			"--bg-main":        "rgba(20, 20, 22, 0.55)",
			"--bg-panel":       "rgba(255, 255, 255, 0.06)",
			"--bg-hover":       "rgba(255, 255, 255, 0.08)",
			"--bg-active":      "rgba(0, 122, 255, 0.25)",
			"--bg-input":       "rgba(255, 255, 255, 0.08)",
			"--bg-input-focus": "rgba(255, 255, 255, 0.12)",
			"--bg-terminal":    "#0b0b0d",

			// Option popups
			"--option-bg":       "#1e1e22",
			"--option-fg":       "#e4e4e7",
			"--option-group-fg": "#9aa0a6",

			// Foreground
			"--fg-primary":   "rgba(255, 255, 255, 0.95)",
			"--fg-secondary": "rgba(255, 255, 255, 0.65)",
			"--fg-tertiary":  "rgba(255, 255, 255, 0.4)",

			// Borders
			"--border-subtle":     "rgba(255, 255, 255, 0.08)",
			"--border-strong":     "rgba(255, 255, 255, 0.14)",
			"--input-border-idle": "transparent",
			"--panel-border":      "none",

			// Semantic
			"--accent":       "#0a84ff",
			"--accent-hover": "#409cff",
			"--danger":       "#ff453a",
			"--success":      "#32d74b",
			"--warn":         "#f5d76e",

			// Radii
			"--radius-sm": "4px",
			"--radius-md": "6px",
			"--radius-lg": "10px",

			// Elevation
			"--shadow-panel":    "none",
			"--shadow-floating": "none",

			// Backdrop blur — off in the default skin
			"--blur-strength": "0px",

			// Scrollbar
			"--scrollbar-thumb":       "rgba(255, 255, 255, 0.18)",
			"--scrollbar-thumb-hover": "rgba(255, 255, 255, 0.28)",

			// Floating panels — off in the flush-edges default layout
			"--shell-padding":   "0",
			"--shell-gap":       "0",
			"--panel-radius":    "0",
			"--panel-shadow":    "none",
			"--sidebar-divider": "1px solid var(--border-subtle)",

			// Chrome
			"--titlebar-height": "38px",
		},
	},
	{
		ID:            "macos-26",
		Name:          "macOS 26 (Liquid Glass)",
		Source:        "builtin",
		Description:   "Frosted surfaces, larger squircle radii, sentence-case labels, brighter accents. Evokes the Liquid Glass design language.",
		SupportsLight: true,
		LightVars: map[string]string{
			"--bg-sidebar":     "rgba(0, 0, 0, 0.04)",
			"--bg-main":        "rgba(255, 255, 255, 0.45)",
			"--bg-panel":       "rgba(255, 255, 255, 0.7)",
			"--bg-hover":       "rgba(0, 0, 0, 0.06)",
			"--bg-active":      "rgba(0, 122, 255, 0.22)",
			"--bg-input":       "rgba(0, 0, 0, 0.06)",
			"--bg-input-focus": "rgba(0, 0, 0, 0.10)",

			"--option-bg":       "#ffffff",
			"--option-fg":       "#1d1d1f",
			"--option-group-fg": "rgba(0, 0, 0, 0.55)",

			"--fg-primary":   "rgba(0, 0, 0, 0.9)",
			"--fg-secondary": "rgba(0, 0, 0, 0.65)",
			"--fg-tertiary":  "rgba(0, 0, 0, 0.42)",

			"--border-subtle": "rgba(0, 0, 0, 0.1)",
			"--border-strong": "rgba(0, 0, 0, 0.18)",

			"--accent":       "#007aff",
			"--accent-hover": "#2a93ff",

			"--panel-shadow":          "0 10px 40px rgba(0, 0, 0, 0.12), 0 1px 0 rgba(255, 255, 255, 0.6) inset",
			"--scrollbar-thumb":       "rgba(0, 0, 0, 0.22)",
			"--scrollbar-thumb-hover": "rgba(0, 0, 0, 0.35)",
		},
		Vars: map[string]string{
			// Same type scale but slightly larger surfaces
			"--font-size-section": "16px",
			"--font-size-h1":      "26px",

			// Sentence-case labels, less letter spacing — the Liquid Glass
			// departure from small-caps row headers.
			"--label-transform":      "none",
			"--label-letter-spacing": "0",
			"--label-weight":         "500",
			"--font-size-label":      "12px",

			// Brighter, more translucent panels. The window background
			// bleeds through more; surfaces feel like frosted overlays.
			"--bg-sidebar":     "rgba(255, 255, 255, 0.10)",
			"--bg-main":        "rgba(18, 20, 28, 0.35)",
			"--bg-panel":       "rgba(255, 255, 255, 0.12)",
			"--bg-hover":       "rgba(255, 255, 255, 0.16)",
			"--bg-active":      "rgba(10, 132, 255, 0.32)",
			"--bg-input":       "rgba(255, 255, 255, 0.12)",
			"--bg-input-focus": "rgba(255, 255, 255, 0.18)",

			// Foreground gets a touch more contrast
			"--fg-primary":   "rgba(255, 255, 255, 0.98)",
			"--fg-secondary": "rgba(255, 255, 255, 0.72)",
			"--fg-tertiary":  "rgba(255, 255, 255, 0.48)",

			// Borders are more visible (the "glass edge")
			"--border-subtle": "rgba(255, 255, 255, 0.14)",
			"--border-strong": "rgba(255, 255, 255, 0.22)",

			// Brighter accent
			"--accent":       "#2a93ff",
			"--accent-hover": "#5aa9ff",

			// Bigger continuous radii
			"--radius-sm": "7px",
			"--radius-md": "12px",
			"--radius-lg": "18px",

			// Soft elevation (used on panels that should feel "floating")
			"--shadow-panel":    "0 1px 0 rgba(255, 255, 255, 0.08) inset, 0 8px 24px rgba(0, 0, 0, 0.28)",
			"--shadow-floating": "0 12px 36px rgba(0, 0, 0, 0.35)",

			// Backdrop blur — pairs with bg-panel translucency to sell the glass effect
			"--blur-strength": "24px",

			// Scrollbar
			"--scrollbar-thumb":       "rgba(255, 255, 255, 0.22)",
			"--scrollbar-thumb-hover": "rgba(255, 255, 255, 0.34)",

			// The floating-bubble layout: detach sidebar and main from the
			// window edges, round their corners, drop shadows underneath.
			// The window's vibrancy shows through the gap around them.
			"--shell-padding":   "10px",
			"--shell-gap":       "10px",
			"--panel-radius":    "18px",
			"--panel-shadow":    "0 10px 40px rgba(0, 0, 0, 0.28), 0 1px 0 rgba(255, 255, 255, 0.06) inset",
			"--sidebar-divider": "none",

			// Slightly taller titlebar region
			"--titlebar-height": "44px",
		},
	},
	{
		ID:            "high-contrast",
		Name:          "High Contrast",
		Source:        "builtin",
		Description:   "Accessibility-focused: solid surfaces, maximum contrast, bright accents, visible borders on every control. Available in both dark (black/white) and light (white/black) variants.",
		SupportsLight: true,
		LightVars: map[string]string{
			"--bg-window":      "#ffffff",
			"--bg-sidebar":     "#ffffff",
			"--bg-main":        "#ffffff",
			"--bg-panel":       "#f5f5f5",
			"--bg-hover":       "#e0e0e0",
			"--bg-active":      "#cfe0f5",
			"--bg-input":       "#ffffff",
			"--bg-input-focus": "#f0f0f0",

			"--option-bg":       "#ffffff",
			"--option-fg":       "#000000",
			"--option-group-fg": "#333333",

			"--fg-primary":   "#000000",
			"--fg-secondary": "#1a1a1a",
			"--fg-tertiary":  "#3a3a3a",

			"--border-subtle":     "#000000",
			"--border-strong":     "#000000",
			"--input-border-idle": "#000000",
			"--panel-border":      "1px solid #000000",
			"--sidebar-divider":   "2px solid #000000",

			"--accent":       "#0040a8",
			"--accent-hover": "#002a70",
			"--danger":       "#b00020",
			"--success":      "#006b1d",
			"--warn":         "#8a5a00",

			"--scrollbar-thumb":       "#000000",
			"--scrollbar-thumb-hover": "#333333",
		},
		Vars: map[string]string{
			// Bigger type for readability
			"--font-size-base":    "14px",
			"--font-size-label":   "12px",
			"--font-size-section": "16px",
			"--font-size-h1":      "26px",
			"--label-weight":      "600",

			// Solid surfaces — zero translucency
			"--bg-window":      "#000000",
			"--bg-sidebar":     "#000000",
			"--bg-main":        "#000000",
			"--bg-panel":       "#0f0f0f",
			"--bg-hover":       "#2a2a2a",
			"--bg-active":      "#1b4b7a",
			"--bg-input":       "#000000",
			"--bg-input-focus": "#1a1a1a",
			"--bg-terminal":    "#000000",

			// Native popups — black with white text
			"--option-bg":       "#000000",
			"--option-fg":       "#ffffff",
			"--option-group-fg": "#cccccc",

			// Maximum foreground contrast
			"--fg-primary":   "#ffffff",
			"--fg-secondary": "#e0e0e0",
			"--fg-tertiary":  "#b8b8b8",

			// Borders on everything — this is the defining feature
			"--border-subtle":     "#ffffff",
			"--border-strong":     "#ffffff",
			"--input-border-idle": "#ffffff",
			"--panel-border":      "1px solid #ffffff",
			"--sidebar-divider":   "2px solid #ffffff",

			// Bright, distinct accents (WCAG AAA contrast against black)
			"--accent":       "#5bc2ff",
			"--accent-hover": "#9ddaff",
			"--danger":       "#ff6a6a",
			"--success":      "#5fff7f",
			"--warn":         "#ffd700",

			// Sharp, squared edges
			"--radius-sm": "2px",
			"--radius-md": "3px",
			"--radius-lg": "4px",

			// No soft effects — rely on color and borders, not blur or shadow
			"--shadow-panel":    "none",
			"--shadow-floating": "none",
			"--blur-strength":   "0px",

			// Scrollbar has to be clearly visible
			"--scrollbar-thumb":       "#ffffff",
			"--scrollbar-thumb-hover": "#cccccc",

			// Flush layout; no floating bubble (the border already separates surfaces)
			"--shell-padding":   "0",
			"--shell-gap":       "0",
			"--panel-radius":    "0",
			"--panel-shadow":    "none",
			"--titlebar-height": "38px",
		},
	},
	{
		ID:            "windows-11",
		Name:          "Windows 11 (Fluent)",
		Source:        "builtin",
		Description:   "Windows 11 Fluent / Mica-inspired: Segoe UI, Cascadia Code for mono, sentence-case labels, stroke borders, 4/8px radii, Fluent accent blue. Adapts to system light/dark.",
		SupportsLight: true,
		LightVars: map[string]string{
			"--bg-window":      "#f3f3f3",
			"--bg-sidebar":     "#ebebeb",
			"--bg-main":        "#f9f9f9",
			"--bg-panel":       "#ffffff",
			"--bg-hover":       "rgba(0, 0, 0, 0.04)",
			"--bg-active":      "rgba(0, 120, 212, 0.2)",
			"--bg-input":       "#ffffff",
			"--bg-input-focus": "#f0f0f0",

			"--option-bg":       "#ffffff",
			"--option-fg":       "#1f1f1f",
			"--option-group-fg": "rgba(0, 0, 0, 0.6)",

			"--fg-primary":   "#1f1f1f",
			"--fg-secondary": "rgba(0, 0, 0, 0.75)",
			"--fg-tertiary":  "rgba(0, 0, 0, 0.5)",

			"--border-subtle":     "rgba(0, 0, 0, 0.08)",
			"--border-strong":     "rgba(0, 0, 0, 0.14)",
			"--input-border-idle": "rgba(0, 0, 0, 0.10)",
			"--panel-border":      "1px solid rgba(0, 0, 0, 0.08)",
			"--sidebar-divider":   "1px solid rgba(0, 0, 0, 0.08)",

			// Fluent light-mode accent — darker blue for contrast vs dark
			// mode's #60cdff
			"--accent":       "#0078d4",
			"--accent-hover": "#106ebe",
			"--danger":       "#c42b1c",
			"--success":      "#107c10",
			"--warn":         "#9d5d00",

			"--shadow-panel":          "0 2px 4px rgba(0, 0, 0, 0.08)",
			"--shadow-floating":       "0 8px 16px rgba(0, 0, 0, 0.14)",
			"--scrollbar-thumb":       "rgba(0, 0, 0, 0.22)",
			"--scrollbar-thumb-hover": "rgba(0, 0, 0, 0.35)",
		},
		Vars: map[string]string{
			// Segoe UI Variable on Windows falls through to the next available
			// family on macOS (no Segoe pre-installed). Cascadia Code is the
			// Windows Terminal default; falls back to Consolas, then generics.
			"--font-ui":   `"Segoe UI Variable Display", "Segoe UI Variable", "Segoe UI", system-ui, -apple-system, sans-serif`,
			"--font-mono": `"Cascadia Code", "Cascadia Mono", Consolas, "Courier New", ui-monospace, monospace`,

			// Sentence-case labels — Fluent's row labels are not iOS small-caps
			"--label-transform":      "none",
			"--label-letter-spacing": "0",
			"--label-weight":         "600",
			"--font-size-label":      "12px",
			"--font-size-h1":         "22px",

			// Mica-ish opaque surfaces. Sidebar sits slightly lighter than main
			// to mimic the Windows 11 navigation pane tint.
			"--bg-window":      "#1a1a1a",
			"--bg-sidebar":     "#202020",
			"--bg-main":        "#1c1c1c",
			"--bg-panel":       "#2d2d2d",
			"--bg-hover":       "rgba(255, 255, 255, 0.06)",
			"--bg-active":      "rgba(96, 205, 255, 0.22)",
			"--bg-input":       "#2c2c2c",
			"--bg-input-focus": "#363636",
			"--bg-terminal":    "#0c0c0c",

			// Native popups (Windows uses these for <select> dropdowns)
			"--option-bg":       "#2d2d2d",
			"--option-fg":       "#ffffff",
			"--option-group-fg": "rgba(255, 255, 255, 0.6)",

			// Foreground
			"--fg-primary":   "#ffffff",
			"--fg-secondary": "rgba(255, 255, 255, 0.8)",
			"--fg-tertiary":  "rgba(255, 255, 255, 0.55)",

			// Fluent-style strokes: always-visible 1px card borders
			"--border-subtle":     "rgba(255, 255, 255, 0.08)",
			"--border-strong":     "rgba(255, 255, 255, 0.14)",
			"--input-border-idle": "rgba(255, 255, 255, 0.10)",
			"--panel-border":      "1px solid rgba(255, 255, 255, 0.08)",
			"--sidebar-divider":   "1px solid rgba(255, 255, 255, 0.08)",

			// Windows 11 dark-mode accent (light sky blue) + status palette
			"--accent":       "#60cdff",
			"--accent-hover": "#86d8ff",
			"--danger":       "#ff99a4",
			"--success":      "#6ccb5f",
			"--warn":         "#ffc83d",

			// Fluent uses 4px for controls, 8px for cards
			"--radius-sm": "4px",
			"--radius-md": "4px",
			"--radius-lg": "8px",

			// Subtle card elevation; no blur (Mica is a material, not a CSS effect)
			"--shadow-panel":    "0 2px 4px rgba(0, 0, 0, 0.18)",
			"--shadow-floating": "0 8px 16px rgba(0, 0, 0, 0.3)",
			"--blur-strength":   "0px",

			// Scrollbars — Windows 11 uses thin, subtle
			"--scrollbar-thumb":       "rgba(255, 255, 255, 0.20)",
			"--scrollbar-thumb-hover": "rgba(255, 255, 255, 0.35)",

			// Flush layout (no floating bubble — that's macOS 26's move)
			"--shell-padding":   "0",
			"--shell-gap":       "0",
			"--panel-radius":    "0",
			"--panel-shadow":    "none",
			"--titlebar-height": "32px",
		},
	},
	{
		ID:            "gnome-adwaita",
		Name:          "GNOME (Adwaita)",
		Source:        "builtin",
		Description:   "GNOME / libadwaita: Cantarell typography, bigger rounded corners, sentence-case labels, GNOME's blue accent and destructive-coral palette, generous whitespace. Adapts to system light/dark.",
		SupportsLight: true,
		LightVars: map[string]string{
			"--bg-window":      "#fafafa",
			"--bg-sidebar":     "#ebebeb",
			"--bg-main":        "#fafafa",
			"--bg-panel":       "#ffffff",
			"--bg-hover":       "rgba(0, 0, 0, 0.05)",
			"--bg-active":      "rgba(53, 132, 228, 0.22)",
			"--bg-input":       "rgba(0, 0, 0, 0.05)",
			"--bg-input-focus": "#ffffff",
			"--bg-terminal":    "#ffffff",

			"--option-bg":       "#ffffff",
			"--option-fg":       "rgba(0, 0, 0, 0.88)",
			"--option-group-fg": "rgba(0, 0, 0, 0.55)",

			"--fg-primary":   "rgba(0, 0, 0, 0.88)",
			"--fg-secondary": "rgba(0, 0, 0, 0.65)",
			"--fg-tertiary":  "rgba(0, 0, 0, 0.45)",

			"--border-subtle":   "rgba(0, 0, 0, 0.08)",
			"--border-strong":   "rgba(0, 0, 0, 0.14)",
			"--sidebar-divider": "1px solid rgba(0, 0, 0, 0.08)",

			// Adwaita light uses the same blue, with different destructive red
			"--accent":       "#3584e4",
			"--accent-hover": "#2779d5",
			"--danger":       "#c01c28",
			"--success":      "#26a269",
			"--warn":         "#e5a50a",

			"--shadow-panel":          "0 1px 2px rgba(0, 0, 0, 0.12), 0 2px 6px rgba(0, 0, 0, 0.08)",
			"--scrollbar-thumb":       "rgba(0, 0, 0, 0.18)",
			"--scrollbar-thumb-hover": "rgba(0, 0, 0, 0.32)",
		},
		Vars: map[string]string{
			// Cantarell is GNOME's default; not present on macOS/Windows so
			// fall through to the next sensible sans. Source Code Pro is
			// shipped with GNOME Terminal; solid monospace fallbacks follow.
			"--font-ui":   `Cantarell, "Inter", "Helvetica Neue", -apple-system, system-ui, sans-serif`,
			"--font-mono": `"Source Code Pro", "DejaVu Sans Mono", "Cascadia Code", Menlo, ui-monospace, monospace`,

			// Sentence-case, slightly bigger body type (GNOME tends to use 14px
			// for readability; labels remain medium weight rather than semi-bold)
			"--label-transform":      "none",
			"--label-letter-spacing": "0",
			"--label-weight":         "500",
			"--font-size-base":       "14px",
			"--font-size-label":      "12px",
			"--font-size-section":    "16px",
			"--font-size-h1":         "24px",

			// Adwaita Dark surfaces. Sidebar sits a touch darker than content
			// (GNOME's navigation-pane convention — opposite of Win 11's
			// lighter nav pane).
			"--bg-window":      "#242424",
			"--bg-sidebar":     "#1e1e1e",
			"--bg-main":        "#242424",
			"--bg-panel":       "#303030",
			"--bg-hover":       "rgba(255, 255, 255, 0.07)",
			"--bg-active":      "rgba(53, 132, 228, 0.22)",
			"--bg-input":       "rgba(255, 255, 255, 0.08)",
			"--bg-input-focus": "rgba(255, 255, 255, 0.12)",
			"--bg-terminal":    "#1e1e1e",

			// Native popups
			"--option-bg":       "#303030",
			"--option-fg":       "rgba(255, 255, 255, 0.9)",
			"--option-group-fg": "rgba(255, 255, 255, 0.55)",

			// Foreground
			"--fg-primary":   "rgba(255, 255, 255, 0.90)",
			"--fg-secondary": "rgba(255, 255, 255, 0.65)",
			"--fg-tertiary":  "rgba(255, 255, 255, 0.45)",

			// Borders minimal — GNOME separates by color and spacing, not strokes
			"--border-subtle":     "rgba(255, 255, 255, 0.06)",
			"--border-strong":     "rgba(255, 255, 255, 0.12)",
			"--input-border-idle": "transparent",
			"--panel-border":      "none",
			// The sidebar divider mimics GNOME's tiny shadow between nav pane
			// and content (done here as a dark line rather than a shadow).
			"--sidebar-divider": "1px solid rgba(0, 0, 0, 0.5)",

			// GNOME palette
			"--accent":       "#3584e4",
			"--accent-hover": "#4a90e8",
			"--danger":       "#ff7b63",
			"--success":      "#33d17a",
			"--warn":         "#f5c211",

			// Rounder corners, typical of libadwaita since GNOME 40
			"--radius-sm": "6px",
			"--radius-md": "8px",
			"--radius-lg": "12px",

			// Soft double-shadow elevation (Adwaita's pattern)
			"--shadow-panel":    "0 1px 2px rgba(0, 0, 0, 0.3), 0 2px 6px rgba(0, 0, 0, 0.22)",
			"--shadow-floating": "0 10px 30px rgba(0, 0, 0, 0.4)",
			"--blur-strength":   "0px",

			// Thin scrollbars, GNOME style
			"--scrollbar-thumb":       "rgba(255, 255, 255, 0.15)",
			"--scrollbar-thumb-hover": "rgba(255, 255, 255, 0.28)",

			// Flush sidebar; GNOME doesn't float panels the way Liquid Glass does
			"--shell-padding":   "0",
			"--shell-gap":       "0",
			"--panel-radius":    "0",
			"--panel-shadow":    "none",
			"--titlebar-height": "46px",
		},
	},
	{
		ID:            "kde-breeze",
		Name:          "KDE (Breeze)",
		Source:        "builtin",
		Description:   "KDE Plasma Breeze: cool blue-gray surfaces, thin subtle borders, the iconic KDE sky-blue accent, slightly more angular radii than Fluent/Adwaita. Adapts to system light/dark.",
		SupportsLight: true,
		LightVars: map[string]string{
			"--bg-window":      "#eff0f1",
			"--bg-sidebar":     "#e5e5e5",
			"--bg-main":        "#fcfcfc",
			"--bg-panel":       "#ffffff",
			"--bg-hover":       "rgba(61, 174, 233, 0.12)",
			"--bg-active":      "rgba(61, 174, 233, 0.3)",
			"--bg-input":       "#ffffff",
			"--bg-input-focus": "#f6f6f6",
			"--bg-terminal":    "#ffffff",

			"--option-bg":       "#ffffff",
			"--option-fg":       "#232629",
			"--option-group-fg": "rgba(35, 38, 41, 0.55)",

			"--fg-primary":   "#232629",
			"--fg-secondary": "rgba(35, 38, 41, 0.7)",
			"--fg-tertiary":  "rgba(35, 38, 41, 0.45)",

			"--border-subtle":     "rgba(0, 0, 0, 0.08)",
			"--border-strong":     "rgba(0, 0, 0, 0.14)",
			"--input-border-idle": "rgba(0, 0, 0, 0.08)",
			"--panel-border":      "1px solid rgba(0, 0, 0, 0.06)",
			"--sidebar-divider":   "1px solid rgba(0, 0, 0, 0.1)",

			// Breeze light uses the same sky-blue; status palette stays
			"--accent":       "#3daee9",
			"--accent-hover": "#2996c9",

			"--shadow-panel":          "0 2px 6px rgba(0, 0, 0, 0.08)",
			"--scrollbar-thumb":       "rgba(0, 0, 0, 0.18)",
			"--scrollbar-thumb-hover": "rgba(61, 174, 233, 0.5)",
		},
		Vars: map[string]string{
			// KDE Plasma ships Noto Sans by default. Oxygen (the older
			// default) is still common on KDE systems. Hack is the Plasma
			// developer-terminal default; falls through to other monos.
			"--font-ui":   `"Noto Sans", "Oxygen", "Cantarell", "Inter", system-ui, -apple-system, sans-serif`,
			"--font-mono": `"Hack", "Fira Code", "Source Code Pro", "JetBrains Mono", Consolas, ui-monospace, monospace`,

			// Sentence-case labels, medium weight
			"--label-transform":      "none",
			"--label-letter-spacing": "0",
			"--label-weight":         "500",
			"--font-size-base":       "13px",
			"--font-size-label":      "12px",
			"--font-size-section":    "15px",
			"--font-size-h1":         "23px",

			// Breeze Dark surfaces — blue-gray rather than neutral gray.
			// The cool tint is the visual fingerprint of Breeze vs Adwaita.
			"--bg-window":      "#1d2025",
			"--bg-sidebar":     "#1a1c21",
			"--bg-main":        "#232629",
			"--bg-panel":       "#31363b",
			"--bg-hover":       "rgba(61, 174, 233, 0.10)",
			"--bg-active":      "rgba(61, 174, 233, 0.25)",
			"--bg-input":       "#1b1e21",
			"--bg-input-focus": "#222529",
			"--bg-terminal":    "#232629",

			// Native popups
			"--option-bg":       "#31363b",
			"--option-fg":       "#eff0f1",
			"--option-group-fg": "rgba(239, 240, 241, 0.6)",

			// Foreground — Breeze uses a slightly cool white
			"--fg-primary":   "#eff0f1",
			"--fg-secondary": "rgba(239, 240, 241, 0.70)",
			"--fg-tertiary":  "rgba(239, 240, 241, 0.45)",

			// Thin subtle borders — Breeze's stroke presence is between
			// Fluent (loud) and Adwaita (absent)
			"--border-subtle":     "rgba(255, 255, 255, 0.08)",
			"--border-strong":     "rgba(255, 255, 255, 0.14)",
			"--input-border-idle": "rgba(255, 255, 255, 0.08)",
			"--panel-border":      "1px solid rgba(255, 255, 255, 0.06)",
			"--sidebar-divider":   "1px solid rgba(0, 0, 0, 0.35)",

			// Iconic KDE sky-blue accent + Breeze status palette
			"--accent":       "#3daee9",
			"--accent-hover": "#5fbdf0",
			"--danger":       "#da4453",
			"--success":      "#27ae60",
			"--warn":         "#f67400",

			// Breeze is slightly more angular than GNOME or Fluent
			"--radius-sm": "2px",
			"--radius-md": "3px",
			"--radius-lg": "5px",

			// Flat, single-layer elevation
			"--shadow-panel":    "0 2px 6px rgba(0, 0, 0, 0.25)",
			"--shadow-floating": "0 8px 24px rgba(0, 0, 0, 0.35)",
			"--blur-strength":   "0px",

			// Scrollbars — Breeze uses a visible but slim style
			"--scrollbar-thumb":       "rgba(255, 255, 255, 0.18)",
			"--scrollbar-thumb-hover": "rgba(61, 174, 233, 0.5)",

			// Flush layout
			"--shell-padding":   "0",
			"--shell-gap":       "0",
			"--panel-radius":    "0",
			"--panel-shadow":    "none",
			"--titlebar-height": "36px",
		},
	},
	{
		ID:            "macos-classic",
		Name:          "macOS Classic",
		Source:        "builtin",
		Description:   "Pre-Big Sur macOS: Lucida Grande / Helvetica Neue, 12px baseline, sharp 3-4px radii, classic source-list selection blue, subtle inset bevels, visible edges. Adapts to light (Aqua/Sierra-era) and dark (Mojave-era).",
		SupportsLight: true,
		LightVars: map[string]string{
			"--bg-window":      "#eceff1",
			"--bg-sidebar":     "#dddfe1",
			"--bg-main":        "#ffffff",
			"--bg-panel":       "#f5f5f7",
			"--bg-hover":       "rgba(0, 0, 0, 0.06)",
			// Subtle blue tint for selection. Solid saturated blue reads as
			// electric on a pale sidebar; modern light-mode macOS uses a low-
			// opacity tint with normal text color instead of inverting to
			// white-on-blue.
			"--bg-active":      "rgba(0, 96, 223, 0.18)",
			"--bg-input":       "#ffffff",
			"--bg-input-focus": "#f5f5f5",
			"--bg-terminal":    "#f5f5f7",

			"--option-bg":       "#ffffff",
			"--option-fg":       "#1d1d1f",
			"--option-group-fg": "rgba(0, 0, 0, 0.55)",

			"--fg-primary":   "rgba(0, 0, 0, 0.9)",
			"--fg-secondary": "rgba(0, 0, 0, 0.65)",
			"--fg-tertiary":  "rgba(0, 0, 0, 0.45)",

			"--border-subtle":     "rgba(0, 0, 0, 0.15)",
			"--border-strong":     "rgba(0, 0, 0, 0.25)",
			"--input-border-idle": "rgba(0, 0, 0, 0.18)",
			"--panel-border":      "1px solid rgba(0, 0, 0, 0.14)",
			"--sidebar-divider":   "1px solid rgba(0, 0, 0, 0.15)",

			"--accent":       "#0060df",
			"--accent-hover": "#0070e8",

			// Flip inset highlight direction (was bright inset on dark; now
			// dark inset on light to read as a classic bevel)
			"--shadow-panel":          "inset 0 1px 0 rgba(0, 0, 0, 0.06), 0 1px 2px rgba(0, 0, 0, 0.08)",
			"--shadow-floating":       "0 6px 18px rgba(0, 0, 0, 0.15)",
			"--scrollbar-thumb":       "rgba(0, 0, 0, 0.25)",
			"--scrollbar-thumb-hover": "rgba(0, 0, 0, 0.4)",
		},
		Vars: map[string]string{
			// Pre-SF Pro era fonts. SF was introduced in 10.11 but Mojave's
			// dark mode UI still felt Lucida/Helvetica-y; system-ui falls back
			// to whatever the OS picks.
			"--font-ui":   `"Lucida Grande", "Helvetica Neue", Helvetica, Arial, sans-serif`,
			"--font-mono": `Menlo, Monaco, "Courier New", ui-monospace, monospace`,

			// Smaller, tighter typography — classic macOS chrome was denser
			"--font-size-base":    "12px",
			"--font-size-label":   "10px",
			"--font-size-section": "13px",
			"--font-size-h1":      "20px",

			// Small-caps row labels were the classic macOS convention
			"--label-transform":      "uppercase",
			"--label-letter-spacing": "0.06em",
			"--label-weight":         "500",

			// Mojave-era surfaces: solid opaque grays, no vibrancy
			"--bg-window":      "#323232",
			"--bg-sidebar":     "#3a3a3a",
			"--bg-main":        "#2d2d2d",
			"--bg-panel":       "#3d3d3d",
			"--bg-hover":       "rgba(255, 255, 255, 0.07)",
			// Classic source-list selection: a saturated deep blue, not the
			// bright system accent. Very recognizably "old macOS Finder".
			"--bg-active":      "#1a5cb4",
			"--bg-input":       "#414141",
			"--bg-input-focus": "#4a4a4a",
			"--bg-terminal":    "#1d1d1d",

			// Native popups
			"--option-bg":       "#2d2d2d",
			"--option-fg":       "#f0f0f0",
			"--option-group-fg": "rgba(240, 240, 240, 0.6)",

			// Foreground — off-white, slightly warm
			"--fg-primary":   "#f0f0f0",
			"--fg-secondary": "rgba(240, 240, 240, 0.72)",
			"--fg-tertiary":  "rgba(240, 240, 240, 0.45)",

			// Classic macOS used dark borders for hairlines between controls,
			// not the bright-white 8%-opacity approach of modern macOS.
			"--border-subtle":     "rgba(0, 0, 0, 0.4)",
			"--border-strong":     "rgba(0, 0, 0, 0.6)",
			"--input-border-idle": "rgba(0, 0, 0, 0.4)",
			"--panel-border":      "1px solid rgba(0, 0, 0, 0.35)",
			"--sidebar-divider":   "1px solid rgba(0, 0, 0, 0.5)",

			// Mojave system blue + classic iOS-palette status colors
			"--accent":       "#0071ea",
			"--accent-hover": "#1a87f4",
			"--danger":       "#ff3b30",
			"--success":      "#28cd41",
			"--warn":         "#ffcc00",

			// Sharp pre-Big Sur corners
			"--radius-sm": "3px",
			"--radius-md": "4px",
			"--radius-lg": "6px",

			// Subtle inset highlight + drop shadow — the classic bevel feel
			"--shadow-panel":    "inset 0 1px 0 rgba(255, 255, 255, 0.06), 0 1px 2px rgba(0, 0, 0, 0.4)",
			"--shadow-floating": "0 6px 18px rgba(0, 0, 0, 0.5)",
			"--blur-strength":   "0px",

			// Classic macOS scrollbars were thicker and more visible
			"--scrollbar-thumb":       "rgba(255, 255, 255, 0.25)",
			"--scrollbar-thumb-hover": "rgba(255, 255, 255, 0.4)",

			// Flush layout; classic macOS never floated panels
			"--shell-padding":   "0",
			"--shell-gap":       "0",
			"--panel-radius":    "0",
			"--panel-shadow":    "none",
			// Thinner classic titlebar (Big Sur bulked these up)
			"--titlebar-height": "24px",
		},
	},
	{
		ID:            "windows-xp",
		Name:          "Windows XP (Luna)",
		Source:        "builtin",
		Description:   "Windows XP Luna: Tahoma typography, the iconic gradient-blue sidebar, gray #ece9d8 surfaces, slight rounding on controls. Adapts to light (proper Luna) and dark (Luna-family approximation).",
		SupportsLight: true,
		LightVars: map[string]string{
			// Classic XP surfaces
			"--bg-window":      "#ece9d8",
			"--bg-sidebar":     "#d6dff7",
			"--bg-main":        "#ffffff",
			"--bg-panel":       "#ece9d8",
			"--bg-hover":       "rgba(58, 119, 212, 0.18)",
			// XP used a solid source-list blue with white text; it's
			// iconic but can overwhelm. Using the Luna selection blue.
			"--bg-active":      "#316ac5",
			"--bg-input":       "#ffffff",
			"--bg-input-focus": "#ffffcc",
			"--bg-terminal":    "#000000",

			"--option-bg":       "#ffffff",
			"--option-fg":       "#000000",
			"--option-group-fg": "rgba(0, 0, 0, 0.6)",

			"--fg-primary":   "#000000",
			"--fg-secondary": "rgba(0, 0, 0, 0.72)",
			"--fg-tertiary":  "rgba(0, 0, 0, 0.5)",

			// XP's slightly-blueish 3D bevel borders (the "#7f9db9" color
			// was everywhere in XP chrome)
			"--border-subtle":     "rgba(0, 0, 0, 0.3)",
			"--border-strong":     "rgba(0, 0, 0, 0.5)",
			"--input-border-idle": "#7f9db9",
			"--panel-border":      "1px solid #7f9db9",
			"--sidebar-divider":   "1px solid #9eb3da",

			// Darker XP blue for light-mode contrast against text
			"--accent":       "#0752b7",
			"--accent-hover": "#0a5dc8",
			"--danger":       "#c42d20",
			"--success":      "#4caf50",
			"--warn":         "#f4b400",

			"--shadow-panel":          "inset 0 1px 0 rgba(255, 255, 255, 0.6), 0 1px 2px rgba(0, 0, 0, 0.12)",
			"--scrollbar-thumb":       "#c7d4e8",
			"--scrollbar-thumb-hover": "#98b5d9",
		},
		Vars: map[string]string{
			// Tahoma was THE XP UI font. Consolas didn't ship until Vista;
			// XP's mono was Courier New.
			"--font-ui":   `Tahoma, Geneva, Verdana, "DejaVu Sans", system-ui, sans-serif`,
			"--font-mono": `"Courier New", Consolas, Menlo, "Lucida Console", monospace`,

			// XP UI was denser — 11-12px was typical
			"--font-size-base":    "12px",
			"--font-size-label":   "11px",
			"--font-size-section": "14px",
			"--font-size-h1":      "20px",

			// XP didn't use small-caps for row labels — sentence case
			"--label-transform":      "none",
			"--label-letter-spacing": "0",
			"--label-weight":         "400",

			// Dark "XP family" — not official XP, but XP Silver-inspired
			"--bg-window":      "#1a1e2d",
			"--bg-sidebar":     "#14182a",
			"--bg-main":        "#242a3d",
			"--bg-panel":       "#2c3246",
			"--bg-hover":       "rgba(58, 119, 212, 0.2)",
			"--bg-active":      "#316ac5",
			"--bg-input":       "#1a1e2d",
			"--bg-input-focus": "#252a3d",
			"--bg-terminal":    "#000000",

			"--option-bg":       "#242a3d",
			"--option-fg":       "#e0e0e0",
			"--option-group-fg": "rgba(224, 224, 224, 0.6)",

			"--fg-primary":   "#e0e0e0",
			"--fg-secondary": "rgba(224, 224, 224, 0.72)",
			"--fg-tertiary":  "rgba(224, 224, 224, 0.48)",

			"--border-subtle":     "rgba(0, 0, 0, 0.5)",
			"--border-strong":     "rgba(0, 0, 0, 0.7)",
			"--input-border-idle": "rgba(0, 0, 0, 0.4)",
			"--panel-border":      "1px solid rgba(0, 0, 0, 0.4)",
			"--sidebar-divider":   "2px solid rgba(0, 0, 0, 0.5)",

			"--accent":       "#3a77d4",
			"--accent-hover": "#5b8fd9",
			"--danger":       "#c42d20",
			"--success":      "#4caf50",
			"--warn":         "#f4b400",

			// Slight rounding on controls — XP had gentle bevels, not the
			// sharp corners of Windows 95/2000 and not the modern 8px+ radii
			"--radius-sm": "2px",
			"--radius-md": "3px",
			"--radius-lg": "5px",

			// XP's signature 3D bevel — an inset highlight on top of a
			// subtle drop shadow — gives controls that "raised" look
			"--shadow-panel":    "inset 0 1px 0 rgba(255, 255, 255, 0.08), 0 1px 2px rgba(0, 0, 0, 0.4)",
			"--shadow-floating": "0 6px 20px rgba(0, 0, 0, 0.45)",
			"--blur-strength":   "0px",

			"--scrollbar-thumb":       "rgba(58, 119, 212, 0.4)",
			"--scrollbar-thumb-hover": "rgba(58, 119, 212, 0.65)",

			"--shell-padding":   "0",
			"--shell-gap":       "0",
			"--panel-radius":    "0",
			"--panel-shadow":    "none",
			"--titlebar-height": "30px",
		},
	},
	{
		ID:            "elementary-pantheon",
		Name:          "elementary OS (Pantheon)",
		Source:        "builtin",
		Description:   "elementary OS / Pantheon: Inter typography, flat minimalist surfaces, gentle radii, signature elementary blue, subtle shadows. Adapts to system light/dark.",
		SupportsLight: true,
		LightVars: map[string]string{
			"--bg-window":      "#fafafa",
			"--bg-sidebar":     "#f0f0f0",
			"--bg-main":        "#ffffff",
			"--bg-panel":       "#ffffff",
			"--bg-hover":       "rgba(0, 0, 0, 0.05)",
			"--bg-active":      "rgba(54, 137, 230, 0.22)",
			// Inputs/dropdowns need a visible surface on the white main bg.
			// Real Pantheon ComboBox controls have a subtle light-gray fill
			// plus a 1px border; focus brightens to white with the accent
			// border treatment from the base input:focus rule.
			"--bg-input":       "#f5f5f5",
			"--bg-input-focus": "#ffffff",
			"--bg-terminal":    "#1d1d1d",

			"--option-bg":       "#ffffff",
			"--option-fg":       "rgba(0, 0, 0, 0.87)",
			"--option-group-fg": "rgba(0, 0, 0, 0.55)",

			"--fg-primary":   "rgba(0, 0, 0, 0.87)",
			"--fg-secondary": "rgba(0, 0, 0, 0.62)",
			"--fg-tertiary":  "rgba(0, 0, 0, 0.42)",

			"--border-subtle":     "rgba(0, 0, 0, 0.08)",
			"--border-strong":     "rgba(0, 0, 0, 0.14)",
			"--input-border-idle": "rgba(0, 0, 0, 0.11)",
			"--sidebar-divider":   "1px solid rgba(0, 0, 0, 0.08)",

			"--shadow-panel":          "0 1px 3px rgba(0, 0, 0, 0.08), 0 1px 2px rgba(0, 0, 0, 0.06)",
			"--shadow-floating":       "0 8px 24px rgba(0, 0, 0, 0.16)",
			"--scrollbar-thumb":       "rgba(0, 0, 0, 0.16)",
			"--scrollbar-thumb-hover": "rgba(0, 0, 0, 0.3)",
		},
		Vars: map[string]string{
			// Inter is elementary OS's designated UI font. Cross-platform
			// fallbacks; Roboto Mono is a popular default on Linux dev
			// setups and reads as elementary-consistent.
			"--font-ui":   `"Inter", "Inter UI", "Inter Display", system-ui, -apple-system, sans-serif`,
			"--font-mono": `"Roboto Mono", "Source Code Pro", "JetBrains Mono", Menlo, ui-monospace, monospace`,

			// Sentence-case row labels; elementary is firmly anti-iOS-caps
			"--label-transform":      "none",
			"--label-letter-spacing": "0",
			"--label-weight":         "500",
			"--font-size-base":       "13px",
			"--font-size-label":      "12px",
			"--font-size-section":    "15px",
			"--font-size-h1":         "23px",

			// elementary dark surfaces — medium-dark gray, not black. Sidebar
			// slightly darker than main (elementary's navigation-pane
			// convention).
			"--bg-window":      "#2d2d2d",
			"--bg-sidebar":     "#252525",
			"--bg-main":        "#2d2d2d",
			"--bg-panel":       "#3a3a3a",
			"--bg-hover":       "rgba(255, 255, 255, 0.06)",
			"--bg-active":      "rgba(54, 137, 230, 0.25)",
			"--bg-input":       "rgba(255, 255, 255, 0.08)",
			"--bg-input-focus": "rgba(255, 255, 255, 0.12)",
			"--bg-terminal":    "#1d1d1d",

			"--option-bg":       "#3a3a3a",
			"--option-fg":       "rgba(255, 255, 255, 0.87)",
			"--option-group-fg": "rgba(255, 255, 255, 0.55)",

			"--fg-primary":   "rgba(255, 255, 255, 0.87)",
			"--fg-secondary": "rgba(255, 255, 255, 0.62)",
			"--fg-tertiary":  "rgba(255, 255, 255, 0.42)",

			// Minimal borders — elementary favors spacing and color over
			// strokes, even more so than GNOME
			"--border-subtle":     "rgba(255, 255, 255, 0.06)",
			"--border-strong":     "rgba(255, 255, 255, 0.12)",
			"--input-border-idle": "transparent",
			"--panel-border":      "none",
			"--sidebar-divider":   "1px solid rgba(0, 0, 0, 0.35)",

			// Signature elementary blue + elementary color palette
			// (which has named "Strawberry", "Grape", "Mint", etc. for the
			// status colors — these are the canonical hex values)
			"--accent":       "#3689e6",
			"--accent-hover": "#4a95ea",
			"--danger":       "#c6262e",
			"--success":      "#68b723",
			"--warn":         "#f9c440",

			// Gentle radii; elementary is rounder than KDE but squarer than
			// libadwaita
			"--radius-sm": "4px",
			"--radius-md": "6px",
			"--radius-lg": "8px",

			// Subtle two-layer elevation, very restrained
			"--shadow-panel":    "0 1px 3px rgba(0, 0, 0, 0.24), 0 1px 2px rgba(0, 0, 0, 0.12)",
			"--shadow-floating": "0 10px 25px rgba(0, 0, 0, 0.4)",
			"--blur-strength":   "0px",

			"--scrollbar-thumb":       "rgba(255, 255, 255, 0.16)",
			"--scrollbar-thumb-hover": "rgba(255, 255, 255, 0.3)",

			// Flush layout
			"--shell-padding":   "0",
			"--shell-gap":       "0",
			"--panel-radius":    "0",
			"--panel-shadow":    "none",
			"--titlebar-height": "40px",
		},
	},
	{
		ID:            "e-ink",
		Name:          "E-Ink (Paper)",
		Source:        "builtin",
		Description:   "Monochrome e-reader aesthetic: warm cream paper surfaces, deep ink text, serif typography, hairline borders, no shadows or translucency. Adapts to dark (sepia night mode) and light (cream paper).",
		SupportsLight: true,
		LightVars: map[string]string{
			// Warm paper-cream surfaces, not pure white — matches what Kindle
			// shows in Sepia / Paperwhite modes
			"--bg-window":      "#f4ece0",
			"--bg-sidebar":     "#ebe3d6",
			"--bg-main":        "#f8f1e5",
			"--bg-panel":       "#ebe3d6",
			"--bg-hover":       "rgba(0, 0, 0, 0.05)",
			"--bg-active":      "rgba(0, 0, 0, 0.13)",
			"--bg-input":       "#fcf6eb",
			"--bg-input-focus": "#ffffff",
			"--bg-terminal":    "#f4ece0",

			"--option-bg":       "#f8f1e5",
			"--option-fg":       "#1a1a1a",
			"--option-group-fg": "#7a7468",

			// Deep-ink foreground, not pure #000 (softer on the eye, matches
			// how actual printed text reads)
			"--fg-primary":   "#1a1a1a",
			"--fg-secondary": "#4a4538",
			"--fg-tertiary":  "#7a7468",

			// Hairline borders — books show structure via ink lines
			"--border-subtle":     "rgba(26, 26, 26, 0.18)",
			"--border-strong":     "rgba(26, 26, 26, 0.35)",
			"--input-border-idle": "rgba(26, 26, 26, 0.25)",
			"--panel-border":      "1px solid rgba(26, 26, 26, 0.15)",
			"--sidebar-divider":   "1px solid rgba(26, 26, 26, 0.18)",

			// Monochrome accent (ink); primary buttons become black with white
			// text — very clean, very e-reader
			"--accent":       "#1a1a1a",
			"--accent-hover": "#3a3a3a",
			"--danger":       "#5a1a1a",
			"--success":      "#2a4a2a",
			"--warn":         "#5a4a1a",

			"--scrollbar-thumb":       "rgba(26, 26, 26, 0.25)",
			"--scrollbar-thumb-hover": "rgba(26, 26, 26, 0.45)",
		},
		Vars: map[string]string{
			// Serif UI stack — genuinely commits to the paper-book feel.
			// Iowan Old Style ships on macOS; Georgia is everywhere; fallbacks
			// cover Linux and Windows.
			"--font-ui":   `"Iowan Old Style", "Palatino Linotype", Palatino, "Bookman Old Style", Georgia, "Source Serif Pro", serif`,
			"--font-mono": `"Courier Prime", "Courier Prime Code", "Courier New", Courier, monospace`,

			// Slightly larger type — e-readers optimize for readability
			"--font-size-base":    "14px",
			"--font-size-label":   "12px",
			"--font-size-section": "17px",
			"--font-size-h1":      "26px",

			// Sentence case labels (books don't use small-caps for body
			// section headers; they use italics or light weight, which we
			// emulate via weight)
			"--label-transform":      "none",
			"--label-letter-spacing": "0",
			"--label-weight":         "400",

			// Dark mode = Kindle "Dark" / sepia-night. Warm dark gray surface
			// with off-cream text — less blueish than a typical dark theme.
			"--bg-window":      "#1a1a1a",
			"--bg-sidebar":     "#1f1f1f",
			"--bg-main":        "#1a1a1a",
			"--bg-panel":       "#252525",
			"--bg-hover":       "rgba(208, 201, 187, 0.06)",
			"--bg-active":      "rgba(208, 201, 187, 0.12)",
			"--bg-input":       "#252525",
			"--bg-input-focus": "#2d2d2d",
			"--bg-terminal":    "#1a1a1a",

			"--option-bg":       "#252525",
			"--option-fg":       "#d0c9bb",
			"--option-group-fg": "rgba(208, 201, 187, 0.55)",

			"--fg-primary":   "#d0c9bb",
			"--fg-secondary": "rgba(208, 201, 187, 0.72)",
			"--fg-tertiary":  "rgba(208, 201, 187, 0.48)",

			"--border-subtle":     "rgba(208, 201, 187, 0.15)",
			"--border-strong":     "rgba(208, 201, 187, 0.3)",
			"--input-border-idle": "rgba(208, 201, 187, 0.2)",
			"--panel-border":      "1px solid rgba(208, 201, 187, 0.15)",
			"--sidebar-divider":   "1px solid rgba(208, 201, 187, 0.2)",

			// Muted warm accent — readable against white button text
			"--accent":       "#4a4538",
			"--accent-hover": "#6a6458",
			"--danger":       "#a87258",
			"--success":      "#7a9070",
			"--warn":         "#c0a060",

			// Sharp edges — books and e-readers aren't rounded UI
			"--radius-sm": "2px",
			"--radius-md": "3px",
			"--radius-lg": "4px",

			// No shadows, no blur — e-ink has no depth
			"--shadow-panel":    "none",
			"--shadow-floating": "0 4px 16px rgba(0, 0, 0, 0.3)",
			"--blur-strength":   "0px",

			"--scrollbar-thumb":       "rgba(208, 201, 187, 0.18)",
			"--scrollbar-thumb-hover": "rgba(208, 201, 187, 0.35)",

			"--shell-padding":   "0",
			"--shell-gap":       "0",
			"--panel-radius":    "0",
			"--panel-shadow":    "none",
			"--titlebar-height": "38px",
		},
	},
	{
		ID:            "xfce-greybird",
		Name:          "Xfce (Greybird)",
		Source:        "builtin",
		Description:   "Xfce / Greybird: utilitarian cool-gray surfaces, classic Xfce blue accent, Ubuntu/Droid Sans typography, small radii, thin borders. Adapts to system light/dark.",
		SupportsLight: true,
		LightVars: map[string]string{
			// The iconic Greybird warm-gray (#dcdad5) — instantly recognizable
			// as Xfce to anyone who's run Xubuntu
			"--bg-window":      "#dcdad5",
			"--bg-sidebar":     "#d3d0cc",
			"--bg-main":        "#ebe8e6",
			"--bg-panel":       "#f2f0ed",
			"--bg-hover":       "rgba(0, 0, 0, 0.05)",
			"--bg-active":      "rgba(52, 101, 164, 0.22)",
			"--bg-input":       "#ffffff",
			"--bg-input-focus": "#fafafa",
			"--bg-terminal":    "#1d1d1d",

			"--option-bg":       "#ffffff",
			"--option-fg":       "#1e1e1e",
			"--option-group-fg": "rgba(0, 0, 0, 0.55)",

			"--fg-primary":   "#1e1e1e",
			"--fg-secondary": "rgba(30, 30, 30, 0.72)",
			"--fg-tertiary":  "rgba(30, 30, 30, 0.48)",

			"--border-subtle":     "rgba(0, 0, 0, 0.10)",
			"--border-strong":     "rgba(0, 0, 0, 0.18)",
			"--input-border-idle": "rgba(0, 0, 0, 0.15)",
			"--panel-border":      "1px solid rgba(0, 0, 0, 0.08)",
			"--sidebar-divider":   "1px solid rgba(0, 0, 0, 0.12)",

			// Classic Xfce deeper blue (from the original Xfce logo era)
			"--accent":       "#3465a4",
			"--accent-hover": "#4175b9",

			"--shadow-panel":          "0 1px 3px rgba(0, 0, 0, 0.08)",
			"--scrollbar-thumb":       "rgba(0, 0, 0, 0.2)",
			"--scrollbar-thumb-hover": "rgba(0, 0, 0, 0.35)",
		},
		Vars: map[string]string{
			// Ubuntu font is visually distinctive — tailed Q, single-story a.
			// DejaVu Sans is the reliable Linux fallback. Both likely absent
			// on macOS but system-ui picks up Inter / SF.
			"--font-ui":   `"Ubuntu", "Droid Sans", "DejaVu Sans", "Noto Sans", system-ui, -apple-system, sans-serif`,
			"--font-mono": `"Ubuntu Mono", "DejaVu Sans Mono", "Source Code Pro", Consolas, ui-monospace, monospace`,

			// Sentence case, normal letter-spacing — Xfce is not iOS-small-caps
			"--label-transform":      "none",
			"--label-letter-spacing": "0",
			"--label-weight":         "500",
			"--font-size-base":       "13px",
			"--font-size-label":      "12px",
			"--font-size-section":    "15px",
			"--font-size-h1":         "22px",

			// Cool blue-gray surfaces — that's the Xfce look (more blueish
			// than Breeze, cooler than Adwaita)
			"--bg-window":      "#282a35",
			"--bg-sidebar":     "#20222b",
			"--bg-main":        "#2e3038",
			"--bg-panel":       "#363842",
			"--bg-hover":       "rgba(255, 255, 255, 0.06)",
			"--bg-active":      "rgba(74, 144, 217, 0.25)",
			"--bg-input":       "rgba(0, 0, 0, 0.22)",
			"--bg-input-focus": "rgba(0, 0, 0, 0.35)",
			"--bg-terminal":    "#1a1c23",

			"--option-bg":       "#363842",
			"--option-fg":       "#e8e8e8",
			"--option-group-fg": "rgba(232, 232, 232, 0.55)",

			"--fg-primary":   "#e8e8e8",
			"--fg-secondary": "rgba(232, 232, 232, 0.72)",
			"--fg-tertiary":  "rgba(232, 232, 232, 0.48)",

			"--border-subtle":     "rgba(255, 255, 255, 0.08)",
			"--border-strong":     "rgba(255, 255, 255, 0.14)",
			"--input-border-idle": "rgba(255, 255, 255, 0.10)",
			"--panel-border":      "1px solid rgba(255, 255, 255, 0.06)",
			"--sidebar-divider":   "1px solid rgba(0, 0, 0, 0.4)",

			// Brighter dark-mode variant of the Xfce blue
			"--accent":       "#4a90d9",
			"--accent-hover": "#5fa0e3",
			"--danger":       "#d03030",
			"--success":      "#6eb04b",
			"--warn":         "#f0b030",

			// Small, utilitarian radii — Xfce is not a rounded environment
			"--radius-sm": "2px",
			"--radius-md": "3px",
			"--radius-lg": "4px",

			"--shadow-panel":    "0 1px 3px rgba(0, 0, 0, 0.3)",
			"--shadow-floating": "0 6px 18px rgba(0, 0, 0, 0.4)",
			"--blur-strength":   "0px",

			"--scrollbar-thumb":       "rgba(255, 255, 255, 0.2)",
			"--scrollbar-thumb-hover": "rgba(255, 255, 255, 0.35)",

			// Flush layout
			"--shell-padding":   "0",
			"--shell-gap":       "0",
			"--panel-radius":    "0",
			"--panel-shadow":    "none",
			"--titlebar-height": "32px",
		},
	},
	{
		ID:          "cyberpunk",
		Name:        "Cyberpunk (Synthwave)",
		Source:      "builtin",
		Description: "80s retrofuture / synthwave: deep purple surfaces, neon magenta and cyan accents, text with a soft pink glow, a subtle grid overlay. Dark-only — the aesthetic depends on neon on darkness.",
		Vars: map[string]string{
			// Tight techy sans with monospace-leaning fallbacks. Rajdhani and
			// Chakra Petch are Google Fonts commonly used in cyberpunk /
			// synthwave design; not installed on most systems but the stack
			// falls through to clean defaults.
			"--font-ui":   `"Rajdhani", "Chakra Petch", "Orbitron", "Inter", "SF Pro Display", system-ui, sans-serif`,
			"--font-mono": `"Share Tech Mono", "IBM Plex Mono", "Fira Code", "JetBrains Mono", Menlo, ui-monospace, monospace`,

			"--font-size-base":    "13px",
			"--font-size-label":   "11px",
			"--font-size-section": "15px",
			"--font-size-h1":      "24px",

			// All-caps labels with wide spacing — the techy / arcade feel
			"--label-transform":      "uppercase",
			"--label-letter-spacing": "0.12em",
			"--label-weight":         "600",

			// Deep-purple surfaces, the synthwave baseline
			"--bg-window":      "#120522",
			"--bg-sidebar":     "#0a0317",
			"--bg-main":        "#1a0d2e",
			"--bg-panel":       "#2a1a4a",
			"--bg-hover":       "rgba(255, 0, 110, 0.10)",
			"--bg-active":      "rgba(255, 0, 110, 0.22)",
			"--bg-input":       "#0a0317",
			"--bg-input-focus": "#1a0d2e",
			"--bg-terminal":    "#0a0317",

			"--option-bg":       "#0a0317",
			"--option-fg":       "#f0e6ff",
			"--option-group-fg": "rgba(255, 0, 110, 0.7)",

			// Slightly pink-tinted white — adds to the neon aftermath feel
			"--fg-primary":   "#f0e6ff",
			"--fg-secondary": "rgba(240, 230, 255, 0.72)",
			"--fg-tertiary":  "rgba(240, 230, 255, 0.45)",

			// Magenta-glow borders on surfaces, cyan-glow on inputs —
			// deliberate color split to evoke the synthwave pink+cyan pairing
			"--border-subtle":     "rgba(255, 0, 110, 0.22)",
			"--border-strong":     "rgba(255, 0, 110, 0.42)",
			"--input-border-idle": "rgba(0, 240, 255, 0.30)",
			"--panel-border":      "1px solid rgba(255, 0, 110, 0.25)",
			"--sidebar-divider":   "1px solid rgba(255, 0, 110, 0.38)",

			// Neon accents — hot pink primary, electric status colors
			"--accent":       "#ff006e",
			"--accent-hover": "#ff3393",
			"--danger":       "#ff4500",
			"--success":      "#39ff14",
			"--warn":         "#ffe600",

			// Sharp geometry
			"--radius-sm": "2px",
			"--radius-md": "3px",
			"--radius-lg": "4px",

			// Neon glows instead of drop shadows. Panels get a magenta aura,
			// floating elements get a bigger pink halo.
			"--shadow-panel":    "0 0 0 1px rgba(255, 0, 110, 0.22), 0 4px 24px rgba(255, 0, 110, 0.15)",
			"--shadow-floating": "0 12px 48px rgba(255, 0, 110, 0.35), 0 0 0 1px rgba(0, 240, 255, 0.2)",
			"--blur-strength":   "0px",

			// Scrollbar is a hot-pink glow strip
			"--scrollbar-thumb":       "rgba(255, 0, 110, 0.4)",
			"--scrollbar-thumb-hover": "rgba(255, 0, 110, 0.7)",

			// Flush layout; don't float — sharper geometry reads better edge-
			// to-edge for synthwave
			"--shell-padding":   "0",
			"--shell-gap":       "0",
			"--panel-radius":    "0",
			"--panel-shadow":    "none",
			"--titlebar-height": "32px",

			// Subtle pink glow on every character — the neon-text signature
			"--text-shadow": "0 0 4px rgba(255, 0, 110, 0.28)",

			// A very subtle crossing grid of pink+cyan lines across the
			// window, 40px cell size at 3-4% opacity. Reads as "synthwave
			// backdrop grid" without fighting content legibility.
			"--overlay": `repeating-linear-gradient(0deg, transparent 0 39px, rgba(255, 0, 110, 0.04) 39px 40px), repeating-linear-gradient(90deg, transparent 0 39px, rgba(0, 240, 255, 0.04) 39px 40px)`,
		},
	},
	{
		ID:          "crt",
		Name:        "CRT (Green Phosphor)",
		Source:      "builtin",
		Description: "Vintage VT100 / green phosphor CRT: pure black surfaces, phosphor-green foreground, monospace everywhere, all-caps widely-spaced labels, scan-line overlay, and a subtle glow around every character.",
		Vars: map[string]string{
			// Monospace everywhere — UI text included. The UI font stack
			// intentionally uses monos so the whole app reads as terminal.
			"--font-ui":   `"VT323", "IBM Plex Mono", "Fira Code", Menlo, Monaco, "Courier New", monospace`,
			"--font-mono": `"VT323", "IBM Plex Mono", "Fira Code", Menlo, Monaco, "Courier New", monospace`,

			"--font-size-base":    "14px",
			"--font-size-label":   "11px",
			"--font-size-section": "15px",
			"--font-size-h1":      "22px",

			// Wide letter spacing + all-caps for that teletype feel
			"--label-transform":      "uppercase",
			"--label-letter-spacing": "0.12em",
			"--label-weight":         "normal",

			// Pure-black surfaces. Sidebar, main, panels are all effectively
			// the same shade — the UI reads as one continuous terminal rather
			// than distinct cards.
			"--bg-window":      "#000000",
			"--bg-sidebar":     "#000000",
			"--bg-main":        "#000000",
			"--bg-panel":       "#030703",
			"--bg-hover":       "rgba(51, 255, 51, 0.12)",
			"--bg-active":      "rgba(51, 255, 51, 0.22)",
			"--bg-input":       "#000000",
			"--bg-input-focus": "rgba(51, 255, 51, 0.05)",
			"--bg-terminal":    "#000000",

			// Native popups — phosphor green on black
			"--option-bg":       "#000000",
			"--option-fg":       "#33ff33",
			"--option-group-fg": "rgba(51, 255, 51, 0.55)",

			// Phosphor-green foreground, with diminishing alpha for secondary text
			"--fg-primary":   "#33ff33",
			"--fg-secondary": "rgba(51, 255, 51, 0.72)",
			"--fg-tertiary":  "rgba(51, 255, 51, 0.45)",

			// Borders are phosphor-tinted — CRTs rarely had soft borders,
			// they had thin bright outlines
			"--border-subtle":     "rgba(51, 255, 51, 0.28)",
			"--border-strong":     "rgba(51, 255, 51, 0.55)",
			"--input-border-idle": "rgba(51, 255, 51, 0.32)",
			"--panel-border":      "1px solid rgba(51, 255, 51, 0.22)",
			"--sidebar-divider":   "1px solid rgba(51, 255, 51, 0.35)",

			// Accents stay in the phosphor-green family; status colors
			// shift to their closest CRT-friendly hues
			"--accent":       "#55ff55",
			"--accent-hover": "#77ff77",
			"--danger":       "#ff5555",
			"--success":      "#88ff88",
			"--warn":         "#ffff55",

			// No rounded corners — CRTs drew characters on a grid
			"--radius-sm": "0",
			"--radius-md": "0",
			"--radius-lg": "0",

			// No drop shadow; instead the whole app has a text glow.
			// Floating shadow becomes a halo of phosphor light.
			"--shadow-panel":    "none",
			"--shadow-floating": "0 0 24px rgba(51, 255, 51, 0.18)",
			"--blur-strength":   "0px",

			// Scrollbars phosphor-tinted
			"--scrollbar-thumb":       "rgba(51, 255, 51, 0.35)",
			"--scrollbar-thumb-hover": "rgba(51, 255, 51, 0.6)",

			// Flush layout
			"--shell-padding":   "0",
			"--shell-gap":       "0",
			"--panel-radius":    "0",
			"--panel-shadow":    "none",
			"--titlebar-height": "28px",

			// The CRT fingerprint: phosphor glow around every character,
			// plus a horizontal scan-line overlay across the whole window.
			"--text-shadow": "0 0 2px currentColor, 0 0 6px rgba(51, 255, 51, 0.3)",
			"--overlay":     "repeating-linear-gradient(0deg, transparent 0px, transparent 2px, rgba(0, 0, 0, 0.22) 2px, rgba(0, 0, 0, 0.22) 3px)",
		},
	},
}
