<script lang="ts">
  import {
    api,
    type Theme,
    type Settings,
    type Skin,
    type HighlightPack,
  } from "./api";
  import PreviewTerminal from "./PreviewTerminal.svelte";
  import Select, { type SelectItems } from "./Select.svelte";
  import KeyCapture from "./KeyCapture.svelte";
  import {
    SHORTCUT_ACTIONS,
    SHORTCUT_LABELS,
    effectiveShortcut,
    type ShortcutAction,
  } from "./shortcuts";

  type Props = {
    themes?: Theme[];
    skins?: Skin[];
    settings: Settings;
    configDir?: string;
    defaultConfigDir?: string;
    defaultLogDir?: string;
    onSetDefault: (id: string) => void;
    onImport: () => void;
    onDelete: (id: string) => void;
    onSetFontSize: (size: number) => void;
    onSetScrollback: (lines: number) => void;
    onSetLogDir: (dir: string) => void;
    onPickLogDir: () => void;
    onSetDetectDrivers: (enabled: boolean) => void;
    onSetCopyOnSelect: (enabled: boolean) => void;
    onSetScreenReaderMode: (enabled: boolean) => void;
    onPickConfigDir: () => void;
    onResetConfigDir: () => void;
    onSetSkin: (id: string) => void;
    onImportSkin: () => void;
    onDeleteSkin: (id: string) => void;
    onSetAppearance: (mode: "auto" | "light" | "dark") => void;
    onSetShortcuts: (shortcuts: Record<string, string>) => void;
    highlightPacks?: HighlightPack[];
    onSetEnabledHighlightPresets: (ids: string[]) => void;
    onImportHighlightPack: () => void;
    onDeleteHighlightPack: (id: string) => void;
  };

  let {
    themes = [],
    skins = [],
    settings,
    configDir = "",
    defaultConfigDir = "",
    defaultLogDir = "",
    onSetDefault,
    onImport,
    onDelete,
    onSetFontSize,
    onSetScrollback,
    onSetLogDir,
    onPickLogDir,
    onSetDetectDrivers,
    onSetCopyOnSelect,
    onSetScreenReaderMode,
    onPickConfigDir,
    onResetConfigDir,
    onSetSkin,
    onImportSkin,
    onDeleteSkin,
    onSetAppearance,
    onSetShortcuts,
    highlightPacks = [],
    onSetEnabledHighlightPresets,
    onImportHighlightPack,
    onDeleteHighlightPack,
  }: Props = $props();

  // Platform marker for formatShortcut() in the keyboard-shortcuts
  // section. Same test App.svelte uses.
  const IS_MAC =
    typeof navigator !== "undefined" && /Mac/i.test(navigator.platform);

  function currentShortcut(action: ShortcutAction): string {
    return effectiveShortcut(action, settings.shortcuts, IS_MAC);
  }

  function onShortcutChange(action: ShortcutAction, spec: string) {
    const next = { ...(settings.shortcuts ?? {}), [action]: spec };
    onSetShortcuts(next);
  }

  function onShortcutReset(action: ShortcutAction) {
    const next = { ...(settings.shortcuts ?? {}) };
    delete next[action];
    onSetShortcuts(next);
  }

  let previewTheme = $state<Theme | null>(null);

  function openPreview(t: Theme) {
    previewTheme = t;
  }

  function closePreview() {
    previewTheme = null;
  }

  function onPreviewKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") closePreview();
  }

  const configIsCustom = $derived(
    !!configDir && !!defaultConfigDir && configDir !== defaultConfigDir,
  );

  let importing = $state(false);
  let importError = $state("");
  let importingSkin = $state(false);
  let skinImportError = $state("");

  async function handleImport() {
    importing = true;
    importError = "";
    try {
      onImport();
    } catch (e) {
      importError = String(e);
    } finally {
      importing = false;
    }
  }

  async function handleSkinImport() {
    importingSkin = true;
    skinImportError = "";
    try {
      onImportSkin();
    } catch (e) {
      skinImportError = String(e);
    } finally {
      importingSkin = false;
    }
  }

  function onDefaultChangeValue(v: string | number) {
    onSetDefault(String(v));
  }

  function onFontSizeChange(e: Event) {
    onSetFontSize(Number((e.target as HTMLInputElement).value));
  }

  // Scrollback presets. Custom values (set by editing settings.json
  // directly) are preserved and shown as "N lines (custom)" in the
  // dropdown so they don't silently reset.
  const SCROLLBACK_PRESETS = [1000, 5000, 10000, 50000, 100000];
  const scrollbackValue = $derived(settings.scrollbackLines || 10000);
  const scrollbackIsCustom = $derived(!SCROLLBACK_PRESETS.includes(scrollbackValue));

  function onScrollbackChangeValue(v: string | number) {
    onSetScrollback(Number(v));
  }

  const scrollbackOptions: SelectItems = $derived.by(() => {
    const base: SelectItems = [
      { value: 1000, label: "1,000 lines (~0.4 MB)" },
      { value: 5000, label: "5,000 lines (~2 MB)" },
      { value: 10000, label: "10,000 lines (~4 MB) · default" },
      { value: 50000, label: "50,000 lines (~20 MB)" },
      { value: 100000, label: "100,000 lines (~40 MB)" },
    ];
    if (scrollbackIsCustom) {
      base.push({
        value: scrollbackValue,
        label: `${scrollbackValue.toLocaleString()} lines (custom)`,
      });
    }
    return base;
  });

  function onLogDirChange(e: Event) {
    onSetLogDir((e.target as HTMLInputElement).value.trim());
  }

  function onDetectDriversChange(e: Event) {
    onSetDetectDrivers((e.target as HTMLInputElement).checked);
  }

  function onCopyOnSelectChange(e: Event) {
    onSetCopyOnSelect((e.target as HTMLInputElement).checked);
  }

  function onScreenReaderModeChange(e: Event) {
    onSetScreenReaderMode((e.target as HTMLInputElement).checked);
  }

  function isHighlightPackEnabled(id: string): boolean {
    return (settings.enabledHighlightPresets ?? []).includes(id);
  }

  function onTogglePresetPack(id: string, enabled: boolean) {
    const current = settings.enabledHighlightPresets ?? [];
    const next = enabled
      ? Array.from(new Set([...current, id]))
      : current.filter((p) => p !== id);
    onSetEnabledHighlightPresets(next);
  }

  let importingHighlight = $state(false);
  let highlightImportError = $state("");

  async function handleHighlightImport() {
    importingHighlight = true;
    highlightImportError = "";
    try {
      onImportHighlightPack();
    } catch (e) {
      highlightImportError = String(e);
    } finally {
      importingHighlight = false;
    }
  }

  // A pack is deletable iff it's a user-source pack AND not the
  // editable scratchpad (id === "user"). Bundled packs are read-only
  // and the scratchpad is always present.
  function canDeletePack(pack: HighlightPack): boolean {
    return pack.source === "user" && pack.id !== "user";
  }

  async function openInFileManager(path: string) {
    if (!path) return;
    try {
      await api.openPath(path);
    } catch {
      // Swallow — the Open button is a nicety; errors would only
      // surface from malformed paths that the user can see in the
      // adjacent text field.
    }
  }

  function onSkinChangeValue(v: string | number) {
    onSetSkin(String(v));
  }

  function onAppearanceChangeValue(v: string | number) {
    if (v === "auto" || v === "light" || v === "dark") {
      onSetAppearance(v);
    }
  }

  const builtinThemes = $derived(themes.filter((t) => t.source === "builtin"));
  const userThemes = $derived(themes.filter((t) => t.source === "user"));

  // Option arrays for the skin / appearance / default-theme pickers.
  // Grouped shape for pickers that separate built-in vs. user entries.
  const skinOptions: SelectItems = $derived.by(() => {
    const out: SelectItems = [];
    const builtins = skins.filter((s) => s.source === "builtin");
    const users = skins.filter((s) => s.source === "user");
    if (builtins.length) {
      out.push({
        label: "Built-in",
        options: builtins.map((s) => ({ value: s.id, label: s.name })),
      });
    }
    if (users.length) {
      out.push({
        label: "Custom",
        options: users.map((s) => ({ value: s.id, label: s.name })),
      });
    }
    return out;
  });

  const appearanceOptions: SelectItems = [
    { value: "auto", label: "Auto (Follow System)" },
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
  ];

  const defaultThemeOptions: SelectItems = $derived.by(() => {
    const out: SelectItems = [];
    if (builtinThemes.length) {
      out.push({
        label: "Built-in",
        options: builtinThemes.map((t) => ({ value: t.id, label: t.name })),
      });
    }
    if (userThemes.length) {
      out.push({
        label: "Custom",
        options: userThemes.map((t) => ({ value: t.id, label: t.name })),
      });
    }
    return out;
  });
