<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import {
    api,
    BAUD_RATES,
    PARITIES,
    STOP_BITS,
    DATA_BITS,
    FLOW_CONTROL,
    LINE_ENDINGS,
    type Profile,
    type PortInfo,
  } from "./api";

  export let profile: Profile;
  export let isNew: boolean;
  export let canConnect: boolean;
  export let isConnected: boolean;
  export let isConnecting: boolean;

  const dispatch = createEventDispatcher<{
    save: Profile;
    delete: string;
    connect: void;
    disconnect: void;
  }>();

  let draft: Profile = { ...profile };
  let syncedFrom: Profile = profile;
  let dirty = false;
  let ports: PortInfo[] = [];
  let loadingPorts = false;
  let saving = false;
  let error = "";

  $: if (profile !== syncedFrom) {
    draft = { ...profile };
    syncedFrom = profile;
    dirty = false;
    error = "";
  }

  $: locked = isConnected || isConnecting;

  $: baudIsCustom = !BAUD_RATES.includes(draft.baudRate);

  onMount(refreshPorts);

  async function refreshPorts() {
    loadingPorts = true;
    try {
      ports = (await api.listPorts()) ?? [];
    } catch (e) {
      console.error("list ports", e);
    } finally {
      loadingPorts = false;
    }
  }

  function markDirty() {
    dirty = true;
    error = "";
  }

  async function save() {
    saving = true;
    error = "";
    try {
      dispatch("save", { ...draft });
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }

  function formatPortLabel(p: PortInfo): string {
    const label = p.product || p.serialNumber;
    return label ? `${p.name} — ${label}` : p.name;
  }

  function portMissing(name: string): boolean {
    return !!name && !ports.some((p) => p.name === name);
  }

  function onBaudChange(e: Event) {
    const v = (e.target as HTMLSelectElement).value;
    if (v !== "custom") draft.baudRate = Number(v);
    markDirty();
  }
</script>

<div class="form">
  <div class="titlebar" style="--wails-draggable: drag;"></div>

  <header>
    <div class="header-left">
      <input
        class="name-input"
        type="text"
        bind:value={draft.name}
        on:input={markDirty}
        placeholder="Profile name"
        disabled={locked}
      />
      <span class="subtitle">
        {isNew ? "New profile" : "Edit profile"}
      </span>
    </div>
    <div class="header-actions">
      {#if !isNew}
        <button
          class="danger"
          on:click={() => dispatch("delete", draft.id)}
          disabled={locked}
        >
          Delete
        </button>
      {/if}
      <button on:click={save} disabled={!dirty || saving || locked}>
        {isNew ? "Create" : "Save"}
      </button>
      {#if isConnected}
        <button class="primary" on:click={() => dispatch("disconnect")}>
          Disconnect
        </button>
      {:else}
        <button
          class="primary"
          on:click={() => dispatch("connect")}
          disabled={!canConnect || isConnecting}
        >
          {isConnecting ? "Connecting…" : "Connect"}
        </button>
      {/if}
    </div>
  </header>

  {#if error}
    <div class="error">{error}</div>
  {/if}

  <section>
    <h3>Connection</h3>
    <div class="grid">
      <div class="field full">
        <label for="port">Serial Port</label>
        <div class="port-row">
          <select
            id="port"
            bind:value={draft.portName}
            on:change={markDirty}
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
            on:click={refreshPorts}
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
          on:change={onBaudChange}
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
            bind:value={draft.baudRate}
            on:input={markDirty}
            disabled={locked}
          />
        {/if}
      </div>

      <div class="field">
        <label for="databits">Data Bits</label>
        <select
          id="databits"
          bind:value={draft.dataBits}
          on:change={markDirty}
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
          on:change={markDirty}
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
          on:change={markDirty}
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
          on:change={markDirty}
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
          on:change={markDirty}
          disabled={locked}
        >
          {#each LINE_ENDINGS as opt}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
      </div>

      <div class="field checkbox">
        <label>
          <input
            type="checkbox"
            bind:checked={draft.localEcho}
            on:change={markDirty}
            disabled={locked}
          />
          Local echo
        </label>
      </div>
    </div>
  </section>
</div>

<style>
  .form {
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: 0 28px 28px 28px;
  }

  .titlebar {
    height: var(--titlebar-height);
    margin: 0 -28px;
    flex-shrink: 0;
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding-bottom: 18px;
    margin-bottom: 20px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .header-left {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .name-input {
    font-size: 20px;
    font-weight: 600;
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
  }

  .header-actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  section {
    margin-bottom: 24px;
  }

  section h3 {
    margin: 0 0 12px 0;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--fg-secondary);
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
</style>
