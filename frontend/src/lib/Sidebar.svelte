<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import type { Profile } from "./api";
  import { api } from "./api";

  export let profiles: Profile[];
  export let selectedID: string | null;
  export let activeID: string;

  const dispatch = createEventDispatcher<{
    select: string;
    create: void;
  }>();

  $: sorted = [...profiles].sort((a, b) => a.name.localeCompare(b.name));
</script>

<aside class="sidebar">
  <div class="titlebar" style="--wails-draggable: drag;"></div>

  <div class="header">
    <span class="title">Profiles</span>
    <button
      class="icon-btn"
      title="New profile"
      on:click={() => dispatch("create")}
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

  {#if sorted.length === 0}
    <div class="empty">
      <p>No profiles yet.</p>
      <button on:click={() => dispatch("create")}>Create one</button>
    </div>
  {:else}
    <ul class="list">
      {#each sorted as p (p.id)}
        <li>
          <button
            class="row"
            class:selected={p.id === selectedID}
            on:click={() => dispatch("select", p.id)}
          >
            <span class="indicator" class:active={p.id === activeID}></span>
            <span class="row-body">
              <span class="row-name">{p.name}</span>
              <span class="row-meta">
                {p.portName || "no port"} · {p.baudRate}
              </span>
            </span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    width: 240px;
    min-width: 240px;
    height: 100%;
    background: var(--bg-sidebar);
    border-right: 1px solid var(--border-subtle);
    overflow: hidden;
  }

  .titlebar {
    height: var(--titlebar-height);
    flex-shrink: 0;
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 14px 10px 14px;
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
</style>
