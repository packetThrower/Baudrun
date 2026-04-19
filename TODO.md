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

- [ ] **Paste safety.** Two profile toggles: a confirm prompt when
      pasting multi-line content (avoids the "I pasted into the wrong
      window" disaster) and an optional per-character delay for
      devices that can't buffer at line rate. UARTs on microcontrollers
      and older switches silently corrupt a fast 115200 paste; slow
      paste fixes that. High value for both network and embedded users.
- [ ] **Hex send.** Input field that parses space-separated hex
      (`02 FF AA 55`) and writes the raw bytes. Counterpart to the
      existing hex view on RX. Useful for Modbus RTU, firmware
      bootloaders, custom binary protocols.
- [ ] **Macros / quick-send buttons.** **[on request]** Profile-level
      canned strings bound to session-header buttons — `show
      running-config`, `AT+RST`, vendor-specific reboot commands,
      whatever the user wants one-click access to. Saves typing for
      repeated commands; risk is bloating the session header or
      profile form. Wait until someone asks so the UI shape is
      informed by a real workflow.
- [ ] **File transfer (XMODEM/YMODEM/ZMODEM).** **[on request]**
      Firmware uploads to embedded devices. Big implementation — each
      protocol is a state machine with CRC checks, NAK handling,
      and strict block timing. Worth building if courting embedded
      devs becomes a priority; skip until that audience shows up.

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
