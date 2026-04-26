# TODO

## Serial features (PuTTY carryovers)

- [x] **Send Break.** Session-header button; 300ms break pulse via
      go.bug.st/serial's Port.Break. No keyboard shortcut (Ctrl+B is
      a real terminal character; modern Macs have no Break key).
- [x] **Auto-reconnect.** Opt-in per profile. Polls for the port to
      reappear (1s interval, 30s timeout) and reopens with the same
      config. xterm stays mounted so scrollback survives the gap;
      session shows an amber pulsing dot + "reconnecting…" label.
- [x] **Backspace/Delete key mapping.** Profile-level dropdown —
      DEL (0x7f, default) or BS (0x08).
- [x] **Copy-on-select.** Global setting in Settings → Advanced.
      Writes to the clipboard on xterm's onSelectionChange.

Non-goals: character-set translation (UTF-8 is universal on modern
network gear), answerback strings (legacy VT-terminal feature, no
real use for serial consoles).

## Serial features (next tranche)

Candidates beyond the PuTTY carryovers. Items marked **[on request]**
are deferred until someone actually asks — they're useful but either
high-effort or niche enough that priority tracks real demand.

- [x] **Paste safety.** Profile toggles for multi-line paste
      confirmation (lines count + first-line preview) and slow paste
      (configurable 0-500ms inter-character delay). Heuristic detects
      paste vs. typed input by length threshold + line-break presence.
- [x] **Hex send.** Session-header "Hex" button opens a modal with
      a flexible parser (space-separated, compact, or 0x-prefixed
      hex; `02 FF AA 55` / `02FFAA55` / `0x02 0xFF 0xAA 0x55` all
      equivalent). Writes the raw bytes via the existing sendBytes
      path. Useful for Modbus RTU, firmware bootloaders, custom
      binary protocols.
- [ ] **Macros / quick-send buttons.** **[on request]** Profile-level
      canned strings bound to session-header buttons — `show
      running-config`, `AT+RST`, vendor-specific reboot commands,
      whatever the user wants one-click access to. Saves typing for
      repeated commands; risk is bloating the session header or
      profile form. Wait until someone asks so the UI shape is
      informed by a real workflow.
- [x] **File transfer (XMODEM/YMODEM).** Session-header "Send File"
      button opens a modal with protocol picker (XMODEM, XMODEM-CRC,
      XMODEM-1K, YMODEM) and native file dialog. Transfer runs in
      the Go backend; Session's RX dispatch is redirected to the
      protocol state machine during the send. Progress events stream
      to the frontend for a live progress bar. Cancellable mid-
      transfer (CAN CAN to the receiver).
- [ ] **ZMODEM file transfer.** **[on request]** ZMODEM is a much
      larger state machine than XMODEM/YMODEM — frame negotiation,
      ZDL escape encoding, crash recovery, variable block sizes.
      Not implemented in the first cut; XMODEM/YMODEM cover the
      vast majority of embedded bootloader use cases. Add if a
      specific use case surfaces.
- [ ] **Auto-detect baud rate.** Profile-form "Auto-detect" button
      next to the baud field. Cycles through the standard rates
      (9600 / 19200 / 38400 / 57600 / 115200, plus the others
      already in `BAUD_RATES`), opens the port at each for ~500ms,
      scores the bytes that arrive — printable ASCII + line endings
      → good, high-bit garbage + control characters → wrong baud —
      and picks the highest-scoring rate. ~4 seconds per probe pass.
      Caveats:
      - Only works while the device is actively emitting; a silent
        prompt gives nothing to score.
      - Binary protocols (Modbus RTU, custom firmware bootloaders)
        look like garbage at every baud and the heuristic guesses
        wrong.
      - Sub-bit-rate timing measurement isn't feasible through a
        USB-serial adapter — by the time bytes reach us they're
        already framed.
      Realistic accuracy: high (90%+) for ASCII-emitting network
      gear; low elsewhere. Implementation: ~150 LOC of Rust on
      the backend (probe loop with cancel) + a small modal in the
      profile form showing per-rate progress so the user can bail
      mid-probe.

## Accessibility

- [x] **xterm screenReaderMode toggle.** Settings → Advanced →
      "Enable xterm screen-reader mode." Persisted as
      Settings.ScreenReaderMode; routed to xterm via
      `term.options.screenReaderMode` on init + $effect.
- [x] **Respect `prefers-reduced-motion`.** The reconnecting pulse
      and port-scanning pulse both wrap their animations in
      `@media (prefers-reduced-motion: reduce)` — dots stay visible
      as static indicators instead of blanking entirely.
- [x] **ARIA-label audit.** All icon-only / text-light buttons
      checked; existing labels on New Profile, Rescan ports,
      Dismiss driver notice, Remove/Preview theme, Remove skin,
      Overflow "More actions", modal × dismiss. Other controls
      have visible text that stands on its own.
- [x] **Ctrl/Cmd +/- terminal zoom.** Window-level keydown handler
      in App.svelte — `+` / `-` bump font size ±1, `0` resets to
      13. Clamped 8-28. Persisted to Settings.fontSize so zoom
      sticks across sessions.
