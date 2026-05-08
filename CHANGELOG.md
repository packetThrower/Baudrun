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

- **WebGL terminal renderer** with graceful DOM fallback. Glyphs
  now render onto a GPU-accelerated canvas instead of DOM `<span>`
  elements, which makes long-buffer scrolling (e.g. paging through
  `show tech-support`) noticeably snappier. The DOM renderer
  remains the fallback when WebGL2 isn't available — software-only
  renderers, headless containers, RDP / X11 forwarding without GLX
  — and on those paths the v0.9.5 CSS-variable backstop carries
  rendering correctness as before. As a side benefit, the entire
  class of "WebKit dropped xterm's runtime stylesheet" rendering
  bugs (ANSI palette, selection background, cursor fill) physically
  cannot recur on the WebGL path, since glyphs are painted with
  explicit per-cell RGBA. Resolves #13.
- **Per-install renderer override.** Setting
  `localStorage.setItem('baudrun-renderer', 'dom')` from DevTools
  forces the DOM path even when WebGL would work. Useful for VMs
  where WebGL falls back to a CPU-only software renderer
  (SwiftShader, Mesa LLVMpipe) and the canvas paint ends up slower
  than DOM. Reload the window after toggling.

### Fixed

- **Multi-second typing lag on Windows-on-ARM.** The `send` Tauri
  command (called on every keystroke) was a sync command, and Tauri
  2 dispatches sync commands on the WebView main thread. WebView2's
  IPC channel on ARM under emulation has enough per-call overhead
  that serializing every keystroke through that thread compounded
  visibly — multi-second lag in extreme cases. Same anti-pattern
  we fixed for the file-picker freeze in v0.9.5-beta.5; this is the
  same bug class wearing a different hat. Marking `send` (plus
  `set_rts`, `set_dtr`, `send_break`, `connect`, `disconnect`)
  `async` moves them onto the Tokio runtime so the main thread
  stays free for inbound echoes and UI repaint. macOS WKWebView
  was unaffected — its IPC dispatch is fast enough that the
  serialization wasn't visible.

### Changed

- **Cargo deps:** `dirs` 5 → 6 (deduplicates the dependency tree;
  many transitive deps were already on 6), `thiserror` 1 → 2.
- **Docs site (Astro + Starlight) deps:** Astro 6.2 → 6.3,
  `@astrojs/starlight` 0.38 → 0.39 — both minor.
- **Frontend deps:** Svelte 5.55.4 → 5.55.5, svelte-check 4.4.6 →
  4.4.8, `@sveltejs/vite-plugin-svelte` 7.0 → 7.1.2,
  `@tauri-apps/plugin-dialog` 2.7.0 → 2.7.1,
  `@tauri-apps/plugin-opener` 2.5.3 → 2.5.4 — all patch / minor.
- **GitHub Actions in the Docs workflow** bumped to current majors
  (`actions/checkout@v6`, `pnpm/action-setup@v6`,
  `actions/setup-node@v6`, `actions/upload-pages-artifact@v5`,
  `actions/deploy-pages@v5`). The CI workflow was already on these
  versions; this just brings Docs to parity.

### Internal

- **Dependabot config rewritten** to match the actual repo layout.
  Previously scanned `gomod` at root (left over from the pre-Tauri
  Go era) and `npm` at `/frontend` (path no longer exists). Now
  scans `cargo` at `/src-tauri`, `npm` at the repo root, `npm` at
  `/docs-next` (the Astro docs project), and GitHub Actions.
  Resolves #14.

## [0.9.5] — 2026-05-07

A long iteration cycle (nine alphas, seven betas) covering a Settings
overhaul, a docs-site rewrite, and a sustained pass on Linux + Windows
rendering quality. Highlights below; the per-commit story lives in
`git log`.

### Added

