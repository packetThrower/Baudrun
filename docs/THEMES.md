# Authoring custom themes

Themes control the **terminal viewport's color scheme** — the 16 ANSI
slots plus cursor, selection, and background/foreground. Distinct
from **skins**, which restyle the surrounding app chrome. A profile
picks one theme for its terminal; the rest of the app stays on
whatever skin is active.

## File location

Custom themes live under the user config dir:

- **macOS**: `~/Library/Application Support/Seriesly/themes/<id>.json`
- **Windows**: `%APPDATA%\Seriesly\themes\<id>.json`
- **Linux**: `$XDG_CONFIG_HOME/Seriesly/themes/<id>.json` (usually
  `~/.config/Seriesly/themes/`)

Two ways to add a custom theme:

1. **Import `.itermcolors`** via **Settings → Installed Themes →
   Import .itermcolors…**. Runs a native file-picker; the app parses
   iTerm2's XML plist format and writes a normalized JSON copy into
   the themes directory.
2. **Drop a raw JSON file** into the themes directory. On next launch
   (or re-open of Settings), the theme appears in the picker. No UI
   importer for JSON — hand-editing is a feature, not a workflow.

Deleting a user theme from Settings also removes its file.

## Built-in themes

| ID                | Name                  | Pairs nicely with skin     |
| ----------------- | --------------------- | -------------------------- |
| `seriesly`        | Seriesly              | Seriesly (default)         |
| `dracula`         | Dracula               | any dark skin              |
| `solarized-dark`  | Solarized Dark        | any dark skin              |
| `solarized-light` | Solarized Light       | any light skin             |
| `nord`            | Nord                  | any dark skin              |
| `one-dark`        | One Dark              | any dark skin              |
| `monokai`         | Monokai               | any dark skin              |
| `gruvbox-dark`    | Gruvbox Dark          | any dark skin              |
| `tomorrow-night`  | Tomorrow Night        | any dark skin              |
| `colorblind-safe` | Colorblind Safe       | any skin — see note        |
| `crt-phosphor`    | CRT Phosphor (Green)  | **CRT** skin               |
| `synthwave`       | Synthwave             | **Cyberpunk** skin         |

