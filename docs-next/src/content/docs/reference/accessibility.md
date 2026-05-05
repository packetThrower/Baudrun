---
title: Accessibility
description: 'Built-in accessibility features, screen-reader integration, and known caveats.'
editUrl: https://github.com/packetThrower/Baudrun/edit/main/docs/ACCESSIBILITY.md
---

Reference for the accessibility features Baudrun ships with, the
OS settings they interact with, and the known gaps.

## Screen reader support

- Settings → Advanced → **Enable xterm screen-reader mode**.
- Backed by `Settings.screenReaderMode`; pushed into xterm via
  `term.options.screenReaderMode` at init and on every subsequent
  toggle.
- When enabled, xterm routes incoming terminal output through an
  ARIA live region, which macOS VoiceOver / Windows Narrator / NVDA
  / Orca can narrate.
- Off by default — there's a small performance cost on heavy
  output (the live region gets updated on every write), so users
  who don't need it don't pay for it.

**Verification:** with the toggle on, run in the app's DevTools
console:

```js
document.querySelector('.xterm-accessibility')
```

If it returns an element, xterm is in screen-reader mode. If
`null`, the toggle didn't land.

## Reduced motion

Baudrun respects the OS-level `prefers-reduced-motion` preference.

**Where the setting lives:**

| OS | Location |
|---|---|
| macOS | System Settings → Accessibility → Display → **Reduce motion** |
| Windows | Settings → Accessibility → Visual effects → **Animation effects** (off) |
| Linux (GNOME) | Settings → Accessibility → **Reduce animations** |

**What's gated on it:**

- The **reconnecting pulse** — the amber dot in the session header
  while auto-reconnect polls for the port. With Reduce Motion on,
  the dot stays visible as a static amber indicator instead of
  pulsing.
- The **port-scanning pulse** — the accent-colored dot next to the
  "Scanning for COM ports…" status in the footer. Static under
  Reduce Motion.

**Caveat:** WKWebView on macOS sometimes caches the
`prefers-reduced-motion` value at page load. If the setting is
toggled while Baudrun is open, the animations may not change
until the app is quit and relaunched. This is an upstream
WebKit behavior shared across Tauri / Electron / any embedded
WebKit consumer, not something Baudrun can work around.

## Terminal zoom

- **Cmd + "="** / **Ctrl + "="** (or the literal `+` key): font +1
- **Cmd + "-"** / **Ctrl + "-"**: font −1
- **Cmd + "0"** / **Ctrl + "0"**: reset to 13 (the app default)

Clamped 8-28 px. The shortcut also echoes `Font size: N` to the
status bar so users get immediate feedback that the change
registered.

Zoom writes through to `Settings.fontSize`, so it persists across
app launches. The xterm instance rebuilds on each change to pick
up the new cell metrics — xterm caches glyph dimensions at
construction, so an in-place font-size change wouldn't re-measure.

## Session shortcuts

For the three session-header buttons that are otherwise mouse-only
— **Clear**, **Send Break**, **Suspend**. See
[Keyboard shortcuts](/Baudrun/usage/shortcuts/) for the default bindings table,
the per-OS rationale, and how to rebind them via
Settings → Keyboard Shortcuts.

Each shortcut gates on the same enablement as the button it
mirrors — Break and Suspend are no-ops without an active
(unsuspended, not-reconnecting) session; Clear needs the terminal
view to be up. The shortcuts also appear in each button's
`title` tooltip and `aria-keyshortcuts` attribute so
keyboard-first and screen-reader users discover them without
having to read this page.

## ARIA labels

Every icon-only or text-light control has an explicit `aria-label`:

- Sidebar → **New Profile** (+ icon)
- Profile form → **Rescan ports** (↻ icon), **Dismiss driver notice**
  (× on the driver-install banner)
- Settings → **Preview theme**, **Remove theme**, **Remove skin**
- Session header → **More actions** (⋯ overflow menu)
- Modal dismiss buttons (×) on Hex send, File transfer, Theme preview

Text-labeled controls (DTR/RTS pills, Clear, Suspend, Disconnect,
Send Break, Send Hex…, Send File…, Settings) rely on their visible
text.

## Not yet supported

- **High-contrast theme mode independent of skin.** Today the
  High Contrast skin is the explicit a11y surface; there's no
  per-user "force high contrast" toggle that overrides the chosen
  skin. Likely unnecessary since the skin exists, but noted.
- **Focus-ring overrides per skin.** Some skins don't currently
  set a custom `:focus-visible` style; the browser default falls
  through, which is usable but could be more consistent.

## Verifying with DevTools

For debug builds, right-click inside the app → Inspect. The
DevTools Rendering panel can emulate accessibility preferences
without changing the OS setting:

- **Emulate CSS media feature** → `prefers-reduced-motion: reduce`
  to test the static-dot behavior.
- **Emulate vision deficiencies** → blurred vision, achromatopsia,
  etc. — useful for evaluating color contrast on themes and skins.
