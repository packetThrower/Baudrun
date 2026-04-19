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
	{
		ID:          "gnome-adwaita",
		Name:        "GNOME (Adwaita Dark)",
		Source:      "builtin",
		Description: "GNOME / Adwaita Dark: Cantarell typography, bigger rounded corners, sentence-case labels, GNOME's blue accent and destructive-coral palette, generous whitespace.",
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
		ID:          "kde-breeze",
		Name:        "KDE (Breeze Dark)",
		Source:      "builtin",
		Description: "KDE Plasma Breeze Dark: cool blue-gray surfaces, thin subtle borders, the iconic KDE sky-blue accent, slightly more angular radii than Fluent/Adwaita.",
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
		ID:          "macos-classic",
		Name:        "macOS Classic (Mojave Dark)",
		Source:      "builtin",
		Description: "Pre-Big Sur macOS dark mode: Lucida Grande / Helvetica Neue, 12px baseline, sharp 3-4px radii, classic source-list selection blue, subtle inset bevels, visible edges. No vibrancy.",
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
}
