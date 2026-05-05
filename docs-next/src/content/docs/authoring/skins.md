---
title: App skins
description: 'Authoring custom Baudrun chrome skins — colors, typography, radii, and elevation.'
editUrl: https://github.com/packetThrower/Baudrun/edit/main/docs/SKINS.md
---

Skins swap Baudrun's app chrome — colors, typography, radii, elevation,
layout shape. Distinct from **themes**, which only recolor the terminal
viewport. A skin is a flat map of CSS custom-property values applied to
`document.documentElement`.

## File location

Custom skins live under the user config dir, not inside the app bundle:

- **macOS**: `~/Library/Application Support/Baudrun/skins/<id>.json`
- **Windows**: `%APPDATA%\Baudrun\skins\<id>.json`
- **Linux**: `$XDG_CONFIG_HOME/Baudrun/skins/<id>.json` (usually
  `~/.config/Baudrun/skins/`)

Drop a JSON file there manually, or use **Settings → App Skin → Import**
which runs a native file-picker and copies it into the directory. Deleting
a user skin from Settings also removes the file.

## JSON schema

```json
{
  "id": "cobalt-pro",
  "name": "Cobalt Pro",
  "description": "Muted blue on slate, high-contrast accents.",
  "supportsLight": true,
  "vars": { "--bg-main": "#1a2333", "--fg-primary": "#e6ecf5" },
  "darkVars": { },
  "lightVars": { "--bg-main": "#f5f5f7", "--fg-primary": "#1d1d1f" }
}
```

