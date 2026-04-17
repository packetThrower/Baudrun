<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import type { Theme, Settings } from "./api";

  export let themes: Theme[] = [];
  export let settings: Settings;

  const dispatch = createEventDispatcher<{
    setDefault: string;
    import: void;
    delete: string;
    setFontSize: number;
    setLogDir: string;
    pickLogDir: void;
  }>();

  let importing = false;
  let importError = "";

  async function handleImport() {
    importing = true;
    importError = "";
    try {
      dispatch("import");
    } catch (e) {
      importError = String(e);
    } finally {
      importing = false;
    }
  }

  function onDefaultChange(e: Event) {
    dispatch("setDefault", (e.target as HTMLSelectElement).value);
  }

  function onFontSizeChange(e: Event) {
    dispatch("setFontSize", Number((e.target as HTMLInputElement).value));
  }

  function onLogDirChange(e: Event) {
    dispatch("setLogDir", (e.target as HTMLInputElement).value.trim());
  }

  export let defaultLogDir: string = "";

  $: builtinThemes = themes.filter((t) => t.source === "builtin");
  $: userThemes = themes.filter((t) => t.source === "user");
</script>

<div class="settings">
  <div class="titlebar" style="--wails-draggable: drag;"></div>

  <header>
    <div class="header-left">
      <h1>Settings</h1>
      <span class="subtitle">Global preferences</span>
    </div>
  </header>

  <section>
    <h3>Default Theme</h3>
    <p class="section-hint">
      Used by any profile that doesn't set its own theme.
    </p>
    <div class="grid">
      <div class="field">
        <label for="default-theme">Theme</label>
        <select
          id="default-theme"
          value={settings.defaultThemeId}
          on:change={onDefaultChange}
        >
          {#if builtinThemes.length > 0}
            <optgroup label="Built-in">
              {#each builtinThemes as t (t.id)}
                <option value={t.id}>{t.name}</option>
              {/each}
            </optgroup>
          {/if}
          {#if userThemes.length > 0}
            <optgroup label="Custom">
              {#each userThemes as t (t.id)}
                <option value={t.id}>{t.name}</option>
              {/each}
            </optgroup>
          {/if}
        </select>
      </div>

      <div class="field">
        <label for="font-size">Terminal Font Size</label>
        <input
          id="font-size"
          type="number"
          min="8"
          max="28"
          value={settings.fontSize || 13}
          on:change={onFontSizeChange}
        />
      </div>
    </div>
  </section>

  <section>
    <div class="section-head">
      <h3>Installed Themes</h3>
      <button on:click={handleImport} disabled={importing}>
        {importing ? "Importing…" : "Import .itermcolors…"}
      </button>
    </div>
    {#if importError}
      <div class="error">{importError}</div>
    {/if}

    <ul class="theme-list">
      {#each themes as t (t.id)}
        <li class="theme-row">
          <div class="swatch">
            <span class="sw bg" style:background={t.background}></span>
            <span class="sw" style:background={t.red}></span>
            <span class="sw" style:background={t.green}></span>
            <span class="sw" style:background={t.yellow}></span>
            <span class="sw" style:background={t.blue}></span>
            <span class="sw" style:background={t.magenta}></span>
            <span class="sw" style:background={t.cyan}></span>
            <span class="sw fg" style:background={t.foreground}></span>
          </div>
          <div class="theme-meta">
            <span class="theme-name">{t.name}</span>
            <span class="theme-source">{t.source === "builtin" ? "Built-in" : "Custom"}</span>
          </div>
          {#if t.source === "user"}
            <button
              class="danger small"
              on:click={() => dispatch("delete", t.id)}
              title="Delete theme"
              aria-label="Delete theme"
            >
              Remove
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  </section>

  <section class="advanced">
    <details>
      <summary>
        <h3>Advanced</h3>
        <span class="hint">Session logging and other global defaults</span>
      </summary>

      <div class="sub">
        <h4>Session Log Directory</h4>
        <p class="section-hint">
          Where profiles with "Record session to file" enabled write their logs.
          Leave blank to use the default.
        </p>
        <div class="log-row">
          <input
            type="text"
            value={settings.logDir || ""}
            placeholder={defaultLogDir}
            on:change={onLogDirChange}
          />
          <button on:click={() => dispatch("pickLogDir")}>Choose…</button>
          {#if settings.logDir}
            <button on:click={() => dispatch("setLogDir", "")} title="Reset to default">
              Reset
            </button>
          {/if}
        </div>
      </div>
    </details>
  </section>
</div>

<style>
  .settings {
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
    padding-bottom: 18px;
    margin-bottom: 20px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .header-left h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .subtitle {
    display: block;
    margin-top: 4px;
    font-size: 11px;
    color: var(--fg-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  section {
    margin-bottom: 28px;
  }

  section h3 {
    margin: 0 0 4px 0;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--fg-secondary);
  }

  .section-hint {
    margin: 0 0 12px 0;
    font-size: 12px;
    color: var(--fg-tertiary);
  }

  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 14px 20px;
  }

  .field select,
  .field input {
    width: 100%;
  }

  .theme-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .theme-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    background: var(--bg-panel);
    border-radius: var(--radius-md);
  }

  .swatch {
    display: flex;
    border-radius: var(--radius-sm);
    overflow: hidden;
    border: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }

  .sw {
    display: inline-block;
    width: 16px;
    height: 22px;
  }

  .theme-meta {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .theme-name {
    font-size: 13px;
    font-weight: 500;
  }

  .theme-source {
    font-size: 11px;
    color: var(--fg-tertiary);
    margin-top: 1px;
  }

  button.small {
    font-size: 11px;
    padding: 3px 8px;
  }

  .error {
    padding: 8px 12px;
    margin-bottom: 12px;
    background: rgba(255, 69, 58, 0.12);
    color: var(--danger);
    border-radius: var(--radius-md);
    font-size: 12px;
  }

  .advanced details summary {
    display: flex;
    align-items: baseline;
    gap: 10px;
    cursor: pointer;
    list-style: none;
    padding: 2px 0;
    margin-bottom: 12px;
  }

  .advanced details summary::-webkit-details-marker {
    display: none;
  }

  .advanced details summary h3 {
    margin: 0;
  }

  .advanced details summary::before {
    content: "▸";
    color: var(--fg-tertiary);
    font-size: 10px;
    margin-right: 2px;
    transition: transform 0.1s;
    display: inline-block;
  }

  .advanced details[open] summary::before {
    transform: rotate(90deg);
  }

  .advanced .hint {
    font-size: 11px;
    color: var(--fg-tertiary);
    font-weight: normal;
    text-transform: none;
    letter-spacing: normal;
  }

  .advanced .sub {
    padding-left: 16px;
    border-left: 2px solid var(--border-subtle);
  }

  .advanced .sub h4 {
    margin: 0 0 4px 0;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--fg-secondary);
  }

  .log-row {
    display: flex;
    gap: 8px;
  }

  .log-row input {
    flex: 1;
    font-family: var(--font-mono);
    font-size: 12px;
  }
</style>
