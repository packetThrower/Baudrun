# TODO

## Migration: alacritty + gpui

Active multi-phase rewrite on the `experiments/alacritty-gpui`
branch. The xterm.js + Tauri stack had unfixable per-keystroke
typing latency on Windows; the alacritty_terminal + gpui prototype
matches PuTTY/screen on the same hardware after the read-side
polling fix landed in `b076aec`. All migration work stays on this
branch until full feature parity with `main`, then we cut over.

macOS is the primary dev/test target during migration. Windows +
Linux are kept in mind in code, verified periodically — Linux less
often since gpui's Linux backend is the least-mature of the three.

Phases 0–1 are foundation; 2–6 are mostly parallelizable after that.

- [ ] **Phase 0 — Terminal viewport feature-completeness.** Make the
      viewport widget itself stand alone before any chrome wraps it.
      Window resize → grid resize, scrollback (mouse wheel +
      `display_offset`), cell flags (bold/italic/underline/dim/
      strikethrough — extend the row-coalescer key in
      [terminal_grid.rs](prototype/src/terminal_grid.rs)),
      mouse-drag selection + clipboard copy, sanitized clipboard
      paste, cursor blink, bell handling (visual flash; sound
      deferred), basic keyboard layout / IME plumbing.
- [ ] **Phase 0.5 — Swap to Zed-git gpui + adopt gpui-component.**
      [gpui-component](https://github.com/longbridge/gpui-component)
      is a 60+-component library (Apache-2.0/MIT, used in Longbridge
      Pro) that covers exactly the chrome we'd otherwise build from
      div primitives in Phases 2–3: sidebar, dialog, sheet, dock,
      input, form, select, switch, table, list, tab, menu,
      notification, tooltip, kbd, title_bar, resizable. The catch:
      it pins to Zed's git HEAD, not crates.io, and uses a separate
      `gpui_platform` crate that the 0.2.x crates.io build bundles.
      Done as its own commit, no other changes, so any API drift
      between gpui 0.2 and Zed-current is isolated and easy to
      debug. **Pin a specific Zed commit** rather than tracking
      HEAD so fresh builds stay reproducible.
- [ ] **Phase 1 — Port the data layer.** Move existing pure-Rust
      modules from `src-tauri/src/` into the prototype workspace
      as-is — they're data/IO, not UI: `profiles`, `appdata`,
      `settings`, `serial/`, `usbserial/`, `themes/` (incl.
      `.itermcolors` parsing), `transfer.rs`, `highlight.rs`,
      `skins.rs`, `sanitize.rs`. All callable from gpui code, none
      used yet.
- [ ] **Phase 2 — Profile sidebar + connection management.** Split-
      pane (sidebar | terminal) layout via gpui-component's
      `resizable` + `sidebar`, profile list (`list`), profile
      add/edit form (`form` + `input` + `select`), connect-by-
      profile (replaces `cargo run -- <port>`), connection state
      indicator (`notification` + `badge`), quick-connect dialog
      (`dialog`).
- [ ] **Phase 3 — Settings panel.** `sheet` or `dialog` for the
      panel structure, theme picker with live preview (reuse the
      viewport widget at small size), skin picker, keybinding
      editor (`kbd` for display + custom capture), connection
      defaults. All settings round-trip via Phase-1 code.
      gpui-component has its own theme system — decide whether to
      adopt it for app chrome or override; the *terminal viewport*
      keeps its own palette either way.
- [ ] **Phase 4 — Themes & skins.** Plug the theme parser into the
      viewport's color resolution (drop the hardcoded palette in
      [term_bridge.rs](prototype/src/term_bridge.rs)'s `resolve`).
      Built-in themes shipped. Skin system drives app-chrome accent
      colors / backgrounds / motion-on-off.
- [ ] **Phase 5 — Specialty terminal features.** Hex view toggle,
      highlight packs (regex-driven coloring overlaid on terminal
      output), status bar (port, baud, byte counters, connection
      state).
- [ ] **Phase 6 — File transfer.** Send-file dialog (XMODEM /
      YMODEM / raw paste), progress UI, cancellation.
- [ ] **Phase 7 — Multi-window + session migration.** Open new
      window, drag-tab-between-windows protocol (or simpler: "move
      session to new window" command), window state persistence.
- [ ] **Phase 8 — System integration.** Application menu (macOS),
      icon, metadata, file associations, single-instance behavior
      cross-platform, prefers-reduced-motion equivalent.
- [ ] **Phase 9 — Auto-updater + distribution.** `tauri-plugin-updater`
      is gone post-Tauri; investigate `cargo-dist` or the
      `self_update` crate. Code signing per platform. CI build
      pipeline (probably GitHub Actions ARM + x64 for each OS).
- [ ] **Phase 10 — Polish & cutover.** Cross-platform perf passes.
      Migration of existing user data (profiles, themes) from old
      app's config dir. Beta on the experiments branch. `git merge
      experiments/alacritty-gpui → main`. Cut a `1.0.0-rc`.

Risks tracked alongside the plan:
- We ride Zed's gpui git pin via gpui-component (Phase 0.5), so
  picking up upstream gpui changes is a forced bump cadence rather
  than a stable crates.io version. Counter-risk: if gpui-component
  stalls, we lose 60+ widgets we'd then have to build ourselves.
  Mitigation: pin a specific Zed commit; bump deliberately, not on
  every fresh build.
- Linux gpui is the least-mature platform; smoke-test early rather
  than late-cycle surprise.
- Auto-updater is real work; gpui ships nothing.
- Multi-window with live session migration needs custom design.

## Serial features (next tranche)

Candidates beyond the v0.9.x feature set. Items marked **[on request]**
are deferred until someone actually asks — useful but high-effort or
niche enough that priority tracks real demand. All of these will be
implemented in the new stack post-migration.

- [ ] **Macros / quick-send buttons.** **[on request]** Profile-level
      canned strings bound to session-header buttons — `show
      running-config`, `AT+RST`, vendor-specific reboot commands,
      whatever the user wants one-click access to. Saves typing for
      repeated commands; risk is bloating the session header or
      profile form. Wait until someone asks so the UI shape is
      informed by a real workflow.
- [ ] **ZMODEM file transfer.** **[on request]** ZMODEM is a much
      larger state machine than XMODEM/YMODEM — frame negotiation,
      ZDL escape encoding, crash recovery, variable block sizes.
      XMODEM/YMODEM cover the vast majority of embedded bootloader
      use cases; add ZMODEM if a specific use case surfaces.
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
      gear; low elsewhere. Implementation: ~150 LOC of Rust on the
      backend (probe loop with cancel) + a small modal in the
      profile form showing per-rate progress so the user can bail
      mid-probe.

Non-goals: character-set translation (UTF-8 is universal on modern
network gear), answerback strings (legacy VT-terminal feature, no
real use for serial consoles).

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

## Syntax highlighting — deferred

Tier 2 (data-driven, shareable rule packs) shipped in v0.9.0. The
remaining importers are all **[on request]**:

- [ ] **Import from iTerm2 Triggers.** **[on request]** iTerm stores
      triggers in its plist; the "highlight foreground" action maps
      cleanly to our rule shape. Useful for users coming from an
      iTerm-driven workflow but not enough demand to prioritize ahead
      of other features.
- [ ] **Import from grc configs.** **[on request]** grc's text format
      is simple (regex + colour codes + optional `count=more`).
      One-shot importer would help users bring `grc.conf` rules over.

Non-goals: tree-sitter / Pygments / chroma lexers — overkill for
line-level streaming. Our regex engine is the right shape.

## Localization (i18n)

- [ ] **UI translation infrastructure.** **[on request]** Pick a
      gpui-friendly i18n approach post-migration (likely `fluent-rs`
      or similar — svelte-i18n notes from the pre-migration plan no
      longer apply). Extract every hardcoded string in the new
      sidebar / settings / profile form / modals into a canonical
      English locale file as the source of truth. Backend strings
      either return error codes the UI translates, or pass through
      a small Rust i18n helper.
      - Ongoing tax: every PR has to extract its new strings; every
        new feature ships untranslated until a translator catches
        up. Worth doing once a real translator volunteers — until
        then, English-only is fine for the network-engineer
        audience.
      - Phased path if/when this lands: ship the infrastructure
        with English-only first; subsequent languages become
        translator-only PRs adding `<lang>.ftl` (or equivalent).
        Pluralization, RTL languages (Arabic, Hebrew), and locale-
        specific number/date formatting are second-pass concerns.
      - Settings → Language picker once at least one non-English
        locale ships, persisted in the settings model. Falls back
        to the OS locale when unset.