| Field           | Required | Notes                                                                                                                                                                  |
| --------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id`            | no       | Stable slug. If omitted, derived from `name` (lowercased, hyphenated). On import, a numeric suffix (`-2`, `-3`) is appended if it clashes with an existing skin.       |
| `name`          | **yes**  | Display name shown in the Skin picker.                                                                                                                                 |
| `description`   | no       | One-line hint, reserved for future UI surfaces.                                                                                                                        |
| `supportsLight` | no       | Default `false`. Set `true` if your skin has a working `lightVars` overlay. When `false`, the skin is pinned dark regardless of the user's Appearance preference.      |
| `vars`          | yes\*    | Always-applied variables. Effectively the dark base, since the window material itself is pinned dark.                                                                  |
| `darkVars`      | no       | Overlay applied when appearance = dark. Usually omitted — most dark skins put everything in `vars`.                                                                    |
| `lightVars`     | no       | Overlay applied when appearance = light. Use opaque surfaces (see [Light / dark handling](#light--dark-handling)).                                                     |

\* At least one of `vars` / `darkVars` / `lightVars` must be non-empty.

All keys must start with `--`. The importer rejects any skin that violates
this or has an empty name / no variables.

## Variable reference

Skins can override any CSS custom property the app's stylesheet reads.
The full list lives in
[`src/style.css`](https://github.com/packetThrower/Baudrun/blob/main/src/style.css) under `:root`.
Grouped for navigation:

### Typography

| Variable                | Example                        | Purpose                      |
| ----------------------- | ------------------------------ | ---------------------------- |
| `--font-ui`             | `-apple-system, sans-serif`    | UI body font                 |
| `--font-mono`           | `"SF Mono", Menlo, monospace`  | Terminal + hex-view          |
| `--font-size-base`      | `13px`                         | Default UI text              |
| `--font-size-label`     | `11px`                         | Form labels                  |
| `--font-size-section`   | `15px`                         | Section headers              |
| `--font-size-h1`        | `24px`                         | Page titles                  |
| `--label-transform`     | `uppercase` / `none`           | Form-label casing            |
| `--label-letter-spacing`| `0.04em`                       | Form-label tracking          |
| `--label-weight`        | `500`                          | Form-label weight            |

### Surfaces (backgrounds)

| Variable           | Purpose                                                          |
| ------------------ | ---------------------------------------------------------------- |
| `--bg-window`      | Root window fill (often transparent)                             |
| `--bg-sidebar`     | Left profile pane                                                |
| `--bg-main`        | Main content pane                                                |
| `--bg-panel`       | Cards / sub-panels inside sections                               |
| `--bg-hover`       | Hover state on interactive rows                                  |
| `--bg-active`      | Selected / active item highlight                                 |
| `--bg-input`       | Text + select inputs                                             |
| `--bg-input-focus` | Focused input                                                    |
| `--bg-terminal`    | Terminal viewport fill (fallback for themes that don't set one)  |
| `--option-bg`      | Dropdown popover background (Select component, opaque)           |
| `--shell-bg`       | Backdrop behind floating panels (Liquid Glass light uses this)   |

### Foreground (text)

| Variable            | Purpose                                       |
| ------------------- | --------------------------------------------- |
| `--fg-primary`      | Body text, strong labels                      |
| `--fg-secondary`    | Section hints, secondary labels               |
| `--fg-tertiary`     | Meta info (timestamps, footnotes)             |
| `--option-fg`       | Dropdown popover option text                  |
| `--option-group-fg` | Dropdown popover group-header label text      |

### Borders

| Variable              | Purpose                                                                    |
| --------------------- | -------------------------------------------------------------------------- |
| `--border-subtle`     | Section dividers, faint lines                                              |
| `--border-strong`     | Emphasized borders                                                         |
| `--input-border-idle` | Input outline when unfocused                                               |
| `--panel-border`      | Full border declaration for cards (`1px solid ...` or `none`)              |
| `--sidebar-divider`   | Line between sidebar and main (full declaration: `1px solid <color>` or `none`) |

### Semantic colors

| Variable         | Purpose                                         |
| ---------------- | ----------------------------------------------- |
| `--accent`       | Primary action color, selected text             |
| `--accent-hover` | Primary on hover                                |
| `--danger`       | Errors, destructive actions                     |
| `--success`      | OK states, connected-session dot                |
| `--warn`         | Warnings, reconnecting pulse                    |

### Radii + elevation

| Variable            | Purpose                                                     |
| ------------------- | ----------------------------------------------------------- |
| `--radius-sm`       | Small inputs, swatches                                      |
| `--radius-md`       | Buttons, cards                                              |
| `--radius-lg`       | Large panels, modals                                        |
| `--shadow-panel`    | Card/panel shadow                                           |
| `--shadow-floating` | Elevated surfaces (modals, dropdowns)                       |
| `--blur-strength`   | Used by `backdrop-filter: blur(var(--blur-strength))`       |

### Layout (floating bubble vs. flush edge)

| Variable            | Purpose                                                                   |
| ------------------- | ------------------------------------------------------------------------- |
| `--shell-padding`   | Outer padding around sidebar + main. `0` = flush, `10px` = floating cards |
| `--shell-gap`       | Gap between sidebar and main                                              |
| `--panel-radius`    | Corner radius on sidebar + main                                           |
| `--panel-shadow`    | Drop shadow on sidebar + main                                             |
| `--titlebar-height` | Space reserved at top for the macOS traffic lights                        |

### Scrollbars

| Variable                  | Purpose             |
| ------------------------- | ------------------- |
| `--scrollbar-thumb`       | Thumb fill          |
| `--scrollbar-thumb-hover` | Thumb on hover      |

### Decorative overlay

| Variable     | Purpose                                                                                                                                              |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--overlay`  | Applied as `body::after` background. Any valid `background` value — images, gradients, repeating patterns. Drives CRT scanlines, Blueprint grid, etc. |

## Light / dark handling

The window's macOS `NSVisualEffectView` material is pinned dark at
startup. Tauri v2's runtime appearance setters don't reliably swap
NSAppearance live on macOS, so the vibrancy material can't follow
the user's light/dark preference. Practical consequence for skin
authors: **light-mode overlays need opaque or near-opaque surface
colors** — translucent light CSS on a dark vibrancy backdrop reads
as muddy gray.

```json
{
  "supportsLight": true,
  "vars": {
    "--bg-main": "rgba(20, 20, 22, 0.55)"
  },
  "lightVars": {
    "--bg-main": "#ffffff"
  }
}
```

For skins that only make sense dark (CRT, synthwave), set
`"supportsLight": false` and omit `lightVars`. The applier pins dark
for those regardless of the user's Appearance preference.

## What you can't do

