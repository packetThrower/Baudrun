<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import type { Theme, Settings, Skin } from "./api";
  import PreviewTerminal from "./PreviewTerminal.svelte";

  export let themes: Theme[] = [];
  export let skins: Skin[] = [];
  export let settings: Settings;

  let previewTheme: Theme | null = null;

  function openPreview(t: Theme) {
    previewTheme = t;
  }

  function closePreview() {
    previewTheme = null;
  }

  function onPreviewKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") closePreview();
  }

  const dispatch = createEventDispatcher<{
    setDefault: string;
    import: void;
    delete: string;
    setFontSize: number;
    setLogDir: string;
    pickLogDir: void;
    setDetectDrivers: boolean;
    setCopyOnSelect: boolean;
    setSkin: string;
    importSkin: void;
    deleteSkin: string;
    setAppearance: "auto" | "light" | "dark";
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

  function onDetectDriversChange(e: Event) {
    dispatch("setDetectDrivers", (e.target as HTMLInputElement).checked);
  }

  function onCopyOnSelectChange(e: Event) {
    dispatch("setCopyOnSelect", (e.target as HTMLInputElement).checked);
  }

  function onSkinChange(e: Event) {
    dispatch("setSkin", (e.target as HTMLSelectElement).value);
  }

  function onAppearanceChange(e: Event) {
    const v = (e.target as HTMLSelectElement).value;
    if (v === "auto" || v === "light" || v === "dark") {
      dispatch("setAppearance", v);
    }
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

  <div class="scroll">
  <section>
    <h3>App Skin</h3>
    <p class="section-hint">
      The overall look of the app's chrome — colors, typography, radii.
      Distinct from terminal themes below, which control the terminal
      viewport's color scheme.
    </p>
    <div class="grid">
      <div class="field">
        <label for="skin">Skin</label>
        <select
          id="skin"
          value={settings.skinId || "seriesly"}
          on:change={onSkinChange}
        >
          {#if skins.some((s) => s.source === "builtin")}
            <optgroup label="Built-in">
              {#each skins.filter((s) => s.source === "builtin") as s (s.id)}
                <option value={s.id}>{s.name}</option>
              {/each}
            </optgroup>
          {/if}
          {#if skins.some((s) => s.source === "user")}
            <optgroup label="Custom">
              {#each skins.filter((s) => s.source === "user") as s (s.id)}
                <option value={s.id}>{s.name}</option>
              {/each}
            </optgroup>
          {/if}
        </select>
      </div>

      <div class="field">
        <label for="appearance">Appearance</label>
        <select
          id="appearance"
          value={settings.appearance || "auto"}
          on:change={onAppearanceChange}
        >
          <option value="auto">Auto (Follow System)</option>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
        </select>
      </div>
    </div>
  </section>

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
          <button
            class="small"
            on:click={() => openPreview(t)}
            title="Preview theme"
            aria-label="Preview theme"
          >
            Preview
          </button>
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

      <div class="sub">
        <h4>USB Driver Detection</h4>
        <p class="section-hint">
          Show a banner in the profile form when a USB-serial adapter is
          plugged in without its vendor driver installed.
        </p>
        <label class="toggle">
          <input
            type="checkbox"
            checked={!settings.disableDriverDetection}
            on:change={onDetectDriversChange}
          />
          Detect un-drivered USB adapters
        </label>
      </div>

      <div class="sub">
        <h4>Copy on Select</h4>
        <p class="section-hint">
          PuTTY-style — copy the terminal selection to the clipboard
          automatically when the mouse is released. Avoids having to
          press Cmd/Ctrl+C for every snippet.
        </p>
        <label class="toggle">
          <input
            type="checkbox"
            checked={settings.copyOnSelect ?? false}
            on:change={onCopyOnSelectChange}
          />
          Copy terminal selection to clipboard automatically
        </label>
      </div>
    </details>
  </section>
  </div>
</div>

{#if previewTheme}
  <div
    class="modal-backdrop"
    on:click={closePreview}
    on:keydown={onPreviewKeydown}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div class="modal" on:click|stopPropagation on:keydown|stopPropagation role="presentation">
      <header class="modal-header">
        <div class="modal-title">
          <strong>{previewTheme.name}</strong>
          <span class="modal-subtitle">
            {previewTheme.source === "builtin" ? "Built-in theme" : "Custom theme"} · sample network-gear output
          </span>
        </div>
        <button on:click={closePreview}>Close</button>
      </header>
      <PreviewTerminal theme={previewTheme} />
    </div>
  </div>
{/if}

<style>
  .settings {
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
    padding: 0 28px 18px 28px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .header-left h1 {
    margin: 0;
    font-size: 24px;
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
    margin-bottom: 32px;
  }

  section h3 {
    margin: 0 0 8px 0;
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.005em;
    color: var(--fg-primary);
  }

  .section-hint {
    margin: 0 0 14px 0;
    font-size: 12px;
    line-height: 1.45;
    color: var(--fg-secondary);
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
    border: var(--panel-border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-panel);
    backdrop-filter: blur(var(--blur-strength));
    -webkit-backdrop-filter: blur(var(--blur-strength));
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
    padding: 14px 16px;
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

  .log-row {
    display: flex;
    gap: 8px;
  }

  .log-row input {
    flex: 1;
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .toggle {
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

  .toggle input {
    width: auto;
    accent-color: var(--accent);
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

  .modal {
    background: var(--bg-main);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    width: 100%;
    max-width: 720px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px;
    border-bottom: 1px solid var(--border-subtle);
    gap: 16px;
  }

  .modal-title {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .modal-title strong {
    font-size: 15px;
    font-weight: 600;
  }

  .modal-subtitle {
    font-size: 12px;
    color: var(--fg-tertiary);
  }
</style>
