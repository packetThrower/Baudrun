package skins

// Built-in skins. Every variable present in style.css :root should have an
// entry in seriesly (the default), so swapping to seriesly resets everything
// to known good values. Other skins only need to override what differs —
// unset variables fall back to the seriesly defaults via the cascade.
var builtins = []Skin{
	{
		ID:          "seriesly",
		Name:        "Seriesly",
		Source:      "builtin",
		Description: "The default dark look with translucent panels and compact iOS-style labels.",
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
		ID:          "macos-26",
		Name:        "macOS 26 (Liquid Glass)",
		Source:      "builtin",
		Description: "Frosted surfaces, larger squircle radii, sentence-case labels, brighter accents. Evokes the Liquid Glass design language.",
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
		ID:          "high-contrast",
		Name:        "High Contrast",
		Source:      "builtin",
		Description: "Accessibility-focused: solid black background, pure white foreground, strong borders everywhere, bright accent colors. No translucency, no blur, no soft shadows.",
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
		ID:          "windows-11",
		Name:        "Windows 11 (Fluent)",
		Source:      "builtin",
		Description: "Windows 11 Fluent / Mica-inspired: Segoe UI, Cascadia Code for mono, sentence-case labels, solid dark surfaces with stroke borders, 4/8px radii, Fluent accent blue.",
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
}
