<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import Sidebar from "./lib/Sidebar.svelte";
  import ProfileForm from "./lib/ProfileForm.svelte";
  import Terminal from "./lib/Terminal.svelte";
  import Settings from "./lib/Settings.svelte";
  import { api, type Profile, type Theme } from "./lib/api";
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
  import { session } from "./stores/session";

  let draft: Profile | null = null;
  let terminalRef: Terminal | null = null;
  let statusMsg = "";
  let offDisconnect: (() => void) | null = null;
  let settingsOpen = false;
  let suspended = false;

  $: selectedExisting = $profiles.find((p) => p.id === $selectedProfileID) ?? null;
  $: currentProfile = draft ?? selectedExisting;
  $: isNew = !!draft;
  $: activeProfileID =
    $session.status === "connected" || $session.status === "connecting"
      ? $session.profileID
      : "";
  $: isConnected =
    $session.status === "connected" &&
    currentProfile?.id === $session.profileID;
  $: isConnecting =
    $session.status === "connecting" &&
    currentProfile?.id === $session.profileID;
  $: viewingTerminal = isConnected && !suspended;
  $: hasSession =
    $session.status === "connected" || $session.status === "connecting";
  $: activeProfile = $profiles.find((p) => p.id === activeProfileID) ?? null;
  $: termLineEnding = (activeProfile?.lineEnding ?? currentProfile?.lineEnding ?? "crlf") as
    | "cr"
    | "lf"
    | "crlf";
  $: termLocalEcho = activeProfile?.localEcho ?? currentProfile?.localEcho ?? false;
  $: termHighlight = activeProfile?.highlight ?? currentProfile?.highlight ?? true;

  $: effectiveThemeID =
    (activeProfile?.themeId || currentProfile?.themeId) ||
    $settings.defaultThemeId ||
    "seriesly";
  $: effectiveTheme = resolveTheme(effectiveThemeID, $themes);
  $: termFontSize = $settings.fontSize || 13;

  function resolveTheme(id: string, all: Theme[]): Theme | undefined {
    return (
      all.find((t) => t.id === id) ??
      all.find((t) => t.id === "seriesly") ??
      all[0]
    );
  }

  onMount(async () => {
    await Promise.all([loadProfiles(), loadThemes(), loadSettings()]);

    offDisconnect = api.onDisconnect((reason) => {
      session.set({ status: "idle" });
      statusMsg = reason ? `Disconnected: ${reason}` : "Disconnected";
    });

    try {
      const activeID = await api.activeProfileID();
      if (activeID) session.set({ status: "connected", profileID: activeID });
    } catch {}
  });

  onDestroy(() => {
    offDisconnect?.();
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

  async function handleToggleSettings() {
    if (!settingsOpen && viewingTerminal) {
      await maybeAutoDisconnect();
    }
    settingsOpen = !settingsOpen;
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
    try {
      if (isConnected) await api.disconnect();
      await deleteProfile(id);
      draft = null;
      statusMsg = "Deleted";
    } catch (e) {
      statusMsg = `Delete failed: ${e}`;
    }
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
      statusMsg = `Connected to ${currentProfile.portName} @ ${currentProfile.baudRate}`;
      await tick();
      terminalRef?.focus();
    } catch (e) {
      session.set({ status: "idle" });
      statusMsg = `Connect failed: ${e}`;
    }
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
</script>

<div class="shell">
  <Sidebar
    profiles={$profiles}
    selectedID={$selectedProfileID}
    activeID={activeProfileID}
    {settingsOpen}
    on:select={(e) => handleSelect(e.detail)}
    on:create={handleCreate}
    on:settings={handleToggleSettings}
  />

  <main class="main">
    {#if !viewingTerminal}
      {#if settingsOpen}
        <Settings
          themes={$themes}
          settings={$settings}
          on:setDefault={(e) => handleSetDefault(e.detail)}
          on:import={handleImportTheme}
          on:delete={(e) => handleDeleteTheme(e.detail)}
          on:setFontSize={(e) => handleSetFontSize(e.detail)}
        />
      {:else if !currentProfile}
        <div class="titlebar" style="--wails-draggable: drag;"></div>
        <div class="empty-main">
          <div class="empty-inner">
            <div class="brand">Seriesly</div>
            <p>A serial terminal for network devices.</p>
            <button class="primary" on:click={handleCreate}>
              Create a Profile
            </button>
          </div>
        </div>
      {:else}
        <ProfileForm
          profile={currentProfile}
          {isNew}
          canConnect={!!currentProfile.id && !!currentProfile.portName}
          {isConnected}
          {isConnecting}
          themes={$themes}
          defaultThemeID={$settings.defaultThemeId}
          on:save={(e) => handleSave(e.detail)}
          on:delete={(e) => handleDelete(e.detail)}
          on:connect={handleConnect}
          on:disconnect={handleDisconnect}
          on:resume={handleResume}
        />
      {/if}
    {/if}

    {#if hasSession}
      <div class="terminal-layer" class:hidden={!viewingTerminal}>
        <div class="titlebar" style="--wails-draggable: drag;"></div>
        <header class="session-header">
          <div class="session-meta">
            <span class="dot"></span>
            <div class="session-text">
              <strong>{activeProfile?.name ?? currentProfile?.name ?? ""}</strong>
              <span class="session-sub">
                {activeProfile?.portName ?? currentProfile?.portName ?? ""} ·
                {activeProfile?.baudRate ?? currentProfile?.baudRate ?? ""}
                /{activeProfile?.dataBits ?? currentProfile?.dataBits ?? ""}
                {((activeProfile?.parity ?? currentProfile?.parity) || " ")[0].toUpperCase()}
                {activeProfile?.stopBits ?? currentProfile?.stopBits ?? ""}
              </span>
            </div>
          </div>
          <div class="session-actions">
            <button on:click={() => terminalRef?.clear()}>Clear</button>
            <button on:click={handleSuspend} title="Keep session alive; return to profile">
              Suspend
            </button>
            <button class="primary" on:click={handleDisconnect}>Disconnect</button>
          </div>
        </header>
        <Terminal
          bind:this={terminalRef}
          lineEnding={termLineEnding}
          localEcho={termLocalEcho}
          theme={effectiveTheme}
          fontSize={termFontSize}
          highlight={termHighlight}
          onStatus={(m) => (statusMsg = m)}
        />
      </div>
    {/if}

    <footer class="status">
      <span class="status-text">{statusMsg || " "}</span>
    </footer>
  </main>
</div>

<style>
  .shell {
    display: flex;
    flex: 1;
    min-height: 0;
    height: 100%;
    background: var(--bg-main);
  }

  .main {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--bg-main);
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

  .status {
    padding: 4px 16px;
    height: 22px;
    font-size: 11px;
    color: var(--fg-tertiary);
    border-top: 1px solid var(--border-subtle);
    display: flex;
    align-items: center;
    flex-shrink: 0;
    background: rgba(0, 0, 0, 0.2);
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
</style>
