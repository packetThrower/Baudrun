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

## Accessibility

- [ ] **xterm screenReaderMode toggle.** Flip xterm's
      `screenReaderMode: true` behind a Settings → Accessibility
      toggle. Exposes incoming output to screen readers via a live
      region — critical for blind users. Default off (small perf
      cost on heavy output).
- [ ] **Respect `prefers-reduced-motion`.** The reconnecting pulse
      animation and any future skin-level overlays (CRT scanlines,
      blueprint grid shimmer) should disable when the OS reports a
      reduced-motion preference. Pulse stays visible as a static
      amber dot. `@media (prefers-reduced-motion: reduce) { ... }`
      wrapper in style.css.
- [ ] **ARIA-label audit.** Pass over icon-only / text-light
      controls: DTR/RTS pills, driver-install banner × button, theme
      swatch rows, the sidebar settings button. Add explicit
      `aria-label` where the visible text doesn't stand on its own.
- [ ] **Ctrl/Cmd +/- terminal zoom.** Standard in every other
      terminal (iTerm2, Windows Terminal, VS Code). Easier for
      low-vision users than the Settings → Font size dropdown.
      Re-uses the existing fontSize setting; shortcut clamps to
      8-28px.
- [ ] **Keyboard shortcuts for Break / Clear / Suspend.** **[on request]**
      Session-header buttons are currently mouse-only, which blocks
      keyboard-only users from sending Break. Cmd/Ctrl+Shift+B is
      the obvious default, but needs a scheme decision — some
      modifier combinations collide with common control sequences
      serial devices care about. Wait until someone asks so the
      chosen shortcuts don't surprise the terminal-power-user crowd.

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
      downloads repo serving both Seriesly and get_switch_info:
      1. Create public `otec-it/downloads` (empty, README listing
         apps + links to their Releases pages).
      2. Generate one fine-grained PAT scoped to that repo with
         `Contents: Read and write`. Add to *each* private source
         repo's Actions secrets as `RELEASES_REPO_TOKEN`.
      3. In each `release.yml`, point `softprops/action-gh-release` at
         the shared repo with prefixed tags:
         ```yaml
         repository: otec-it/downloads
         token: ${{ secrets.RELEASES_REPO_TOKEN }}
         tag_name: seriesly-${{ github.ref_name }}      # or portfinder-
         name: Seriesly ${{ github.ref_name }}
         ```
      Do this **after** signing is in place so the first public release
      is already a trustworthy binary.

## Syntax highlighting — Tier 2

Current highlighter is a line-buffered regex rule engine baked into the
binary. Tier 2 is making it data-driven + shareable:

- [ ] **User-editable rule file** at
      `~/Library/Application Support/Seriesly/highlight-rules.json`.
      Format: array of `{ pattern, open, close, group? }` objects.
      Ship current built-in rules as the default file on first run;
      users can add patterns without rebuilding.
- [ ] **Preset packs** as bundled read-only rule sets: `cisco-ios`,
      `junos`, `ruggedcom-ros`, `aruba-cx`, `f5-tmos`. Each ~10-20
      patterns. Enable/disable per profile via a Settings → Advanced
      picker (multi-select).
- [ ] **Import from iTerm2 Triggers.** iTerm stores triggers in its
      plist; the "highlight foreground" action maps cleanly to our
      rule shape. Importer pulls matching entries out, surfaces the
      rest as ignored. Lets users bring existing configs over.
- [ ] **Import from grc configs.** grc's text format is simple
      (regex + colour codes + optional `count=more`). One-shot importer
      from a user-selected grc conf file.

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
      Seriesly/skins/<id>.json`.
- [x] **Svelte store + applier** — sets properties on
      `document.documentElement`, tracks what it wrote to cleanly
      unset before applying a new skin.
- [x] **Settings UI** — Skin picker above the theme picker.
- [x] **Import from user-drop'd JSON** — file dialog + store.

- [x] **Seriesly** — the default, flush-edge layout, iOS-style labels.
- [x] **macOS 26 (Liquid Glass)** — floating sidebar/main bubbles,
      backdrop blur, sentence-case labels, brighter accents, bigger
      continuous radii.
- [x] **High Contrast** — a11y: solid black, pure white, visible
      borders everywhere, WCAG-AAA accent colors.
- [ ] **Windows 11** — Fluent/Mica: Segoe UI Variable, 8px radii,
      solid surfaces, Fluent accent palette.
- [ ] **GNOME Adwaita** — Cantarell font, generous spacing, GNOME
      green accent, flatter.
- [ ] **KDE Breeze** — Breeze palette, slightly more angular.
- [ ] **macOS Classic** — pre-Big Sur square style, no vibrancy.
- [ ] **CRT** — green phosphor on black, monospace everywhere.

Known caveats to document in README (done):
- Native `<select>` dropdowns stay OS-native regardless of skin
  (Chromium delegates popup rendering to the OS). Close-but-not-exact
  for Windows 11 / GNOME skins.
- Window chrome (`mac.TitleBarHiddenInset`, Windows Mica backdrop,
  vibrancy) requires Wails startup config and relaunch. Skins can
  hint at this via an `.extras.requiresRelaunch` flag and prompt
  the user. **Not yet implemented** — currently window chrome is
  fixed at app launch.
- Window shape (macOS squircle vs. Windows rect) is fixed per-OS.