- [x] **Keyboard shortcuts for Break / Clear / Suspend.** Session-
      header actions bind as follows:
      - macOS: `⌘K` Clear, `⌘⇧B` Send Break, `⌘⇧S` Suspend.
      - Linux/Windows: `Ctrl+Shift+K/B/S` respectively.
      Split modifier scheme is intentional — `Cmd+*` on macOS
      doesn't touch the terminal byte stream, so plain `⌘K`
      is safe; on other OSes `Ctrl+letter` has real device
      meaning (XOFF at `Ctrl+S`, VT at `Ctrl+K`, etc.) so the
      shortcuts add `Shift` to keep the plain `Ctrl+*`
      passthroughs intact. Surfaced on the button tooltips and
      via `aria-keyshortcuts` so screen readers announce them.
      Documented in docs/ACCESSIBILITY.md.

## Distribution

- [ ] **Code sign + notarize macOS binary.** Requires enrollment in the
      Apple Developer Program ($99/yr). Steps once enrolled:
      1. Create a Developer ID Application certificate in Apple's
         developer portal; download as .p12.
      2. Add as GitHub Actions secrets: `APPLE_CERT_P12` (base64 of the
         .p12), `APPLE_CERT_PASSWORD`, `APPLE_ID`, `APPLE_APP_PASSWORD`
         (app-specific password), `APPLE_TEAM_ID`.
      3. In `build-macos` job: import the cert into the keychain, sign
         with `codesign`, submit to Apple for notarization via
         `xcrun notarytool`, then `xcrun stapler staple` the result.
- [ ] **Code sign Windows binary (Authenticode).** Certificate from
      DigiCert / Sectigo / SSL.com (~$200+/yr; EV cert is pricier but
      skips SmartScreen warmup). Add as secrets, sign the .exe with
      `signtool` in the `build-windows` job.
- [ ] **Public downloads for a private source repo.** Shared
      downloads repo serving both Baudrun and get_switch_info:
      1. Create public `packetThrower/downloads` (empty, README listing
         apps + links to their Releases pages).
      2. Generate one fine-grained PAT scoped to that repo with
         `Contents: Read and write`. Add to *each* private source
         repo's Actions secrets as `RELEASES_REPO_TOKEN`.
      3. In each `release.yml`, point `softprops/action-gh-release` at
         the shared repo with prefixed tags:
         ```yaml
         repository: packetThrower/downloads
         token: ${{ secrets.RELEASES_REPO_TOKEN }}
         tag_name: baudrun-${{ github.ref_name }}       # or portfinder-
         name: Baudrun ${{ github.ref_name }}
         ```
      Do this **after** signing is in place so the first public release
      is already a trustworthy binary.
- [x] **Bundle libusb with the shipped binaries.** Done as part of
      the `usbserial-go` integration. Approach per platform:
      - **Linux:** runtime dep declared in the fpm output
        (`libusb-1.0-0` for .deb, `libusbx` for .rpm, `libusb` for
        pacman). AppImage copies `/usr/lib/*/libusb-1.0.so.0*` into
        `AppDir/usr/lib/` and AppRun prepends that to
        `LD_LIBRARY_PATH` so the bundled copy is found ahead of any
        system path.
      - **macOS:** dropped the darwin/universal build in favour of
        per-arch matrix jobs (macos-26 arm64 + macos-15-intel amd64),
        which sidesteps the "brew libusb is per-arch only" problem.
        Each build bundles libusb into the .app under
        `Contents/Frameworks/libusb-1.0.0.dylib` via
        `install_name_tool -change` against the path `otool -L`
        reports. A guard fails the step loudly if any Homebrew
        path remains so a broken bundle can't silently ship.
      - **Windows:** nothing. `usbserial-go` on Windows falls
        through to `go.bug.st/serial` — its gousb imports are all
        behind `//go:build linux || darwin` file tags, so the
        Windows build has no libusb dependency.

## Syntax highlighting — Tier 2

Current highlighter is a line-buffered regex rule engine. Tier 2
work — data-driven, shareable rule packs — landed in v0.9.0:

- [x] **User-editable rule file** at
      `$SUPPORT_DIR/highlight-rules.json`. Shipped as the editable
      "User overrides" pack, seeded from `baudrun-default` on first
      run. Pack format is `{id, name, description?, source, rules:
      [{pattern, color, ignoreCase?, group?}]}` — see
      [docs/examples/highlight-pack.example.json](docs/examples/highlight-pack.example.json).
- [x] **Preset packs** — six bundled, read-only: `baudrun-default`
      (vendor-neutral), `cisco-ios` (IOS / IOS XE / IOS XR), `junos`,
      `aruba-cx`, `arista-eos`, and `mikrotik-routeros`. Selectable
      per profile under the Syntax Highlighting card; profile picks
      override the global default. (Original wishlist included
      `ruggedcom-ros` and `f5-tmos` — those didn't land yet; user-
      authored packs cover the gap via Import.)
