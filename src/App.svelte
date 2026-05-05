<script lang="ts">
  // ─── perf instrumentation (alpha track) ───────────────────────────
  // Mark this point ASAP so all subsequent timestamps are relative to
  // the moment the JS bundle began executing in this webview. Doing
  // this at module scope (outside onMount) catches the import-cost
  // overhead that mount-time marks would miss.
  const PERF_T0 =
    typeof performance !== "undefined" ? performance.now() : Date.now();
  if (typeof performance !== "undefined") {
    performance.mark("app:script-start");
  }
  /** Log + record a Performance API mark. The mark shows up in the
   *  DevTools → Performance flame chart; the console line provides a
   *  copy-pasteable summary that doesn't require re-recording. */
  function perfMark(name: string): void {
    const t =
      typeof performance !== "undefined" ? performance.now() : Date.now();
    if (typeof performance !== "undefined") {
      performance.mark(`app:${name}`);
    }
    // eslint-disable-next-line no-console
    console.log(`[perf] app:${name} t=${Math.round(t - PERF_T0)}ms`);
  }
  /** Time a single async call and log the elapsed ms. The call's
   *  result is forwarded unchanged so this is a transparent wrap. */
  async function timed<T>(name: string, fn: () => Promise<T>): Promise<T> {
    const start =
      typeof performance !== "undefined" ? performance.now() : Date.now();
    try {
      return await fn();
    } finally {
      const end =
        typeof performance !== "undefined" ? performance.now() : Date.now();
      const elapsed = Math.round(end - start);
      if (typeof performance !== "undefined") {
        // duration form is well-supported in modern WebView2 / WebKit.
        performance.measure(`ipc:${name}`, {
          start,
          duration: elapsed,
        } as PerformanceMeasureOptions);
      }
      // eslint-disable-next-line no-console
      console.log(`[perf] ipc:${name} took=${elapsed}ms`);
    }
  }
  // ──────────────────────────────────────────────────────────────────

  import { onMount, onDestroy, tick } from "svelte";
  import Sidebar from "./lib/Sidebar.svelte";
  import ProfileForm from "./lib/ProfileForm.svelte";
  import Terminal from "./lib/Terminal.svelte";
  import Settings from "./lib/Settings.svelte";
  import { api, type Profile, type Theme, type TransferProtocol } from "./lib/api";
  import { formatPortName } from "./lib/ports";
  import Select from "./lib/Select.svelte";
  import {
    effectiveShortcut,
    formatShortcut,
    matchesShortcut,
  } from "./lib/shortcuts";
  import {
    profiles,
    selectedProfileID,
    loadProfiles,
    createProfile,
    updateProfile,
    deleteProfile,
  } from "./stores/profiles";
  import {
    themes,
    settings,
    loadThemes,
    loadSettings,
    importTheme,
    deleteTheme,
    setDefaultTheme,
  } from "./stores/themes";
  import {
    skins,
    activeSkinID,
    appearance,
    systemIsDark,
    loadSkins,
    applySkin,
    resolveSkin,
    importSkin,
    deleteSkin,
    type Appearance,
  } from "./stores/skins";
  import { session } from "./stores/session";
  import { portScanning } from "./stores/scanning";
  import {
    highlightPacks,
    loadHighlightPacks,
    applyEnabledHighlightPresets,
  } from "./stores/highlight";
  import { getVersion } from "@tauri-apps/api/app";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { check as tauriCheckUpdate } from "@tauri-apps/plugin-updater";
  import { checkForUpdate, type AvailableUpdate } from "./lib/updater";

  let draft = $state<Profile | null>(null);
  let terminalRef = $state<Terminal | null>(null);
  let statusMsg = $state("");
  let offDisconnect: (() => void) | null = null;
  let offReconnecting: (() => void) | null = null;
  let offReconnected: (() => void) | null = null;
  let offSettingsUpdated: (() => void) | null = null;
  // App.svelte mounts in every Tauri window. The label tells us
  // whether this is the main shell (profiles + terminal) or the
  // dedicated Settings window opened via toggle_settings_window.
  // The Settings window renders ONLY the Settings component; it
  // skips session listeners, terminal mounting, the update toast,
  // and the empty-state branding.
  //
  // IIFE so the value is `const` (set once at module init, never
  // reassigned). svelte-check otherwise warns that a `let` not
  // wrapped in `$state(...)` won't trigger reactive updates — true
  // in general, but irrelevant here since this never changes after
  // the window is created. The try/catch covers the plain-Vite-dev
  // case where `getCurrentWebviewWindow()` throws (no Tauri
  // runtime); the catch branch falls back to main-shell rendering.
  const isSettingsWindow = (() => {
    try {
      return getCurrentWebviewWindow().label === "settings";
    } catch {
      return false;
    }
  })();

  // Sidebar's Settings button "active" hint. Mirrors the IPC return
  // value from toggleSettingsWindow. May go stale if the user closes
  // the Settings window via its own X button (no event back to the
  // main window yet) — minor visual lag, not load-bearing.
  let settingsOpen = $state(false);
  let suspended = $state(false);
  let ctrlDTR = $state(true);
  let ctrlRTS = $state(true);

  const selectedExisting = $derived(
    $profiles.find((p) => p.id === $selectedProfileID) ?? null,
  );
  const currentProfile = $derived(draft ?? selectedExisting);
  const isNew = $derived(!!draft);
  const activeProfileID = $derived(
    $session.status === "connected" ||
      $session.status === "connecting" ||
      $session.status === "reconnecting"
      ? $session.profileID
      : "",
  );
  const isConnected = $derived(
    $session.status === "connected" &&
      currentProfile?.id === $session.profileID,
  );
  const isConnecting = $derived(
    $session.status === "connecting" &&
      currentProfile?.id === $session.profileID,
  );
  const isReconnecting = $derived(
    $session.status === "reconnecting" &&
      currentProfile?.id === $session.profileID,
  );
  // Keep the terminal visible during a reconnect so scrollback survives.
  const viewingTerminal = $derived(
    (isConnected || isReconnecting) && !suspended,
  );
  const hasSession = $derived(
    $session.status === "connected" ||
      $session.status === "connecting" ||
      $session.status === "reconnecting",
  );
  const activeProfile = $derived(
    $profiles.find((p) => p.id === activeProfileID) ?? null,
  );
  const termLineEnding = $derived(
    (activeProfile?.lineEnding ?? currentProfile?.lineEnding ?? "crlf") as
      | "cr"
      | "lf"
      | "crlf",
  );
  const termLocalEcho = $derived(
    activeProfile?.localEcho ?? currentProfile?.localEcho ?? false,
  );
  const termHighlight = $derived(
    activeProfile?.highlight ?? currentProfile?.highlight ?? true,
  );
  const termHexView = $derived(
    activeProfile?.hexView ?? currentProfile?.hexView ?? false,
  );
  const termTimestamps = $derived(
    activeProfile?.timestamps ?? currentProfile?.timestamps ?? false,
  );
  const termBackspaceKey = $derived(
    ((activeProfile?.backspaceKey || currentProfile?.backspaceKey) || "del") as
      | "bs"
      | "del",
  );
  const termCopyOnSelect = $derived($settings.copyOnSelect ?? false);
  const termScreenReaderMode = $derived($settings.screenReaderMode ?? false);
  const termPasteWarnMultiline = $derived(
    activeProfile?.pasteWarnMultiline ?? currentProfile?.pasteWarnMultiline ?? false,
  );
  const termPasteSlow = $derived(
    activeProfile?.pasteSlow ?? currentProfile?.pasteSlow ?? false,
  );
  const termPasteCharDelayMs = $derived(
    activeProfile?.pasteCharDelayMs ?? currentProfile?.pasteCharDelayMs ?? 10,
  );

  const effectiveThemeID = $derived(
    (activeProfile?.themeId || currentProfile?.themeId) ||
      $settings.defaultThemeId ||
      "baudrun",
  );
  const effectiveTheme = $derived(resolveTheme(effectiveThemeID, $themes));
  const termFontSize = $derived($settings.fontSize || 13);
  const termScrollback = $derived($settings.scrollbackLines || 10000);

  // Re-apply skin whenever the active selection, loaded list, appearance
  // preference, or system color scheme changes. The window's own NSAppearance
  // is pinned dark at launch (main.go) because Wails v2.12's runtime theme
  // setters are empty stubs on macOS — only CSS swaps live.
  $effect(() => {
    applySkin(
      resolveSkin($activeSkinID, $skins),
      $appearance,
      $systemIsDark,
    );
  });

  // Per-profile override > global setting. activeProfile drives this
  // when a session is open (so a connected profile's overrides win
  // even while the user is editing another profile in the form);
  // currentProfile covers the no-session case (profile form open,
  // user toggles a pack — the change should preview live).
  const effectiveEnabledHighlightPresets = $derived(
    activeProfile?.enabledHighlightPresets ??
      currentProfile?.enabledHighlightPresets ??
      $settings.enabledHighlightPresets,
  );

  // Recompile the highlight engine whenever the resolved selection
  // changes. The initial apply happens in onMount once both packs
  // and settings have loaded; this fires for every subsequent change.
  $effect(() => {
    applyEnabledHighlightPresets(effectiveEnabledHighlightPresets);
  });

  function resolveTheme(id: string, all: Theme[]): Theme | undefined {
    return (
      all.find((t) => t.id === id) ??
      all.find((t) => t.id === "baudrun") ??
      all[0]
    );
  }

  let defaultLogDir = $state("");
  let configDir = $state("");
  let defaultConfigDir = $state("");
  let availableUpdate = $state<AvailableUpdate | null>(null);
  // Auto-install state. The Tauri updater plugin only handles stable
  // releases (its endpoint serves a single latest.json). Pre-release
  // updates fall back to opening the release notes in the browser.
  let updateInstalling = $state(false);
  let updateProgress = $state<{ downloaded: number; total: number } | null>(
    null,
  );
  let updateInstallError = $state("");

  // Delayed-delete state for the profile undo flow. The profile stays
  // in the backend until the timer fires or the user undoes; the
  // sidebar just filters it out during the pending window.
  type PendingDelete = {
    profile: Profile;
    timerId: ReturnType<typeof setTimeout>;
  };
  let pendingDelete = $state<PendingDelete | null>(null);
  const UNDO_WINDOW_MS = 10_000;
  // Seconds remaining on the current pending delete's undo window.
  // Ticks down via a $effect-managed interval so the button label
  // shows a live countdown.
  let pendingDeleteSecondsLeft = $state(0);

  $effect(() => {
    if (!pendingDelete) return;
    pendingDeleteSecondsLeft = Math.ceil(UNDO_WINDOW_MS / 1000);
    const tick = setInterval(() => {
      pendingDeleteSecondsLeft = Math.max(0, pendingDeleteSecondsLeft - 1);
    }, 1000);
    return () => clearInterval(tick);
  });

  const visibleProfiles = $derived(
    pendingDelete
      ? $profiles.filter((p) => p.id !== pendingDelete!.profile.id)
      : $profiles,
  );

  onMount(async () => {
    perfMark(
      isSettingsWindow ? "settings-onmount" : "main-onmount",
    );
    // Svelte has no formal error boundaries; without these two
    // listeners an unhandled promise rejection or a runtime error
    // silently blanks the UI with no indication of what went wrong.
    // Routing them to the status bar at least surfaces *something*
    // the user can report.
    window.addEventListener("error", (e) => {
      statusMsg = `Error: ${e.message}`;
      console.error(e);
    });
    window.addEventListener("unhandledrejection", (e) => {
      const reason = (e.reason && (e.reason.message || e.reason)) || "unknown";
      statusMsg = `Unhandled: ${reason}`;
      console.error(e.reason);
    });

    // Single-IPC bootstrap. Replaces the 5+3 separate calls (5
    // store loads + 3 dir lookups) the previous version fanned out
    // on mount. Each `ipc.localhost` call has non-trivial latency on
    // Windows and queues against in-flight calls from sibling
    // windows; trace data showed the new-window-open path stalling
    // ~500ms after Promise.all because the main window's
    // list_ports was tail-blocking the response thread. Collapsing
    // to one call removes that exposure entirely.
    perfMark("before-bootstrap");
    try {
      const boot = await timed("bootstrap", () =>
        api.bootstrapWindowState(),
      );
      profiles.set(boot.profiles);
      themes.set(boot.themes);
      skins.set(boot.skins);
      settings.set(boot.settings);
      highlightPacks.set(boot.highlightPacks);
      defaultLogDir = boot.defaultLogDir;
      configDir = boot.configDir;
      defaultConfigDir = boot.defaultConfigDir;
    } catch (err) {
      // Bootstrap failure is fatal for an empty UI — surface it via
      // the status bar and fall back to the per-store loaders so we
      // at least try to populate something. (This path also covers
      // a hypothetical mismatched-version scenario where the JS is
      // newer than a backend that doesn't know `bootstrap_window_state`.)
      console.error("bootstrap failed, falling back to per-store loaders:", err);
      statusMsg = "Bootstrap failed — UI may be incomplete";
      await Promise.all([
        loadProfiles(),
        loadThemes(),
        loadSkins(),
        loadSettings(),
        loadHighlightPacks(),
      ]);
      try { defaultLogDir = await api.defaultLogDirectory(); } catch {}
      try {
        configDir = await api.getConfigDirectory();
        defaultConfigDir = await api.getDefaultConfigDirectory();
      } catch {}
    }
    perfMark("after-bootstrap");
    activeSkinID.set($settings.skinId || "baudrun");
    appearance.set(($settings.appearance as Appearance) || "auto");
    applySkin(resolveSkin($activeSkinID, $skins), $appearance, $systemIsDark);
    perfMark("after-skin-apply");
    // Initial apply happens here; the $effect below picks up any
    // subsequent settings or profile-override changes.
    applyEnabledHighlightPresets(effectiveEnabledHighlightPresets);

    // Show-on-ready: settings + profile windows are built invisible
    // by the Rust side (commands/window.rs). Now that the skin has
    // applied to the DOM, request one more frame so the OS
    // compositor commits the painted surface, then reveal the
    // window. The user perceives: click → brief delay → fully-
    // painted window appears. No black flash possible — the OS
    // never had a chance to show an empty webview.
    //
    // Skipped for the main window (already visible from
    // tauri.conf.json) — calling show() on an already-visible
    // window is harmless but pointless.
    if (isSettingsWindow || isProfileWindow()) {
      requestAnimationFrame(async () => {
        try {
          await getCurrentWebviewWindow().show();
          perfMark(isSettingsWindow ? "settings-shown" : "profile-shown");
        } catch (err) {
          console.error("show window:", err);
        }
      });
    }

    // Settings broadcast — fires in EVERY window (including the one
    // that originated the change) so renderers can refresh local
    // stores cross-window. Without this, changing the skin in the
    // Settings window only repaints Settings; the main window's
    // local $settings stays stale and applySkin never re-runs there.
    // Set up before the main-only early return so both windows get
    // it. The originating window's redundant store set is
    // idempotent.
    offSettingsUpdated = api.onSettingsUpdated((next) => {
      settings.set(next);
      activeSkinID.set(next.skinId || "baudrun");
      appearance.set((next.appearance as Appearance) || "auto");
    });

    // Session listeners + terminal snapshot restoration + the pending
    // profile id only matter for the main window or a profile tear-
    // off window; the Settings window has no terminal, never holds a
    // session, and isn't spawned with a pending profile id. Skipping
    // these saves several IPC roundtrips on Windows where each call
    // is non-trivial via WebView2's message channel.
    if (isSettingsWindow) {
      // Paint mark: rAF after the data is in place fires on the
      // next frame, which is the earliest the user could see a
      // populated UI. Subtract this from script-start to get the
      // user-perceived "Settings window opened" latency.
      requestAnimationFrame(() => perfMark("settings-first-paint"));
      return;
    }

    // Multi-window: a window spawned via "Open profile in new window"
    // has its initial profile id stashed in the backend keyed by the
    // window label (see open_profile_window in
    // src-tauri/src/commands/window.rs). Drain it on mount and
    // pre-select that profile so the user lands on it instead of
    // the empty state. Restricted to the safe-id alphabet on both
    // ends so a hostile renderer can't smuggle anything weird.
    //
    // Earlier versions rode this in `?profile=<id>` URL params, but
    // `?` is invalid in Windows file paths and Tauri's
    // `WebviewUrl::App(PathBuf)` mangled the URL → blank webview on
    // Windows spawned windows.
    try {
      const initialProfile = await api.takePendingProfileId();
      if (initialProfile && /^[A-Za-z0-9_-]+$/.test(initialProfile)) {
        selectedProfileID.set(initialProfile);
      }
    } catch (err) {
      console.warn("take pending profile id:", err);
    }

    offDisconnect = api.onDisconnect((reason) => {
      session.set({ status: "idle" });
      statusMsg = reason ? `Disconnected: ${reason}` : "Disconnected";
    });

    offReconnecting = api.onReconnecting((portName) => {
      // Keep profileID from the current state so the UI stays anchored
      // to the same profile while we wait for the adapter to reappear.
      session.update((s) => {
        const id =
          s.status === "connected" ||
          s.status === "connecting" ||
          s.status === "reconnecting"
            ? s.profileID
            : "";
        return { status: "reconnecting", profileID: id };
      });
      statusMsg = `Reconnecting to ${formatPortName(portName)}…`;
    });

    offReconnected = api.onReconnected((profileID) => {
      session.set({ status: "connected", profileID });
      statusMsg = "Reconnected";
    });

    try {
      const activeID = await api.activeProfileID();
      if (activeID) session.set({ status: "connected", profileID: activeID });
    } catch {}

    // If this window was spawned by a session-migration tear-off,
    // the backend has a serialized xterm buffer waiting for us to
    // pull. Wait one tick for the Terminal component to mount
    // (gated by hasSession becoming true above), then write the
    // snapshot in. Falls through silently for the common case
    // (windows that weren't migration targets get null back).
    try {
      const snapshot = await api.takePendingTerminalSnapshot();
      if (snapshot) {
        await tick();
        terminalRef?.restoreSnapshot(snapshot);
      }
    } catch (err) {
      console.warn("restore migrated terminal snapshot:", err);
    }

    // (defaultLogDir / configDir / defaultConfigDir are loaded above
    // the isSettingsWindow gate — see comment there.)

    // Update check fires once per launch when enabled. The GitHub
    // request happens off the critical path — we don't await it
    // before rendering. Failures fall back to `null` internally so
    // network errors can't surface a UI error here.
    if (!$settings.disableUpdateCheck) {
      void runUpdateCheck();
    }
    // Same first-paint marker as the Settings branch above, for
    // apples-to-apples comparison between window types.
    requestAnimationFrame(() =>
      perfMark(
        isProfileWindow() ? "profile-first-paint" : "main-first-paint",
      ),
    );
  });

  /** Profile tear-off windows use a `win-<uuid>` label. We can't
   *  cache this at module init because window-label detection might
   *  race the Tauri runtime — call lazily inside the paint marker. */
  function isProfileWindow(): boolean {
    try {
      return getCurrentWebviewWindow().label.startsWith("win-");
    } catch {
      return false;
    }
  }

  async function runUpdateCheck() {
    try {
      const current = await getVersion();
      const update = await checkForUpdate(
        current,
        $settings.includePrereleaseUpdates ?? false,
      );
      if (!update) return;
      // Skip if the user already dismissed this exact version.
      if ($settings.dismissedUpdateVersion === update.version) return;
      availableUpdate = update;
    } catch (err) {
      console.warn("update check threw:", err);
    }
  }

  async function dismissUpdate() {
    if (!availableUpdate) return;
    const version = availableUpdate.version;
    availableUpdate = null;
    try {
      const updated = await api.updateSettings({
        ...$settings,
        dismissedUpdateVersion: version,
      });
      settings.set(updated);
    } catch (e) {
      // Non-fatal; the toast stays dismissed for this session
      // either way because we already cleared availableUpdate.
      console.warn("persist dismissed-update-version:", e);
    }
  }

  async function openUpdateUrl() {
    if (!availableUpdate) return;
    try {
      await openUrl(availableUpdate.url);
    } catch (e) {
      statusMsg = `Open release page failed: ${e}`;
    }
  }

  // Stable releases get the auto-install path: download the signed
  // artifact, verify with the embedded pubkey, replace the binary,
  // relaunch. Pre-releases skip this — the configured endpoint only
  // tracks stable, so there's nothing to download via the plugin and
  // the user clicks through to GitHub instead.
  async function installUpdate() {
    if (!availableUpdate || availableUpdate.prerelease) {
      void openUpdateUrl();
      return;
    }
    updateInstalling = true;
    updateInstallError = "";
    updateProgress = null;
    try {
      const update = await tauriCheckUpdate();
      if (!update) {
        updateInstallError =
          "Update endpoint returned no update — manifest may not be published yet";
        statusMsg = updateInstallError;
        return;
      }
      let total = 0;
      let downloaded = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
          downloaded = 0;
          updateProgress = { downloaded, total };
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          updateProgress = { downloaded, total };
        } else if (event.event === "Finished") {
          updateProgress = { downloaded: total, total };
        }
      });
      // Replaced binary; relaunch so the user lands on the new version
      // without manually quitting and re-opening.
      await relaunch();
    } catch (e) {
      const msg = String(e);
      updateInstallError = msg;
      statusMsg = `Update install failed: ${msg}`;
      console.error("update install:", e);
    } finally {
      updateInstalling = false;
    }
  }

  onDestroy(() => {
    offDisconnect?.();
    offReconnecting?.();
    offReconnected?.();
    offSettingsUpdated?.();
  });

  async function maybeAutoDisconnect() {
    if (hasSession && !suspended) {
      try {
        await api.disconnect();
        session.set({ status: "idle" });
      } catch (e) {
        statusMsg = `Disconnect failed: ${e}`;
      }
    }
  }

  async function handleSelect(id: string) {
    // If leaving an active terminal view (not suspended), tear down serial.
    if (viewingTerminal && id !== currentProfile?.id) {
      await maybeAutoDisconnect();
    }
    draft = null;
    settingsOpen = false;
    selectedProfileID.set(id);
  }

  async function handleCreate() {
    if (viewingTerminal) await maybeAutoDisconnect();
    const base = await api.defaultProfile();
    draft = { ...base, id: "", name: "Untitled" } as Profile;
    settingsOpen = false;
  }

  async function handleOpenInNewWindow(profile: Profile) {
    // If the profile is the active connection in THIS window, the
    // user expects the live session to follow regardless of which
    // gesture they used (right-click → "Open in new window" or
    // drag-out). Capture the terminal scrollback BEFORE we spawn so
    // it carries over too. If migration fails we still have the
    // disconnected new window; nothing's lost.
    const migrate =
      $session.status === "connected" && profile.id === activeProfileID;
    const snapshot = migrate ? terminalRef?.snapshot() ?? "" : "";
    try {
      const targetLabel = await api.openProfileWindow(profile.id, profile.name);
      if (migrate) {
        try {
          await api.migrateSession(targetLabel, snapshot || undefined);
          handleSessionMigrated();
        } catch (err) {
          statusMsg = `Session migrate failed: ${err}`;
        }
      }
    } catch (e) {
      statusMsg = `Open new window failed: ${e}`;
    }
  }

  function handleSessionMigrated() {
    // Sidebar dragged a connected profile out of this window; the
    // backend has already moved the SessionHandle to the new window
    // and updated the read-pump's emit target. Clear THIS window's
    // session UI so it stops claiming to be connected — the new
    // window will reflect the live connection via its own onMount
    // activeProfileID() pull plus the serial:reconnected event the
    // backend fires after migrate completes.
    session.set({ status: "idle" });
    suspended = false;
    statusMsg = "Session moved to new window";
  }

  // Toggle the dedicated Settings window. The backend handles
  // singleton enforcement (existing → close, none → spawn) and
  // returns the new state. Local `settingsOpen` mirror updates so
  // the sidebar button can light up as active. Auto-disconnect on
  // open isn't needed anymore — the terminal session keeps running
  // in the main window while Settings is open in its own window.
  async function handleToggleSettings() {
    try {
      settingsOpen = await api.toggleSettingsWindow();
    } catch (e) {
      statusMsg = `Settings window: ${e}`;
      console.error("toggle settings window:", e);
    }
  }

  async function handleSave(p: Profile) {
    try {
      if (isNew) {
        const saved = await createProfile(p);
        draft = null;
        selectedProfileID.set(saved.id);
      } else {
        await updateProfile(p);
      }
      statusMsg = "Saved";
    } catch (e) {
      statusMsg = `Save failed: ${e}`;
    }
  }

  async function handleDelete(id: string) {
    const profile = $profiles.find((p) => p.id === id);
    if (!profile) return;

    // If there's already a pending delete, commit it immediately —
    // the user has moved on and won't expect two simultaneous undos.
    if (pendingDelete) {
      await commitPendingDelete();
    }

    try {
      if (isConnected) await api.disconnect();
    } catch (e) {
      statusMsg = `Disconnect failed: ${e}`;
      return;
    }
    draft = null;
    if ($selectedProfileID === id) selectedProfileID.set(null);

    const timerId = setTimeout(() => {
      void commitPendingDelete();
    }, UNDO_WINDOW_MS);
    pendingDelete = { profile, timerId };
    statusMsg = `Deleted ${profile.name} — undo available for ${Math.ceil(UNDO_WINDOW_MS / 1000)}s`;
  }

  async function commitPendingDelete() {
    if (!pendingDelete) return;
    const { profile, timerId } = pendingDelete;
    clearTimeout(timerId);
    pendingDelete = null;
    try {
      await deleteProfile(profile.id);
    } catch (e) {
      statusMsg = `Delete failed: ${e}`;
    }
  }

  function undoDelete() {
    if (!pendingDelete) return;
    clearTimeout(pendingDelete.timerId);
    const restored = pendingDelete.profile;
    pendingDelete = null;
    statusMsg = `Restored ${restored.name}`;
    // Select the restored profile so the user lands back on it.
    selectedProfileID.set(restored.id);
  }

  async function handleConnect() {
    if (!currentProfile?.id) return;
    const id = currentProfile.id;
    session.set({ status: "connecting", profileID: id });
    suspended = false;
    statusMsg = "Connecting…";
    try {
      await api.connect(id);
      session.set({ status: "connected", profileID: id });
      statusMsg = `Connected to ${formatPortName(currentProfile.portName)} @ ${currentProfile.baudRate}`;
      await refreshControlLines();
      await tick();
      terminalRef?.focus();
    } catch (e) {
      session.set({ status: "idle" });
      statusMsg = `Connect failed: ${e}`;
    }
  }

  async function refreshControlLines() {
    try {
      const cl = await api.getControlLines();
      ctrlDTR = cl.dtr;
      ctrlRTS = cl.rts;
    } catch {
      // Not connected yet or backend unavailable — keep assumed state.
    }
  }

  async function toggleDTR() {
    const next = !ctrlDTR;
    try {
      await api.setDTR(next);
      ctrlDTR = next;
    } catch (e) {
      statusMsg = `DTR toggle failed: ${e}`;
    }
  }

  async function toggleRTS() {
    const next = !ctrlRTS;
    try {
      await api.setRTS(next);
      ctrlRTS = next;
    } catch (e) {
      statusMsg = `RTS toggle failed: ${e}`;
    }
  }

  async function sendBreak() {
    try {
      await api.sendBreak();
      statusMsg = "Break sent";
    } catch (e) {
      statusMsg = `Break failed: ${e}`;
    }
  }

  let hexSendOpen = $state(false);
  let hexInput = $state("");
  let hexError = $state("");

  // Paste-confirm modal. Terminal.svelte awaits handlePasteConfirm()
  // when a multi-line paste arrives and pasteWarnMultiline is on.
  // We can't use window.confirm here — Wails v2's WKWebView doesn't
  // wire a UI delegate through, so the native JS confirm dialog
  // returns false immediately. Custom modal it is.
  let pasteConfirmOpen = $state(false);
  let pasteConfirmLines = $state(0);
  let pasteConfirmPreview = $state("");
  let pasteConfirmResolver: ((ok: boolean) => void) | null = null;

  function handlePasteConfirm(data: string): Promise<boolean> {
    const parts = data.split(/\r\n|\r|\n/);
    pasteConfirmLines = parts.length;
    pasteConfirmPreview = parts[0].slice(0, 80);
    pasteConfirmOpen = true;
    return new Promise<boolean>((resolve) => {
      pasteConfirmResolver = resolve;
    });
  }

  function resolvePasteConfirm(ok: boolean) {
    pasteConfirmOpen = false;
    const r = pasteConfirmResolver;
    pasteConfirmResolver = null;
    r?.(ok);
  }

  function openHexSend() {
    hexSendOpen = true;
    hexError = "";
  }

  function closeHexSend() {
    hexSendOpen = false;
    hexError = "";
  }

  // Accept "02 FF AA", "02FFAA", "0x02 0xFF", or any mix. Whitespace,
  // commas, and 0x prefixes are stripped; what's left must be an even
  // number of hex digits.
  function parseHex(input: string): Uint8Array | string {
    const cleaned = input
      .trim()
      .replace(/0x/gi, "")
      .replace(/[\s,]+/g, "");
    if (cleaned.length === 0) return "empty";
    if (cleaned.length % 2 !== 0) return "odd number of hex digits";
    if (!/^[0-9a-fA-F]+$/.test(cleaned)) return "non-hex characters";
    const bytes = new Uint8Array(cleaned.length / 2);
    for (let i = 0; i < bytes.length; i++) {
      bytes[i] = parseInt(cleaned.slice(i * 2, i * 2 + 2), 16);
    }
    return bytes;
  }

  async function submitHex() {
    const parsed = parseHex(hexInput);
    if (typeof parsed === "string") {
      hexError = parsed;
      return;
    }
    try {
      await api.sendBytes(parsed);
      statusMsg = `Sent ${parsed.length} byte${parsed.length === 1 ? "" : "s"}`;
      hexInput = "";
      hexError = "";
    } catch (e) {
      hexError = String(e);
    }
  }

  function onHexKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") closeHexSend();
    else if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void submitHex();
    }
  }

  // File transfer (XMODEM / YMODEM) state.
  type TransferState =
    | { status: "picking" }
    | { status: "sending"; filename: string; sent: number; total: number }
    | { status: "done"; filename: string }
    | { status: "error"; reason: string };

  let transferOpen = $state(false);
  let transferProtocol = $state<TransferProtocol>("ymodem");
  let transferPath = $state("");
  let transferState = $state<TransferState>({ status: "picking" });
  let offTransferProgress: (() => void) | null = null;
  let offTransferComplete: (() => void) | null = null;
  let offTransferError: (() => void) | null = null;

  function openTransfer() {
    transferOpen = true;
    transferState = { status: "picking" };
    transferPath = "";
  }

  function closeTransfer() {
    if (transferState.status === "sending") return; // block close mid-flight
    transferOpen = false;
  }

  async function pickTransferFile() {
    try {
      const path = await api.pickSendFile();
      if (path) transferPath = path;
    } catch (e) {
      transferState = { status: "error", reason: String(e) };
    }
  }

  async function startTransfer() {
    if (!transferPath) return;
    const filename = transferPath.split(/[\\/]/).pop() || transferPath;
    transferState = { status: "sending", filename, sent: 0, total: 0 };

    offTransferProgress = api.onTransferProgress((p) => {
      if (transferState.status === "sending") {
        transferState = { ...transferState, sent: p.sent, total: p.total };
      }
    });
    offTransferComplete = api.onTransferComplete((name) => {
      transferState = { status: "done", filename: name };
    });
    offTransferError = api.onTransferError((reason) => {
      transferState = { status: "error", reason };
    });

    try {
      await api.sendFile(transferProtocol, transferPath);
    } catch {
      // Error already surfaced via onTransferError.
    } finally {
      offTransferProgress?.();
      offTransferComplete?.();
      offTransferError?.();
      offTransferProgress = null;
      offTransferComplete = null;
      offTransferError = null;
    }
  }

  function cancelTransfer() {
    api.cancelTransfer().catch(() => {});
  }

  // Overflow menu holds the less-frequently-used actions (Break,
  // Hex, Send File) so the session header stays compact. DTR/RTS
  // stay visible because their pill shows live line state; moving
  // them to a menu hides that information.
  let overflowOpen = $state(false);

  function toggleOverflow() {
    overflowOpen = !overflowOpen;
  }

  function closeOverflow() {
    overflowOpen = false;
  }

  function runFromOverflow(fn: () => void) {
    overflowOpen = false;
    fn();
  }

  // Terminal zoom shortcuts — standard convention across terminals
  // (iTerm2, Windows Terminal, VS Code). Persisted to Settings.fontSize
  // so zoom sticks across sessions.
  const FONT_MIN = 8;
  const FONT_MAX = 28;
  const FONT_DEFAULT = 13;

  async function applyFontSize(size: number) {
    const clamped = Math.max(FONT_MIN, Math.min(FONT_MAX, size));
    try {
      const updated = await api.updateSettings({ ...$settings, fontSize: clamped });
      settings.set(updated);
    } catch (e) {
      statusMsg = `Font size update failed: ${e}`;
    }
  }

  // Platform detection drives the default shortcut scheme (users
  // can override each one in Settings → Keyboard Shortcuts). macOS
  // uses Cmd+* because Cmd is never a terminal control character,
  // so plain Cmd+K is safe. Linux + Windows use Ctrl+Shift+* to
  // keep plain Ctrl+letter passthroughs (Ctrl+B STX, Ctrl+K VT,
  // Ctrl+S XOFF, etc.) intact for serial devices.
  const IS_MAC =
    typeof navigator !== "undefined" && /Mac/i.test(navigator.platform);

  // Effective shortcut specs reactively derived from settings so
  // the keydown handler and button tooltips update live when the
  // user edits a binding.
  const shortcutClearSpec = $derived(
    effectiveShortcut("clear", $settings.shortcuts, IS_MAC),
  );
  const shortcutBreakSpec = $derived(
    effectiveShortcut("break", $settings.shortcuts, IS_MAC),
  );
  const shortcutSuspendSpec = $derived(
    effectiveShortcut("suspend", $settings.shortcuts, IS_MAC),
  );
  const shortcutClearLabel = $derived(formatShortcut(shortcutClearSpec, IS_MAC));
  const shortcutBreakLabel = $derived(formatShortcut(shortcutBreakSpec, IS_MAC));
  const shortcutSuspendLabel = $derived(
    formatShortcut(shortcutSuspendSpec, IS_MAC),
  );

  function handleWindowKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && overflowOpen) {
      overflowOpen = false;
      return;
    }
    // Cmd+, (Mac) / Ctrl+, (Win/Linux) toggles the Settings window
    // globally — same shortcut works from either the main window or
    // the Settings window itself, mirroring the universal macOS
    // Preferences convention. Persona red flag fix: Alex's "no
    // Cmd+, indicator" complaint from the Settings critique.
    if ((e.metaKey || e.ctrlKey) && e.key === ",") {
      e.preventDefault();
      e.stopPropagation();
      void handleToggleSettings();
      return;
    }
    // Zoom first — it hasn't moved to settings-driven config.
    const mod = e.metaKey || e.ctrlKey;
    if (mod) {
      const current = $settings.fontSize || FONT_DEFAULT;
      if (e.key === "=" || e.key === "+") {
        e.preventDefault();
        e.stopPropagation();
        statusMsg = `Font size: ${Math.min(FONT_MAX, current + 1)}`;
        void applyFontSize(current + 1);
        return;
      } else if (e.key === "-" || e.key === "_") {
        e.preventDefault();
        e.stopPropagation();
        statusMsg = `Font size: ${Math.max(FONT_MIN, current - 1)}`;
        void applyFontSize(current - 1);
        return;
      } else if (e.key === "0") {
        e.preventDefault();
        e.stopPropagation();
        statusMsg = `Font size: ${FONT_DEFAULT}`;
        void applyFontSize(FONT_DEFAULT);
        return;
      }
    }

    // Session shortcuts: match each binding against the event and
    // dispatch to the gated action. Each helper no-ops when its
    // underlying button would be disabled.
    if (matchesShortcut(e, shortcutClearSpec)) {
      shortcutClear(e);
    } else if (matchesShortcut(e, shortcutBreakSpec)) {
      shortcutBreak(e);
    } else if (matchesShortcut(e, shortcutSuspendSpec)) {
      shortcutSuspend(e);
    }
  }

  function shortcutClear(e: KeyboardEvent) {
    if (!viewingTerminal || !terminalRef) return;
    e.preventDefault();
    e.stopPropagation();
    terminalRef.clear();
  }

  function shortcutBreak(e: KeyboardEvent) {
    // Break only makes sense against an actively-connected session —
    // suspended sessions still hold the port but the user explicitly
    // stepped away, so firing Break from there would be surprising.
    if (!isConnected || suspended || isReconnecting) return;
    e.preventDefault();
    e.stopPropagation();
    void sendBreak();
  }

  function shortcutSuspend(e: KeyboardEvent) {
    // Parallels the Suspend button's enablement: live session,
    // currently looking at the terminal, not already suspended.
    if (!isConnected || suspended || !viewingTerminal) return;
    e.preventDefault();
    e.stopPropagation();
    handleSuspend();
  }

  async function handleDisconnect() {
    try {
      await api.disconnect();
      session.set({ status: "idle" });
      suspended = false;
      statusMsg = "Disconnected";
    } catch (e) {
      statusMsg = `Disconnect failed: ${e}`;
    }
  }

  function handleSuspend() {
    suspended = true;
    statusMsg = "Session kept alive in background";
  }

  async function handleResume() {
    suspended = false;
    await tick();
    terminalRef?.refit();
    terminalRef?.focus();
  }

  async function handleImportTheme() {
    try {
      const t = await importTheme();
      if (t) statusMsg = `Imported theme: ${t.name}`;
    } catch (e) {
      statusMsg = `Import failed: ${e}`;
    }
  }

  async function handleDeleteTheme(id: string) {
    try {
      await deleteTheme(id);
      statusMsg = "Theme removed";
    } catch (e) {
      statusMsg = `Delete failed: ${e}`;
    }
  }

  async function handleSetDefault(id: string) {
    try {
      await setDefaultTheme(id);
      statusMsg = "Default theme updated";
    } catch (e) {
      statusMsg = `Update failed: ${e}`;
    }
  }

  async function handleSetFontSize(size: number) {
    try {
      const updated = await api.updateSettings({ ...$settings, fontSize: size });
      settings.set(updated);
    } catch (e) {
      statusMsg = `Font update failed: ${e}`;
    }
  }

  async function handleSetScrollback(lines: number) {
    try {
      const updated = await api.updateSettings({ ...$settings, scrollbackLines: lines });
      settings.set(updated);
    } catch (e) {
      statusMsg = `Scrollback update failed: ${e}`;
    }
  }

  async function handleSetLogDir(dir: string) {
    try {
      const updated = await api.updateSettings({ ...$settings, logDir: dir });
      settings.set(updated);
      statusMsg = dir ? "Log directory updated" : "Log directory reset to default";
    } catch (e) {
      statusMsg = `Log dir update failed: ${e}`;
    }
  }

  async function handlePickLogDir() {
    try {
      const dir = await api.pickLogDirectory();
      if (dir) await handleSetLogDir(dir);
    } catch (e) {
      statusMsg = `Directory pick failed: ${e}`;
    }
  }

  async function handleSetDetectDrivers(enabled: boolean) {
    try {
      const updated = await api.updateSettings({
        ...$settings,
        disableDriverDetection: !enabled,
      });
      settings.set(updated);
    } catch (e) {
      statusMsg = `Setting update failed: ${e}`;
    }
  }

  async function handleSetCopyOnSelect(enabled: boolean) {
    try {
      const updated = await api.updateSettings({ ...$settings, copyOnSelect: enabled });
      settings.set(updated);
    } catch (e) {
      statusMsg = `Setting update failed: ${e}`;
    }
  }

  async function handleSetScreenReaderMode(enabled: boolean) {
    try {
      const updated = await api.updateSettings({ ...$settings, screenReaderMode: enabled });
      settings.set(updated);
    } catch (e) {
      statusMsg = `Setting update failed: ${e}`;
    }
  }

  async function handleSetUpdateCheckEnabled(enabled: boolean) {
    try {
      const updated = await api.updateSettings({
        ...$settings,
        disableUpdateCheck: !enabled,
      });
      settings.set(updated);
      // Hide any pending toast when checks are turned off so the UI
      // reflects the setting immediately.
      if (!enabled) availableUpdate = null;
    } catch (e) {
      statusMsg = `Setting update failed: ${e}`;
    }
  }

  async function handleSetIncludePrereleaseUpdates(enabled: boolean) {
    try {
      const updated = await api.updateSettings({
        ...$settings,
        includePrereleaseUpdates: enabled,
      });
      settings.set(updated);
      // Re-run the check right away so flipping the toggle re-evaluates
      // against the new policy without waiting for the next launch.
      if (!updated.disableUpdateCheck) {
        availableUpdate = null;
        void runUpdateCheck();
      }
    } catch (e) {
      statusMsg = `Setting update failed: ${e}`;
    }
  }

  async function handleSetEnabledHighlightPresets(ids: string[]) {
    try {
      const updated = await api.updateSettings({
        ...$settings,
        enabledHighlightPresets: ids,
      });
      settings.set(updated);
      // The $effect on $settings.enabledHighlightPresets recompiles
      // the active rule set automatically, so no explicit
      // applyEnabledHighlightPresets call here.
    } catch (e) {
      statusMsg = `Setting update failed: ${e}`;
    }
  }

  async function handleImportHighlightPack() {
    try {
      const pack = await api.importHighlightPack();
      await loadHighlightPacks();
      // Auto-enable on import — fresh packs the user just chose
      // should participate in the active rule set without a second
      // click. They can untick it in Settings if they don't want it.
      const enabled = new Set($settings.enabledHighlightPresets ?? []);
      enabled.add(pack.id);
      const next = Array.from(enabled);
      try {
        const updated = await api.updateSettings({
          ...$settings,
          enabledHighlightPresets: next,
        });
        settings.set(updated);
      } catch (e) {
        statusMsg = `Settings update after import failed: ${e}`;
      }
      statusMsg = `Imported highlight pack: ${pack.name}`;
    } catch (e) {
      const msg = String(e);
      // Tauri's dialog.blocking_pick_file returns "cancelled" if the
      // user dismisses the picker — not worth a status-bar error.
      if (!msg.includes("cancelled")) {
        statusMsg = `Import failed: ${msg}`;
      }
    }
  }

  async function handleDeleteHighlightPack(id: string) {
    try {
      await api.deleteHighlightPack(id);
      await loadHighlightPacks();
      // Pull the deleted id out of the enabled list so Settings
      // doesn't keep a dangling reference.
      const enabled = ($settings.enabledHighlightPresets ?? []).filter(
        (p) => p !== id,
      );
      try {
        const updated = await api.updateSettings({
          ...$settings,
          enabledHighlightPresets: enabled,
        });
        settings.set(updated);
      } catch (e) {
        statusMsg = `Settings cleanup after delete failed: ${e}`;
      }
      statusMsg = "Highlight pack removed";
    } catch (e) {
      statusMsg = `Delete failed: ${e}`;
    }
  }

  async function handlePickConfigDir() {
    try {
      const dir = await api.pickConfigDirectory();
      if (!dir) return;
      await api.setConfigDirectory(dir);
      configDir = dir;
      statusMsg = "Config directory updated — restart Baudrun to apply";
    } catch (e) {
      statusMsg = `Config directory change failed: ${e}`;
    }
  }

  async function handleResetConfigDir() {
    try {
      await api.setConfigDirectory("");
      configDir = defaultConfigDir;
      statusMsg = "Config directory reset — restart Baudrun to apply";
    } catch (e) {
      statusMsg = `Reset failed: ${e}`;
    }
  }

  async function handleSetSkin(id: string) {
    try {
      const updated = await api.updateSettings({ ...$settings, skinId: id });
      settings.set(updated);
      activeSkinID.set(id);
      statusMsg = `Skin: ${$skins.find((s) => s.id === id)?.name ?? id}`;
    } catch (e) {
      statusMsg = `Skin change failed: ${e}`;
    }
  }

  async function handleImportSkin() {
    try {
      const s = await importSkin();
      if (s) statusMsg = `Imported skin: ${s.name}`;
    } catch (e) {
      statusMsg = `Import failed: ${e}`;
    }
  }

  async function handleDeleteSkin(id: string) {
    try {
      await deleteSkin(id);
      if ($activeSkinID === id) await handleSetSkin("baudrun");
      statusMsg = "Skin removed";
    } catch (e) {
      statusMsg = `Delete failed: ${e}`;
    }
  }


  async function handleSetAppearance(mode: Appearance) {
    try {
      const updated = await api.updateSettings({ ...$settings, appearance: mode });
      settings.set(updated);
      appearance.set(mode);
    } catch (e) {
      statusMsg = `Appearance change failed: ${e}`;
    }
  }

  async function handleSetShortcuts(shortcuts: Record<string, string>) {
    try {
      const updated = await api.updateSettings({ ...$settings, shortcuts });
      settings.set(updated);
    } catch (e) {
      statusMsg = `Shortcut update failed: ${e}`;
    }
  }

