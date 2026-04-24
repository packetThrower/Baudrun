<script lang="ts">
  // Click-to-record keyboard-shortcut capture widget. Used by the
  // Settings → Keyboard Shortcuts section to let users override
  // the default bindings for Clear / Send Break / Suspend.
  //
  // Flow: user clicks the button → "recording" state → the next
  // non-modifier keydown is captured as the new spec and flushed
  // via onchange. Escape cancels. Clicking outside cancels. Pure
  // modifier presses (Shift, Ctrl, Alt, Meta alone) are ignored so
  // the capture loop keeps waiting for a real key.
  //
  // Key capture uses a window-level listener in the capture phase,
  // not an element-level handler on the button. Two reasons:
  //   1. WKWebView on macOS follows the Safari convention where a
  //      plain click doesn't focus a <button>, so an
  //      element-level onkeydown on the button wouldn't ever
  //      fire on Wails-on-macOS.
  //   2. Capture phase puts our handler ahead of App.svelte's
  //      window-bubble keydown, so recording can't be stolen by
  //      the existing shortcut dispatcher if the user happens to
  //      press the currently-bound combo for Clear / Break /
  //      Suspend while re-binding it.

  import { formatShortcut, specFromEvent } from "./shortcuts";

  type Props = {
    value: string;
    onchange: (value: string) => void;
    onreset?: () => void;
    id?: string;
    isMac: boolean;
    placeholder?: string;
  };

  let {
    value,
    onchange,
    onreset,
    id,
    isMac,
    placeholder = "Press a key…",
  }: Props = $props();

  let recording = $state(false);
  let rootEl = $state<HTMLDivElement | null>(null);

  const label = $derived(formatShortcut(value, isMac));

  function startRecording() {
    recording = true;
  }

  function cancelRecording() {
    recording = false;
  }

  function handleReset(e: MouseEvent) {
    e.stopPropagation();
    if (onreset) onreset();
  }

  // Window-level keydown listener, active only while recording.
  $effect(() => {
    if (!recording) return;
    const handler = (e: KeyboardEvent) => {
      // Claim the event unconditionally during recording so App's
      // own dispatcher can't fire the currently-bound action for
      // this combo (e.g. rebinding Clear by pressing ⌘K).
      e.preventDefault();
      e.stopPropagation();

      if (e.key === "Escape") {
        cancelRecording();
        return;
      }
      const spec = specFromEvent(e);
      if (!spec) return; // pure-modifier press; keep listening
      recording = false;
      onchange(spec);
    };
    // Capture phase so we beat App.svelte's svelte:window handler
    // which is attached at the bubble phase.
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  });

  // Cancel recording if the user clicks outside the widget.
  $effect(() => {
    if (!recording) return;
    const handler = (e: MouseEvent) => {
      const target = e.target as Node | null;
      if (target && rootEl?.contains(target)) return;
      cancelRecording();
    };
    window.addEventListener("mousedown", handler, true);
    return () => window.removeEventListener("mousedown", handler, true);
  });
</script>

<div class="keycapture" bind:this={rootEl}>
  <button
    type="button"
    class="capture-btn"
    class:recording
    {id}
    onclick={startRecording}
    aria-label={recording ? placeholder : `Shortcut: ${label}. Click to change.`}
  >
    {recording ? placeholder : label || "—"}
  </button>
  {#if onreset}
    <button
      type="button"
      class="reset-btn"
      onclick={handleReset}
      title="Reset to default"
      aria-label="Reset shortcut to default"
    >
      ↺
    </button>
  {/if}
</div>

<style>
  .keycapture {
    display: flex;
    gap: 4px;
    align-items: center;
  }

  .capture-btn {
    min-width: 120px;
    height: 30px;
    padding: 0 10px;
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    color: var(--fg-primary);
    background: var(--bg-input);
    border: 1px solid var(--input-border-idle);
    border-radius: var(--radius-md);
    cursor: pointer;
    text-align: left;
    outline: none;
    transition: background 0.12s, border-color 0.12s;
    /* Monospaced glyphs in the label so ⌘⇧B and Ctrl+Shift+B
       line up column-wise across a grid of rows. */
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.02em;
  }

  .capture-btn:hover:not(.recording) {
    background: var(--bg-hover);
  }
  .capture-btn:focus-visible,
  .capture-btn.recording {
    background: var(--bg-input-focus);
    border-color: var(--accent);
  }
  .capture-btn.recording {
    font-style: italic;
    color: var(--fg-secondary);
  }

  .reset-btn {
    width: 28px;
    height: 30px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 15px;
    color: var(--fg-secondary);
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    cursor: pointer;
    transition: background 0.12s, color 0.12s;
  }
  .reset-btn:hover {
    background: var(--bg-hover);
    color: var(--fg-primary);
  }
</style>