- [x] **Importable user packs** — Settings → Syntax Highlighting →
      Import pack reads any JSON pack into
      `$SUPPORT_DIR/highlight/<id>.json` and auto-enables it.
      Supersedes the iTerm2-Triggers / grc-conf import items below
      since the JSON format is well-documented and the playground
      lets users author packs directly without going through a
      vendor format.
- [x] **Browser-based playground** — static page hosted on the docs
      site that runs the same regex compiler + ANSI color map as
      the app. Drop a real capture in, edit the JSON, watch colors
      apply live; everything stays client-side so the file never
      leaves the user's machine. See
      [packetthrower.github.io/Baudrun/playground.html](https://packetthrower.github.io/Baudrun/playground.html).
- [ ] **Import from iTerm2 Triggers.** **[on request]** iTerm stores
      triggers in its plist; the "highlight foreground" action maps
      cleanly to our rule shape. Useful for users coming from an
      iTerm-driven workflow but not enough demand to prioritize
      ahead of other features.
- [ ] **Import from grc configs.** **[on request]** grc's text
      format is simple (regex + colour codes + optional
      `count=more`). One-shot importer would help users bring
      `grc.conf` rules over.

Non-goals / won't-do:
- tree-sitter / Pygments / chroma lexers — overkill for line-level
  streaming. Our regex engine is the right shape; we're just moving
  the rules out of the binary.

## Skin system

Infrastructure is in; three skins shipped. Remaining work is
cranking out more presets.

- [x] **CSS variable surface** — expanded (fonts, radii, shadows,
      scrollbar, option popups, blur strength, panel border, input
      border, floating-panel controls).
- [x] **Skin JSON format + store** (`internal/skins`). Built-ins via
      `//go:embed`, user-added at `~/Library/Application Support/
      Baudrun/skins/<id>.json`.
- [x] **Svelte store + applier** — sets properties on
      `document.documentElement`, tracks what it wrote to cleanly
      unset before applying a new skin.
- [x] **Settings UI** — Skin picker above the theme picker.
- [x] **Import from user-drop'd JSON** — file dialog + store.

- [x] **Baudrun** — the default, flush-edge layout, iOS-style labels.
- [x] **macOS 26 (Liquid Glass)** — floating sidebar/main bubbles,
      backdrop blur, sentence-case labels, brighter accents, bigger
      continuous radii.
- [x] **macOS Classic** — pre-Big Sur square style, no vibrancy.
- [x] **Windows 11 (Fluent)** — Segoe UI Variable, 8px radii, solid
      surfaces, Fluent accent palette.
- [x] **Windows XP (Luna)** — Bliss-era teal chrome, rounded buttons,
      Tahoma UI.
- [x] **GNOME (Adwaita)** — Cantarell font, generous spacing, GNOME
      green accent, flatter.
- [x] **KDE (Breeze)** — Breeze palette, slightly more angular.
- [x] **elementary OS (Pantheon)** — Open Sans, clean surfaces,
      elementary blue.
- [x] **Xfce (Greybird)** — Greybird grey palette, compact spacing.
- [x] **Cyberpunk (Synthwave)** — neon magenta + cyan on deep
      purple, 40px grid overlay, soft pink text glow.
- [x] **Blueprint** — engineering-drawing blue + crisp white grid
      (dark) / drafting paper with blue ink (light), monospace
      typography.
- [x] **CRT (Green Phosphor)** — green-on-black phosphor, monospace
      everywhere, 2/3px scan-line overlay.
- [x] **E-Ink (Paper)** — high-contrast paper-and-ink aesthetic.
- [x] **High Contrast** — a11y: solid black, pure white, visible
      borders everywhere, WCAG-AAA accent colors.

Known caveats to document in README (done):
- Window chrome (macOS overlay titlebar, Windows decorated chrome,
  Linux GTK) is set at window-creation time via
  `WebviewWindowBuilder` and `tauri.conf.json`. Skins can't change
  the OS chrome live — only the in-window CSS surface.
- Window shape (macOS squircle vs. Windows rect) is fixed per-OS.

## Localization (i18n)

- [ ] **UI translation infrastructure.** **[on request]** Adopt
      svelte-i18n (most conventional Svelte choice — JSON locale
      files, `$_('settings.title')` lookups, system locale
      detection via `@tauri-apps/plugin-os`). Extract every
      hardcoded string in Settings, ProfileForm, Sidebar, Terminal,
      modals, and tooltips into `src/i18n/en.json` as the canonical
      source. Backend strings (Tauri command errors, status bar
      messages like "Reconnected" / "Session moved to new window")
      either return error codes the renderer translates, or pass
      through a small Rust i18n helper.
      - Ongoing tax: every PR has to extract its new strings; every
        new feature ships untranslated until a translator catches
        up. Worth doing once a real translator volunteers — until
        then, a native English target is fine for the network-
        engineer audience.
      - Phased path if/when this lands: ship svelte-i18n with
        English-only first so the infrastructure exists; subsequent
        languages become translator-only PRs adding `<lang>.json`.
        Pluralization, RTL languages (Arabic, Hebrew), and locale-
        specific number / date formatting are second-pass concerns.
      - Settings → Language picker once at least one non-English
        locale ships, persisted to `Settings.locale`. Falls back to
        the OS locale when unset.