- **Settings is now its own Tauri window** rather than a modal over
  the session view. Multi-window users can keep Settings open on a
  second monitor while the main window stays focused on the device.
  Cross-window settings sync keeps changes consistent even when the
  user is editing in one window and connected in another.
- **Settings section filter** with `⌘F` / `Ctrl+F` (or `/`) to jump
  between Appearance, Themes, Highlighting, Shortcuts, and Advanced.
  Keywords for App Skin and Shortcut sections are broad so search
  hits intuitive terms.
- **Vertical-tab layout** for Settings and the Profile editor.
  Connection / Highlighting / Advanced groups in the profile form
  mirror Settings's own grouping; less scrolling, clearer scope.
- **Every keyboard shortcut is user-rebindable.** Settings →
  Shortcuts now exposes every action (connect / disconnect, hex
  view, send-break, font-size, copy / paste, send-file, clear,
  toggle-DTR / RTS, etc.) with conflict detection and per-OS
  defaults. Captured key combos render as `⌘+Shift+K` / `Ctrl+Shift+K`
  pills.
- **Highlight pack toggles update existing scrollback live.** Turn
  Cisco IOS off, the Cisco-specific keywords go plain in the lines
  already on screen, not just in newly-arrived output. Same for the
  master Highlight switch and the Timestamps toggle. Per-line
  arrival times are preserved across the replay so toggling
  Timestamps on doesn't re-stamp every historical line as "now."
- **Windows 11 ARM USB-serial detection.** Baudrun's missing-driver
  banner now fires on Win11 ARM (was silently dropping rows due to
  a JSON-parse bug on `Manufacturer: null`). Detection is
  arch-aware: PL2303 cables on Win11 ARM get a tailored message
  ("no compatible driver path; replace with a CP210x or FTDI cable")
  and link to the adapters guide instead of a dead-end Prolific
  download page.
- **OG image, Twitter cards, JSON-LD `SoftwareApplication` schema,
  and Google Search Console verification** on the docs site, so
  link previews and search-engine listings render as a real product
  page rather than a plain text card.

### Changed

- **Docs site rewritten on Astro + Starlight** (replaces MkDocs
  Material). Same content, faster builds, integrated Pagefind
  search, Cards / CardGrid components, custom Hero with
  screenshot-tour link, "Docs" quick-access pill on every page.
  Editorial pass across all twelve pages: removed AI-flavored
  comparison phrasing in favor of feature-focused language;
  no em dashes anywhere on the homepage.
- **Tauri 2.10 → 2.11.1** with matching `@tauri-apps/api` and
  `@tauri-apps/cli` updates. The 2.11.1 patch closes an IPC
  origin-confusion advisory (GHSA-7gmj-67g7-phm9); see Security.
- **Settings UI polish:** flat highlight section instead of nested
  cards, destructive actions (reset, delete pack) prompt for
  confirmation with an undo, ARIA labels on every toggle.

### Fixed

- **Linux + Windows: terminal output renders correctly on every
  theme.** Cisco / Junos / Aruba syntax highlighting was collapsing
  to one color and most themes rendered as a black box on
  WebKit2GTK and WebView2. xterm.js's runtime-injected stylesheet
  was getting silently dropped on these renderers, taking default
  foreground, the 16 ANSI color classes, the cursor's block fill,
  and the selection background with it. Fixed by routing all of
  those through CSS variables on the wrap element instead, so the
  cascade carries them down regardless of whether the injection
  applied. macOS unaffected.
- **Linux + Windows: text selection in connected sessions.** Same
  root cause as above — the selection-overlay rule wasn't
  applying, so click-and-drag computed a range but rendered no
  visible highlight, making selection feel "read-only."
- **Linux + Windows: Send File no longer freezes the app.** The
  native file picker was using a sync Tauri command, which Tauri
  2 dispatches on the WebView main thread. `blocking_pick_file()`
  on that thread deadlocked the event loop on every renderer
  except macOS. Same async-ification applied to `import_theme`,
  `import_skin`, `pick_log_directory`, and `pick_config_directory`.
