---
title: Keyboard shortcuts
description: 'Complete reference for every keyboard shortcut Baudrun honours, plus how to rebind any of them.'
editUrl: https://github.com/packetThrower/Baudrun/edit/main/docs/SHORTCUTS.md
---

Baudrun binds three classes of keyboard shortcuts: a small set of
system bindings (Quit / New Window / Settings) that always fire,
the **Edit menu** clipboard actions (Copy / Paste / Select All)
that fire only when the terminal pane has focus, and a longer set
of **session bindings** for everything from Connect to terminal
zoom. Everything except the three system bindings is rebindable
under **Settings → Shortcuts**.

## System bindings

| Action | macOS | Windows / Linux |
|---|---|---|
| Quit Baudrun | `⌘Q` | `Ctrl+Q` |
| New Window | `⌘N` | `Ctrl+N` |
| Settings | `⌘,` | `Ctrl+,` |

These three aren't user-editable. Exposing them to the override UI
would let you accidentally trap yourself in a window with no way to
reach Quit or Settings.

## Terminal pane bindings

These only fire when the terminal pane has focus — typing `⌘C`
inside a profile-form text input keeps copying the input text the
way you'd expect; the terminal binding doesn't shadow it.

| Action | macOS default | Windows / Linux default |
|---|---|---|
| Copy selection | `⌘C` | `Ctrl+C` |
| Paste | `⌘V` | `Ctrl+V` |
| Select all (full scrollback + viewport) | `⌘A` | `Ctrl+A` |

The right-click context menu on the terminal grid offers the same
three actions plus **Clear**, with **Copy** greyed when there's no
selection.

:::caution
On Windows / Linux, the default `Ctrl+C` for Copy hijacks the wire's
traditional 0x03 / ETX byte. Network engineers who need that byte
to interrupt a remote process can rebind Copy to `Ctrl+Shift+C` in
**Settings → Shortcuts** to free `Ctrl+C` back to the wire. A
"selection-aware" mode (copies when there's a selection, sends
interrupt otherwise) is on the near-term roadmap.
:::

## Session bindings

| Action | macOS default | Windows / Linux default | Scope |
|---|---|---|---|
| Connect | `⌘↩` | `Ctrl+Enter` | A profile is selected and disconnected |
| Disconnect | `⌘⇧D` | `Ctrl+Shift+D` | A live session exists |
| Suspend session | `⌘⇧S` | `Ctrl+Shift+S` | Active session, not already suspended |
| Resume session | `⌘⇧R` | `Ctrl+Shift+R` | Suspended session exists |
| Clear terminal | `⌘K` | `Ctrl+Shift+K` | Terminal view |
| Send Break (300 ms TX-low) | `⌘⇧B` | `Ctrl+Shift+B` | Active session |
| Send file (X/YMODEM) | `⌘⇧T` | `Ctrl+Shift+T` | Active session |
| New profile | `⌘⇧P` | `Ctrl+Shift+P` | Always |
| Open profile in new window | `⌘⇧↩` | `Ctrl+Shift+Enter` | A profile is selected |
| Zoom in | `⌘=` | `Ctrl+=` | Always |
| Zoom out | `⌘-` | `Ctrl+-` | Always |
| Reset zoom (13 px) | `⌘0` | `Ctrl+0` | Always |
| Cancel overflow menu / modal | `Escape` | `Escape` | When open |
| Abort in-flight slow paste | `Escape` | `Escape` | While a slow paste is sending |

## Why the split scheme

The session bindings differ between macOS and the rest because
terminal devices care about what gets sent to them on the wire.

- On **macOS**, `⌘` is never a terminal control character. So the
  app can use `⌘K` by itself for Clear (same convention as
  Terminal.app and iTerm2) and pick any `⌘+letter` it wants without
  fighting for the keystroke with the device.
- On **Linux and Windows**, plain `Ctrl+letter` often *is* a
  control byte the user wants to send: `Ctrl+B` is STX (0x02),
  `Ctrl+K` is VT (0x0B), `Ctrl+S` is XOFF (0x13). Adding `Shift`
  to the session UI shortcuts keeps plain `Ctrl+*` passing
  straight through to the device, while `Ctrl+Shift+*` carves
  out a dedicated UI layer.

The clipboard bindings break that rule on Windows / Linux —
plain `Ctrl+C` / `Ctrl+V` is too entrenched in desktop muscle
memory to give up. See the caution callout above.

## Customizing

**Settings → Shortcuts** lets you rebind every session and
clipboard action independently.

1. Click the binding you want to change. The button flips into
   "Press a key…" mode.
2. Press the new key combination. It's captured and saved
   immediately.
3. Press `Escape` during recording to cancel.
4. Click the **↺** button next to a binding to reset it to the
   platform default.

Overrides are stored in `settings.json` under `shortcuts`, keyed by
action id:

```json
{
  "shortcuts": {
    "clear": "Meta+L",
    "break": "Meta+Alt+B",
    "copy": "Control+Shift+C"
  }
}
```

The string format is [W3C aria-keyshortcuts](https://www.w3.org/TR/wai-aria-1.2/#aria-keyshortcuts)
syntax: modifier names (`Control`, `Meta`, `Alt`, `Shift`) joined
with `+`, followed by the key. Missing entries fall back to the
platform default, so you can override one binding without having
to restate the others. Empty string is treated as "no override"
so the **↺** reset works by clearing rather than deleting.

## Discoverability

Each session-header button and menu item exposes its current
shortcut two ways:

- **Tooltip** on hover, for mouse users who want to check without
  opening Settings.
- **Menu accelerator label** on macOS — the system menu bar
  reads the registered KeyBinding directly, so the displayed
  combo (`⌘C`, `⌘⇧B`, etc.) always matches the current binding
  including overrides.

Both update live when you change a binding. No app relaunch.

## Key contexts

Bindings are scoped so they only fire when the right element has
focus. The Copy / Paste / Select All bindings live in the
`Terminal` context — gpui's keymap dispatcher only matches them
when the terminal pane is in the focus chain. Inside a
profile-form Input widget the same chord falls through to
gpui-component's own Input::Copy, which copies the field's
selected text instead of the (likely empty) terminal selection.
