# Changelog

Notable user-facing changes to Baudrun. Follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning is
[SemVer](https://semver.org). The per-commit history lives in `git log`
and on the GitHub [Releases](https://github.com/packetThrower/Baudrun/releases)
page's auto-generated "What's Changed" lists; this file is the curated
view of what actually matters to a user reading release notes.

Pre-release tags (`vX.Y.Z-beta.N`, `vX.Y.Z-rc.N`) trigger the same
release workflow but publish under GitHub's "Pre-release" badge and
don't displace the "Latest release" pointer. The `[Unreleased]`
section collects changes as they land on main; it becomes the
final stable entry at tag time.

## [Unreleased]

### Added

- **Direct USB-serial access on Linux and macOS**, via the new
  [`usbserial-go`](https://github.com/packetThrower/usbserial-go) library.
  CP210x adapters — including vendor-rebranded VIDs like the Siemens
  RUGGEDCOM USB Serial console at `0908:01ff` — now open straight
  through libusb with no vendor driver install. Devices surface in the
  port picker as `USB · VID:PID — product name` alongside the regular
  `/dev/*` entries. The driver-missing banner stays silent for any
  chipset the library can open directly. See
  [docs/ADAPTERS.md](docs/ADAPTERS.md) for the full per-chipset
  × per-OS support matrix.
- **Configurable scrollback buffer.** Settings → Advanced → Scrollback
  gives a presets list (1k / 5k / 10k default / 50k / 100k lines) with
  memory-estimate hints. Custom values set directly in `settings.json`
  are preserved and shown as "N lines (custom)" in the picker.
- **Linux `60-baudrun-serial.rules` udev rule** shipped with the
  `.deb` / `.rpm` / `.pkg.tar.zst` packages. Uses `TAG+="uaccess"` so
  the currently-logged-in console user can open serial adapters
  without `sudo` or `dialout` / `plugdev` group membership. Full
  walkthrough in [docs/ADAPTERS.md](docs/ADAPTERS.md#linux-mainline-kernel-module).
- **Helpful error enrichment** when a port open fails with permission
  denied (AppImage or manual-build users who don't get the udev rule
  installed). Error text now names the exact `sudo usermod -aG
  dialout $USER` fix rather than bare "permission denied".
- **Downloads badge** on the README meta row.
- **GitHub Sponsors card** on the docs home page.
- **Custom `Select` component** with full keyboard navigation
  (arrows, Home/End, Enter, Escape, typeahead), portaled popover that
  escapes stacking contexts, and automatic skin theming.
- **Keyboard shortcuts for Clear / Send Break / Suspend.** Default
  bindings are platform-appropriate: `⌘K` / `⌘⇧B` / `⌘⇧S` on macOS,
  `Ctrl+Shift+K` / `Ctrl+Shift+B` / `Ctrl+Shift+S` on Linux and
  Windows. The split scheme preserves plain `Ctrl+letter`
  passthroughs (XOFF, VT, STX, etc.) to the serial device. Full
  details in [docs/SHORTCUTS.md](docs/SHORTCUTS.md).
- **Editable keyboard shortcuts.** Settings → Keyboard Shortcuts
  lets you rebind each session-level action to any combo you want,
  with a click-to-record widget and per-binding reset-to-default.
  Overrides persist in `settings.json` under a `shortcuts` map.

### Changed

- **Syntax-highlighted scrollback survives zoom.** Cmd/Ctrl + / − no
  longer strips the existing history's colors. Uses
  `@xterm/addon-serialize` for the rebuild snapshot so SGR attributes
  round-trip correctly through the dispose-and-rebuild path.
- **Connect button enables the moment you pick a port.** Previously
  stayed disabled until you explicitly Saved. If the form has unsaved
  changes when you click Connect, it now saves first so the backend
  opens the port with the config you actually see on screen.
- **Every dropdown replaced with a themed custom `Select` popover.**
  Native `<select>` popups were rendered by the OS (GTK on Linux,
  Chromium on Windows, the system popup on macOS) and didn't follow
  app skins or the light/dark appearance setting. The new popover
  renders in CSS so every skin — Synthwave and Blueprint included —
  themes the open dropdown the same way they theme the rest of the
  app.
- **macOS release split by architecture.** Earlier versions shipped
  a single universal `.app`; now produces separate
  `Baudrun-macOS-arm64-<version>.zip` (Apple Silicon) and
  `Baudrun-macOS-amd64-<version>.zip` (Intel). Each bundles
  `libusb-1.0.0.dylib` under `Contents/Frameworks/` so the `.app` is
  self-contained and works without Homebrew. Homebrew's libusb is
  per-arch only, so per-arch builds sidestep the universal-dylib
  `lipo` dance.
- **Release notes on GitHub Releases** are now a hand-curated
  "What's Changed" commit list between the previous tag and the
  current one, derived from `git log --no-merges`. Previously the
  notes were a single-line link to the compare view.
- **Bundle ID renamed** from `com.wails.Baudrun` to
  `io.github.packetThrower.Baudrun` so the installed app has a
  stable, repo-anchored identifier.

### Fixed

- **Cyberpunk (Synthwave) and Blueprint grid overlays** rendered at
  wildly wrong cell sizes on Linux — "sometimes normal, sometimes
  huge" depending on DPR and window size. Root cause was WebKit2GTK's
  inconsistent handling of 1px stop bands inside
  `repeating-linear-gradient`. Rewritten to the standard tile-based
  pattern (`linear-gradient … 0 0 / 40px 40px repeat`) which renders
  identically on every engine.
- **Popover z-index conflicts with the full-screen overlay.** The
  `body::after` decorative overlay used for skin-level grid/scan-line
  effects was at `z-index: 9999`, tying with the new Select popover
  and paint-ordering on top of it via DOM order. Overlay moved to
  `z-index: 50`, popover stays at `9999`.

### Ops & packaging

- CI runner matrix swapped `macos-13` (retired) for `macos-15-intel`.
- CI + release workflows install libusb (`libusb-1.0-0-dev` on apt,
  `brew install libusb pkg-config` on macOS) to satisfy `gousb`'s cgo
  link.
- Linux packages declare the matching runtime dep per format:
  `libusb-1.0-0` on `.deb`, `libusbx` on `.rpm`, `libusb` on Arch
  pacman.
- AppImage bundles `libusb-1.0.so.0` into `AppDir/usr/lib/` and
  AppRun prepends that to `LD_LIBRARY_PATH` so the packaged copy is
  found ahead of anything on the host.

## [0.7.0] — 2026-04-22

See the [GitHub release notes](https://github.com/packetThrower/Baudrun/releases/tag/v0.7.0)
for the full list. Highlights:

- Auto-reconnect on by default with a "reconnecting…" status indicator
  that stays visible while the profile is suspended.
- Virtual-serial tool (`scripts/virtual-serial`) — baud-throttled pty
  pair for dev testing without real hardware.
- Paste safety (multi-line confirmation + slow paste) made to actually
  work end-to-end against WKWebView and set on by default for new
  profiles.

## [0.6.0] — 2026-04-22 and earlier

See [GitHub Releases](https://github.com/packetThrower/Baudrun/releases)
for full notes. Earlier versions landed the skin system, profile
form, XMODEM/YMODEM file transfer, hex send, session logging, and
the initial cross-platform release pipeline.
