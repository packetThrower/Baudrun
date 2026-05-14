---
title: Accessibility
description: 'Built-in accessibility features, what the OS-level preferences gate, and the known gaps.'
editUrl: https://github.com/packetThrower/Baudrun/edit/main/docs/ACCESSIBILITY.md
---

Reference for the accessibility features Baudrun ships with, the
OS settings it respects, and the known gaps. Baudrun is a native
gpui app — the chrome and terminal grid are GPU-rendered rather
than HTML — so the conventional web-accessibility surface (ARIA
roles, `aria-live` regions, `prefers-reduced-motion` CSS) doesn't
apply. The platform-level accessibility hooks (NSAccessibility on
macOS, UI Automation on Windows, AT-SPI on Linux) are gpui's
responsibility and are still maturing upstream.

## Reduced motion

Baudrun reads the OS-level "reduce motion" preference once at app
launch and gates its optional animations off it.

**Where the setting lives:**

| OS | Location |
|---|---|
| macOS | System Settings → Accessibility → Display → **Reduce motion** |
| Windows | Settings → Accessibility → Visual effects → **Animation effects** (off) |
| Linux (GNOME) | Settings → Accessibility → **Reduce animations** |
| Linux (KDE) | System Settings → Accessibility → **Animation speed → Disabled** |

**What's gated on it:**

- The **reconnecting pulse**: the amber dot in the session header
  while auto-reconnect polls for the port. With Reduce Motion on,
  the dot stays visible as a static amber indicator instead of
  pulsing.
- The **terminal cursor blink**: with Reduce Motion on, the cursor
  stays solid instead of toggling visible / invisible at the
  half-second cadence.
- The **port-scanning indicator** in the footer, where applicable.

**Settings → Accessibility → Reduce Motion** in the app shows the
current value (read at launch) plus a one-line pointer to the OS
path that controls it.

**Caveat:** the preference is read once at launch and cached for
the session. Flipping the system toggle while Baudrun is running
won't change behaviour until the app is quit and relaunched.
Detecting live changes is a follow-up; the OS query API is
synchronous and we'd need an OS-event subscription to react
without polling.

## Terminal zoom

| Action | macOS | Windows / Linux |
|---|---|---|
| Increase font size | `⌘=` (or `⌘+`) | `Ctrl+=` (or `Ctrl++`) |
| Decrease font size | `⌘-` | `Ctrl+-` |
| Reset to default (13 px) | `⌘0` | `Ctrl+0` |

Clamped to 8–28 px. The status bar reports `Font size: N` after
each change so the keystroke is acknowledged even when the visible
delta is small. The size persists across launches under
`Settings.fontSize` and applies live — the grid re-measures cell
dimensions and reflows without restart.

## Keyboard reachability

Every menu-bar action and every customisable shortcut in
**Settings → Shortcuts** is keyboard-reachable. The full set
covers profile editing, connect / disconnect, suspend / resume,
clear, send break, file transfer, copy / paste / select all,
terminal zoom, and new-window / new-profile. See
[Keyboard shortcuts](/Baudrun/usage/shortcuts/) for the full
table and the per-OS rationale.

The right-click context menu on the terminal grid offers
Copy / Paste / Select All / Clear as a mouse alternative to the
keybindings.

## Skin choices

Two skins are explicitly accessibility-oriented and ship with the
app:

- **High Contrast** — exaggerates the luminance gap between
  surfaces, text, and accents. Black backgrounds with bright
  borders; useful when ambient glare or low-vision conditions
  wash out subtle palette gradations.
- **Colorblind Safe** (terminal theme) — uses Bang Wong's palette
  from *Nature Methods* 2011, which sits perpendicular to the
  protan / deutan confusion axis. The eight ANSI slots stay
  distinguishable across the common red-green vision deficiencies.

Pick either from **Settings → App Skin** / **Settings →
Themes**.

## Not yet supported

Be honest with users — these are real gaps:

- **Screen-reader output for the terminal grid.** gpui doesn't
  yet route alacritty_terminal grid content through the platform
  accessibility APIs (NSAccessibility on macOS, UI Automation on
  Windows, AT-SPI on Linux), so a screen reader pointed at
  Baudrun won't narrate output as it arrives. This is the most
  significant gap; tracking upstream gpui's accessibility work.
- **Screen-reader labels on chrome controls.** The sidebar `+` /
  gear / new-window icons, the session-header buttons, and the
  Settings rail rows don't yet announce themselves through the
  OS accessibility APIs. Hover tooltips remain available for
  sighted users.
- **Live-reactive Reduce Motion.** As above — preference is read
  at launch only; relaunch picks up a change.
- **Per-skin focus-ring customisation.** All skins currently use
  gpui's default focus indicator. Adding a skin-authored token
  for the focus ring colour / thickness is on the follow-up list.
- **Per-user "force high contrast" override.** Today the High
  Contrast skin is the explicit a11y surface; no toggle layers
  high-contrast over an arbitrary skin.

If any of these blocks your use case, please open an issue on
the [GitHub tracker](https://github.com/packetThrower/Baudrun/issues)
so we can prioritise against the rest of the queue.
