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

- [x] **Phase 0 — Terminal viewport feature-completeness.** Done.
      Window resize, scrollback (mouse wheel + `display_offset`),
      cell flags (bold/italic/underline/dim/strikethrough), mouse-
      drag selection + clipboard copy, sanitized clipboard paste,
      cursor blink, and bell flash all live in
      [terminal_view.rs](prototype/src/terminal_view.rs) /
      [terminal_grid.rs](prototype/src/terminal_grid.rs).
- [x] **Phase 0.5 — Adopt gpui-component.** Done. The Zed-git swap
      sketched here originally turned out unnecessary:
      [gpui-component](https://github.com/longbridge/gpui-component)
      0.5.1 on crates.io targets `gpui ^0.2.2`, the same pin we're
      already on. The README example uses git deps (their `main`
      branch's newer development), but the published crate sticks to
      crates.io. So the actual change was one line in
      [prototype/Cargo.toml](prototype/Cargo.toml) — `gpui-component
      = "0.5"` — with no API drift to fix. 60+ widgets now available
      for Phase 2 onward.
- [x] **Phase 1 — Port the data layer.** Done. `profiles`,
      `appdata`, `settings`, `serial/`, `themes/` (incl.
      `.itermcolors` parsing), `transfer.rs`, `highlight.rs`,
      `skins.rs`, `sanitize.rs` all live under
      [prototype/src/data/](prototype/src/data/) and back the live
      UI.
- [x] **Phase 2 — Profile sidebar + connection management.** Done.
      Sidebar with profile list, add/edit form (Connection /
      Highlighting / Advanced sub-tabs), connect-by-profile via
      `serial_io::open`, connection state in the session header
      and the bottom status bar.
- [x] **Phase 3 — Settings panel.** Done. Standalone window with
      Appearance / Themes / Shortcuts / Highlighting / Advanced
      tabs. Theme picker has live preview (the per-row Preview
      modal); skin picker, keybinding capture, and connection
      defaults all round-trip through `data::settings`.
- [x] **Phase 4 — Themes & skins.** Done. Theme parser drives the
      viewport palette via `Palette::from_theme`; the hardcoded
      fallback in [term_bridge.rs](prototype/src/term_bridge.rs)
      only fires when a theme id misses the store. Skins drive the
      `SkinTokens` global that paints all the chrome.
- [x] **Phase 5 — Specialty terminal features.** Done. Hex view
      toggle on the profile, highlight packs with first-match
      precedence, status bar at the bottom of the window.
- [x] **Phase 6 — File transfer.** Done. Send File button in the
      session header opens a file picker → protocol picker
      (XMODEM-CRC / 1K / Classic, YMODEM) → progress dialog with
      live bar and Cancel; success/error surface as toasts. ZMODEM
      stays out of scope (much larger state machine).
- [x] **Phase 7 — Multi-window + session migration.** Done (except
      window-state persistence). Sidebar `⧉` icon opens a new
      top-level window sharing the same stores + `SettingsBus` so
      settings stay in lockstep. Session-header `⋯` overflow menu
      and sidebar right-click both offer "Move Session to New
      Window" — `extract_session` / `install_session` hand the
      live TerminalView entity, OS threads, drain task, and
      transfer state over without dropping the port. Right-click
      on a non-connected profile offers "Connect in New Window"
      which spawns + auto-connects in one step. `Disconnect` got a
      `Drop` impl so window close also tears the port down within
      ~50 ms.
- [x] **Suspend / resume.** Done. Pill in the session header keeps
      the port open + bytes flowing into scrollback while hiding
      the terminal viewport so the user can browse other profiles
      / Settings without disconnecting. Resume banner appears at
      the top of the connected profile's editor, or a centered
      placeholder pane shows when the user navigates away with no
      editor open. Clicking the connected sidebar row implicitly
      resumes.
- [x] **Send Break + Send Hex.** Done. Session-header `⋯`
      overflow menu hosts Send Break (300 ms `set_break` /
      `clear_break` via a dedicated `break_tx` channel polled by
      the write thread), Send Hex (modal with the same parser the
      Tauri version uses — `0x` / spaces / commas all strip),
      and Send File alongside Move-to-New-Window.
- [ ] **Phase 7.5 — Settings + Profile Form parity.** Items that
      exist in the Tauri build but are still missing or only
      partially wired in the gpui prototype. Diffed against
      `src/lib/Settings.svelte` + `src/lib/ProfileForm.svelte`.

      Settings — Appearance tab
      - [x] **Scrollback lines** input. Done. Appearance tab grew
            a Scrollback Input matching the Font Size slot.
            `Settings::scrollback_lines` flows into
            `TerminalView::new` on boot and `set_scrollback_lines`
            (which pushes a fresh `term::Config` through
            `Term::set_options`) on live edits. Status bar shows
            `<filled>/<max>` on the right.
      - [x] **Installed Skins list.** Done. New "Installed Skins"
            card in the Appearance tab — header with import on the
            right, one row per user-imported skin showing name +
            "Custom" tag (with "· dark-only" suffix when
            `supports_light` is false) + 🗑 button. Empty state
            shows a muted hint. Deleting the active skin falls
            back to the built-in `baudrun` default. Undo-toast
            still pending (tracked under "Settings — chrome").

      Settings — Advanced tab
      - [x] **Choose… / Reset buttons** next to Session Log
            Directory. Done. `choose_log_dir` opens the OS folder
            picker via `cx.prompt_for_paths` with `directories:
            true` and mirrors the result into both the Input and
            the persisted setting; `reset_log_dir` clears both
            back to the default-location signal (empty string).
      - [ ] **Screen Reader Support toggle**
            (`settings.screen_reader_mode`). Lower priority than
            the others; check whether gpui exposes an equivalent
            ARIA hook before committing.
      - [ ] **Config Directory** read-only display + Choose… /
            Reset. Lets the user point Baudrun at a different
            support dir (portable installs, shared profiles).
      - Terminal Renderer (DOM/WebGL toggle) is **N/A** —
        prototype uses gpui paint, not xterm.js.

      Settings — chrome
      - [ ] **Filter / search input** at the top of the Settings
            window. Tauri uses `keywords` per section to scroll-
            and-highlight matches as the user types; current
            prototype has tabs only.
      - [ ] **Undo-delete** for imported skins / themes / packs.
            Replace the immediate delete with a 10-s "removed,
            Undo" toast (Tauri uses the status bar; we can use
            the existing notification layer).

      Profile Form
      - [ ] **Missing-driver banner** above the Serial Port field
            when an unenrolled USB-serial adapter is plugged in.
            Backend `data::serial::detect` is ready (already used
            for the Settings toggle); profile editor just needs
            the banner UI.
      - [ ] **Header buttons when connected.** When the editor is
            open for the connected profile while suspended, the
            form header still shows Connect — Tauri swaps that
            for Disconnect + Resume. We have a Resume banner
            above the form already; this would move both
            affordances into the header for visual parity.

      Cosmetic / non-blocking
      - Welcome pane wording differs slightly from Tauri; not
        worth a dedicated bullet but worth a pass when the
        rest of the list lands.
- [x] **Phase 7.6 — Tauri features dropped on purpose.** Things
      the Tauri build has that the gpui prototype intentionally
      does not. Logged here so the migration audit doesn't keep
      relitigating them.

      - **Terminal Renderer setting** (Settings → Advanced →
        `terminal_renderer`). Tauri exposed a DOM / WebGL choice
        for xterm.js's renderer; gpui paints natively, so the
        whole setting is moot. The field is left on `Settings`
        only so existing `settings.json` files round-trip
        without losing data.
      - **ZMODEM** file transfer. Phase 6 ships XMODEM-Classic /
        CRC / 1K and YMODEM. ZMODEM is a substantially larger
        state machine and most embedded-bootloader / network-
        gear targets don't speak it. Documented in
        `data/transfer.rs` and called out in the Phase 6 commit;
        revisit on real demand.
      - **Drag-tab-between-windows.** Replaced by the explicit
        "Move Session to New Window" actions in the toolbar
        overflow + sidebar right-click menu. Cross-window drag-
        out-to-spawn needs platform NSDraggingSource / OLE
        plumbing that gpui doesn't expose generically; the
        explicit gesture covers the same UX without the
        platform work.
      - **`tauri-plugin-updater`.** Gone with Tauri itself.
        Phase 9 picks a replacement (`cargo-dist` or
        `self_update`).
      - **WKWebView paste-confirm modal hack.** Tauri needed a
        custom modal because WKWebView swallows `window.confirm`.
        gpui's dialog layer doesn't have the same limitation, so
        the prototype can wire paste-confirm through
        `window.open_dialog` directly when that feature lands
        (currently the multi-line warn is just a checkbox in
        the profile editor with no live confirm UI).
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
- gpui-component bridge crate stalling on crates.io, leaving us on
  a frozen 0.5.x line. Mitigation: we can either (a) ride out
  whatever's already published, since the components we need are
  in 0.5.1, or (b) move to their git tip later, which would force
  the Zed-git pin we managed to avoid in Phase 0.5.
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