**Colorblind Safe** uses Bang Wong's palette
([Nature Methods, 2011](https://www.nature.com/articles/nmeth.1618)).
Red and green slots are vermillion and bluish-green — perpendicular to
the protan/deutan confusion axis — so `up` vs. `down` output stays
distinguishable for the ~6% of men with red-green colorblindness.

**CRT Phosphor** is monochrome green: every ANSI slot is a shade of
green, so status distinctions read as luminance rather than hue. Pairs
with the CRT skin for a matched single-hue aesthetic.

**Synthwave** is a neon palette (hot-pink magenta, electric cyan,
acid yellow) over a near-black canvas. Status colors hit hard for
contrast against a busy Cyberpunk skin.

## JSON schema

```json
{
  "id": "my-theme",
  "name": "My Theme",
  "source": "user",
  "background": "#0b0b0d",
  "foreground": "#e4e4e7",
  "cursor": "#ffffff",
  "cursorAccent": "#0b0b0d",
  "selection": "#1a3a5c",
  "selectionForeground": "#ffffff",
  "black":       "#1e1e22",
  "red":         "#ff6961",
  "green":       "#7cd992",
  "yellow":      "#f5d76e",
  "blue":        "#6cb6ff",
  "magenta":     "#d794ff",
  "cyan":        "#7ce0e0",
  "white":       "#d4d4d8",
  "brightBlack":    "#4a4a52",
  "brightRed":      "#ff8a80",
  "brightGreen":    "#a2e5b3",
  "brightYellow":   "#fce488",
  "brightBlue":     "#94ccff",
  "brightMagenta":  "#e5b6ff",
  "brightCyan":     "#a6ecec",
  "brightWhite":    "#ffffff"
}
```

### Fields

| Field                   | Required | Purpose                                                                                     |
| ----------------------- | -------- | ------------------------------------------------------------------------------------------- |
| `id`                    | no       | Stable slug. Omitted IDs are derived from `name`; clashes get a numeric suffix on import.   |
| `name`                  | **yes**  | Display name in the theme picker.                                                           |
| `source`                | no       | Written automatically; importer sets `"user"` on import.                                    |
| `background`            | **yes**  | Terminal viewport background.                                                               |
| `foreground`            | **yes**  | Default text color.                                                                         |
| `cursor`                | **yes**  | Cursor block fill.                                                                          |
| `cursorAccent`          | no       | Character color under the cursor. Defaults to `background` if unset.                        |
| `selection`             | **yes**  | Selection highlight background.                                                             |
| `selectionForeground`   | no       | Text color inside the selection. Unset leaves xterm in its "use foreground" default.        |
| `black` … `brightWhite` | **yes**  | The 16 ANSI slots (indexes 0-15). Devices pick these via SGR codes `30-37`, `40-47`, `90-97`, `100-107`. |

All color values accept any format xterm.js understands — `#rrggbb`,
`#rgb`, `rgb()`, `rgba()`, or CSS named colors.

## Variable meaning (ANSI palette)

| Index | JSON key         | Common SGR usage                              |
| ----- | ---------------- | --------------------------------------------- |
| 0     | `black`          | Foreground 30 / background 40                 |
| 1     | `red`            | 31 / 41 — errors, `down`, `failed` in highlighter |
| 2     | `green`          | 32 / 42 — `up`, `ok`, `active` in highlighter |
| 3     | `yellow`         | 33 / 43 — warnings                            |
| 4     | `blue`           | 34 / 44 — interface names in highlighter      |
| 5     | `magenta`        | 35 / 45 — MAC addresses in highlighter        |
| 6     | `cyan`           | 36 / 46 — IPv4/IPv6 in highlighter            |
| 7     | `white`          | 37 / 47                                       |
| 8     | `brightBlack`    | 90 / 100 — dim gray (timestamps/dates in highlighter) |
| 9-14  | `brightRed`…`brightCyan` | 91-96 / 101-106 — bright variants            |
| 15    | `brightWhite`    | 97 / 107                                      |

Seriesly's **syntax highlighter** uses the ANSI slots by name — if
you change `red` you change what `down` looks like. Device-sourced
colors (ANSI CSI sequences in the serial stream) also map to the
ANSI slots, so themes affect both highlighter output and raw device
colors uniformly.

## Previewing

**Settings → Installed Themes → Preview** opens a modal with a canned
RuggedCom-style output sample — prompts, interface status, MAC
addresses, IPs, timestamps, warnings, errors. Shows the palette
applied through the highlighter against realistic network-gear text
so you can judge a theme without switching and reconnecting.

## Picking a theme

- **Global default** — Settings → Default Theme. Applied to any
  profile that doesn't set its own.
- **Per-profile override** — each profile's form has its own theme
  dropdown. Choose "Default" there to fall back to the global.

A profile's theme wins over the global default; skins never interact
with theme selection.

## Creating a custom theme

### From `.itermcolors`

The fastest path. iTerm2's color-scheme ecosystem has thousands of
themes at [iterm2colorschemes.com](https://iterm2colorschemes.com/),
[Gogh](https://github.com/Gogh-Co/Gogh), and plenty of individual
gists. Grab an `.itermcolors` file and use **Settings → Import**.

The importer parses the plist format via `howett.net/plist`, maps
ANSI 0-15 to `black`-`brightWhite`, and uses the file basename
(minus `.itermcolors`) as the display name and ID slug.

### From scratch (JSON)

Drop a JSON file matching the [schema](#json-schema) above into the
themes directory. Re-open Settings (no restart needed) and it appears
in the picker. All 22 color fields should be set; missing ANSI slots
render as empty strings and xterm falls back to its built-in
defaults, which almost certainly clash.

## Where `.itermcolors` themes come from

- [iterm2colorschemes.com](https://iterm2colorschemes.com/) — hundreds
  of curated themes, preview images included.
- [mbadolato/iTerm2-Color-Schemes](https://github.com/mbadolato/iTerm2-Color-Schemes)
  — the GitHub repo many of the above draw from. Contains `.itermcolors`
  sources plus exports for other terminals.
- [Gogh](https://github.com/Gogh-Co/Gogh) — curated set, primarily
  distributed as shell scripts for other terminals; the repo includes
  `.itermcolors` where available.

## Tips

- **Background contrast with your skin** — if the skin's `--bg-main`
  and the theme's `background` are close in hue/lightness, the
  terminal edge blends into the rest of the UI. Either pick a theme
  with a distinct background or set `--bg-terminal` in your skin to
  match the theme.
- **Bright vs. normal slots** — devices use bright when bold attribute
  is active. Low contrast between normal and bright kills
  `show version` readability on most network gear.
- **Check the highlighter** — `up` / `down` / interface names get
  colored by the highlighter using `green` / `red` / `blue`. If any
  of those are hard to read on your background, the highlighter turns
  into visual noise.
- **Test on real output** — a theme that looks great on
  `ls --color` might struggle on a Cisco `show interface` wall of
  text. Preview is a good start; a real session reveals the rest.

## Sharing

Themes are plain JSON. Export path: the user themes directory. Drop
the file where recipients can find it — no registry, no server. For
broader distribution, publish the `.itermcolors` source and link to
one of the ecosystems above.
