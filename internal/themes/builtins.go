package themes

// Eight popular themes shipped with the app. Colors sourced from the
// canonical palettes published by each theme's author.
var builtins = []Theme{
	{
		ID: "seriesly", Name: "Seriesly", Source: "builtin",
		Background: "#0b0b0d", Foreground: "#e4e4e7",
		Cursor: "#ffffff", CursorAccent: "#0b0b0d",
		Selection: "#1a3a5c",
		Black:       "#1e1e22", Red: "#ff6961", Green: "#7cd992", Yellow: "#f5d76e",
		Blue:        "#6cb6ff", Magenta: "#d794ff", Cyan: "#7ce0e0", White: "#d4d4d8",
		BrightBlack: "#4a4a52", BrightRed: "#ff8a80", BrightGreen: "#a2e5b3", BrightYellow: "#fce488",
		BrightBlue:  "#94ccff", BrightMagenta: "#e5b6ff", BrightCyan: "#a6ecec", BrightWhite: "#ffffff",
	},
	{
		ID: "dracula", Name: "Dracula", Source: "builtin",
		Background: "#282a36", Foreground: "#f8f8f2",
		Cursor: "#f8f8f2", CursorAccent: "#282a36",
		Selection: "#44475a",
		Black:       "#21222c", Red: "#ff5555", Green: "#50fa7b", Yellow: "#f1fa8c",
		Blue:        "#bd93f9", Magenta: "#ff79c6", Cyan: "#8be9fd", White: "#f8f8f2",
		BrightBlack: "#6272a4", BrightRed: "#ff6e6e", BrightGreen: "#69ff94", BrightYellow: "#ffffa5",
		BrightBlue:  "#d6acff", BrightMagenta: "#ff92df", BrightCyan: "#a4ffff", BrightWhite: "#ffffff",
	},
	{
		ID: "solarized-dark", Name: "Solarized Dark", Source: "builtin",
		Background: "#002b36", Foreground: "#839496",
		Cursor: "#93a1a1", CursorAccent: "#002b36",
		Selection: "#073642",
		Black:       "#073642", Red: "#dc322f", Green: "#859900", Yellow: "#b58900",
		Blue:        "#268bd2", Magenta: "#d33682", Cyan: "#2aa198", White: "#eee8d5",
		BrightBlack: "#002b36", BrightRed: "#cb4b16", BrightGreen: "#586e75", BrightYellow: "#657b83",
		BrightBlue:  "#839496", BrightMagenta: "#6c71c4", BrightCyan: "#93a1a1", BrightWhite: "#fdf6e3",
	},
	{
		ID: "solarized-light", Name: "Solarized Light", Source: "builtin",
		Background: "#fdf6e3", Foreground: "#657b83",
		Cursor: "#586e75", CursorAccent: "#fdf6e3",
		Selection: "#eee8d5",
		Black:       "#073642", Red: "#dc322f", Green: "#859900", Yellow: "#b58900",
		Blue:        "#268bd2", Magenta: "#d33682", Cyan: "#2aa198", White: "#eee8d5",
		BrightBlack: "#002b36", BrightRed: "#cb4b16", BrightGreen: "#586e75", BrightYellow: "#657b83",
		BrightBlue:  "#839496", BrightMagenta: "#6c71c4", BrightCyan: "#93a1a1", BrightWhite: "#fdf6e3",
	},
	{
		ID: "nord", Name: "Nord", Source: "builtin",
		Background: "#2e3440", Foreground: "#d8dee9",
		Cursor: "#d8dee9", CursorAccent: "#2e3440",
		Selection: "#434c5e",
		Black:       "#3b4252", Red: "#bf616a", Green: "#a3be8c", Yellow: "#ebcb8b",
		Blue:        "#81a1c1", Magenta: "#b48ead", Cyan: "#88c0d0", White: "#e5e9f0",
		BrightBlack: "#4c566a", BrightRed: "#bf616a", BrightGreen: "#a3be8c", BrightYellow: "#ebcb8b",
		BrightBlue:  "#81a1c1", BrightMagenta: "#b48ead", BrightCyan: "#8fbcbb", BrightWhite: "#eceff4",
	},
	{
		ID: "one-dark", Name: "One Dark", Source: "builtin",
		Background: "#282c34", Foreground: "#abb2bf",
		Cursor: "#528bff", CursorAccent: "#282c34",
		Selection: "#3e4451",
		Black:       "#282c34", Red: "#e06c75", Green: "#98c379", Yellow: "#e5c07b",
		Blue:        "#61afef", Magenta: "#c678dd", Cyan: "#56b6c2", White: "#abb2bf",
		BrightBlack: "#545862", BrightRed: "#e06c75", BrightGreen: "#98c379", BrightYellow: "#e5c07b",
		BrightBlue:  "#61afef", BrightMagenta: "#c678dd", BrightCyan: "#56b6c2", BrightWhite: "#c8ccd4",
	},
	{
		ID: "monokai", Name: "Monokai", Source: "builtin",
		Background: "#272822", Foreground: "#f8f8f2",
		Cursor: "#f8f8f2", CursorAccent: "#272822",
		Selection: "#49483e",
		Black:       "#272822", Red: "#f92672", Green: "#a6e22e", Yellow: "#f4bf75",
		Blue:        "#66d9ef", Magenta: "#ae81ff", Cyan: "#a1efe4", White: "#f8f8f2",
		BrightBlack: "#75715e", BrightRed: "#f92672", BrightGreen: "#a6e22e", BrightYellow: "#f4bf75",
		BrightBlue:  "#66d9ef", BrightMagenta: "#ae81ff", BrightCyan: "#a1efe4", BrightWhite: "#f9f8f5",
	},
	{
		ID: "gruvbox-dark", Name: "Gruvbox Dark", Source: "builtin",
		Background: "#282828", Foreground: "#ebdbb2",
		Cursor: "#ebdbb2", CursorAccent: "#282828",
		Selection: "#504945",
		Black:       "#282828", Red: "#cc241d", Green: "#98971a", Yellow: "#d79921",
		Blue:        "#458588", Magenta: "#b16286", Cyan: "#689d6a", White: "#a89984",
		BrightBlack: "#928374", BrightRed: "#fb4934", BrightGreen: "#b8bb26", BrightYellow: "#fabd2f",
		BrightBlue:  "#83a598", BrightMagenta: "#d3869b", BrightCyan: "#8ec07c", BrightWhite: "#ebdbb2",
	},
	{
		ID: "tomorrow-night", Name: "Tomorrow Night", Source: "builtin",
		Background: "#1d1f21", Foreground: "#c5c8c6",
		Cursor: "#c5c8c6", CursorAccent: "#1d1f21",
		Selection: "#373b41",
		Black:       "#1d1f21", Red: "#cc6666", Green: "#b5bd68", Yellow: "#f0c674",
		Blue:        "#81a2be", Magenta: "#b294bb", Cyan: "#8abeb7", White: "#c5c8c6",
		BrightBlack: "#969896", BrightRed: "#cc6666", BrightGreen: "#b5bd68", BrightYellow: "#f0c674",
		BrightBlue:  "#81a2be", BrightMagenta: "#b294bb", BrightCyan: "#8abeb7", BrightWhite: "#ffffff",
	},
	// ANSI palette built from Bang Wong's colorblind-safe color set
	// (Nature Methods, 2011). The red and green slots are vermillion and
	// bluish-green — perpendicular to the protan/deutan confusion axis, so
	// "up" vs "down" output stays distinguishable for ~6% of men who
	// otherwise see standard red and green as similar.
	{
		ID: "colorblind-safe", Name: "Colorblind Safe", Source: "builtin",
		Background: "#1a1a1a", Foreground: "#e0e0e0",
		Cursor: "#e0e0e0", CursorAccent: "#1a1a1a",
		Selection: "#3a3a3a",
		Black:       "#000000", Red: "#d55e00", Green: "#009e73", Yellow: "#f0e442",
		Blue:        "#0072b2", Magenta: "#cc79a7", Cyan: "#56b4e9", White: "#e0e0e0",
		BrightBlack: "#666666", BrightRed: "#f08a3e", BrightGreen: "#33c49f", BrightYellow: "#f8f070",
		BrightBlue:  "#3e9dd8", BrightMagenta: "#e0a4c3", BrightCyan: "#85ccf1", BrightWhite: "#ffffff",
	},
	// Monochrome green phosphor — the pure VT100 / IBM 3270 look. Every
	// ANSI slot maps to a shade of green so output stays single-hue.
	// Bright phosphor = active ("up", "ok"); dim green = inactive/failed;
	// the effect is that status distinctions come across as luminance rather
	// than color, exactly like a real green CRT. Pairs with the CRT skin.
	{
		ID: "crt-phosphor", Name: "CRT Phosphor (Green)", Source: "builtin",
		Background: "#000000", Foreground: "#33ff33",
		Cursor: "#33ff33", CursorAccent: "#000000",
		Selection: "#1a3a1a",
		Black:       "#000000", Red: "#1a4a1a", Green: "#33ff33", Yellow: "#88ff88",
		Blue:        "#2e5e2e", Magenta: "#5aaa5a", Cyan: "#aaffaa", White: "#33ff33",
		BrightBlack: "#4a5a4a", BrightRed: "#2a6a2a", BrightGreen: "#77ff77", BrightYellow: "#bbffbb",
		BrightBlue:  "#4a8a4a", BrightMagenta: "#77bb77", BrightCyan: "#ddffdd", BrightWhite: "#ffffff",
	},
	// 80s retrofuture / synthwave palette: deep purple backdrop, hot-pink
	// magenta, electric cyan, neon green, acid yellow. Status colors hit
	// hard — "down" is neon orange, "up" is bright lime. Pairs with the
	// Cyberpunk skin for a matched look but lives fine on any dark skin.
	{
		ID: "synthwave", Name: "Synthwave", Source: "builtin",
		Background: "#120522", Foreground: "#f0e6ff",
		Cursor: "#ff006e", CursorAccent: "#120522",
		Selection: "#3a1a5a",
		Black:       "#0a0317", Red: "#ff4500", Green: "#39ff14", Yellow: "#ffe600",
		Blue:        "#00f0ff", Magenta: "#ff006e", Cyan: "#a0ffff", White: "#f0e6ff",
		BrightBlack: "#4a3a5a", BrightRed: "#ff6533", BrightGreen: "#7dff5a", BrightYellow: "#ffff33",
		BrightBlue:  "#55ffff", BrightMagenta: "#ff33a0", BrightCyan: "#c0ffff", BrightWhite: "#ffffff",
	},
}
