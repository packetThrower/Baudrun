<script lang="ts">
  import type { Profile } from "./api";
  import { formatPortName } from "./ports";

  type Props = {
    profiles: Profile[];
    selectedID: string | null;
    activeID: string;
    settingsOpen?: boolean;
    onSelect: (id: string) => void;
    onCreate: () => void;
    onSettings: () => void;
  };

  let {
    profiles,
    selectedID,
    activeID,
    settingsOpen = false,
    onSelect,
    onCreate,
    onSettings,
  }: Props = $props();

  const sorted = $derived(
    [...profiles].sort((a, b) => a.name.localeCompare(b.name)),
  );
</script>

<aside class="sidebar">
  <div class="titlebar" data-tauri-drag-region></div>

  <div class="header">
    <span class="title">Profiles</span>
    <div class="header-actions">
      <button
        class="icon-btn"
        title="New profile"
        onclick={onCreate}
        aria-label="New profile"
      >
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
          <path
            d="M7 2v10M2 7h10"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linecap="round"
          />
        </svg>
      </button>
    </div>
  </div>

  {#if sorted.length === 0}
    <div class="empty">
      <p>No profiles yet.</p>
      <button onclick={onCreate}>Create one</button>
    </div>
  {:else}
    <ul class="list">
      {#each sorted as p (p.id)}
        <li>
          <button
            class="row"
            class:selected={p.id === selectedID && !settingsOpen}
            onclick={() => onSelect(p.id)}
          >
            <span class="indicator" class:active={p.id === activeID}></span>
            <span class="row-body">
              <span class="row-name">{p.name}</span>
              <span class="row-meta">
                {p.portName ? formatPortName(p.portName) : "no port"} · {p.baudRate}
              </span>
            </span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <button
    class="footer-btn"
    class:active={settingsOpen}
    onclick={onSettings}
    title="Settings"
  >
    Settings
  </button>
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    width: 240px;
    min-width: 240px;
    height: 100%;
    background: var(--bg-sidebar);
    border-right: var(--sidebar-divider, 1px solid var(--border-subtle));
    border-radius: var(--panel-radius);
    box-shadow: var(--panel-shadow);
    overflow: hidden;
    /* Only visible when the active skin sets --blur-strength to > 0
       AND --bg-sidebar is translucent AND something with visual
       interest sits behind it (e.g. a gradient --shell-bg). Default
       skins set --blur-strength: 0 so this costs nothing for them. */
    backdrop-filter: blur(var(--blur-strength));
    -webkit-backdrop-filter: blur(var(--blur-strength));
  }

  .titlebar {
    height: var(--titlebar-height);
    flex-shrink: 0;
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 14px 10px 14px;
  }

  .title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--fg-secondary);
  }

  .icon-btn {
    width: 22px;
    height: 22px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--fg-secondary);
  }

  .icon-btn:hover {
    background: var(--bg-hover);
    color: var(--fg-primary);
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0 8px;
    overflow-y: auto;
    flex: 1;
  }

  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    background: transparent;
    border: none;
    border-radius: var(--radius-md);
    text-align: left;
    margin-bottom: 2px;
  }

  .row:hover {
    background: var(--bg-hover);
  }

  .row.selected {
    background: var(--bg-active);
  }

  .indicator {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: transparent;
    flex-shrink: 0;
  }

  .indicator.active {
    background: var(--success);
    box-shadow: 0 0 6px var(--success);
  }

  .row-body {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .row-name {
    font-size: 13px;
    color: var(--fg-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-meta {
    font-size: 11px;
    color: var(--fg-tertiary);
    margin-top: 1px;
  }

  .empty {
    padding: 30px 20px;
    text-align: center;
    color: var(--fg-tertiary);
  }

  .empty p {
    margin: 0 0 12px 0;
    font-size: 12px;
  }

  .header-actions {
    display: flex;
    gap: 4px;
  }

  .footer-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 30px;
    background: rgba(0, 0, 0, 0.2);
    border: none;
    border-top: 1px solid var(--border-subtle);
    border-radius: 0;
    color: var(--fg-secondary);
    font-size: 11px;
    flex-shrink: 0;
  }

  .footer-btn:hover {
    background: var(--bg-hover);
    color: var(--fg-primary);
  }

  .footer-btn.active {
    background: var(--bg-active);
    color: var(--fg-primary);
  }
</style>