</script>

<svelte:window
  onclick={() => { if (overflowOpen) overflowOpen = false; }}
  onkeydown={handleWindowKeydown}
/>

{#if isSettingsWindow}
  <!-- Settings window mode: this Tauri window's only content is the
       Settings component. The main-window sidebar / terminal /
       transfer modals etc. don't apply. The Settings.svelte
       component already includes its own data-tauri-drag-region
       titlebar, so no extra chrome is needed here.

       The .settings-window-shell wrapper exists so this window has
       something painting `--shell-bg` (the same gradient `.shell`
       paints in the main window). Without it, the body's
       `--bg-window` (which most skins leave at rgba(0) so macOS
       vibrancy can show through) leaves the webview transparent —
       which on Windows and Linux paints black, and on macOS-light
       skins bleeds the dark vibrancy through translucent panels.
       Falling back to `--bg-main` covers skins like windows-11 that
       don't define `--shell-bg`. -->
  <div class="settings-window-shell">
  <Settings
    themes={$themes}
    skins={$skins}
    settings={$settings}
    {defaultLogDir}
    {configDir}
    {defaultConfigDir}
    onSetDefault={handleSetDefault}
    onImport={handleImportTheme}
    onDelete={handleDeleteTheme}
    onSetFontSize={handleSetFontSize}
    onSetScrollback={handleSetScrollback}
    onSetLogDir={handleSetLogDir}
    onPickLogDir={handlePickLogDir}
    onSetDetectDrivers={handleSetDetectDrivers}
    onSetCopyOnSelect={handleSetCopyOnSelect}
    onSetScreenReaderMode={handleSetScreenReaderMode}
    onSetUpdateCheckEnabled={handleSetUpdateCheckEnabled}
    onSetIncludePrereleaseUpdates={handleSetIncludePrereleaseUpdates}
    onPickConfigDir={handlePickConfigDir}
    onResetConfigDir={handleResetConfigDir}
    onSetSkin={handleSetSkin}
    onImportSkin={handleImportSkin}
    onDeleteSkin={handleDeleteSkin}
    onSetAppearance={handleSetAppearance}
    onSetShortcuts={handleSetShortcuts}
    highlightPacks={$highlightPacks}
    onSetEnabledHighlightPresets={handleSetEnabledHighlightPresets}
    onImportHighlightPack={handleImportHighlightPack}
    onDeleteHighlightPack={handleDeleteHighlightPack}
  />
  </div>
{:else}
<div class="shell">
  <Sidebar
    profiles={visibleProfiles}
    selectedID={$selectedProfileID}
    activeID={activeProfileID}
    {settingsOpen}
    onSelect={handleSelect}
    onCreate={handleCreate}
    onSettings={handleToggleSettings}
    onOpenInNewWindow={handleOpenInNewWindow}
  />

  <main class="main">
    {#if !viewingTerminal}
      {#if !currentProfile}
        <div class="titlebar" data-tauri-drag-region></div>
        <div class="empty-main">
          <div class="empty-inner">
            <div class="brand">Baudrun</div>
            <p>A serial terminal for network devices.</p>
            <button class="primary" onclick={handleCreate}>
              Create a Profile
            </button>
          </div>
        </div>
      {:else}
        <ProfileForm
          profile={currentProfile}
          {isNew}
          {isConnected}
          {isConnecting}
          {isReconnecting}
          {suspended}
          themes={$themes}
          defaultThemeID={$settings.defaultThemeId}
          detectDrivers={!$settings.disableDriverDetection}
          highlightPacks={$highlightPacks}
          globalEnabledHighlightPresets={$settings.enabledHighlightPresets}
          onSave={handleSave}
          onDelete={handleDelete}
          onConnect={handleConnect}
          onDisconnect={handleDisconnect}
          onResume={handleResume}
        />
      {/if}
    {/if}

    {#if hasSession}
      <div class="terminal-layer" class:hidden={!viewingTerminal}>
        <div class="titlebar" data-tauri-drag-region></div>
        <header class="session-header">
          <div class="session-meta">
            <span class="dot" class:reconnecting={isReconnecting}></span>
            <div class="session-text">
              <strong>{activeProfile?.name ?? currentProfile?.name ?? ""}</strong>
              <span class="session-sub">
                {formatPortName(activeProfile?.portName ?? currentProfile?.portName ?? "")} ·
                {activeProfile?.baudRate ?? currentProfile?.baudRate ?? ""}
                /{activeProfile?.dataBits ?? currentProfile?.dataBits ?? ""}
                {((activeProfile?.parity ?? currentProfile?.parity) || " ")[0].toUpperCase()}
                {activeProfile?.stopBits ?? currentProfile?.stopBits ?? ""}
                {#if isReconnecting} · reconnecting…{/if}
              </span>
            </div>
          </div>
          <div class="session-actions">
            <div class="overflow-wrap">
              <button
                class="overflow-btn"
                class:open={overflowOpen}
                onclick={(e) => { e.stopPropagation(); toggleOverflow(); }}
                disabled={isReconnecting}
                title="More actions"
                aria-label="More actions"
                aria-haspopup="menu"
                aria-expanded={overflowOpen}
              >⋯</button>
              {#if overflowOpen}
                <div
                  class="overflow-menu"
                  role="menu"
                  tabindex="-1"
                  onclick={(e) => e.stopPropagation()}
                  onkeydown={(e) => { if (e.key === "Escape") overflowOpen = false; }}
                >
                  <button
                    role="menuitem"
                    onclick={() => runFromOverflow(sendBreak)}
                    title="~300ms serial break (Cisco ROMMON, Juniper diag, boot-loader interrupt). Shortcut: {shortcutBreakLabel}"
                    aria-keyshortcuts={shortcutBreakSpec}
                  >Send Break</button>
                  <button
                    role="menuitem"
                    onclick={() => runFromOverflow(openHexSend)}
                    title="Send raw bytes as hex (Modbus, firmware bootloaders, binary protocols)"
                  >Send Hex…</button>
                  <button
                    role="menuitem"
                    onclick={() => runFromOverflow(openTransfer)}
                    title="Send a file via XMODEM or YMODEM (firmware uploads, embedded bootloaders)"
                  >Send File…</button>
                </div>
              {/if}
            </div>
            <button
              class="line-btn"
              class:asserted={ctrlDTR}
              onclick={toggleDTR}
              disabled={isReconnecting}
              title="Toggle DTR line ({ctrlDTR ? 'asserted' : 'deasserted'})"
            >
              <span class="line-dot"></span>DTR
            </button>
            <button
              class="line-btn"
              class:asserted={ctrlRTS}
              onclick={toggleRTS}
              disabled={isReconnecting}
              title="Toggle RTS line ({ctrlRTS ? 'asserted' : 'deasserted'})"
            >
              <span class="line-dot"></span>RTS
            </button>
            <button
              onclick={() => terminalRef?.clear()}
              title="Clear the visible scrollback. Shortcut: {shortcutClearLabel}"
              aria-keyshortcuts={shortcutClearSpec}
            >
              Clear
            </button>
            <button
              onclick={handleSuspend}
              title="Keep session alive; return to profile. Shortcut: {shortcutSuspendLabel}"
              aria-keyshortcuts={shortcutSuspendSpec}
            >
              Suspend
            </button>
            <button
              class="primary"
              onclick={handleDisconnect}
              title="Close the serial port and end this session. Use Suspend to keep the port open instead."
            >
              Disconnect
            </button>
          </div>
        </header>
        <Terminal
          bind:this={terminalRef}
          lineEnding={termLineEnding}
          localEcho={termLocalEcho}
          theme={effectiveTheme}
          fontSize={termFontSize}
          scrollback={termScrollback}
          highlight={termHighlight}
          hexView={termHexView}
          timestamps={termTimestamps}
          backspaceKey={termBackspaceKey}
          copyOnSelect={termCopyOnSelect}
          screenReaderMode={termScreenReaderMode}
          pasteWarnMultiline={termPasteWarnMultiline}
          pasteSlow={termPasteSlow}
          pasteCharDelayMs={termPasteCharDelayMs}
          onStatus={(m) => (statusMsg = m)}
          onPasteConfirm={handlePasteConfirm}
        />
      </div>
    {/if}

    <footer class="status">
      {#if $portScanning}
        <span class="scanning-pill" title="Enumerating serial ports and USB adapters">
          <span class="scanning-dot"></span>
          Scanning for COM ports…
        </span>
      {/if}
      <span class="status-text">{statusMsg || " "}</span>
      <div class="status-right">
        {#if pendingDelete}
          <button
            class="undo-btn"
            onclick={undoDelete}
            title="Restore {pendingDelete.profile.name}"
          >
            Undo <span class="undo-countdown">({pendingDeleteSecondsLeft}s)</span>
          </button>
        {/if}
        {#if availableUpdate}
          <div
            class="update-toast"
            class:update-toast-prerelease={availableUpdate.prerelease}
            role="status"
            aria-live="polite"
          >
            {#if updateInstalling}
              <span class="update-installing">
                <span class="update-dot pulsing"></span>
                {#if updateProgress && updateProgress.total > 0}
                  Installing v{availableUpdate.version}…
                  <span class="update-progress-num">
                    {Math.round(
                      (updateProgress.downloaded / updateProgress.total) * 100,
                    )}%
                  </span>
                {:else}
                  Installing v{availableUpdate.version}…
                {/if}
              </span>
            {:else}
              <button
                class="update-link"
                onclick={installUpdate}
                title={availableUpdate.prerelease
                  ? "Open release notes on GitHub (pre-releases install manually)"
                  : "Download, verify signature, install, and relaunch"}
              >
                <span class="update-dot"></span>
                {availableUpdate.prerelease ? "Pre-release available:" : "Update available:"}
                <strong>v{availableUpdate.version}</strong>
                {#if availableUpdate.prerelease}
                  <span class="update-prerelease-tag">pre-release</span>
                {:else}
                  <span class="update-action-hint">Install</span>
                {/if}
              </button>
              <button
                class="update-link secondary"
                onclick={openUpdateUrl}
                title="Open release notes on GitHub"
                aria-label="Open release notes for v{availableUpdate.version}"
              >Notes</button>
              <button
                class="update-dismiss"
                onclick={dismissUpdate}
                title="Dismiss — won't show again for this version"
                aria-label="Dismiss update notification"
              >×</button>
            {/if}
          </div>
        {/if}
      </div>
    </footer>
  </main>
</div>

{#if transferOpen}
  <div
    class="modal-backdrop"
    onclick={closeTransfer}
    onkeydown={(e) => { if (e.key === "Escape") closeTransfer(); }}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div class="transfer-modal" onclick={(e) => e.stopPropagation()} role="presentation">
      <header class="hex-header">
        <strong>Send file</strong>
        <button
          onclick={closeTransfer}
          disabled={transferState.status === "sending"}
          aria-label="Close"
        >×</button>
      </header>

      {#if transferState.status === "picking"}
        <div class="field">
          <label for="transfer-protocol">Protocol</label>
          <Select
            id="transfer-protocol"
            bind:value={transferProtocol as any}
            options={[
              { value: "ymodem", label: "YMODEM — 1024-byte blocks with filename + size" },
              { value: "xmodem-1k", label: "XMODEM-1K — 1024-byte blocks, CRC-16" },
              { value: "xmodem-crc", label: "XMODEM-CRC — 128-byte blocks, CRC-16" },
              { value: "xmodem", label: "XMODEM — 128-byte blocks, 8-bit checksum (legacy)" },
            ]}
          />
        </div>

        <div class="field">
          <label for="transfer-path">File</label>
          <div class="file-row">
            <input
              id="transfer-path"
              type="text"
              readonly
              value={transferPath || ""}
              placeholder="No file selected"
            />
            <button onclick={pickTransferFile}>Choose…</button>
          </div>
        </div>

        <p class="hex-hint">
          Start the receiver on the target device first (<code>rx</code>,
          <code>loady</code>, bootloader "Receive File" menu, etc.) before
          clicking Send. The transfer waits up to 60 s for the receiver's
          handshake before giving up.
        </p>

        <div class="hex-actions">
          <button onclick={closeTransfer}>Cancel</button>
          <button class="primary" onclick={startTransfer} disabled={!transferPath}>
            Send
          </button>
        </div>
      {:else if transferState.status === "sending"}
        <div class="transfer-status">
          <div class="transfer-filename">{transferState.filename}</div>
          <div class="progress-track">
            <div
              class="progress-fill"
              style="width: {transferState.total > 0
                ? ((transferState.sent / transferState.total) * 100).toFixed(1)
                : 0}%"
            ></div>
          </div>
          <div class="transfer-bytes">
            {transferState.sent.toLocaleString()} /
            {transferState.total.toLocaleString()} bytes
            {#if transferState.total > 0}
              ({((transferState.sent / transferState.total) * 100).toFixed(0)}%)
            {/if}
          </div>
        </div>
        <div class="hex-actions">
          <button onclick={cancelTransfer}>Cancel transfer</button>
        </div>
      {:else if transferState.status === "done"}
        <p class="transfer-done">✓ Sent {transferState.filename}</p>
        <div class="hex-actions">
          <button class="primary" onclick={closeTransfer}>Close</button>
        </div>
      {:else if transferState.status === "error"}
        <div class="hex-error">{transferState.reason}</div>
        <div class="hex-actions">
          <button onclick={() => (transferState = { status: "picking" })}>Try again</button>
          <button class="primary" onclick={closeTransfer}>Close</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if pasteConfirmOpen}
  <div
    class="modal-backdrop"
    onclick={() => resolvePasteConfirm(false)}
    onkeydown={(e) => {
      if (e.key === "Escape") resolvePasteConfirm(false);
      else if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); resolvePasteConfirm(true); }
    }}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div
      class="hex-modal"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
      role="presentation"
    >
      <header class="hex-header">
        <strong>Confirm paste</strong>
        <button onclick={() => resolvePasteConfirm(false)} aria-label="Close">×</button>
      </header>
      <p class="hex-hint">
        Send <strong>{pasteConfirmLines}</strong>
        {pasteConfirmLines === 1 ? "line" : "lines"} to the session?
        Multi-line pastes can execute partial commands on network gear
        before the device has a chance to echo them back.
      </p>
      <div class="paste-preview">
        <div class="paste-preview-label">First line</div>
        <code>{pasteConfirmPreview || "(empty)"}</code>
      </div>
      <div class="hex-actions">
        <button onclick={() => resolvePasteConfirm(false)}>Cancel</button>
        <button class="primary" onclick={() => resolvePasteConfirm(true)}>Send</button>
      </div>
    </div>
  </div>
{/if}

{#if hexSendOpen}
  <div
    class="modal-backdrop"
    onclick={closeHexSend}
    onkeydown={onHexKeydown}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div
      class="hex-modal"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => { e.stopPropagation(); onHexKeydown(e); }}
      role="presentation"
    >
      <header class="hex-header">
        <strong>Send hex bytes</strong>
        <button onclick={closeHexSend} aria-label="Close">×</button>
      </header>
      <p class="hex-hint">
        Space-separated, compact, or 0x-prefixed — all equivalent:
        <code>02 FF AA 55</code>, <code>02FFAA55</code>, <code>0x02 0xFF 0xAA 0x55</code>.
      </p>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        type="text"
        class="hex-input"
        bind:value={hexInput}
        placeholder="02 FF AA 55"
        autofocus
      />
      {#if hexError}
        <div class="hex-error">Invalid: {hexError}</div>
      {/if}
      <div class="hex-actions">
        <button onclick={closeHexSend}>Cancel</button>
        <button class="primary" onclick={submitHex}>Send</button>
      </div>
    </div>
  </div>
{/if}
{/if}

<style>
  .shell {
    display: flex;
    flex: 1;
    min-height: 0;
    height: 100%;
    padding: var(--shell-padding);
    gap: var(--shell-gap);
    background: var(--shell-bg, transparent);
    /* Dark-mode skins leave --shell-bg unset so the window's vibrancy
       shows through the gaps around floating panels. Light skins with
       translucent surfaces must override it — the NSVisualEffectView is
       pinned dark, so a transparent shell would otherwise frame each
       panel with dark vibrancy. */
  }

  /* Settings window: paints the skin's --shell-bg behind the Settings
     component so the window has a visible substrate on every platform.
     The fallback chain `--shell-bg → --bg-main → #1d1d1d` is the
     production-state contract: skins that don't define --shell-bg
     (windows-11, gnome, high-contrast, baudrun-light, etc.) all
     define --bg-main with a solid color matching their surface, so
     the wrapper picks up the right tone for any skin. The hard-coded
     `#1d1d1d` is the safety net for skins that define neither.

     The `#app` pre-paint in index.html sits BEHIND this wrapper and
     paints solid `#1d1d1d` (or `#f2f2f7` per prefers-color-scheme)
     before any skin is applied — so even though :root's default
     `--bg-main` is translucent (rgba(...,0.55) for macOS vibrancy),
     the layers below it are solid during boot. No black flash. */
  .settings-window-shell {
    flex: 1;
    min-height: 0;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--shell-bg, var(--bg-main, #1d1d1d));
  }

  .main {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--bg-main);
    border-radius: var(--panel-radius);
    box-shadow: var(--panel-shadow);
    overflow: hidden;
  }

  .titlebar {
    height: var(--titlebar-height);
    flex-shrink: 0;
  }

  .empty-main {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .empty-inner {
    text-align: center;
    color: var(--fg-secondary);
  }

  .brand {
    font-size: 26px;
    font-weight: 600;
    color: var(--fg-primary);
    margin-bottom: 6px;
    letter-spacing: -0.02em;
  }

  .empty-inner p {
    margin: 0 0 20px 0;
  }

  .session-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 22px 14px 22px;
    border-bottom: 1px solid var(--border-subtle);
    gap: 16px;
  }

  .session-meta {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }

  .session-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .session-text strong {
    font-size: 14px;
    font-weight: 600;
  }

  .session-sub {
    font-size: 11px;
    color: var(--fg-tertiary);
    font-family: var(--font-mono);
    margin-top: 2px;
  }

  .session-actions {
    display: flex;
    gap: 8px;
  }

  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--success);
    box-shadow: 0 0 8px var(--success);
  }

  .dot.reconnecting {
    background: var(--warn);
    box-shadow: 0 0 8px var(--warn);
    animation: reconnect-pulse 1s ease-in-out infinite;
  }

  @keyframes reconnect-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.35; }
  }

  @media (prefers-reduced-motion: reduce) {
    .dot.reconnecting { animation: none; }
  }

  .status {
    padding: 5px 14px;
    height: 30px;
    font-size: 11px;
    color: var(--fg-tertiary);
    border-top: 1px solid var(--border-subtle);
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
    background: rgba(0, 0, 0, 0.2);
  }

  .scanning-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--fg-secondary);
    font-family: var(--font-mono);
    letter-spacing: 0.02em;
  }

  .scanning-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 6px var(--accent);
    animation: scanning-pulse 1.1s ease-in-out infinite;
  }

  @keyframes scanning-pulse {
    0%, 100% { opacity: 0.35; }
    50% { opacity: 1; }
  }

  @media (prefers-reduced-motion: reduce) {
    .scanning-dot { animation: none; opacity: 1; }
  }

  .status-right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .undo-btn {
    padding: 2px 9px;
    font-size: 11px;
    font-weight: 500;
    color: var(--accent);
    background: transparent;
    border: 1px solid var(--accent);
    border-radius: var(--radius-sm);
    line-height: 1.3;
  }

  .undo-btn:hover {
    background: var(--bg-active);
  }

  .undo-countdown {
    opacity: 0.75;
    font-variant-numeric: tabular-nums;
    margin-left: 2px;
  }

  /* Toast stays skin-neutral: transparent over the panel background,
     solid --success / --warn border carries the status signal. Text
     uses --fg-primary so it reads on dark AND light skins — only the
     version number + dot + pre-release pill pick up the accent color,
     matching how --undo-btn / --line-btn mix colored borders with
     neutral text across the codebase. */
  .update-toast {
    display: inline-flex;
    align-items: center;
    gap: 0;
    padding: 0;
    background: transparent;
    border: 1px solid var(--success);
    border-radius: var(--radius-sm);
    overflow: hidden;
    font-size: 11px;
    line-height: 1.3;
  }

  .update-toast.update-toast-prerelease {
    border-color: var(--warn);
  }

  .update-link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
    background: transparent;
    border: none;
    color: var(--fg-primary);
    font-family: inherit;
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    line-height: 1.3;
  }

  .update-link:hover {
    background: var(--bg-hover);
  }

  /* Version inherits --fg-primary from .update-link so it reads on
     every skin — emphasis comes from font-weight + letter-spacing
     alone. The colored dot, border, and PRE-RELEASE pill carry the
     visual urgency without making the text itself colored. */
  .update-link strong {
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  .update-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--success);
    box-shadow: 0 0 5px var(--success);
  }

  .update-toast.update-toast-prerelease .update-dot {
    background: var(--warn);
    box-shadow: 0 0 5px var(--warn);
  }

  /* Pill inverts against its own background so contrast is guaranteed
     regardless of skin brightness (light skins have dark --bg-main,
     and vice-versa — Baudrun's skin contract is that bg + fg pair up
     readably). */
  .update-prerelease-tag {
    text-transform: uppercase;
    letter-spacing: 0.06em;
    font-size: 9px;
    padding: 1px 5px;
    background: var(--warn);
    color: var(--bg-main);
    border-radius: 3px;
    margin-left: 2px;
    font-weight: 600;
  }

  .update-dismiss {
    padding: 3px 8px;
    background: transparent;
    border: none;
    border-left: 1px solid var(--success);
    color: var(--fg-tertiary);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
  }

  .update-toast.update-toast-prerelease .update-dismiss {
    border-left-color: var(--warn);
  }

  .update-dismiss:hover {
    background: var(--bg-hover);
    color: var(--fg-primary);
  }

  /* Right-side "Notes" button is the manual fallback to release notes
     even when auto-install is the primary action — handy for users
     who want to read what changed before installing. */
  .update-link.secondary {
    border-left: 1px solid var(--success);
    color: var(--fg-secondary);
    padding: 3px 10px;
    font-weight: 500;
  }

  .update-toast.update-toast-prerelease .update-link.secondary {
    border-left-color: var(--warn);
  }

  /* Inline "Install" hint pill on stable updates — invertes against
     --success so the action affordance reads at a glance without
     needing a separate button. Same trick as the PRE-RELEASE pill. */
  .update-action-hint {
    text-transform: uppercase;
    letter-spacing: 0.06em;
    font-size: 9px;
    padding: 1px 5px;
    background: var(--success);
    color: var(--bg-main);
    border-radius: 3px;
    margin-left: 2px;
    font-weight: 600;
  }

  .update-installing {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 12px;
    color: var(--fg-primary);
    font-size: 11px;
    font-weight: 500;
    line-height: 1.3;
  }

  .update-progress-num {
    color: var(--success);
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }

  .update-toast.update-toast-prerelease .update-progress-num {
    color: var(--warn);
  }

  .update-dot.pulsing {
    animation: update-pulse 1s ease-in-out infinite;
  }

  @keyframes update-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.35; }
  }

  @media (prefers-reduced-motion: reduce) {
    .update-dot.pulsing { animation: none; }
  }

  .terminal-layer {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .terminal-layer.hidden {
    display: none;
  }

  .line-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.04em;
  }

  .line-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    border: 1px solid var(--fg-tertiary);
    background: transparent;
  }

  .line-btn.asserted .line-dot {
    background: var(--success);
    border-color: var(--success);
    box-shadow: 0 0 5px rgba(50, 215, 75, 0.6);
  }

  .overflow-wrap {
    position: relative;
    display: inline-flex;
  }

  .overflow-btn {
    font-size: 16px;
    line-height: 1;
    padding: 5px 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
  }

  .overflow-btn.open {
    background: var(--bg-active);
  }

  .overflow-menu {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    min-width: 180px;
    background: var(--option-bg, var(--bg-panel));
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-floating, 0 10px 30px rgba(0, 0, 0, 0.35));
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    z-index: 200;
  }

  .overflow-menu button {
    text-align: left;
    padding: 7px 10px;
    border-radius: var(--radius-sm);
    background: transparent;
    border: none;
    color: var(--option-fg, var(--fg-primary));
    font-size: 13px;
  }

  .overflow-menu button:hover {
    background: var(--bg-hover);
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 10000;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(4px);
    -webkit-backdrop-filter: blur(4px);
    padding: 24px;
  }

  .hex-modal {
    background: var(--bg-main);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    width: 100%;
    max-width: 520px;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .hex-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .hex-header strong {
    font-size: 15px;
    font-weight: 600;
  }

  .hex-header button {
    font-size: 18px;
    line-height: 1;
    padding: 2px 10px;
  }

  .hex-hint {
    margin: 0;
    font-size: 12px;
    color: var(--fg-secondary);
    line-height: 1.5;
  }

  .hex-hint code {
    font-family: var(--font-mono);
    font-size: 11px;
    background: var(--bg-input);
    padding: 1px 5px;
    border-radius: 3px;
  }

  .hex-input {
    width: 100%;
    font-family: var(--font-mono);
    font-size: 14px;
    padding: 8px 10px;
  }

  .hex-error {
    padding: 8px 12px;
    background: rgba(255, 69, 58, 0.12);
    color: var(--danger);
    border-radius: var(--radius-sm);
    font-size: 12px;
  }

  .hex-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .paste-preview {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 12px;
    background: var(--bg-input);
    border-radius: var(--radius-sm);
  }

  .paste-preview-label {
    font-size: var(--font-size-label);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-letter-spacing);
    font-weight: var(--label-weight);
    color: var(--fg-tertiary);
  }

  .paste-preview code {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--fg-primary);
    word-break: break-all;
  }

  .transfer-modal {
    background: var(--bg-main);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    width: 100%;
    max-width: 560px;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .transfer-modal .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .transfer-modal .field > label {
    font-size: var(--font-size-label);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-letter-spacing);
    font-weight: var(--label-weight);
    color: var(--fg-secondary);
  }

  .file-row {
    display: flex;
    gap: 8px;
  }

  .file-row input {
    flex: 1;
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .transfer-status {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .transfer-filename {
    font-family: var(--font-mono);
    font-size: 13px;
    color: var(--fg-primary);
  }

  .progress-track {
    height: 8px;
    background: var(--bg-input);
    border-radius: 4px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.1s linear;
  }

  .transfer-bytes {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--fg-tertiary);
  }

  .transfer-done {
    font-size: 14px;
    color: var(--success);
    margin: 0;
  }
</style>