- **Send File protocol dropdown shows above the modal.** The
  `Select` popover sat at `z-index: 9999` while modal backdrops
  sit at `10000`; portaled-popover stacking happened to work on
  macOS WebKit by source order but not on WebKit2GTK or WebView2.
  Bumped to `10001`.
- **Pager rendering with timestamps on.** Cisco IOS / Aruba CX
  pager prompts (`--More--`) now stamp cleanly, and the next page
  of output lands on its own row with its own stamp. Previously
  the device's redraw left our stamp untouched and a second stamp
  landed on the same row, producing the `[ts1] [ts2] content`
  artifact.
- **Empty `\r\n` lines no longer stamp.** Pressing Enter at a
  Cisco prompt was emitting `[ts]\r\n`, which xterm wrote at the
  cursor's end-of-line position — gluing a phantom stamp onto
  every prompt line.
- **macOS: flash-of-black on window open** in release builds.
  Both the Baudrun skin and Settings substrate now pre-paint via
  `WebviewWindow::background_color` and an inline `<style>` in
  `index.html`, so there's no white-then-skin transition.
- **macOS: header padding offset past the traffic-light zone**
  in Settings and the Profile editor windows so the title and
  pills don't sit under the close / minimize / zoom buttons.
- **Windows XP skin: tab readability.** Active tab was rendering
  blue-on-blue; switched to `var(--fg-primary)`.
- **Settings dir-load + cross-window sync.** The Settings window
  had the wrong substrate color on first paint (transparent
  instead of skin background), and edits didn't propagate back to
  the main window's session header until next launch.
- **Profile form drag-and-drop** when reordering profiles in the
  sidebar.

### Docs

- **macOS PL2303HXA story corrected.** Apple's `AppleUSBPLCOM`
  DEXT matches `067B:2303` with no `bcdDevice` constraint, so the
  legacy chip rev (TRENDnet TU-S9 etc.) is bound out of the box on
  every supported macOS. The "PL2303HXA is broken on macOS"
  narrative was a Windows-driver story leaking into the wrong
  column.
- **New Windows 11 ARM section** in the adapters guide, with
  per-vendor ARM64 driver state: SiLabs CP210x ships via Windows
  Update; FTDI ARM64 driver requires manual install; Prolific HXA
  has no working path; modern Prolific (REV_05+) needs the
  v6.5.0.0 ARM installer.