</script>

<div class="settings">
  <div class="titlebar" data-tauri-drag-region></div>

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
        <Select
          id="skin"
          value={settings.skinId || "baudrun"}
          onchange={onSkinChangeValue}
          options={skinOptions}
        />
      </div>

      <div class="field">
        <label for="appearance">Appearance</label>
        <Select
          id="appearance"
          value={settings.appearance || "auto"}
          onchange={onAppearanceChangeValue}
          options={appearanceOptions}
        />
      </div>
    </div>
  </section>

  <section class="flat">
    <div class="section-head">
      <h3>Installed Skins</h3>
      <button
        onclick={handleSkinImport}
        disabled={importingSkin}
        title="Pick a skin JSON file to install as a custom skin. See docs/SKINS.md for the schema, or start from docs/examples/skin.example.json."
      >
        {importingSkin ? "Importing…" : "Import skin…"}
      </button>
    </div>
    {#if skinImportError}
      <div class="error">{skinImportError}</div>
    {/if}

    {#if skins.some((s) => s.source === "user")}
      <ul class="theme-list">
        {#each skins.filter((s) => s.source === "user") as s (s.id)}
          <li class="theme-row">
            <div class="theme-meta">
              <span class="theme-name">{s.name}</span>
              <span class="theme-source">
                Custom{#if s.supportsLight === false} · dark-only{/if}
              </span>
            </div>
            <button
              class="danger small"
              onclick={() => onDeleteSkin(s.id)}
              title="Remove skin"
              aria-label="Remove skin"
            >
              Remove
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="section-hint" style="margin: 0;">
        No custom skins installed. Use Import to add a skin JSON file.
        See <code>docs/SKINS.md</code> for the authoring guide, or
        start from <code>docs/examples/skin.example.json</code>.
      </p>
    {/if}
  </section>

  <section>
    <h3>Default Theme</h3>
    <p class="section-hint">
      Used by any profile that doesn't set its own theme.
    </p>
    <div class="grid">
      <div class="field">
        <label for="default-theme">Theme</label>
        <Select
          id="default-theme"
          value={settings.defaultThemeId}
          onchange={onDefaultChangeValue}
          options={defaultThemeOptions}
        />
      </div>

      <div class="field">
        <label for="font-size">Terminal Font Size</label>
        <input
          id="font-size"
          type="number"
          min="8"
          max="28"
          value={settings.fontSize || 13}
          onchange={onFontSizeChange}
        />
      </div>

      <div class="field">
        <label for="scrollback-lines">Scrollback</label>
        <Select
          id="scrollback-lines"
          value={scrollbackValue}
          onchange={onScrollbackChangeValue}
          options={scrollbackOptions}
        />
      </div>
    </div>
  </section>

  <section>
    <h3>Keyboard Shortcuts</h3>
    <p class="section-hint">
      Session-header actions. Click a binding to record a new key combo;
      Escape cancels the recording. Use the ↺ button to reset to the
      platform default. On macOS the defaults use ⌘ (Cmd is never a
      terminal control character so plain ⌘K is safe); on Linux and
      Windows the defaults use Ctrl+Shift so plain Ctrl+letter still
      passes through to the serial device as a control byte.
    </p>
    <div class="shortcut-rows">
      {#each SHORTCUT_ACTIONS as action (action)}
        <div class="shortcut-row">
          <label for={`shortcut-${action}`}>{SHORTCUT_LABELS[action]}</label>
          <KeyCapture
            id={`shortcut-${action}`}
            value={currentShortcut(action)}
            isMac={IS_MAC}
            onchange={(spec) => onShortcutChange(action, spec)}
            onreset={() => onShortcutReset(action)}
          />
        </div>
      {/each}
    </div>
  </section>

  <section class="flat">
    <div class="section-head">
      <h3>Installed Themes</h3>
      <button
        onclick={handleImport}
        disabled={importing}
        title="Pick an .itermcolors file (iTerm2 color scheme) to install as a terminal theme. Baudrun maps the iTerm color slots onto its own palette automatically."
      >
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
            onclick={() => openPreview(t)}
            title="Preview theme"
            aria-label="Preview theme"
          >
            Preview
          </button>
          {#if t.source === "user"}
            <button
              class="danger small"
              onclick={() => onDelete(t.id)}
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
        <h3>Syntax Highlighting</h3>
        <span class="hint">
          {(settings.enabledHighlightPresets ?? []).length} of {highlightPacks.length} pack{highlightPacks.length === 1 ? "" : "s"} enabled — profile-level "Highlight" must be on
        </span>
      </summary>

      <div class="sub">
        <p class="section-hint">
          Highlight rules grouped into packs. The default vendor-neutral set
          covers IPs, MACs, interface names, and status keywords. Device-
          specific packs (Cisco IOS, Junos, Aruba CX, Arista EOS, MikroTik
          RouterOS) add patterns common to each platform's output. Bundled
          packs are read-only; the "User overrides" scratchpad is editable
          at <code>$SUPPORT_DIR/highlight-rules.json</code>. Imported packs
          live under <code>$SUPPORT_DIR/highlight/&lt;id&gt;.json</code> and
          can be removed from here.
        </p>

        <div class="section-head">
          <button
            onclick={handleHighlightImport}
            disabled={importingHighlight}
            title="Pick a highlight pack JSON to add alongside the bundled ones. See docs/examples/highlight-pack.example.json for the schema."
          >
            {importingHighlight ? "Importing…" : "Import pack…"}
          </button>
        </div>
        {#if highlightImportError}
          <div class="error">{highlightImportError}</div>
        {/if}

        <div class="preset-list">
          {#each highlightPacks as pack (pack.id)}
            <div class="preset-row">
              <label class="toggle preset">
                <input
                  type="checkbox"
                  checked={isHighlightPackEnabled(pack.id)}
                  onchange={(e) =>
                    onTogglePresetPack(
                      pack.id,
                      (e.target as HTMLInputElement).checked,
                    )}
                />
                <span class="preset-meta">
                  <span class="preset-name">{pack.name}</span>
                  {#if pack.description}
                    <span class="preset-desc">{pack.description}</span>
                  {/if}
                  <span class="preset-count">
                    {pack.rules.length} rule{pack.rules.length === 1 ? "" : "s"}
                    {#if pack.id === "user"}
                      · editable scratchpad
                    {:else if pack.source === "user"}
                      · imported
                    {:else}
                      · built-in
                    {/if}
                  </span>
                </span>
              </label>
              {#if canDeletePack(pack)}
                <button
                  class="danger small"
                  onclick={() => onDeleteHighlightPack(pack.id)}
                  title="Remove this imported pack"
                  aria-label="Remove imported pack {pack.name}"
                >
                  Remove
                </button>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    </details>
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
            onchange={onLogDirChange}
          />
          <button
            onclick={() => openInFileManager(settings.logDir || defaultLogDir)}
            title="Open this folder in Finder / Explorer"
          >Open</button>
          <button onclick={onPickLogDir}>Choose…</button>
          {#if settings.logDir}
            <button onclick={() => onSetLogDir("")} title="Reset to default">
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
            onchange={onDetectDriversChange}
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
            onchange={onCopyOnSelectChange}
          />
          Copy terminal selection to clipboard automatically
        </label>
      </div>

      <div class="sub">
        <h4>Screen Reader Support</h4>
        <p class="section-hint">
          Route incoming terminal output through an ARIA live region
          so screen readers (VoiceOver, NVDA, Orca) can narrate it.
          Small perf cost on heavy output — leave off unless needed.
        </p>
        <label class="toggle">
          <input
            type="checkbox"
            checked={settings.screenReaderMode ?? false}
            onchange={onScreenReaderModeChange}
          />
          Enable xterm screen-reader mode
        </label>
      </div>

      <div class="sub">
        <h4>Config Directory</h4>
        <p class="section-hint">
          Where profiles, themes, skins, and settings are stored.
          Relocate to keep Baudrun's config alongside your other
          dotfiles. Takes effect on next app launch. <strong>Existing
          files are not moved</strong> — copy them over yourself to
          preserve profiles.
        </p>
        <div class="log-row">
          <input type="text" readonly value={configDir} />
          <button
            onclick={() => openInFileManager(configDir)}
            title="Open this folder in Finder / Explorer"
          >Open</button>
          <button onclick={onPickConfigDir}>Choose…</button>
          {#if configIsCustom}
            <button onclick={onResetConfigDir} title="Reset to default">
              Reset
            </button>
          {/if}
        </div>
        {#if configIsCustom}
          <p class="section-hint" style="margin-top: 8px;">
            Default: <code>{defaultConfigDir}</code>
          </p>
        {/if}
      </div>
    </details>
  </section>
  </div>
</div>

{#if previewTheme}
  <div
    class="modal-backdrop"
    onclick={closePreview}
    onkeydown={onPreviewKeydown}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div class="modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="presentation">
      <header class="modal-header">
        <div class="modal-title">
          <strong>{previewTheme.name}</strong>
          <span class="modal-subtitle">
            {previewTheme.source === "builtin" ? "Built-in theme" : "Custom theme"} · sample network-gear output
          </span>
        </div>
        <button onclick={closePreview}>Close</button>
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
    margin-bottom: 16px;
    padding: 16px 20px;
    background: var(--bg-panel);
    border: var(--panel-border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-panel);
    backdrop-filter: blur(var(--blur-strength));
    -webkit-backdrop-filter: blur(var(--blur-strength));
  }

  /* Sections that contain their own card rows (theme list) or their
     own nested sub-panels (Advanced) opt out of the outer panel
     treatment so layers don't stack. */
  section.advanced,
  section.flat {
    padding: 0;
    background: transparent;
    border: none;
    box-shadow: none;
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
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

  /* Custom Select trigger is already width: 100% inside its own
     component CSS; inputs still need the explicit rule. */
  .field input {
    width: 100%;
  }

  .shortcut-rows {
    display: grid;
    grid-template-columns: 1fr auto;
    row-gap: 10px;
    column-gap: 20px;
    align-items: center;
  }

  .shortcut-row {
    display: contents;
  }

  .shortcut-row label {
    color: var(--fg-primary);
    font-size: 13px;
    text-transform: none;
    letter-spacing: normal;
    font-weight: normal;
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

  .preset-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .preset-row {
    display: flex;
    align-items: stretch;
    gap: 8px;
  }

  .preset-row .toggle.preset {
    flex: 1;
  }

  .preset-row button.danger.small {
    flex-shrink: 0;
    align-self: center;
  }

  .toggle.preset {
    align-items: flex-start;
    padding: 8px 10px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--bg-input);
  }

  .toggle.preset input {
    margin-top: 3px;
  }

  .preset-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .preset-name {
    font-weight: 500;
    color: var(--fg-primary);
  }

  .preset-desc {
    font-size: 12px;
    color: var(--fg-secondary);
    line-height: 1.35;
  }

  .preset-count {
    font-size: 11px;
    color: var(--fg-tertiary);
    font-variant-numeric: tabular-nums;
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
