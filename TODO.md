# TODO

## Migration: alacritty + gpui

The alacritty_terminal + gpui rewrite is now `main`. The pre-rewrite
xterm.js + Tauri code (which had unfixable per-keystroke typing
latency on Windows) lives on `tauri-archive` for back-port reference.
The cutover happened as a branch rename — the experiments work moved
to `main` and the Tauri history moved to `tauri-archive` — rather
than as a merge, so the gpui history is linear and the Tauri history
stays untouched. Releases up to `v0.9.5` (the last stable Tauri
build) and the `v0.9.6-beta.*` line came from `tauri-archive`; the
`v0.9.7-alpha.*` line and forward come from the gpui `main`.

macOS is the primary dev / test target. Windows + Linux are kept in
mind in code, verified periodically — Linux less often since gpui's
Linux backend is the least-mature of the three.

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
      - [x] **About Baudrun panel.** New `About` action wired to
            the Baudrun → "About Baudrun" menu item (above Quit).
            `shortcut_about` opens a gpui-component `Dialog` over
            the active window with name, version (from
            `CARGO_PKG_VERSION`), tagline (from
            `CARGO_PKG_DESCRIPTION`), copyright, and a "View on
            GitHub" link that calls `cx.open_url` to launch the
            repo in the user's default browser. No accelerator —
            About is menu-only, so no Settings → Shortcuts entry.
      - [x] **Dock menu** (macOS). `install_dock_menu` registers
            a `cx.set_dock_menu` with `New Window` + a separator +
            up to 10 profiles by stable creation order; clicking a
            profile dispatches a new `ConnectToProfile { profile_id }`
            action (hand-derived `Action` with payload, `no_json`
            since it's never serialized in a keymap). The global
            handler routes via `dispatch_to_app_view` into the
            active window's AppView and calls
            `connect_profile_in_new_window`, mirroring the
            sidebar right-click → "Open in New Window" flow.
            Built once at boot — doesn't live-update on profile
            create/rename/delete because the profile store
            doesn't emit change events; a relaunch refreshes it.

      Branding + bundle metadata
      - [x] **App icon.** Icons copied from `src-tauri/icons/`
            into `prototype/resources/icons/` (icon.icns, icon.ico,
            icon.png + the 32 / 64 / 128 / 128@2x size variants).
            macOS dev mode picks up a runtime override —
            `install_macos_dock_icon` loads icon.icns, composites
            it onto a 1024×1024 canvas with a ~10% transparent
            margin (Apple's "live area" inset; the Tauri-generated
            source fills its canvas edge-to-edge so a raw
            `setApplicationIconImage` looked oversized next to
            other dock apps), then calls
            `NSApplication.applicationIconImage`. Production
            builds (Phase 9) pick the .icns up from the .app
            bundle's Resources via Info.plist. Direct objc2 +
            objc2-app-kit + objc2-foundation deps were added under
            `target.'cfg(target_os = "macos")'` so the small bit
            of ObjC plumbing stays mac-only.
      - [x] **macOS Info.plist** committed at
            `prototype/resources/Info.plist`. CFBundleIdentifier
            (`io.github.packetThrower.Baudrun` — matches the
            Tauri version so existing keychain entries stay
            valid), display + executable names, version pair
            (CFBundleShortVersionString + CFBundleVersion),
            CFBundleIconFile pointing at the .icns sibling,
            LSMinimumSystemVersion = 11.0, NSHighResolutionCapable
            = true, NSPrincipalClass = NSApplication,
            NSHumanReadableCopyright, LSApplicationCategoryType =
            developer-tools. Static values for now — Phase 9 will
            wire build-script generation when CI starts cutting
            signed release builds. File associations + URL
            scheme entries are intentionally left out; they'll
            land with the corresponding Phase 8 sub-items.
            `plutil -lint` clean.
      - [x] **Window title + taskbar / dock label** match the
            bundle name. Window titles were already "Baudrun" /
            "Settings · Baudrun" (no `(prototype)` suffix
            remaining), but `cargo run` dev mode showed
            "baudrun-prototype" in the macOS menu-bar app slot,
            Cmd+Tab label, and dock tooltip because non-bundled
            apps fall back to the binary file name. Added a
            `[[bin]] name = "Baudrun"` to `prototype/Cargo.toml`
            so the built executable is `target/debug/Baudrun`
            and macOS picks "Baudrun" everywhere even without a
            .app wrapper. Cargo package name stays kebab-case.

      Behavior
      - [x] **Single-instance launch** (macOS). Adds
            `LSMultipleInstancesProhibited=true` to Info.plist so
            Launch Services routes any second launch attempt
            (double-click .app, `open -a Baudrun`, profile-JSON
            associations) to the existing process. The matching
            runtime `handle_reopen` (registered via
            `Application::on_reopen` before `run`) calls
            `cx.activate(true)` and — when no windows are open —
            spawns a fresh welcome window via the same
            `open_app_window` path the menubar uses. Stores are
            shared with the pre-`run` callback through a new
            `AppShared` gpui `Global` that `run` populates after
            it builds the settings_bus Entity. Windows / Linux
            still need a named-mutex / unix-socket equivalent —
            tracked separately when those platforms become a
            primary target.
      - [x] **Quit confirmation when a session is active.**
            Cmd+Q now routes through `confirm_quit_then_quit`,
            which (deferred out of the action's window-update so
            the per-window probes don't re-enter the same window)
            scans `cx.windows()` for any AppView whose new
            `has_live_session` returns true — connected profile,
            in-flight X/YMODEM transfer, or active auto-reconnect
            retry. No live session → quits immediately. Live
            session → opens an alert dialog ("Quit Baudrun?" with
            Quit / Cancel + Esc-to-dismiss) anchored to the
            window that owns the session. The window-close-all
            path on macOS doesn't actually quit (handled by the
            single-instance commit), so the Cmd+Q gate covers the
            real "stray keystroke kills your session" risk.
      - [x] **Prefers-reduced-motion** (macOS). New `ReduceMotion`
            gpui `Global` initialised at boot from
            `NSWorkspace.accessibilityDisplayShouldReduceMotion`
            (objc2-app-kit NSWorkspace + NSAccessibility
            features). AppView reads it in the two reconnect-dot
            pulse spots (session header + sidebar row) and skips
            the `with_animation` wrap; TerminalView's blink task
            re-reads the global every tick so the terminal
            cursor goes solid when reduce-motion is on (the
            task stays alive but its toggle becomes a no-op).
            Boot-time read for the dot pulses, per-tick read for
            the cursor blink — runtime toggle of the OS setting
            stops the cursor immediately but reconnect pulses
            need a relaunch. Windows / Linux return false until
            those platforms get their own
            `SystemParametersInfo` / `gtk-enable-animations`
            queries.

      File / URL associations (lower priority)
      - [ ] **`.baudrun-profile.json` file association.** Double-
            clicking a profile JSON in Finder / Explorer launches
            Baudrun and imports the profile.
      - [ ] **`baudrun://` URL scheme.** `baudrun://connect/<port>?baud=9600`
            deep-links from a browser, docs link, or another app
            into a one-click connect. **[on request]** — niche
            feature; revisit if a real use case shows up.

            **Security model (decide before implementing).** Once
            Baudrun registers as the `baudrun://` handler any
            webpage or app can fire a URL — URL schemes have no
            origin check, so the threat model is "drive-by from
            a hostile / phished page". Design guardrails up
            front, before the first `cx.on_open_urls` line lands:

            * **Profile-only addressing.** URL references a saved
              profile id / name. No raw port paths, no raw baud /
              parity in the URL — those have already passed
              through the user's own config. Stops the "open
              `/dev/cu.SecureModem` you didn't ask for" path.
            * **Mandatory confirmation sheet**, default-button =
              Cancel, showing the resolved profile name + port +
              baud. No silent connects ever. macOS-style "this
              page wants to open Baudrun" prompt.
            * **Hard non-feature: no bytes-to-send payload.** Not
              even base64. Document explicitly so a future "let's
              just add a small `?send=` parameter" PR fails review
              at the doc level. Otherwise a malicious link could
              write `reload in 1\r` to a Cisco console.
            * **Rate-limit the handler.** Bin to one accept per
              ~2s so window-spam / activation-DoS is harmless.
            * **Always foreground.** Bring Baudrun to front so
              the user sees what's happening; never silent-open
              a background session.
            * **No URL parameter that would change Settings.**
              The URL is read-only with respect to the user's
              config — can pick a profile to connect to, can't
              edit one.
- [ ] **Phase 9 — Auto-updater + distribution.** `tauri-plugin-updater`
      is gone post-Tauri; the goal is one signed, notarized,
      auto-updating Baudrun.app per tagged release. Pick whichever
      sub-item is easiest to land first; they're independently
      shippable.

      Foundation
      - [x] **Wire prototype into CI.** `.github/workflows/ci.yml`
            now runs against the root crate (`baudrun`) — the
            Tauri / Svelte `frontend` job is gone, Linux deps
            drop GTK / WebKit / soup / appindicator and add
            xkbcommon / wayland / x11 / xcb for `gpui_linux`,
            macOS / Windows installs are unchanged. Triggers
            on pushes / PRs against `main` only — the
            `experiments/alacritty-gpui` trigger was dropped
            when the rewrite branch was renamed to `main`.
            Cleaned up 20 clippy lints that had accumulated
            during Phase 2–8 (deprecated NSImage::lockFocus,
            field-reassign-with-default in terminal_grid, dead
            code attributes for transfer state held for future
            use, doc-list-item indentation in settings_view, a
            duplicated `#[allow(clippy::too_many_arguments)]`
            on advanced_pane, manual is_multiple_of, an
            unnecessary unsafe block in detect_reduce_motion,
            `WindowInit::WithSession` boxed to dodge the
            variant-size lint) so the `-D warnings` gate
            mirrors the strictness the Tauri build used to
            hold. `cargo test` runs the 46 in-binary unit
            tests (no `--lib` flag — the crate has no library
            target).

      Bundling
      - [x] **Pick a bundler — cargo-packager** (revised from an
            earlier `cargo-dist` decision). The original
            cargo-dist pick was based on its workflow generator
            + axoupdater self-update integration, but a `dist
            plan` run against the actual crate showed
            cargo-dist's macOS output is `.tar.xz` of the bare
            binary — no `.app`, no `.dmg`. Same gap on Linux
            (no `.deb`/`.rpm`/`.AppImage`) and Windows (no
            `.msi`/NSIS). cargo-dist is built for CLI tools
            (ripgrep, bat, eza) shipped via curl-pipe / brew /
            scoop wrappers, not desktop apps.

            cargo-packager (the Tauri team's spin-off, last
            release Nov 2025) is the direct replacement for
            `tauri-action` in our old release.yml. Native
            support for `.dmg` on macOS, `.msi` + NSIS on
            Windows, `.deb` + `.AppImage` on Linux. We'll add
            an `fpm` step for `.rpm` and `.pkg.tar.zst` —
            same pattern the old Tauri release.yml used for
            `.pkg.tar.zst` since Tauri's bundler didn't target
            pacman. Coverage matrix matches what Tauri shipped.

            For the auto-updater sub-item below: axoupdater is
            usable standalone outside cargo-dist if we want it,
            otherwise cargo-packager has its own updater plugin
            in the Tauri ecosystem we can lift.
      - [x] **Local `.app` build that works.** `cargo packager
            --release -f app -f dmg` produces a usable Baudrun.app
            (binary at `Contents/MacOS/Baudrun` matching our
            `[[bin]] name`, the hand-curated `resources/Info.plist`
            copied to `Contents/Info.plist`,
            `resources/icons/icon.icns` copied to
            `Contents/Resources/icon.icns`) plus a 5.5 MB
            `Baudrun_0.0.1_aarch64.dmg` with the standard
            `Baudrun.app` + `/Applications` symlink layout for
            drag-to-install. Launch verified — process appears
            under `Baudrun.app/Contents/MacOS/Baudrun` and the
            dock icon picks up correctly (no need for the
            dev-mode `install_macos_dock_icon` override in this
            path because the .icns is at the bundle's standard
            location). `[package.metadata.packager]` config lives
            in Cargo.toml; merges with cargo-packager's defaults
            for the values we don't specify. CFBundleExecutable
            in `resources/Info.plist` corrected from `baudrun`
            (lowercase, mismatch with binary file name) to
            `Baudrun`. No signing yet.
      - [ ] **Windows + Linux smoke builds.** `cargo build
            --release` succeeds on both; .msi / .deb / AppImage
            output once the bundler choice settles. Lower
            priority than macOS — Windows is the primary
            secondary target, Linux is best-effort until gpui's
            Wayland story matures.

      Signing + notarization (macOS first)
      - [ ] **Developer ID Application signing.** Port the
            existing `.github/workflows/release.yml`'s signing
            block (it's already wired against the Tauri build) to
            the new bundle. Secrets stay the same:
            `APPLE_SIGNING_IDENTITY`,
            `APPLE_SIGNING_CERTIFICATE`,
            `APPLE_CERTIFICATE_PASSWORD`.
      - [ ] **Notarization + staple.** Submit signed .app to
            Apple, wait for OK, staple ticket so Gatekeeper
            doesn't network-fetch on every launch. The existing
            release.yml does this for Tauri; same job needs to
            run against our output.
      - [ ] **Hardened runtime + entitlements.** Required for
            notarization. We need `com.apple.security.cs.disable-
            library-validation` (gpui dynamically loads its
            renderer dylib) and probably nothing else — the
            prototype reads serial ports via standard POSIX which
            doesn't need an entitlement.

      Release pipeline
      - [x] **New release workflow for prototype.** Replaced the
            736-line Tauri-flavoured `.github/workflows/release.yml`
            with a 431-line cargo-packager-driven workflow. Same
            six-platform matrix (`macos-26` arm64, `macos-15-intel`
            amd64, `windows-latest` x64, `windows-11-arm` arm64,
            `ubuntu-latest` amd64, `ubuntu-24.04-arm` arm64). Per
            platform:
            * macOS: `.dmg` + drag-droppable `.app.zip` (via
              `ditto -c -k --keepParent` to preserve resource
              forks for future signing).
            * Windows: NSIS `-setup.exe` always, WiX `.msi` on
              stable tags only (WiX rejects alphanumeric pre-
              release identifiers), portable `.zip` of bare
              `Baudrun.exe`.
            * Linux: `.deb` + `.AppImage` from cargo-packager
              natively. `.rpm` + `.pkg.tar.zst` via fpm because
              (a) cargo-packager doesn't target rpm, and
              (b) its pacman format emits a `.tar.gz` + PKGBUILD
              meant for AUR submission, not a directly-installable
              `.pkg.tar.zst`. Both fpm targets share the same
              `pkg/` staging directory the .rpm step already
              built up (binary, .desktop, udev rule, hicolor
              icon).
            Version tag (`v0.5.0` → `0.5.0`) gets patched into
            `Cargo.toml`'s `version` field plus
            `resources/Info.plist`'s `CFBundleShortVersionString`
            + `CFBundleVersion` before bundling. SHA256SUMS +
            generated release notes (with the same prev-tag
            walk + pre-release flagging the Tauri version
            had) keep working. Pre-release detection still
            via hyphen-in-tag.

            Also enabled `rusb`'s `vendored` feature — libusb
            compiles statically into the binary, so the old
            release.yml's `install_name_tool` dance to rewrite
            `/opt/homebrew/lib/libusb-1.0.0.dylib` load commands
            inside the .app is gone. `otool -L` on the resulting
            binary shows zero Homebrew references; user machines
            need no libusb install.

            Still pending in subsequent sub-items: code signing
            (`secrets.APPLE_SIGNING_*` not wired through yet),
            notarization, hardened-runtime entitlements,
            Windows code signing.
      - [ ] **CI build coverage.** Compile the prototype's
            release-mode bundle on every PR (no signing, no
            upload) so a broken bundle catches before it merges.
            Skip signing in PRs — only the tag job has the
            signing secrets.

      Auto-updater
      - [ ] **Pick the updater crate.** `self_update` is the
            obvious starting candidate: pulls assets from GitHub
            Releases, verifies a checksum, replaces the running
            binary. `cargo-dist` bundles its own updater
            (`cargo-dist update` flow) if we pick it as the
            bundler. Decide alongside the bundler choice — they
            interact.
      - [ ] **On-launch update check.** Honour the existing
            `disable_update_check` + `include_prerelease_updates`
            settings (they're already in `settings.json` and
            persisted from the Tauri version). Compare local
            version (`CARGO_PKG_VERSION`) against the highest
            GitHub Release tag; surface a footer toast +
            "Download" link when a newer release exists.
            `dismissed_update_version` already gates the toast
            against re-prompting after the user clicks away.
      - [ ] **Apply update on quit.** One-click "Install" in the
            toast that downloads the new .app, replaces the
            running bundle, and re-launches. Has to defer the
            replace until after the current process exits — same
            constraint Sparkle handles on macOS via a relauncher
            helper.
- [ ] **Phase 10 — Polish & cutover.** Cross-platform perf passes.
      Migration of existing user data (profiles, themes) from old
      app's config dir. Alpha + beta runs (`v0.9.7-alpha.*` /
      `v0.9.7-beta.*`) shake out cross-platform regressions on the
      now-`main` branch. Once Phase 9 (signing + auto-updater) ships
      stable, cut a `1.0.0-rc`. (The branch cutover from
      `experiments/alacritty-gpui` → `main` already happened as a
      rename — old `main` is now `tauri-archive`.)

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

## Footer / notification parity with Tauri

Surveyed against `tauri-archive` (see analysis in conversation history,
2026-05-16). The Tauri build had a richer set of footer-pill / toast
notifications and a couple of features that the gpui rewrite hasn't
brought back yet. The cheap one-line-each notification wires were
landed in commit TODO; the items below are the remaining feature work.

- [ ] **In-app auto-installer with progress.** Tauri shipped a
      footer `update-toast` pill with **Install / Notes / Dismiss**
      buttons; clicking Install downloaded the new build, surfaced
      a progress bar (`"Installing v${version}… 47%"`), verified
      signature, applied the update in-place, and relaunched. On
      `main` the updater ([src/updater.rs](src/updater.rs)) detects
      newer releases and lights up an amber dot on the Settings
      gear, but the user still has to open the Releases page, pick
      the right asset for their platform, download, and run the
      installer/replace-the-.app by hand. Platform-specific apply:
      macOS swap `Baudrun.app` then `relaunch`; Windows invoke NSIS
      uninstaller-then-installer; Linux `dpkg -i` / `rpm -U` / etc.
      Likely a v0.12 headline feature on its own — significant
      scope including signature verification and partial-write
      recovery. Defer until there's time for a focused arc.

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
- [ ] **arm64 Windows MSI via cargo-wix.** The `build-windows` job
      builds the MSI with cargo-packager, gated to amd64 only.
      cargo-packager 0.11.8 (the latest — even its unreleased `main`)
      hardcodes a download of WiX 3.11 (2017), and WiX 3.11's
      `candle.exe` predates `-arch arm64` (added in WiX 3.14). There's
      no `WIX_URL` / `WIX_PATH` override in cargo-packager, so arm64
      MSI is impossible through it — the `candle.exe -arch arm64`
      invocation just exits non-zero, and cargo-packager's
      `Error::WixFailed` Display (`"Error running {0}: {0}"`) throws
      the real error away. Fix: move MSI generation off cargo-packager
      onto [`cargo-wix`](https://github.com/volks73/cargo-wix), which
      uses system-installed WiX (so we pick the version) and supports
      WiX 4+/3.14.1+ with native arm64.
      1. `cargo wix init` to generate `wix/main.wxs`; customise
         (product name, shortcuts, install dir) and commit it.
      2. Set the `.wxs` `UpgradeCode` to match what cargo-packager's
         MSI used, so existing amd64 installs upgrade in place rather
         than installing side-by-side.
      3. In `build-windows`: add a WiX install step
         (`dotnet tool install --global wix` — the .NET SDK is already
         on both runners), drop `-f wix` from the `cargo packager`
         call, add a `cargo wix` step for both arches.
      4. cargo-packager keeps `.dmg` / `.app` / NSIS / `.deb` / `.rpm`
         / `.AppImage`; cargo-wix owns MSI for x64 **and** arm64 — one
         generator, consistent installers.
      Best done alongside Windows code-signing above: both touch the
      `build-windows` job, and the same `signtool` step can sign the
      cargo-wix MSI output.
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