- **Scoop install on Windows** now mentions `scoop install git`
  as a prerequisite (Scoop's bucket-add fails fast without it).

### Security

- **Tauri 2.11.0 → 2.11.1** patches GHSA-7gmj-67g7-phm9 (Origin
  Confusion: remote pages could invoke local-only IPC commands).
  Practical risk on Baudrun is low because our CSP restricts
  `script-src` to `'self'`, but worth applying.
- **`rand` 0.7.3 advisory** cleared via transitive bumps from
  `cargo update`.

### Internal

- **Dependabot config matches the actual project layout.** Was
  still scanning `gomod` at root (left over from the pre-Tauri Go
  era) and `npm` at `/frontend` (path no longer exists). Now
  scans `cargo` at `/src-tauri`, `npm` at the repo root, `npm` at
  `/docs-next`, and GitHub Actions.

## [0.9.4] — 2026-05-04

### Added

- **Settings header shows the app version.** A small dim
  monospace `v0.9.4` pill sits on the right side of the Settings
  header so it's easy to confirm at a glance which build is
  running. Pulled from `tauri.conf.json` at runtime via the
  `@tauri-apps/api/app` `getVersion()` helper, so it always
  reflects the actual installed bundle.

### Fixed

- **Multi-window: spawned windows were blank and frozen on
  Windows.** Both v0.9.2's URL fix and v0.9.3's CSP fix were
  necessary but not sufficient — the spawned window's renderer
  still couldn't bootstrap because `WebviewWindowBuilder::build()`
  ran synchronously inside the IPC handler. While that handler
  was in flight, Tauri 2's shared Windows-side WebView2 protocol
  dispatcher couldn't service the new window's bootstrap fetches
  (HTML / JS / CSS over `tauri.localhost`, IPC over
  `ipc.localhost`). The renderer stalled on its initial document
  load, the window painted blank, and even F12 / right-click / X
  didn't respond (all need a live renderer). `build()` now runs
  inside `tauri::async_runtime::spawn`, so the IPC handler
  returns the new window's label immediately and the dispatcher
  is free to serve the new window's startup. Right-click + drag
  multi-window now works end-to-end on Windows.
- **`window.set_focus()` removed from the spawn path.** Was the
  first deadlock suspect — `SetForegroundWindow` on Windows waits
  for the target window to become processable, which a Tauri-
  spawned window mid-load isn't. Newly-created windows take
  foreground naturally on every desktop OS, so the explicit call
  was belt-and-braces anyway. Removing it didn't fully unstick
  Windows on its own, but it eliminated one known wait state from
  the IPC handler.

### Docs

- **Note that Scoop needs `git` before adding a third-party
  bucket.** The README + `docs/INSTALL.md` quickstart now run
  `scoop install git` before `scoop bucket add packetThrower
  https://github.com/packetThrower/scoop-bucket`. Scoop fails
  fast with `ERROR Git is required for buckets.` otherwise.

## [0.9.3] — 2026-05-03

### Fixed

- **macOS: Homebrew install was broken because the DMG shipped a
  pre-fix .app.** The release pipeline modifies the macOS bundle
  after Tauri builds it (rewrites the libusb load command via
  `install_name_tool` so the bundled `Frameworks/libusb-1.0.0.dylib`
  loads on user Macs that don't have Homebrew, then re-signs the
  whole bundle ad-hoc). But Tauri creates the DMG during
  `tauri build` *before* that step runs, and we were copying that
  original DMG straight into the release artifacts — so brew users
  got a Baudrun.app with no `Contents/Frameworks/`, a binary still
  pointing at `/opt/homebrew/opt/libusb/lib/`, and a stale signature
  manifest (`spctl` rejected with "code has no resources but
  signature indicates they must be present"). The release workflow
  now rebuilds the DMG via `hdiutil` after the libusb fix + re-sign,
  with a mount-and-`codesign --verify` sanity check baked in. Users
  installing via the `.zip` were unaffected — that artifact already
  packaged the post-fix .app.
- **Multi-window: blank webview on Windows (CSP block).** Spawned
  windows on Windows showed a blank page because every IPC call
  (`list_profiles`, `get_settings`, etc.) was being rejected by the
  Content Security Policy. Tauri 2 on Windows uses
  `http://ipc.localhost` for the IPC protocol; our CSP whitelisted
  only `https://ipc.localhost`. The `Promise.all` in the renderer's
  bootstrap rejected, the UI never rendered, and the blank page
  persisted. Added `http://ipc.localhost` to both `default-src` and
  `connect-src`. macOS / Linux unaffected — they go through `tauri:`
  / `ipc:` schemes that were already in the policy.
- **Multi-window: spawned-window URL still didn't render correctly
  on Windows.** Even after dropping the `?profile=<id>` query string
  in 0.9.2, the Windows webview would render blank with the
  `WebviewUrl::App("index.html".into())` form. Switched to
  `WebviewUrl::default()` (empty PathBuf, the same path the main
  window uses since it has no explicit `url` in
  `tauri.conf.json`) — Tauri's default index resolution is more
  reliable on Windows than the explicit-path form for spawned
  windows.

## [0.9.2] — 2026-05-01

### Fixed

- **Multi-window: blank webview on Windows.** Spawned windows
  opened to a blank white page on Windows. The spawned-window URL
  was assembled as `index.html?profile=<id>`, but `?` is an invalid
  Windows path character — Tauri's `WebviewUrl::App` builds its URL
  from a `PathBuf` and Windows mangled or rejected the value, so
  the document never resolved. The initial profile id is now
  carried through the backend (a per-window pending stash that the
  renderer drains on mount, mirroring the existing
  `take_pending_terminal_snapshot` plumbing) instead of through the
  URL. Cross-platform clean — the spawned URL is plain
  `index.html` everywhere now.
- **Multi-window: drag-out creates a `.txt` file on Linux.** The
  drag handler was setting both `application/x-baudrun-profile`
  (custom MIME, used by our dragend logic) and `text/plain` (the
  profile name). On GTK / Wayland file managers, `text/plain` looks
  like a draggable text snippet — dropping on the desktop made the
  DE create a `.txt` file with the profile name and consume the
  drop before dragend reached our backend cursor-outside check, so
  no new window opened. Removed the `text/plain` payload; only the
  custom MIME is set now, which no DE recognizes, so the drop falls
  through to dragend and spawns the window correctly.

## [0.9.1] — 2026-05-01

### Added

- **Multi-window support.** Right-click any profile in the sidebar
  or drag it out to spawn a fresh window with that profile selected.
  Each window has its own session, so two windows can hold parallel
  serial connections to different devices. When the dragged profile
  is the active connection in the source window, the live session
  and visible scrollback move to the new window — same port, same
  DTR/RTS state, no mid-session bytes lost. Tear-off mid-transfer is
  rejected with a "wait or cancel first" message; everything else
  follows you. See [docs/ADVANCED.md](docs/ADVANCED.md#multi-window)
  for the gesture map and edge cases.

### Fixed

- **Windows: missing minimize / maximize / close buttons**
  ([#7](https://github.com/packetThrower/Baudrun/issues/7)). The
  `tauri-plugin-decorum` overlay-titlebar call was being applied on
  every desktop platform during window setup, but on Windows it
  strips the native frame expecting the renderer to draw its own
  titlebar — Baudrun doesn't, so the system caption buttons went
  missing. Gated the call (and the spawned-window equivalent in the
  multi-window flow) to macOS only, where it has always been
  intended — the plugin's purpose here is repositioning the macOS
  traffic-lights so floating-bubble skins can pull them inside the
  panel. Windows and Linux now get their default decorated chrome
  with all three buttons.
- **Settings button now pins to the bottom of the sidebar when
  there are no profiles**
  ([#8](https://github.com/packetThrower/Baudrun/issues/8)). The
  empty-state container was missing `flex: 1`, so the Settings
  button hugged it instead of sticking to the bottom the way it
  does once a profile exists.

## [0.9.0] — 2026-04-25

### Added

- **Tauri v2 / Rust port.** Backend reimplemented in Rust on Tauri
  2; the renderer stays Svelte 5. Macros / DTR-RTS / hex view /
  XMODEM/YMODEM / auto-reconnect / driver detection all preserved
  with the same on-disk JSON shapes so existing profiles, themes,
  skins, and settings round-trip without a migration step.
- **Signed in-app updater.** Footer toast appears when GitHub has
  a newer release; clicking Install on a stable update downloads
  the platform bundle, verifies its minisign signature against the
  public key embedded in the binary, and relaunches into the new
  version. Settings → Advanced → Updates toggles the launch check
  and a separate "include pre-releases" knob. Pre-releases ship
  signed updater bundles too but don't update the auto-update
  manifest, so stable installs aren't auto-jumped onto an alpha.
- **Highlight rule packs.** Six bundled packs — Baudrun default
  (vendor-neutral), Cisco IOS / IOS XE / IOS XR, Juniper Junos,
  Aruba AOS-CX, Arista EOS, and MikroTik RouterOS — toggle on per
  Settings or per profile. The user pack at
  `$SUPPORT_DIR/highlight-rules.json` is editable on disk and
  layers on top; additional packs can be imported via Settings →
  Syntax Highlighting → Import pack. First-match-wins ordering,
  available colors red / green / yellow / blue / magenta / cyan
  / dim. Per-rule CPU budget bails on regex catastrophic
  backtracking instead of locking the renderer.
- **Highlight rule playground** — a static HTML page at
  [packetthrower.github.io/Baudrun/playground.html](https://packetthrower.github.io/Baudrun/playground.html)
  for testing rule packs against real captures (drop a file or
  paste, edit the JSON, watch colors apply live; everything runs
  client-side, the file you drop never leaves your browser).
- **Per-profile syntax-highlight pack overrides.** A profile can
  pick a different set of packs than the global default — handy
  when one profile talks to a Cisco device and the next to a
  Juniper one. The profile's Syntax Highlighting card collapses to
  save space.
- **Importable highlight packs alongside the bundled ones.**
  Settings → Syntax Highlighting → Import pack reads a JSON file
  into `$SUPPORT_DIR/highlight/<id>.json` and auto-enables it.
  Imported packs show a Remove button. Two starter examples ship
  under [docs/examples/](docs/examples/) — the minimal-skeleton
  schema and a practical syslog/journald set with severity
  keywords, systemd unit states, sshd events, and PID highlighting.

### Changed

- **macOS code signing stays ad-hoc.** First launch of a new
  download still prompts Gatekeeper's "right-click → Open" UX —
  the maintainer doesn't have a paid Apple Developer account yet.
  Auto-update works either way; only the first-launch experience
  is affected.
- **Content Security Policy explicit.** Replaced `csp: null` with
  a tight `default-src 'self' tauri: ipc: …; script-src 'self'; …`
  so any future XSS in the renderer can't reach an arbitrary
  origin. `connect-src` whitelists GitHub for the update check.

### Security

- **Rule-pack ReDoS budget.** User-imported regex now runs under a
  per-line + per-rule wall-clock cap; rules that take longer than
  the per-rule budget once are auto-disabled for the rest of the
  session with a console warning.
- **GitHub API response cap.** Update-check fetch ceilings the
  body at 100 KB and times out after 10 s, so a hostile network
  redirect can't return a giant JSON body or hang the renderer.
- **Skin variable validation.** Imported skins reject CSS values
  containing `url()`, `image-set()`, `expression()`, `@import`, or
  `javascript:` / `data:` URIs to block exfiltration through `var()`
  references in regular CSS.
- **Updater key now passphrase-protected.** Initial keypair was
  generated with an empty password; rotated to one protected by
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Existing pre-release
  installs (alpha.1–alpha.13) won't auto-update against the new
  pubkey but they were never on the auto-update channel anyway.

### Ops & packaging

- macOS installs ship per-arch (`arm64` + `amd64`) `.dmg` + `.zip`
  with `libusb-1.0.0.dylib` bundled inside `Contents/Frameworks/`,
  so users without Homebrew can still run.
- Windows ships `.msi` (stable tags only — WiX rejects alphanumeric
  pre-release identifiers) and `.nsis-setup.exe` for both `x64`
  and `arm64`.
- Linux ships `.deb`, `.rpm`, `.AppImage`, and Arch `.pkg.tar.zst`
  for both `amd64` and `aarch64`. AppImage + the
  `60-baudrun-serial.rules` udev rule mean no `sudo` / `dialout`
  group fiddling.

## [0.7.0] — 2026-04-22

Last release on the Wails / Go backend before the Tauri v2 / Rust
port in v0.9.0.

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

## [0.6.0] — 2026-04-22 and earlier

See [GitHub Releases](https://github.com/packetThrower/Baudrun/releases)
for full notes. Earlier versions landed the skin system, profile
form, XMODEM/YMODEM file transfer, hex send, session logging, and
the initial cross-platform release pipeline.
