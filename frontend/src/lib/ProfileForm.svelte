<script lang="ts">
  import { onMount } from "svelte";
  import { BrowserOpenURL } from "../../wailsjs/runtime/runtime.js";
  import {
    dismissedDrivers,
    dismissDriver,
    driverKey,
  } from "../stores/drivers";
  import { portScanning } from "../stores/scanning";
  import {
    api,
    BAUD_RATES,
    PARITIES,
    STOP_BITS,
    DATA_BITS,
    FLOW_CONTROL,
    LINE_ENDINGS,
    LINE_POLICIES,
    type Profile,
    type PortInfo,
    type Theme,
    type USBSerialCandidate,
  } from "./api";

  type Props = {
    profile: Profile;
    isNew: boolean;
    canConnect: boolean;
    isConnected: boolean;
    isConnecting: boolean;
    isReconnecting?: boolean;
    suspended?: boolean;
    themes?: Theme[];
    defaultThemeID?: string;
    detectDrivers?: boolean;
    onSave: (p: Profile) => void;
    onDelete: (id: string) => void;
    onConnect: () => void;
    onDisconnect: () => void;
    onResume: () => void;
  };

  let {
    profile,
    isNew,
    canConnect,
    isConnected,
    isConnecting,
    isReconnecting = false,
    suspended = false,
    themes = [],
    defaultThemeID = "baudrun",
    detectDrivers = true,
    onSave,
    onDelete,
    onConnect,
    onDisconnect,
    onResume,
  }: Props = $props();

  let draft = $state<Profile>({ ...profile } as Profile);
  // syncedFrom deliberately NOT $state — writing to it inside $effect
  // where the effect also reads it would otherwise retrigger the
  // effect (even though the condition self-stabilizes, cleaner to
  // keep it as a plain tracking variable).
  let syncedFrom: Profile = profile;
  let dirty = $state(false);
  let ports = $state<PortInfo[]>([]);
  let missingDrivers = $state<USBSerialCandidate[]>([]);
  let loadingPorts = $state(false);
  let saving = $state(false);
  let error = $state("");

  $effect(() => {
    if (profile !== syncedFrom) {
      draft = { ...profile } as Profile;
      syncedFrom = profile;
      dirty = false;
      error = "";
    }
  });

  // Lock form fields + Save while the port is actively held or being
  // opened. A suspended session keeps the port open too, but the UI
  // contract there is "I've stepped away, let me edit" — save still
  // just writes JSON; the live session keeps its old settings until
  // the user disconnects and reconnects.
  const locked = $derived((isConnected && !suspended) || isConnecting);

  // customMode sticks after the user picks "Custom…" from the dropdown,
  // even if their typed value happens to coincide with a preset. Cleared
  // whenever the user picks a preset back from the dropdown.
  let customMode = $state(false);
  const baudIsCustom = $derived(
    customMode || !BAUD_RATES.includes(draft.baudRate),
  );

  onMount(refreshPorts);

  async function refreshPorts() {
    loadingPorts = true;
    portScanning.set(true);
    try {
      const pPromise = api.listPorts();
      const mPromise = detectDrivers
        ? api.listMissingDrivers().catch((e) => {
            console.warn("listMissingDrivers failed:", e);
            return [] as USBSerialCandidate[];
          })
        : Promise.resolve([] as USBSerialCandidate[]);
      const [p, missing] = await Promise.all([pPromise, mPromise]);
      ports = p ?? [];
      missingDrivers = missing ?? [];
    } catch (e) {
      console.error("list ports", e);
    } finally {
      loadingPorts = false;
      portScanning.set(false);
    }
  }

  // Rescan when the global toggle flips so banners disappear/reappear live.
  // lastDetectDrivers is deliberately NOT $state — making it reactive
  // would cause the $effect to re-run when we write to it, leading to
  // a spin that kept refreshPorts firing and the scanning indicator
  // stuck on.
  let lastDetectDrivers = detectDrivers;
  $effect(() => {
    if (detectDrivers !== lastDetectDrivers) {
      lastDetectDrivers = detectDrivers;
      refreshPorts();
    }
  });

  const visibleMissing = $derived(
    missingDrivers.filter((d) => !$dismissedDrivers.has(driverKey(d))),
  );

  function markDirty() {
    dirty = true;
    error = "";
  }

  async function save() {
    saving = true;
    error = "";
    try {
      onSave({ ...draft } as Profile);
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }

  const SUSPECT_PRODUCT_RE =
    /please install|please download|support windows|counterfeit|not support/i;

  function formatPortLabel(p: PortInfo): string {
    let product = p.product || "";
    // Windows-issued stub drivers sometimes use the entire warning message as
    // the product name. Replace with a terse "(driver issue)" note.
    if (product && SUSPECT_PRODUCT_RE.test(product)) {
      product = "driver issue";
    }
    const parts: string[] = [p.name];
    const detail = [product, p.chipset].filter(Boolean).join(" · ");
    if (detail) parts.push(detail);
    return parts.join(" — ");
  }

  function portMissing(name: string): boolean {
    return !!name && !ports.some((p) => p.name === name);
  }

  function onBaudChange(e: Event) {
    const v = (e.target as HTMLSelectElement).value;
    if (v === "custom") {
      // Keep the current baud as the starting point so the user has
      // something sensible to edit from; flipping customMode reveals
      // the numeric input below.
      customMode = true;
    } else {
      customMode = false;
      draft.baudRate = Number(v);
    }
    markDirty();
  }

  const defaultThemeName = $derived(
    themes.find((t) => t.id === defaultThemeID)?.name ?? "Baudrun",
  );
</script>

<div class="form">
  <div class="titlebar" style="--wails-draggable: drag;"></div>

  <header>
    <div class="header-left">
      <input
        class="name-input"
        type="text"
        bind:value={draft.name}
        oninput={markDirty}
        placeholder="Profile name"
        disabled={locked}
      />
      <span class="subtitle">
        {#if isReconnecting}
          <span class="dot-pill reconnecting"></span> Session reconnecting…
        {:else if isConnected}
          <span class="dot-pill"></span> Session suspended
        {:else}
          {isNew ? "New profile" : "Edit profile"}
        {/if}
      </span>
    </div>
    <div class="header-actions">
      {#if !isNew}
        <button
          class="danger"
          onclick={() => onDelete(draft.id)}
          disabled={locked}
        >
          Delete
        </button>
      {/if}
      <button onclick={save} disabled={!dirty || saving || locked}>
        {isNew ? "Create" : "Save"}
      </button>
      {#if isConnected}
        <button onclick={onDisconnect}>
          Disconnect
        </button>
        <button class="primary" onclick={onResume}>
          Resume
        </button>
      {:else}
        <button
          class="primary"
          onclick={onConnect}
          disabled={!canConnect || isConnecting}
        >
          {isConnecting ? "Connecting…" : "Connect"}
        </button>
      {/if}
    </div>
  </header>

  <div class="scroll">
    {#if error}
      <div class="error">{error}</div>
    {/if}

  <section>
    <h3>Connection</h3>

    {#if visibleMissing.length > 0}
      <div class="driver-banner">
        {#each visibleMissing as d (driverKey(d))}
          <div class="driver-row">
            <div class="driver-icon" aria-hidden="true">!</div>
            <div class="driver-text">
              <div class="driver-title">
                {d.chipset} detected — driver not loaded
              </div>
              {#if d.reason}
                <div class="driver-sub">{d.reason}</div>
              {/if}
              <div class="driver-sub driver-meta">
                {#if d.product && !/please install|please download|support windows|counterfeit|not support/i.test(d.product)}
                  {d.product}
                {:else if d.manufacturer}
                  {d.manufacturer}
                {:else}
                  USB device
                {/if}
                {#if d.serialNumber}
                  · serial {d.serialNumber}
                {/if}
              </div>
            </div>
            {#if d.driverURL}
              <button onclick={() => BrowserOpenURL(d.driverURL!)}>
                Install driver…
              </button>
            {/if}
            <button
              class="driver-close"
              onclick={() => dismissDriver(driverKey(d))}
              title="Dismiss"
              aria-label="Dismiss driver notice"
            >
              ×
            </button>
          </div>
        {/each}
      </div>
    {/if}

    <div class="grid">
      <div class="field full">
        <label for="port">Serial Port</label>
        <div class="port-row">
          <select
            id="port"
            bind:value={draft.portName}
            onchange={markDirty}
            disabled={locked}
          >
            <option value="">— Select a port —</option>
            {#each ports as p}
              <option value={p.name}>{formatPortLabel(p)}</option>
            {/each}
            {#if portMissing(draft.portName)}
              <option value={draft.portName}>
                {draft.portName} (not connected)
              </option>
            {/if}
          </select>
          <button
            class="icon-btn"
            onclick={refreshPorts}
            disabled={loadingPorts || locked}
            title="Rescan ports"
            aria-label="Rescan ports"
          >
            <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
              <path
                d="M11 6.5A4.5 4.5 0 1 1 6.5 2m0 0L9 2m-2.5 0v2.5"
                stroke="currentColor"
                stroke-width="1.4"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </button>
        </div>
      </div>

      <div class="field">
        <label for="baud">Baud Rate</label>
        <select
          id="baud"
          value={baudIsCustom ? "custom" : draft.baudRate}
          onchange={onBaudChange}
          disabled={locked}
        >
          {#each BAUD_RATES as rate}
            <option value={rate}>{rate}</option>
          {/each}
          <option value="custom">Custom…</option>
        </select>
        {#if baudIsCustom}
          <input
            class="mt-4"
            type="number"
            min="50"
            step="1"
            placeholder="baud (e.g. 500000)"
            bind:value={draft.baudRate}
            oninput={markDirty}
            disabled={locked}
          />
          <span class="inline-hint">Any positive integer the adapter supports.</span>
        {/if}
      </div>

      <div class="field">
        <label for="databits">Data Bits</label>
        <select
          id="databits"
          bind:value={draft.dataBits}
          onchange={markDirty}
          disabled={locked}
        >
          {#each DATA_BITS as b}
            <option value={b}>{b}</option>
          {/each}
        </select>
      </div>

      <div class="field">
        <label for="parity">Parity</label>
        <select
          id="parity"
          bind:value={draft.parity}
          onchange={markDirty}
          disabled={locked}
        >
          {#each PARITIES as opt}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
      </div>

      <div class="field">
        <label for="stopbits">Stop Bits</label>
        <select
          id="stopbits"
          bind:value={draft.stopBits}
          onchange={markDirty}
          disabled={locked}
        >
          {#each STOP_BITS as opt}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
      </div>

      <div class="field">
        <label for="flow">Flow Control</label>
        <select
          id="flow"
          bind:value={draft.flowControl}
          onchange={markDirty}
          disabled={locked}
        >
          {#each FLOW_CONTROL as opt}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
      </div>
    </div>
  </section>

  <section>
    <h3>Terminal</h3>
    <div class="grid">
      <div class="field">
        <label for="lineending">Send Line Ending</label>
        <select
          id="lineending"
          bind:value={draft.lineEnding}
          onchange={markDirty}
          disabled={locked}
        >
          {#each LINE_ENDINGS as opt}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
      </div>

      <div class="field">
        <label for="backspace">Backspace sends</label>
        <select
          id="backspace"
          bind:value={draft.backspaceKey}
          onchange={markDirty}
          disabled={locked}
        >
          <option value="del">DEL (0x7F) — VT100 / xterm / most modern</option>
          <option value="bs">BS (0x08) — some older Cisco / Foundry gear</option>
        </select>
      </div>

      <div class="field checkbox">
        <label>
          <input
            type="checkbox"
            bind:checked={draft.localEcho}
            onchange={markDirty}
            disabled={locked}
          />
          Local echo
        </label>
      </div>

      <div class="field checkbox">
        <label>
          <input
            type="checkbox"
            bind:checked={draft.highlight}
            onchange={markDirty}
          />
          Syntax highlighting
        </label>
      </div>
    </div>
  </section>

  <section>
    <h3>Appearance</h3>
    <div class="grid">
      <div class="field full">
        <label for="theme">Theme</label>
        <select
          id="theme"
          bind:value={draft.themeId}
          onchange={markDirty}
        >
          <option value="">Default — {defaultThemeName}</option>
          {#if themes.some((t) => t.source === "builtin")}
            <optgroup label="Built-in">
              {#each themes.filter((t) => t.source === "builtin") as t (t.id)}
                <option value={t.id}>{t.name}</option>
              {/each}
            </optgroup>
          {/if}
          {#if themes.some((t) => t.source === "user")}
            <optgroup label="Custom">
              {#each themes.filter((t) => t.source === "user") as t (t.id)}
                <option value={t.id}>{t.name}</option>
              {/each}
            </optgroup>
          {/if}
        </select>
      </div>
    </div>
  </section>

  <section class="advanced">
    <details>
      <summary>
        <h3>Advanced</h3>
        <span class="hint">Control lines, hex view, timestamps, session logging</span>
      </summary>

      <section class="sub">
        <h4>Control Lines</h4>
        <p class="section-hint">
          Only needed for specific adapters or devices (RS-485 direction,
          Arduino DTR-reset, firmwares that key off DTR for session lifecycle).
        </p>
        <div class="grid">
          <div class="field">
            <label for="dtr-connect">DTR on connect</label>
            <select
              id="dtr-connect"
              bind:value={draft.dtrOnConnect}
              onchange={markDirty}
              disabled={locked}
            >
              {#each LINE_POLICIES as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          </div>

          <div class="field">
            <label for="rts-connect">RTS on connect</label>
            <select
              id="rts-connect"
              bind:value={draft.rtsOnConnect}
              onchange={markDirty}
              disabled={locked}
            >
              {#each LINE_POLICIES as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          </div>

          <div class="field">
            <label for="dtr-disconnect">DTR on disconnect</label>
            <select
              id="dtr-disconnect"
              bind:value={draft.dtrOnDisconnect}
              onchange={markDirty}
              disabled={locked}
            >
              {#each LINE_POLICIES as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          </div>

          <div class="field">
            <label for="rts-disconnect">RTS on disconnect</label>
            <select
              id="rts-disconnect"
              bind:value={draft.rtsOnDisconnect}
              onchange={markDirty}
              disabled={locked}
            >
              {#each LINE_POLICIES as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          </div>
        </div>
      </section>

      <section class="sub">
        <h4>Output</h4>
        <div class="grid">
          <div class="field checkbox">
            <label>
              <input
                type="checkbox"
                bind:checked={draft.hexView}
                onchange={markDirty}
              />
              Hex view
              <span class="inline-hint">show incoming bytes as hex dump</span>
            </label>
          </div>

          <div class="field checkbox">
            <label>
              <input
                type="checkbox"
                bind:checked={draft.timestamps}
                onchange={markDirty}
              />
              Line timestamps
              <span class="inline-hint">prefix each line with wall-clock time</span>
            </label>
          </div>

          <div class="field checkbox full">
            <label>
              <input
                type="checkbox"
                bind:checked={draft.logEnabled}
                onchange={markDirty}
              />
              Record session to file
              <span class="inline-hint">raw bytes; destination set in Settings → Advanced</span>
            </label>
          </div>

          <div class="field checkbox full">
            <label>
              <input
                type="checkbox"
                bind:checked={draft.autoReconnect}
                onchange={markDirty}
              />
              Auto-reconnect on drop
              <span class="inline-hint">poll for the port to reappear (up to 30s) and reopen transparently</span>
            </label>
          </div>
        </div>
      </section>

      <section class="sub">
        <h4>Paste safety</h4>
        <p class="section-hint">
          Catch the "I pasted into the wrong window" mistake, and pace
          pastes so UARTs on slower devices don't drop bytes.
        </p>
        <div class="grid">
          <div class="field checkbox full">
            <label>
              <input
                type="checkbox"
                bind:checked={draft.pasteWarnMultiline}
                onchange={markDirty}
              />
              Confirm multi-line pastes
              <span class="inline-hint">prompt before sending pasted text that contains line breaks</span>
            </label>
          </div>

          <div class="field checkbox">
            <label>
              <input
                type="checkbox"
                bind:checked={draft.pasteSlow}
                onchange={markDirty}
              />
              Slow paste
              <span class="inline-hint">send one char at a time with a delay</span>
            </label>
          </div>

          <div class="field">
            <label for="paste-delay">Slow-paste delay (ms)</label>
            <input
              id="paste-delay"
              type="number"
              min="0"
              max="500"
              bind:value={draft.pasteCharDelayMs}
              onchange={markDirty}
              disabled={!draft.pasteSlow}
            />
          </div>
        </div>
      </section>
    </details>
  </section>
  </div>
</div>

<style>
  .form {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 20px 28px 28px 28px;
  }

  .titlebar {
    height: var(--titlebar-height);
    flex-shrink: 0;
  }

  header {
    flex-shrink: 0;
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding: 0 28px 18px 28px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .header-left {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .name-input {
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.01em;
    background: transparent;
    border: 1px solid transparent;
    padding: 4px 6px;
    margin-left: -6px;
    border-radius: var(--radius-md);
  }

  .name-input:hover:not(:disabled) {
    background: var(--bg-input);
  }

  .name-input:focus {
    background: var(--bg-input-focus);
  }

  .subtitle {
    margin-top: 4px;
    margin-left: 0;
    font-size: 11px;
    color: var(--fg-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .dot-pill {
    display: inline-block;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--success);
    box-shadow: 0 0 6px var(--success);
  }

  .dot-pill.reconnecting {
    background: var(--warn);
    box-shadow: 0 0 6px var(--warn);
    animation: dot-reconnect-pulse 1s ease-in-out infinite;
  }

  @keyframes dot-reconnect-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.35; }
  }

  @media (prefers-reduced-motion: reduce) {
    .dot-pill.reconnecting { animation: none; }
  }

  .header-actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  section {
    margin-bottom: 16px;
    padding: 16px 20px;
    background: var(--bg-panel);
    border: var(--panel-border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-panel);
    backdrop-filter: blur(var(--blur-strength));
    -webkit-backdrop-filter: blur(var(--blur-strength));
  }

  /* The Advanced section is a disclosure wrapping nested sub-panels;
     skip the outer panel treatment so we don't stack layers. */
  section.advanced {
    padding: 0;
    background: transparent;
    border: none;
    box-shadow: none;
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }

  section h3 {
    margin: 0 0 12px 0;
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.005em;
    color: var(--fg-primary);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 14px 20px;
  }

  .field.full {
    grid-column: 1 / -1;
  }

  .field input,
  .field select {
    width: 100%;
  }

  .field.checkbox label {
    display: flex;
    align-items: center;
    gap: 8px;
    text-transform: none;
    letter-spacing: normal;
    font-size: 13px;
    color: var(--fg-primary);
    font-weight: normal;
    cursor: pointer;
  }

  .field.checkbox input {
    width: auto;
    accent-color: var(--accent);
  }

  .port-row {
    display: flex;
    gap: 6px;
  }

  .port-row select {
    flex: 1;
  }

  .icon-btn {
    width: 28px;
    flex-shrink: 0;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-secondary);
  }

  .mt-4 {
    margin-top: 6px;
  }

  .error {
    padding: 8px 12px;
    margin-bottom: 16px;
    background: rgba(255, 69, 58, 0.12);
    color: var(--danger);
    border-radius: var(--radius-md);
    font-size: 12px;
  }

  .advanced details summary {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    list-style: none;
    padding: 6px 0;
    margin-bottom: 14px;
    border-radius: var(--radius-sm);
    user-select: none;
  }

  .advanced details summary::-webkit-details-marker {
    display: none;
  }

  .advanced details summary h3 {
    margin: 0;
  }

  .advanced details summary::before {
    content: "";
    display: inline-block;
    width: 0;
    height: 0;
    border-top: 5px solid transparent;
    border-bottom: 5px solid transparent;
    border-left: 7px solid var(--fg-secondary);
    margin-right: 2px;
    transition: transform 0.15s ease;
    flex-shrink: 0;
  }

  .advanced details summary:hover::before {
    border-left-color: var(--fg-primary);
  }

  .advanced details[open] summary::before {
    transform: rotate(90deg);
  }

  .advanced .hint {
    font-size: 12px;
    color: var(--fg-tertiary);
    font-weight: normal;
  }

  .advanced .sub {
    padding: 16px;
    margin-bottom: 10px;
    background: var(--bg-panel);
    border: var(--panel-border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-panel);
    backdrop-filter: blur(var(--blur-strength));
    -webkit-backdrop-filter: blur(var(--blur-strength));
  }

  .advanced .sub h4 {
    margin: 0 0 6px 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--fg-primary);
  }

  .advanced .section-hint {
    margin: 0 0 12px 0;
    font-size: 12px;
    line-height: 1.45;
    color: var(--fg-secondary);
  }

  .advanced .inline-hint {
    margin-left: 6px;
    font-size: 12px;
    color: var(--fg-tertiary);
    font-weight: normal;
  }

  .field.checkbox.full {
    grid-column: 1 / -1;
  }

  .driver-banner {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 14px;
  }

  .driver-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    background: rgba(245, 215, 110, 0.1);
    border: 1px solid rgba(245, 215, 110, 0.3);
    border-radius: var(--radius-md);
  }

  .driver-icon {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: rgba(245, 215, 110, 0.25);
    color: #f5d76e;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 13px;
    font-weight: 600;
    flex-shrink: 0;
  }

  .driver-text {
    flex: 1;
    min-width: 0;
  }

  .driver-title {
    font-size: 12px;
    font-weight: 500;
    color: var(--fg-primary);
  }

  .driver-sub {
    font-size: 11px;
    color: var(--fg-tertiary);
    margin-top: 1px;
  }

  .driver-close {
    width: 24px;
    height: 24px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    color: var(--fg-tertiary);
    font-size: 18px;
    line-height: 1;
    flex-shrink: 0;
    border-radius: var(--radius-sm);
  }

  .driver-close:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--fg-primary);
  }
</style>
