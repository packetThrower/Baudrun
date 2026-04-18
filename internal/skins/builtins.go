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
			"--border-subtle": "rgba(255, 255, 255, 0.08)",
			"--border-strong": "rgba(255, 255, 255, 0.14)",

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
}
