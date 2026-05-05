---
title: Keyboard shortcuts
description: 'Complete reference for every rebindable keyboard shortcut.'
editUrl: https://github.com/packetThrower/Baudrun/edit/main/docs/SHORTCUTS.md
---

Baudrun binds a small set of global keyboard shortcuts for terminal
zoom and the session-level actions that would otherwise be
mouse-only. The session shortcuts are user-editable; see
[Customizing](#customizing) below.

## Default bindings

| Action            | macOS | Linux / Windows    | Scope                |
|-------------------|-------|--------------------|----------------------|
| Zoom in           | `⌘=` / `⌘+` | `Ctrl+=` / `Ctrl++` | Always |
| Zoom out          | `⌘-` / `⌘_` | `Ctrl+-` / `Ctrl+_` | Always |
| Reset zoom to default (13px) | `⌘0` | `Ctrl+0` | Always |
| Clear terminal    | `⌘K`  | `Ctrl+Shift+K`     | Terminal view        |
| Send Break        | `⌘⇧B` | `Ctrl+Shift+B`     | Active session       |
| Suspend session   | `⌘⇧S` | `Ctrl+Shift+S`     | Active session, not already suspended |
| Cancel overflow menu / modal | `Escape` | `Escape` | When open |
| Abort in-flight slow paste | `Escape` | `Escape` | While a slow paste is sending |

## Why the split scheme

The default bindings differ by OS because terminal devices care
about what gets sent to them.

- On **macOS**, `⌘` is never a terminal control character. So the
  app can use `⌘K` by itself for Clear (same convention as
  Terminal.app and iTerm2) and pick any `⌘+letter` it wants without
  fighting for the keystroke with the device.
- On **Linux and Windows**, `Ctrl+letter` often *is* a control
  byte a user would want to send: `Ctrl+B` is STX (0x02),
  `Ctrl+K` is VT (0x0B), `Ctrl+S` is XOFF (0x13). Binding the app
  actions to plain `Ctrl+K` would silently steal the keystroke
  from devices that rely on it. Adding `Shift` to the UI
  shortcuts keeps plain `Ctrl+*` passing straight through to the
  device, while `Ctrl+Shift+*` carves out a dedicated UI layer.

You can override any of the session bindings to whatever modifier
combo you prefer. The default-scheme rationale only matters if you
stick with the platform defaults.

## Customizing

**Settings → Keyboard Shortcuts** lets you rebind Clear, Send
Break, and Suspend independently.

1. Click the binding you want to change. The button flips into
   "Press a key…" mode.
2. Press the new key combination. It's captured and saved
   immediately.
3. Press `Escape` during recording to cancel.
4. Click the **↺** button next to a binding to reset it to the
   platform default.

Overrides are stored in `settings.json` under `shortcuts`, keyed by
action name:

```json
{
  "shortcuts": {
    "clear": "Meta+L",
    "break": "Meta+Alt+B"
  }
}
```

The string format is [W3C aria-keyshortcuts](https://www.w3.org/TR/wai-aria-1.2/#aria-keyshortcuts)
syntax: modifier names (`Control`, `Meta`, `Alt`, `Shift`) joined
with `+`, followed by the key. Missing entries fall back to the
platform default, so you can override one binding without having
to restate the others.

## Discoverability

Each session-header button exposes its current shortcut two ways:

- **Tooltip** on hover, for mouse users who want to check without
  opening Settings.
- **`aria-keyshortcuts`** attribute, announced by screen readers
  on focus.

Both update live when you change a binding. No app relaunch.

## Interaction with the terminal

When the terminal view has focus, xterm captures most keystrokes
to forward to the device. The shortcut handler sits at the
`window` level and fires in the normal event-bubble phase, so it
doesn't intercept key events meant for the terminal. It only
claims a keystroke when the modifier+key combo matches a bound
shortcut. Plain `Ctrl+C`, `Ctrl+Z`, `Ctrl+D`, `Ctrl+L`, and the rest
continue to reach the device, sending the corresponding control
bytes.
