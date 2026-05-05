<script lang="ts">
  import { tick } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
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
    onSetUpdateCheckEnabled: (enabled: boolean) => void;
    onSetIncludePrereleaseUpdates: (enabled: boolean) => void;
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
    onSetUpdateCheckEnabled,
    onSetIncludePrereleaseUpdates,
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

  // App version pulled from tauri.conf.json at runtime via the
  // tauri-apps/api/app helper. Settled by a fire-and-forget Promise
  // chain at script-init time — Settings always renders, the
  // version slot just stays empty for the few ms before the IPC
  // resolves.
  let appVersion = $state("");
  getVersion()
    .then((v) => {
      appVersion = v;
    })
    .catch(() => {
      // Non-fatal: leave the slot empty rather than break the
      // Settings UI if the helper ever fails (it shouldn't).
    });

  let previewTheme = $state<Theme | null>(null);
  // Theme-preview modal close button — bind:this so we can move
  // focus into the modal when it opens, matching the Sam-persona
  // accessibility fix from the v0.9.4 critique. Without this, a
  // screen-reader user opening the modal stays focused on the
  // Preview button below it.
  let modalCloseEl: HTMLButtonElement | null = $state(null);

  // ─── Tab navigation ───────────────────────────────────────────────
  // Settings is organized into vertical tabs along the left rail.
  // Each tab groups one or more sections; only the active tab's
  // sections are visible at a time. Sections are kept in the DOM
  // (via `hidden` attribute) rather than removed, so any per-section
  // local state (pending-delete timers, modal focus) survives a tab
  // switch.
  //
  // To add a new section: define it in `sectionMeta` (label +
  // keywords for filter), drop it in the right tab's `sectionKeys`,
  // and render the matching <section id={sectionId(key)} ...>.
  type SectionMeta = { key: string; label: string; keywords: string };
  type TabEntry = { key: string; label: string; sectionKeys: string[] };

  const sectionMeta: SectionMeta[] = [
    { key: "app-skin", label: "App Skin",
      keywords: "App Skin chrome appearance light dark auto system theme" },
    { key: "installed-skins", label: "Installed Skins",
      keywords: "Installed Skins import custom appearance" },
    { key: "terminal-defaults", label: "Terminal Defaults",
      keywords: "Terminal Defaults font size scrollback lines" },
    { key: "default-theme", label: "Default Theme",
      keywords: "Default Theme terminal viewport color scheme" },
    { key: "installed-themes", label: "Installed Themes",
      keywords: "Installed Themes import itermcolors color scheme" },
    { key: "keyboard-shortcuts", label: "Keyboard Shortcuts",
      keywords: "Keyboard Shortcuts clear send break suspend binding key hotkey" },
    { key: "syntax-highlighting", label: "Syntax Highlighting",
      keywords: "Syntax Highlighting packs cisco junos aruba arista mikrotik vendor regex" },
    { key: "advanced", label: "Advanced",
      keywords: "Advanced session log directory usb driver detection copy select screen reader updates pre-release config directory" },
  ];

  const tabEntries: TabEntry[] = [
    { key: "appearance", label: "Appearance",
      sectionKeys: ["app-skin", "installed-skins", "terminal-defaults"] },
    { key: "themes", label: "Themes",
      sectionKeys: ["default-theme", "installed-themes"] },
    { key: "shortcuts", label: "Shortcuts",
      sectionKeys: ["keyboard-shortcuts"] },
    { key: "highlighting", label: "Highlighting",
      sectionKeys: ["syntax-highlighting"] },
    { key: "advanced", label: "Advanced",
      sectionKeys: ["advanced"] },
  ];

  function sectionId(key: string): string {
    return `section-${key}`;
  }
  function sectionKeywords(key: string): string {
    return sectionMeta.find((s) => s.key === key)?.keywords ?? key;
  }
  function tabForSection(sectionKey: string): TabEntry | undefined {
    return tabEntries.find((t) => t.sectionKeys.includes(sectionKey));
  }

  // Active tab. Defaults to first.
  let activeTabKey = $state(tabEntries[0].key);

  /** True when the section is part of the currently active tab.
   *  Used to set the `hidden` attribute on each section. */
  function inActiveTab(sectionKey: string): boolean {
    return tabForSection(sectionKey)?.key === activeTabKey;
  }

  /** True when the filter has any matching section in the given
   *  tab. Drives the dim-non-matching-tabs behavior so the user
   *  can SEE which tabs contain hits without auto-switching. When
   *  the filter is empty, every tab is "matching" trivially. */
  function tabHasFilterMatches(tabKey: string): boolean {
    if (!filterText) return true;
    const tab = tabEntries.find((t) => t.key === tabKey);
    if (!tab) return false;
    return tab.sectionKeys.some(
      (k) => sectionKeywords(k).toLowerCase().includes(filterText.toLowerCase()),
    );
  }

  /** Switch to a tab AND auto-clear the filter if doing so would
   *  leave the user staring at an empty pane. Otherwise the filter
   *  is preserved (so a user can type once, scan tabs for matches,
   *  click into the right one without re-typing). */
  function activateTab(key: string): void {
    activeTabKey = key;
    if (filterText && !tabHasFilterMatches(key)) {
      filterText = "";
    }
  }

  // Section filter. `filterText` is the current substring; sections
  // whose title doesn't include it (case-insensitive) fade out.
  // `Cmd+F` / `Ctrl+F` focuses + selects the input from anywhere
  // in the Settings window; `/` does the same when no other input
  // currently has focus (so a user typing in a text field doesn't
  // get hijacked). `Escape` while the filter is focused clears it
  // and returns focus to the page.
  let filterText = $state("");
  let filterInputEl: HTMLInputElement | null = $state(null);

  function fade(title: string): boolean {
    if (!filterText) return false;
    return !title.toLowerCase().includes(filterText.toLowerCase());
  }

  function isOtherInputFocused(): boolean {
    const el = document.activeElement;
    if (!el) return false;
    if (el === filterInputEl) return false;
    return (
      el.tagName === "INPUT" ||
      el.tagName === "TEXTAREA" ||
      el.tagName === "SELECT" ||
      (el as HTMLElement).isContentEditable
    );
  }

  function onSettingsKeydown(e: KeyboardEvent) {
    // Cmd/Ctrl+F — always focus + select the filter (override
    // any browser-native Find behavior the webview might honour).
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f") {
      e.preventDefault();
      filterInputEl?.focus();
      filterInputEl?.select();
      return;
    }
    // `/` — focus the filter only when no other input owns focus.
    if (e.key === "/" && !isOtherInputFocused()) {
      e.preventDefault();
      filterInputEl?.focus();
      return;
    }
    // Esc while the filter is focused — clear + blur.
    if (
      e.key === "Escape" &&
      document.activeElement === filterInputEl
    ) {
      filterText = "";
      filterInputEl?.blur();
    }
  }

  function openPreview(t: Theme) {
    previewTheme = t;
  }

  function closePreview() {
    previewTheme = null;
  }

  function onPreviewKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") closePreview();
  }

  $effect(() => {
    if (previewTheme) {
      void tick().then(() => modalCloseEl?.focus());
    }
  });

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

  // Destructive-remove undo gestures (themes, skins, highlight packs).
  // Click Remove → row morphs to "Removed <name>. Undo" for 5s, then
  // the actual onDelete* prop fires. Click Undo within 5s and the
  // timer is cancelled, the row restored. Per DESIGN.md interaction
  // model + reference/interaction-design.md "Undo > Confirm". Three
  // separate Maps because each type calls a different onDelete*
  // prop; pulling them into one generic helper with three timers
  // would just push the same shape elsewhere.
  let pendingThemeDelete = $state<Map<string, ReturnType<typeof setTimeout>>>(
    new Map(),
  );
  let pendingSkinDelete = $state<Map<string, ReturnType<typeof setTimeout>>>(
    new Map(),
  );
  let pendingPackDelete = $state<Map<string, ReturnType<typeof setTimeout>>>(
    new Map(),
  );

  function startThemeDelete(id: string) {
    if (pendingThemeDelete.has(id)) return;
    const timer = setTimeout(() => {
      onDelete(id);
      const next = new Map(pendingThemeDelete);
      next.delete(id);
      pendingThemeDelete = next;
    }, 5000);
    const next = new Map(pendingThemeDelete);
    next.set(id, timer);
    pendingThemeDelete = next;
  }
  function undoThemeDelete(id: string) {
    const timer = pendingThemeDelete.get(id);
    if (timer) clearTimeout(timer);
    const next = new Map(pendingThemeDelete);
    next.delete(id);
    pendingThemeDelete = next;
  }

  function startSkinDelete(id: string) {
    if (pendingSkinDelete.has(id)) return;
    const timer = setTimeout(() => {
      onDeleteSkin(id);
      const next = new Map(pendingSkinDelete);
      next.delete(id);
      pendingSkinDelete = next;
    }, 5000);
    const next = new Map(pendingSkinDelete);
    next.set(id, timer);
    pendingSkinDelete = next;
  }
  function undoSkinDelete(id: string) {
    const timer = pendingSkinDelete.get(id);
    if (timer) clearTimeout(timer);
    const next = new Map(pendingSkinDelete);
    next.delete(id);
    pendingSkinDelete = next;
  }

  function startPackDelete(id: string) {
    if (pendingPackDelete.has(id)) return;
    const timer = setTimeout(() => {
      onDeleteHighlightPack(id);
      const next = new Map(pendingPackDelete);
      next.delete(id);
      pendingPackDelete = next;
    }, 5000);
    const next = new Map(pendingPackDelete);
    next.set(id, timer);
    pendingPackDelete = next;
  }
  function undoPackDelete(id: string) {
    const timer = pendingPackDelete.get(id);
    if (timer) clearTimeout(timer);
    const next = new Map(pendingPackDelete);
    next.delete(id);
    pendingPackDelete = next;
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

<svelte:window onkeydown={onSettingsKeydown} />

<div class="settings">
  <div class="titlebar" data-tauri-drag-region></div>

  <header>
    <div class="header-left">
      <h1>Settings</h1>
      {#if appVersion}
        <span
          class="version"
          title="Baudrun version (from the bundle's tauri.conf.json)"
          >v{appVersion}</span
        >
      {/if}
    </div>
    <div class="header-right">
      <input
        type="text"
        class="filter"
        placeholder="Filter…"
        bind:value={filterText}
        bind:this={filterInputEl}
        aria-label="Filter Settings sections"
      />
      <span class="filter-hint" aria-hidden="true">{IS_MAC ? "⌘F" : "Ctrl+F"}</span>
    </div>
  </header>

  <div class="body">
  <!-- Vertical tabs. Each tab groups related sections; only the
       active tab's sections are visible (the rest stay in the DOM,
       hidden, so per-section state survives switches). When the
       filter is active, tabs that have no matching section get
       dimmed so the user can SEE which tab to look in without
       auto-switching mid-typing. -->
  <nav class="tabs" aria-label="Settings categories">
    {#each tabEntries as tab (tab.key)}
      <button
        type="button"
        class="tab-entry"
        class:active={activeTabKey === tab.key}
        class:filtered-out={!tabHasFilterMatches(tab.key)}
        onclick={() => activateTab(tab.key)}
        aria-current={activeTabKey === tab.key ? "true" : undefined}
      >
        {tab.label}
      </button>
    {/each}
  </nav>

  <div class="scroll">
  <section id={sectionId("app-skin")} hidden={!inActiveTab("app-skin")} class:filtered-out={fade(sectionKeywords("app-skin"))}>
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

  <section id={sectionId("installed-skins")} hidden={!inActiveTab("installed-skins")} class="flat" class:filtered-out={fade(sectionKeywords("installed-skins"))}>
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
      <div class="error" role="alert" aria-live="polite">{skinImportError}</div>
    {/if}

    {#if skins.some((s) => s.source === "user")}
      <ul class="theme-list">
        {#each skins.filter((s) => s.source === "user") as s (s.id)}
          <li class="theme-row">
            {#if pendingSkinDelete.has(s.id)}
              <span class="pending-text">
                Removed <strong>{s.name}</strong>.
              </span>
              <button
                class="undo-btn"
                onclick={() => undoSkinDelete(s.id)}
                aria-label="Undo removing skin {s.name}"
              >
                Undo
              </button>
            {:else}
              <div class="theme-meta">
                <span class="theme-name">{s.name}</span>
                <span class="theme-source">
                  Custom{#if s.supportsLight === false} · dark-only{/if}
                </span>
              </div>
              <button
                class="danger small"
                onclick={() => startSkinDelete(s.id)}
                title="Remove skin"
                aria-label="Remove skin {s.name}"
              >
                Remove
              </button>
            {/if}
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

  <section id={sectionId("default-theme")} hidden={!inActiveTab("default-theme")} class:filtered-out={fade(sectionKeywords("default-theme"))}>
    <h3>Default Theme</h3>
    <p class="section-hint">
      Used by any profile that doesn't set its own theme.
    </p>
    <div class="field">
      <label for="default-theme">Theme</label>
      <Select
        id="default-theme"
        value={settings.defaultThemeId}
        onchange={onDefaultChangeValue}
        options={defaultThemeOptions}
      />
    </div>
  </section>

  <section id={sectionId("terminal-defaults")} hidden={!inActiveTab("terminal-defaults")} class:filtered-out={fade(sectionKeywords("terminal-defaults"))}>
    <h3>Terminal Defaults</h3>
    <p class="section-hint">
      Font size and scrollback for every profile's terminal viewport.
    </p>
    <div class="grid">
      <div class="field">
        <label for="font-size">Font Size</label>
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

  <section id={sectionId("keyboard-shortcuts")} hidden={!inActiveTab("keyboard-shortcuts")} class:filtered-out={fade(sectionKeywords("keyboard-shortcuts"))}>
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

  <section id={sectionId("installed-themes")} hidden={!inActiveTab("installed-themes")} class="flat" class:filtered-out={fade(sectionKeywords("installed-themes"))}>
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
      <div class="error" role="alert" aria-live="polite">{importError}</div>
    {/if}

    <ul class="theme-list">
      {#each themes as t (t.id)}
        <li class="theme-row">
          {#if pendingThemeDelete.has(t.id)}
            <span class="pending-text">
              Removed <strong>{t.name}</strong>.
            </span>
            <button
              class="undo-btn"
              onclick={() => undoThemeDelete(t.id)}
              aria-label="Undo removing theme {t.name}"
            >
              Undo
            </button>
          {:else}
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
              aria-label="Preview theme {t.name}"
            >
              Preview
            </button>
            {#if t.source === "user"}
              <button
                class="danger small"
                onclick={() => startThemeDelete(t.id)}
                title="Remove theme"
                aria-label="Remove theme {t.name}"
              >
                Remove
              </button>
            {/if}
          {/if}
        </li>
      {/each}
    </ul>
  </section>

  <section id={sectionId("syntax-highlighting")} hidden={!inActiveTab("syntax-highlighting")} class="advanced" class:filtered-out={fade(sectionKeywords("syntax-highlighting"))}>
    <h3>Syntax Highlighting</h3>
    <p class="section-hint">
      {(settings.enabledHighlightPresets ?? []).length} of {highlightPacks.length} pack{highlightPacks.length === 1 ? "" : "s"} enabled — profile-level "Highlight" must be on.
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
          <div class="error" role="alert" aria-live="polite">{highlightImportError}</div>
        {/if}

        <div class="preset-list">
          {#each highlightPacks as pack (pack.id)}
            <div class="preset-row">
              {#if pendingPackDelete.has(pack.id)}
                <span class="pending-text">
                  Removed <strong>{pack.name}</strong>.
                </span>
                <button
                  class="undo-btn"
                  onclick={() => undoPackDelete(pack.id)}
                  aria-label="Undo removing pack {pack.name}"
                >
                  Undo
                </button>
              {:else}
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
                  onclick={() => startPackDelete(pack.id)}
                  title="Remove this imported pack"
                  aria-label="Remove imported pack {pack.name}"
                >
                  Remove
                </button>
              {/if}
              {/if}
            </div>
          {/each}
        </div>
  </section>

  <section id={sectionId("advanced")} hidden={!inActiveTab("advanced")} class="advanced" class:filtered-out={fade(sectionKeywords("advanced"))}>
    <h3>Advanced</h3>
    <p class="section-hint">Session logging and other global defaults.</p>

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
        <h4>Updates</h4>
        <p class="section-hint">
          Check GitHub on app launch for a newer Baudrun release. When a
          newer version is found, a one-line notice appears in the
          footer linking to the release notes. No auto-update — you
          download and install from GitHub yourself.
        </p>
        <label class="toggle">
          <input
            type="checkbox"
            checked={!settings.disableUpdateCheck}
            onchange={(e) =>
              onSetUpdateCheckEnabled((e.target as HTMLInputElement).checked)}
          />
          Check for updates on launch
        </label>
        <label class="toggle" style="margin-top: 8px;">
          <input
            type="checkbox"
            checked={settings.includePrereleaseUpdates ?? false}
            disabled={settings.disableUpdateCheck ?? false}
            onchange={(e) =>
              onSetIncludePrereleaseUpdates(
                (e.target as HTMLInputElement).checked,
              )}
          />
          Include pre-releases (alpha / beta / rc)
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
  </section>
  </div>
  </div>
</div>

{#if previewTheme}
  <!-- Backdrop is decorative (the dim canvas); the dialog role
       belongs on the actual modal panel. Click on backdrop closes;
       the modal stops propagation so its own clicks don't bubble. -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="modal-backdrop"
    onclick={closePreview}
    onkeydown={onPreviewKeydown}
  >
    <div
      class="modal"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
      role="dialog"
      aria-modal="true"
      aria-label="Theme preview: {previewTheme.name}"
      tabindex="-1"
    >
      <header class="modal-header">
        <div class="modal-title">
          <strong>{previewTheme.name}</strong>
          <span class="modal-subtitle">
            {previewTheme.source === "builtin" ? "Built-in theme" : "Custom theme"} · sample network-gear output
          </span>
        </div>
        <button bind:this={modalCloseEl} onclick={closePreview}>Close</button>
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

  /* Row layout: TOC rail on the left, scrolling content on the right.
     Both children are full-height; only `.scroll` actually scrolls. */
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 20px 28px 28px 28px;
  }

  /* Vertical tabs. Fixed-width left rail; each entry is a full-
     bleed button. Active state uses --bg-active (operator-blue
     tint) + accent color text. No side-stripe borders (banned per
     DESIGN.md) — the bg fill IS the active affordance. Hover and
     filter-fade follow the standard opacity ladder. */
  .tabs {
    flex-shrink: 0;
    width: 168px;
    padding: 14px 10px 14px 16px;
    border-right: 1px solid var(--border-subtle);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .tab-entry {
    appearance: none;
    -webkit-appearance: none;
    background: transparent;
    border: 0;
    margin: 0;
    padding: 8px 12px;
    border-radius: var(--radius-md);
    font: inherit;
    font-size: 13px;
    font-weight: 500;
    color: var(--fg-secondary);
    text-align: left;
    cursor: pointer;
    transition:
      background 0.12s,
      color 0.12s,
      opacity 0.12s;
  }
  .tab-entry:hover {
    background: var(--bg-hover);
    color: var(--fg-primary);
  }
  .tab-entry:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .tab-entry.active {
    background: var(--bg-active);
    color: var(--accent);
  }
  /* Filter feedback: when the user types something that doesn't
     match any section in a given tab, that tab dims. Lets the user
     scan the rail to find which tab contains hits without auto-
     switching mid-typing. The active tab itself stays dim if it
     has no matches — clicking another tab clears the filter via
     activateTab(). */
  .tab-entry.filtered-out {
    opacity: 0.4;
  }

  .titlebar {
    height: var(--titlebar-height);
    flex-shrink: 0;
  }

  header {
    flex-shrink: 0;
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    padding: 0 28px 18px 28px;
    border-bottom: 1px solid var(--border-subtle);
  }

  /* Quick glance to confirm which build is running without competing
     visually with the "Settings" heading. Uses the project's mono
     stack via --font-mono (which already encodes the SF Mono → Menlo
     fallback chain in style.css; no need to duplicate it here).
     Color is --fg-secondary so the pill clears WCAG AA contrast at
     11px against the panel substrate; the previous --fg-tertiary
     (rgba 0.4) failed AA. */
  .version {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--fg-secondary);
    letter-spacing: 0.02em;
    user-select: text;
  }

  .header-left {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }

  .header-left h1 {
    margin: 0;
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  /* Right cluster: filter input + keyboard hint. The hint sits flush
     against the input so the pair reads as one affordance and the
     header still balances the title on the left. */
  .header-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .filter {
    width: 200px;
    padding: 6px 10px;
    font-size: 13px;
    color: var(--fg-primary);
    background: var(--bg-input);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    outline: none;
    transition:
      border-color 0.12s,
      background 0.12s;
  }
  .filter::placeholder {
    color: var(--fg-tertiary);
  }
  .filter:hover {
    border-color: var(--border-strong);
  }
  .filter:focus {
    border-color: var(--accent);
    background: var(--bg-input-focus, var(--bg-input));
  }

  .filter-hint {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--fg-tertiary);
    letter-spacing: 0.02em;
    user-select: none;
  }

  /* When a section's title doesn't match the active filter, fade and
     disable interactions so the matching sections stand out without
     reflowing the page. Pointer-events:none keeps focus from drifting
     into a hidden section. */
  .filtered-out {
    opacity: 0.25;
    pointer-events: none;
    transition: opacity 0.12s;
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
    /* No negative letter-spacing on section headings — DESIGN.md §3
       reserves tight tracking for the 24px H1; section-level type
       at 15px reads better with default tracking. */
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

  /* Pending-delete row + Undo affordance, shared across the three
     destructive lists (themes, skins, highlight packs). The row
     stays on the same surface as the normal rendering — Caution
     Red is NEVER used as fill per DESIGN.md §2; the undo affordance
     itself is the visual cue that something happened. The Undo
     button uses the accent color but stays transparent at rest;
     hover steps to the accent-hover with --bg-hover backdrop. */
  .pending-text {
    flex: 1;
    color: var(--fg-secondary);
    font-size: var(--font-size-base);
    line-height: 1.4;
  }
  .pending-text strong {
    color: var(--fg-primary);
    font-weight: 500;
  }
  .undo-btn {
    flex-shrink: 0;
    background: transparent;
    border: none;
    color: var(--accent);
    font-weight: 500;
    padding: 4px 10px;
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: background 0.12s, color 0.12s;
  }
  .undo-btn:hover {
    color: var(--accent-hover);
    background: var(--bg-hover);
  }
  .undo-btn:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
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

  /* Error text only — no fill. DESIGN.md §2 reserves Caution Red as
     a fill for actual destructive intent; a failed import is mild
     and shouldn't claim that visual budget. font-weight: 500 so the
     red text reads as decisive without needing background help.
     `role="alert"` + `aria-live="polite"` are added on the markup
     side so screen-reader users hear the message without it being
     a banner-shaped block. */
  .error {
    margin-bottom: 12px;
    color: var(--danger);
    font-size: 12px;
    font-weight: 500;
    line-height: 1.4;
  }

  /* (Removed dead .advanced details / summary rules — Syntax
     Highlighting and Advanced no longer collapse, since each lives
     in its own tab now.) */

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

  /* Flat row list. The previous per-row card treatment (border +
     background tint) lived inside the .sub panel card, breaking
     DESIGN.md §5 "The No-Cards-Within-Cards Rule" and registering
     as the v0.9.4 critique's P1 card-in-card violation. Now: rows
     are flat, separated by 1px --border-subtle rules, with a
     full-bleed hover state that extends past the .sub's 16px
     horizontal padding via negative margin so the hover reads as
     the whole row width. */
  .preset-list {
    display: flex;
    flex-direction: column;
  }

  .preset-row {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 16px;
    margin: 0 -16px;
    border-bottom: 1px solid var(--border-subtle);
    transition: background 0.12s;
  }

  .preset-row:last-child {
    border-bottom: none;
  }

  .preset-row:hover {
    background: var(--bg-hover);
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
    /* No card chrome — the row is the surface. */
  }

  .toggle.preset input {
    /* 0.2em aligns the checkbox optically with the first line of
       multi-line .preset-meta without the magic 3px we used to need
       when the row had its own padding. */
    margin-top: 0.2em;
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

  /* Modal elevation flows through CSS variables per DESIGN.md §4
     Elevation-Belongs-To-Skins Rule. Base skin's --shadow-floating
     is `none` and --blur-strength is `0px` — the modal reads as
     flat against its border on the default theme. Skins (macOS 26
     Liquid Glass, Windows 11 Fluent) opt into stronger elevation
     by setting these vars at the document level. */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 10000;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(var(--blur-strength));
    -webkit-backdrop-filter: blur(var(--blur-strength));
    padding: 24px;
  }

  .modal {
    background: var(--bg-main);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-floating);
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
