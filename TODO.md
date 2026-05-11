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
- [x] **Phase 7.5 — Settings + Profile Form parity.** Done. Every
      sub-item below either landed or was reclassified into Phase
      7.6 (deliberate drops). Diffed against
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
      - Screen Reader Support toggle moved to Phase 7.6 — gpui
        0.2 exposes no accessibility primitives (no ARIA, no
        NSAccessibility bridge), so there's nowhere to wire the
        toggle to. Revisit when gpui ships a11y hooks upstream.
      - [x] **Config Directory** display + Choose… / Reset. Done.
            Custom card with the resolved support-dir path in a
            read-only Input, plus three pills: Reveal (opens the
            directory in Finder / Explorer / xdg-open via
            `cx.open_url("file://…")` with a percent-encoded
            path), Choose… (folder picker → `appdata::write_override`),
            and Reset (clears the override). Choose / Reset toast
            "Restart Baudrun to use it" — re-binding every live
            Store at runtime is heavier than this slice covers.
      - Terminal Renderer (DOM/WebGL toggle) is **N/A** —
        prototype uses gpui paint, not xterm.js.

      Settings — chrome
      - [x] **Filter / search input.** Done. Filter Input on the
            right of the window header dims non-matching section
            cards to 0.18 opacity (case-insensitive match against
            title + a `SECTION_KEYWORDS` synonym table) and also
            dims left-rail tabs whose sections all miss the
            filter. Hand-rolled `×` clear glyph on the right of
            the input (gpui-component's built-in `cleanable`
            renders an `IconName::CircleX` SVG the prototype
            doesn't bundle, so the icon ends up blank).
      - [x] **Undo-delete** for imported skins / themes / packs.
            Done. Each store grew a `restore` method that re-
            persists the JSON + re-adds to the in-memory list.
            Delete handlers snapshot the item before calling
            store.delete, then push a notification with an Undo
            action button that hands the snapshot back to the
            restore method. Notification dismisses ~1.5 s after
            the Undo click (entity-scoped spawn so the timer
            survives tab switches mid-wait) and a follow-up
            "Restored …" toast confirms the action.

      Profile Form
      - [x] **Missing-driver banner.** Done. Profile editor's
            Connection card renders a yellow banner above the
            Serial Port picker for each unenrolled USB-serial
            adapter detected by `data::serial::detect`
            (macOS/Windows; Linux returns empty since the kernel
            handles driver loading there). Shows chipset name +
            optional reason + product/manufacturer/serial, with an
            "Install driver…" pill that opens the vendor URL via
            `cx.open_url`. Detection is gated by Settings →
            Advanced → USB Driver Detection.
      - [x] **Header buttons when connected.** Done. The form
            header swaps Connect for Disconnect + Resume when the
            editor is open on the connected profile while suspended
            (the existing `show_resume` signal threads down to
            `form_pane` → `form_header` as a `connected_session`
            flag). The Resume banner above the form stays for the
            "port still open, bytes still flowing" context line.

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
      - **Screen Reader Support toggle.** Tauri exposed this as
        the xterm.js ARIA live-region preference. gpui 0.2
        ships no accessibility primitives (no ARIA, no
        NSAccessibility bridge — the platform layers only
        forward focus + key input). The data field stays on
        `Settings` so old settings.json files round-trip, but
        there's no UI control. Revisit when gpui adds a11y
        hooks upstream.
      - **WKWebView paste-confirm modal hack.** Tauri needed a
        custom modal because WKWebView swallows `window.confirm`.
        gpui's dialog layer doesn't have the same limitation, so
        the prototype can wire paste-confirm through
        `window.open_dialog` directly when that feature lands
        (currently the multi-line warn is just a checkbox in
        the profile editor with no live confirm UI).
- [ ] **Phase 8 — System integration.** The "make it feel like a
      real shipped app" pass: things you'd notice are missing
      every time the app launches but that none of the previous
      phases needed in place. Sub-items are independently
      shippable; pick whichever's easiest to land first.

      Application menu (macOS-first; Windows / Linux mostly inherit
      the in-window menus already)
      - [x] **Standard macOS menubar** — initial skeleton landed.
            `install_app_menu` in main.rs registers `Quit` (Cmd+Q
            → `cx.quit()`) and `NewWindow` (Cmd+N → opens a new
            top-level window with the shared stores) via
            `cx.bind_keys` + `cx.on_action`, then calls
            `cx.set_menus` with Baudrun / File / Window / Help.
            Edit / View entries + the rest of the standard
            macOS slots (Services, Hide, Show All) come along
            with the next sub-items below.
      - [x] **Wire Settings → Shortcuts bindings to menu items.**
            Twelve new gpui actions (`Connect`, `Disconnect`,
            `Suspend`, `Resume`, `ClearTerminal`, `SendBreak`,
            `SendFile`, `NewProfile`, `OpenInNewWindow`,
            `FontIncrease/Decrease/Reset`) map 1:1 to
            `settings_view::SHORTCUT_ACTIONS`. At boot,
            `apply_shortcut_bindings` reads each action's
            effective spec (`effective_shortcut` from
            settings_view), converts via the new
            `spec_to_gpui_binding` helper (W3C `Meta+Shift+K` →
            gpui `cmd-shift-k`), and registers them all via
            `cx.bind_keys`. The same function rebuilds the
            menubar (`Baudrun / File / Session / View / Window /
            Help`) so the accelerators next to each label reflect
            the new bindings. A SettingsBus subscription re-runs
            the whole apply step on every `Updated` event, so
            edits in Settings → Shortcuts propagate to the
            menubar live. Per-window dispatch is wired on
            `AppView`'s outermost div via twelve `.on_action`
            handlers — Connect saves+connects through the open
            editor or kicks off `connect_to` for the sidebar
            selection; ClearTerminal forwards to
            `TerminalView::clear_screen`; Font* writes through
            `SettingsBus::replace` so the existing
            `apply_font_size` re-render path handles the
            push to alacritty.
      - [ ] **About Baudrun panel** — standard macOS "About"
            sheet showing version, copyright, GitHub link. Tauri
            shipped one; gpui's panel can be a small modal.
      - [ ] **Dock menu** (macOS) — right-click on the dock icon
            offers "New Window" + recent profiles for one-click
            reconnect.

      Branding + bundle metadata
      - [ ] **App icon** — design + bundle the `.icns` (macOS),
            `.ico` (Windows), and `.png` set (Linux). The icon
            already exists in the Tauri build; copy it over.
      - [ ] **macOS Info.plist** — CFBundleIdentifier, version,
            human-readable copyright, minimum OS, NSHighResolution
            flag. Required before code signing in Phase 9.
      - [ ] **Window title + taskbar / dock label** match the
            bundle's display name across platforms (currently
            the prototype shows "Baudrun (prototype)" in some
            titlebars).

      Behavior
      - [ ] **Single-instance launch** — opening Baudrun while an
            existing instance is running focuses the existing
            window instead of spawning a second process. Per
            platform: macOS handles this via the bundle's
            `LSMultipleInstancesProhibited` + NSApplication
            delegate; Windows / Linux need a named-mutex or
            unix-socket dance.
      - [ ] **Quit confirmation when a session is active.**
            Prompt before tearing down a live serial connection
            on Cmd+Q / window-close-all so a stray keystroke
            doesn't lose a reconnect-in-progress.
      - [ ] **Prefers-reduced-motion** — query the OS setting
            (NSWorkspace on macOS, SPI_GETCLIENTAREAANIMATION on
            Windows, GTK / Qt on Linux) and skip pulse animations
            (reconnect dot, dialog slide-in) when on.

      File / URL associations (lower priority)
      - [ ] **`.baudrun-profile.json` file association.** Double-
            clicking a profile JSON in Finder / Explorer launches
            Baudrun and imports the profile.
      - [ ] **`baudrun://` URL scheme.** `baudrun://connect/<port>?baud=9600`
            deep-links from a browser, docs link, or another app
            into a one-click connect. **[on request]** — niche
            feature; revisit if a real use case shows up.
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