1. **Per-skin element-level CSS.** Built-ins can target specific
   elements via `[data-skin="<id>"]` selectors in `style.css` (e.g.
   the Windows XP Start-button look on the Settings footer). User
   skins get a unique DOM attribute but no matching rule, because
   the rule is gated on a built-in ID.
2. **Window chrome.** Title-bar style, traffic-light position, and
   bundle background are set in `src-tauri/tauri.conf.json` and the
   `tauri::Builder` setup in `src-tauri/src/lib.rs`. macOS traffic
   lights are repositioned at runtime via `tauri-plugin-decorum` to
   match floating-bubble skin layouts (e.g. Liquid Glass). No CSS
   variable can alter the native window chrome itself.
3. **Appearance modes beyond light / dark.** Only `lightVars` and
   `darkVars` overlays are honored. No sepia, high-contrast, etc.
   beyond what one of those can express.

## Annotated reference skin

For a complete working skin with every option set and a per-field
comment on what it does, see:

- [`docs/https://github.com/packetThrower/Baudrun/blob/main/docs/examples/skin.example.jsonc`](examples/skin.example.jsonc) —
  annotated version, for reading and learning. Not directly
  importable (Baudrun's JSON parser doesn't accept comments).
- [`docs/https://github.com/packetThrower/Baudrun/blob/main/docs/examples/skin.example.json`](examples/skin.example.json) —
  stripped version with the same values, importable via
  **Settings → Installed Skins → Import skin…** or by dropping into
  the skins directory.

The annotated file is the recommended starting point: read it,
copy the stripped version, rename the `id` + `name`, and tweak
the values you care about.

## Example: minimal complete skin

A solid-color dark skin with a cyan accent. Save as `ocean.json` in
the skins directory:

```json
{
  "name": "Ocean",
  "supportsLight": false,
  "vars": {
    "--bg-window": "#0b1b28",
    "--bg-sidebar": "#0e2133",
    "--bg-main": "#12283c",
    "--bg-panel": "#193249",
    "--bg-hover": "rgba(255, 255, 255, 0.05)",
    "--bg-active": "rgba(78, 205, 196, 0.25)",
    "--bg-input": "#0e2133",
    "--bg-input-focus": "#143050",

    "--option-bg": "#0e2133",
    "--option-fg": "#d8e9f5",

    "--fg-primary": "#d8e9f5",
    "--fg-secondary": "#8ba9c0",
    "--fg-tertiary": "#5a7a92",

    "--border-subtle": "rgba(255, 255, 255, 0.06)",
    "--border-strong": "rgba(255, 255, 255, 0.12)",
    "--sidebar-divider": "1px solid rgba(255, 255, 255, 0.08)",

    "--accent": "#4ecdc4",
    "--accent-hover": "#6ed8d1",
    "--danger": "#ff6b6b",
    "--success": "#4ecdc4",
    "--warn": "#ffd93d",

    "--radius-sm": "4px",
    "--radius-md": "6px",
    "--radius-lg": "10px",

    "--shell-padding": "0",
    "--shell-gap": "0",
    "--panel-radius": "0",
    "--panel-shadow": "none",
    "--titlebar-height": "38px"
  }
}
```

Re-import via Settings (or restart the app) and pick "Ocean" from the
Skin dropdown.

## Development tips

- **Iterate with DevTools.** `npm run tauri dev` opens the webview
  inspector (right-click → Inspect Element on the running app).
  Live-tweak variables via
  `document.documentElement.style.setProperty("--bg-main", "#...")` in
  the console until values feel right, then copy into the JSON.
- **Start from a built-in.** Inspect a skin you like with
  `getComputedStyle(document.documentElement).getPropertyValue("--bg-main")`
  to capture the applied values, or read the JSON for each built-in
  skin directly in
  [`src-tauri/resources/builtin_skins.json`](https://github.com/packetThrower/Baudrun/blob/main/src-tauri/resources/builtin_skins.json).
- **Test both appearances** if you set `supportsLight: true`. The
  Appearance dropdown in Settings flips modes without reload.
- **Skins vs. themes.** The terminal viewport is styled by the active
  **theme**, not the skin. `--bg-terminal` is only a fallback; themes
  override it.

## Sharing

Skins are plain JSON. Sharing is email/gist/paste — no registry.
Recipients drop the file into their skins directory or use
Settings → Import.
