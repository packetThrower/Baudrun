<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import Sidebar from "./lib/Sidebar.svelte";
  import ProfileForm from "./lib/ProfileForm.svelte";
  import Terminal from "./lib/Terminal.svelte";
  import Settings from "./lib/Settings.svelte";
  import { api, type Profile, type Theme, type TransferProtocol } from "./lib/api";
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

  let draft: Profile | null = null;
  let terminalRef: Terminal | null = null;
  let statusMsg = "";
  let offDisconnect: (() => void) | null = null;
  let offReconnecting: (() => void) | null = null;
  let offReconnected: (() => void) | null = null;
  let settingsOpen = false;
  let suspended = false;
  let ctrlDTR = true;
  let ctrlRTS = true;

  $: selectedExisting = $profiles.find((p) => p.id === $selectedProfileID) ?? null;
  $: currentProfile = draft ?? selectedExisting;
  $: isNew = !!draft;
  $: activeProfileID =
    $session.status === "connected" ||
    $session.status === "connecting" ||
    $session.status === "reconnecting"
      ? $session.profileID
      : "";
  $: isConnected =
    $session.status === "connected" &&
    currentProfile?.id === $session.profileID;
  $: isConnecting =
    $session.status === "connecting" &&
    currentProfile?.id === $session.profileID;
  $: isReconnecting =
    $session.status === "reconnecting" &&
    currentProfile?.id === $session.profileID;
  // Keep the terminal visible during a reconnect so scrollback survives.
  $: viewingTerminal = (isConnected || isReconnecting) && !suspended;
  $: hasSession =
    $session.status === "connected" ||
    $session.status === "connecting" ||
    $session.status === "reconnecting";
  $: activeProfile = $profiles.find((p) => p.id === activeProfileID) ?? null;
  $: termLineEnding = (activeProfile?.lineEnding ?? currentProfile?.lineEnding ?? "crlf") as
    | "cr"
    | "lf"
    | "crlf";
  $: termLocalEcho = activeProfile?.localEcho ?? currentProfile?.localEcho ?? false;
  $: termHighlight = activeProfile?.highlight ?? currentProfile?.highlight ?? true;
  $: termHexView = activeProfile?.hexView ?? currentProfile?.hexView ?? false;
  $: termTimestamps = activeProfile?.timestamps ?? currentProfile?.timestamps ?? false;
  $: termBackspaceKey = ((activeProfile?.backspaceKey || currentProfile?.backspaceKey) ||
    "del") as "bs" | "del";
  $: termCopyOnSelect = $settings.copyOnSelect ?? false;
  $: termPasteWarnMultiline =
    activeProfile?.pasteWarnMultiline ?? currentProfile?.pasteWarnMultiline ?? false;
  $: termPasteSlow = activeProfile?.pasteSlow ?? currentProfile?.pasteSlow ?? false;
  $: termPasteCharDelayMs =
    activeProfile?.pasteCharDelayMs ?? currentProfile?.pasteCharDelayMs ?? 10;

  $: effectiveThemeID =
    (activeProfile?.themeId || currentProfile?.themeId) ||
    $settings.defaultThemeId ||
    "seriesly";
  $: effectiveTheme = resolveTheme(effectiveThemeID, $themes);
  $: termFontSize = $settings.fontSize || 13;

  // Re-apply skin whenever the active selection, loaded list, appearance
  // preference, or system color scheme changes. The window's own NSAppearance
  // is pinned dark at launch (main.go) because Wails v2.12's runtime theme
  // setters are empty stubs on macOS — only CSS swaps live.
  $: applySkin(
    resolveSkin($activeSkinID, $skins),
    $appearance,
    $systemIsDark,
  );

  function resolveTheme(id: string, all: Theme[]): Theme | undefined {
    return (
      all.find((t) => t.id === id) ??
      all.find((t) => t.id === "seriesly") ??
      all[0]
    );
  }

  let defaultLogDir = "";
  let configDir = "";
  let defaultConfigDir = "";

  onMount(async () => {
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

    await Promise.all([
      loadProfiles(),
      loadThemes(),
      loadSkins(),
      loadSettings(),
    ]);
    activeSkinID.set($settings.skinId || "seriesly");
    appearance.set(($settings.appearance as Appearance) || "auto");
    applySkin(resolveSkin($activeSkinID, $skins), $appearance, $systemIsDark);

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
      statusMsg = `Reconnecting to ${portName}…`;
    });

    offReconnected = api.onReconnected((profileID) => {
      session.set({ status: "connected", profileID });
      statusMsg = "Reconnected";
    });

    try {
      const activeID = await api.activeProfileID();
      if (activeID) session.set({ status: "connected", profileID: activeID });
    } catch {}

    try {
      defaultLogDir = await api.defaultLogDirectory();
    } catch {}

    try {
      configDir = await api.getConfigDirectory();
      defaultConfigDir = await api.getDefaultConfigDirectory();
    } catch {}
  });

  onDestroy(() => {
    offDisconnect?.();
    offReconnecting?.();
    offReconnected?.();
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

  let hexSendOpen = false;
  let hexInput = "";
  let hexError = "";

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

  let transferOpen = false;
  let transferProtocol: TransferProtocol = "ymodem";
  let transferPath = "";
  let transferState: TransferState = { status: "picking" };
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
  let overflowOpen = false;

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

  async function handlePickConfigDir() {
    try {
      const dir = await api.pickConfigDirectory();
      if (!dir) return;
      await api.setConfigDirectory(dir);
      configDir = dir;
      statusMsg = "Config directory updated — restart Seriesly to apply";
    } catch (e) {
      statusMsg = `Config directory change failed: ${e}`;
    }
  }

  async function handleResetConfigDir() {
    try {
      await api.setConfigDirectory("");
      configDir = defaultConfigDir;
      statusMsg = "Config directory reset — restart Seriesly to apply";
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
      if ($activeSkinID === id) await handleSetSkin("seriesly");
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

</script>

<svelte:window
  on:click={() => { if (overflowOpen) overflowOpen = false; }}
  on:keydown={(e) => { if (e.key === "Escape" && overflowOpen) overflowOpen = false; }}
/>

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
          skins={$skins}
          settings={$settings}
          {defaultLogDir}
          on:setDefault={(e) => handleSetDefault(e.detail)}
          on:import={handleImportTheme}
          on:delete={(e) => handleDeleteTheme(e.detail)}
          on:setFontSize={(e) => handleSetFontSize(e.detail)}
          on:setLogDir={(e) => handleSetLogDir(e.detail)}
          on:pickLogDir={handlePickLogDir}
          on:setDetectDrivers={(e) => handleSetDetectDrivers(e.detail)}
          on:setCopyOnSelect={(e) => handleSetCopyOnSelect(e.detail)}
          on:pickConfigDir={handlePickConfigDir}
          on:resetConfigDir={handleResetConfigDir}
          {configDir}
          {defaultConfigDir}
          on:setSkin={(e) => handleSetSkin(e.detail)}
          on:importSkin={handleImportSkin}
          on:deleteSkin={(e) => handleDeleteSkin(e.detail)}
          on:setAppearance={(e) => handleSetAppearance(e.detail)}
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
          detectDrivers={!$settings.disableDriverDetection}
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
            <span class="dot" class:reconnecting={isReconnecting}></span>
            <div class="session-text">
              <strong>{activeProfile?.name ?? currentProfile?.name ?? ""}</strong>
              <span class="session-sub">
                {activeProfile?.portName ?? currentProfile?.portName ?? ""} ·
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
                on:click|stopPropagation={toggleOverflow}
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
                  on:click|stopPropagation
                >
                  <button
                    role="menuitem"
                    on:click={() => runFromOverflow(sendBreak)}
                    title="~300ms serial break (Cisco ROMMON, Juniper diag, boot-loader interrupt)"
                  >Send Break</button>
                  <button
                    role="menuitem"
                    on:click={() => runFromOverflow(openHexSend)}
                    title="Send raw bytes as hex (Modbus, firmware bootloaders, binary protocols)"
                  >Send Hex…</button>
                  <button
                    role="menuitem"
                    on:click={() => runFromOverflow(openTransfer)}
                    title="Send a file via XMODEM or YMODEM (firmware uploads, embedded bootloaders)"
                  >Send File…</button>
                </div>
              {/if}
            </div>
            <button
              class="line-btn"
              class:asserted={ctrlDTR}
              on:click={toggleDTR}
              disabled={isReconnecting}
              title="Toggle DTR line ({ctrlDTR ? 'asserted' : 'deasserted'})"
            >
              <span class="line-dot"></span>DTR
            </button>
            <button
              class="line-btn"
              class:asserted={ctrlRTS}
              on:click={toggleRTS}
              disabled={isReconnecting}
              title="Toggle RTS line ({ctrlRTS ? 'asserted' : 'deasserted'})"
            >
              <span class="line-dot"></span>RTS
            </button>
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
          hexView={termHexView}
          timestamps={termTimestamps}
          backspaceKey={termBackspaceKey}
          copyOnSelect={termCopyOnSelect}
          pasteWarnMultiline={termPasteWarnMultiline}
          pasteSlow={termPasteSlow}
          pasteCharDelayMs={termPasteCharDelayMs}
          onStatus={(m) => (statusMsg = m)}
        />
      </div>
    {/if}

    <footer class="status">
      <span class="status-text">{statusMsg || " "}</span>
    </footer>
  </main>
</div>

{#if transferOpen}
  <div
    class="modal-backdrop"
    on:click={closeTransfer}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div class="transfer-modal" on:click|stopPropagation role="presentation">
      <header class="hex-header">
        <strong>Send file</strong>
        <button
          on:click={closeTransfer}
          disabled={transferState.status === "sending"}
          aria-label="Close"
        >×</button>
      </header>

      {#if transferState.status === "picking"}
        <div class="field">
          <label for="transfer-protocol">Protocol</label>
          <select id="transfer-protocol" bind:value={transferProtocol}>
            <option value="ymodem">YMODEM — 1024-byte blocks with filename + size</option>
            <option value="xmodem-1k">XMODEM-1K — 1024-byte blocks, CRC-16</option>
            <option value="xmodem-crc">XMODEM-CRC — 128-byte blocks, CRC-16</option>
            <option value="xmodem">XMODEM — 128-byte blocks, 8-bit checksum (legacy)</option>
          </select>
        </div>

        <div class="field">
          <label>File</label>
          <div class="file-row">
            <input
              type="text"
              readonly
              value={transferPath || ""}
              placeholder="No file selected"
            />
            <button on:click={pickTransferFile}>Choose…</button>
          </div>
        </div>

        <p class="hex-hint">
          Start the receiver on the target device first (<code>rx</code>,
          <code>loady</code>, bootloader "Receive File" menu, etc.) before
          clicking Send. The transfer waits up to 60 s for the receiver's
          handshake before giving up.
        </p>

        <div class="hex-actions">
          <button on:click={closeTransfer}>Cancel</button>
          <button class="primary" on:click={startTransfer} disabled={!transferPath}>
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
          <button on:click={cancelTransfer}>Cancel transfer</button>
        </div>
      {:else if transferState.status === "done"}
        <p class="transfer-done">✓ Sent {transferState.filename}</p>
        <div class="hex-actions">
          <button class="primary" on:click={closeTransfer}>Close</button>
        </div>
      {:else if transferState.status === "error"}
        <div class="hex-error">{transferState.reason}</div>
        <div class="hex-actions">
          <button on:click={() => (transferState = { status: "picking" })}>Try again</button>
          <button class="primary" on:click={closeTransfer}>Close</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if hexSendOpen}
  <div
    class="modal-backdrop"
    on:click={closeHexSend}
    on:keydown={onHexKeydown}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div
      class="hex-modal"
      on:click|stopPropagation
      on:keydown|stopPropagation={onHexKeydown}
      role="presentation"
    >
      <header class="hex-header">
        <strong>Send hex bytes</strong>
        <button on:click={closeHexSend} aria-label="Close">×</button>
      </header>
      <p class="hex-hint">
        Space-separated, compact, or 0x-prefixed — all equivalent:
        <code>02 FF AA 55</code>, <code>02FFAA55</code>, <code>0x02 0xFF 0xAA 0x55</code>.
      </p>
      <!-- svelte-ignore a11y-autofocus -->
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
        <button on:click={closeHexSend}>Cancel</button>
        <button class="primary" on:click={submitHex}>Send</button>
      </div>
    </div>
  </div>
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
