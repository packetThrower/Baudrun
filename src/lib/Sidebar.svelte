<script lang="ts">
  import { api, type Profile } from "./api";
  import { formatPortName } from "./ports";

  type Props = {
    profiles: Profile[];
    selectedID: string | null;
    activeID: string;
    settingsOpen?: boolean;
    onSelect: (id: string) => void;
    onCreate: () => void;
    onSettings: () => void;
    /** Spawn a new window for `profile`. App.svelte handles the
     *  decision of whether to also migrate this window's active
     *  session — both gestures (right-click + drag-out) flow
     *  through the same callback so the migration logic isn't
     *  duplicated here. */
    onOpenInNewWindow: (profile: Profile) => void;
  };

  let {
    profiles,
    selectedID,
    activeID,
    settingsOpen = false,
    onSelect,
    onCreate,
    onSettings,
    onOpenInNewWindow,
  }: Props = $props();

  const sorted = $derived(
    [...profiles].sort((a, b) => a.name.localeCompare(b.name)),
  );

  // Right-click → tear-off context menu. Position is set per-event
  // from the cursor, then closed by any click anywhere or Escape.
  let menu = $state<{ profile: Profile; x: number; y: number } | null>(null);

  function openMenu(e: MouseEvent, p: Profile) {
    e.preventDefault();
    menu = { profile: p, x: e.clientX, y: e.clientY };
  }

  function closeMenu() {
    menu = null;
  }

  function chooseOpenInNewWindow() {
    if (!menu) return;
    const target = menu.profile;
    menu = null;
    onOpenInNewWindow(target);
  }

  function onMenuKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") closeMenu();
  }

  // --- drag-to-spawn ----------------------------------------------------
  // The HTML5 drag API is window-bound — once the cursor crosses the
  // window edge there's no native "where did the drop land" signal we
  // can read from the browser. After dragend we ask the Rust side to
  // compare the OS cursor position against the source window's outer
  // rect (both in physical pixels, see commands::window::cursor_outside_window)
  // and spawn a new window if the drop landed outside.

  // Tracks the in-flight drag's profile so we can hand it to
  // openProfileWindow on dragend without re-resolving from the DOM.
  let dragging = $state<Profile | null>(null);

  function onDragStart(e: DragEvent, p: Profile) {
    if (!e.dataTransfer) return;
    dragging = p;
    e.dataTransfer.effectAllowed = "copy";
    // Custom MIME only — no DE recognizes this so dropping outside
    // the window falls through to dragend (where we ask the
    // backend whether the cursor is outside and spawn a new
    // window if so). An earlier revision also set
    // `text/plain`, which on Linux GTK / Wayland file managers
    // looks like a draggable text snippet — dropping on the
    // desktop creates a `.txt` file with the profile name and the
    // OS consumes the drop before our dragend gets to fire, so no
    // new window opens. Keeping only the custom MIME avoids that.
    e.dataTransfer.setData("application/x-baudrun-profile", p.id);
  }

  async function onDragEnd() {
    const profile = dragging;
    dragging = null;
    if (!profile) return;
    try {
      const outside = await api.cursorOutsideWindow();
      if (!outside) return;
      // Same path as right-click → "Open in new window". App.svelte
      // decides whether to also migrate the live session and carry
      // over the terminal scrollback.
      onOpenInNewWindow(profile);
    } catch (err) {
      console.warn("drag-end check failed:", err);
    }
  }
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
            class:dragging={dragging?.id === p.id}
            draggable="true"
            onclick={() => onSelect(p.id)}
            oncontextmenu={(e) => openMenu(e, p)}
            ondragstart={(e) => onDragStart(e, p)}
            ondragend={onDragEnd}
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

<!-- Top-level <svelte:window> per Svelte's "no inside blocks" rule.
     The handlers no-op when no menu is open, which is the common
     case, so the always-bound listeners cost nothing. -->
<svelte:window
  onclick={() => {
    if (menu) closeMenu();
  }}
  onkeydown={onMenuKeydown}
/>

{#if menu}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="ctx-menu"
    style:left="{menu.x}px"
    style:top="{menu.y}px"
    role="menu"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={onMenuKeydown}
  >
    <button
      role="menuitem"
      class="ctx-item"
      onclick={chooseOpenInNewWindow}
    >Open profile in new window</button>
  </div>
{/if}

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

  /* Subtle visual cue while a drag is in flight. The OS-provided drag
     image still appears; this just dims the source row so the user
     can see which one is being lifted. */
  .row.dragging {
    opacity: 0.45;
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
    /* flex: 1 so the empty-state fills the gap between the header
       and the footer Settings button — without this the empty div
       only takes its content height and Settings hugs it instead
       of pinning to the bottom (issue #8). */
    flex: 1;
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

  /* Floating context menu for right-click → "Open in new window".
     Positioned at the cursor coords via inline styles in the
     markup; styling here only handles look + stacking. */
  .ctx-menu {
    position: fixed;
    z-index: 10000;
    min-width: 200px;
    background: var(--option-bg, var(--bg-panel));
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-floating, 0 10px 30px rgba(0, 0, 0, 0.35));
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .ctx-item {
    text-align: left;
    padding: 7px 10px;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--option-fg, var(--fg-primary));
    font-size: 13px;
    font-family: inherit;
  }

  .ctx-item:hover {
    background: var(--bg-hover);
  }
</style>
