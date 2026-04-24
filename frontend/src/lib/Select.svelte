<script lang="ts" module>
  // Shared types — exported so callers can annotate the options
  // array they pass in without re-declaring these shapes.
  export type SelectOption = {
    value: string | number;
    label: string;
    disabled?: boolean;
  };
  export type SelectGroup = {
    label: string;
    options: SelectOption[];
  };
  // Mixed array — most callers pass just SelectOption[], but the
  // theme picker wants a standalone "Default —" option above its
  // Built-in / Custom groups. Allowing both in one array keeps the
  // call site shape simple.
  export type SelectItems = Array<SelectOption | SelectGroup>;
</script>

<script lang="ts">
  import { tick } from "svelte";

  // A custom <select> replacement. The native <select> renders its
  // popup list via the host OS (GTK on Linux, Chromium's own on
  // Windows, the system popup on macOS), and those popups do NOT
  // respect CSS — so the dropdown list won't follow our light/dark
  // theme or skin palette. This component keeps the trigger looking
  // identical to a native <select> but renders the popover itself,
  // so skins and themes reach all the way into the list.
  //
  // Keyboard model matches native <select>:
  //   - Space / Enter / Alt+ArrowDown on the trigger: open
  //   - Escape: close without committing
  //   - Enter on an option: commit + close
  //   - ArrowUp / ArrowDown: move highlight
  //   - Home / End: first / last
  //   - Typeahead: letter keys jump to matching option
  //   - Tab: close + move focus away

  type Props = {
    value: string | number;
    options: SelectItems;
    onchange?: (value: string | number) => void;
    id?: string;
    disabled?: boolean;
    // Shown when value doesn't match any option's value. Keeps the
    // control from looking empty in the "unselected" case.
    placeholder?: string;
    // Propagated to the trigger for screen-reader labelling.
    "aria-label"?: string;
  };

  let {
    value = $bindable(),
    options,
    onchange,
    id,
    disabled = false,
    placeholder = "",
    "aria-label": ariaLabel,
  }: Props = $props();

  let open = $state(false);
  let highlightedIndex = $state(-1);
  let triggerEl = $state<HTMLButtonElement | null>(null);
  let listEl = $state<HTMLUListElement | null>(null);
  let typeaheadBuffer = "";
  let typeaheadTimer: ReturnType<typeof setTimeout> | null = null;

  // Flatten groups into a single ordered list for keyboard nav and
  // option lookup. Each entry remembers which group it came from so
  // the rendered list can still draw the group-label separators.
  type FlatOption = {
    value: string | number;
    label: string;
    disabled: boolean;
    groupLabel?: string;
  };

  function isGroup(item: SelectOption | SelectGroup): item is SelectGroup {
    return "options" in item && Array.isArray(item.options);
  }

  const flat = $derived.by<FlatOption[]>(() => {
    const out: FlatOption[] = [];
    for (const item of options) {
      if (isGroup(item)) {
        for (const opt of item.options) {
          out.push({
            value: opt.value,
            label: opt.label,
            disabled: !!opt.disabled,
            groupLabel: item.label,
          });
        }
      } else {
        out.push({
          value: item.value,
          label: item.label,
          disabled: !!item.disabled,
        });
      }
    }
    return out;
  });

  const selectedIndex = $derived(
    flat.findIndex((o) => o.value === value),
  );
  const selectedLabel = $derived(
    selectedIndex >= 0 ? flat[selectedIndex].label : placeholder,
  );

  async function openList() {
    if (disabled) return;
    open = true;
    highlightedIndex = selectedIndex >= 0 ? selectedIndex : firstEnabledIndex();
    await tick();
    scrollHighlightIntoView();
  }

  function closeList() {
    open = false;
    highlightedIndex = -1;
  }

  function toggle() {
    if (open) closeList();
    else void openList();
  }

  function commit(index: number) {
    const opt = flat[index];
    if (!opt || opt.disabled) return;
    const changed = opt.value !== value;
    value = opt.value;
    closeList();
    triggerEl?.focus();
    if (changed) onchange?.(opt.value);
  }

  function firstEnabledIndex(): number {
    return flat.findIndex((o) => !o.disabled);
  }

  function lastEnabledIndex(): number {
    for (let i = flat.length - 1; i >= 0; i--) {
      if (!flat[i].disabled) return i;
    }
    return -1;
  }

  function step(delta: 1 | -1, from: number): number {
    let i = from;
    for (let guard = 0; guard < flat.length; guard++) {
      i = (i + delta + flat.length) % flat.length;
      if (!flat[i].disabled) return i;
    }
    return from;
  }

  function onTriggerKey(e: KeyboardEvent) {
    if (disabled) return;
    if (!open) {
      if (
        e.key === "Enter" ||
        e.key === " " ||
        e.key === "ArrowDown" ||
        e.key === "ArrowUp" ||
        (e.altKey && e.key === "ArrowDown")
      ) {
        e.preventDefault();
        void openList();
      }
      return;
    }

    if (e.key === "Escape") {
      e.preventDefault();
      closeList();
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      if (highlightedIndex >= 0) commit(highlightedIndex);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      highlightedIndex = step(1, highlightedIndex);
      scrollHighlightIntoView();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      highlightedIndex = step(-1, highlightedIndex);
      scrollHighlightIntoView();
    } else if (e.key === "Home") {
      e.preventDefault();
      highlightedIndex = firstEnabledIndex();
      scrollHighlightIntoView();
    } else if (e.key === "End") {
      e.preventDefault();
      highlightedIndex = lastEnabledIndex();
      scrollHighlightIntoView();
    } else if (e.key === "Tab") {
      closeList();
      // don't preventDefault — let focus proceed naturally
    } else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
      handleTypeahead(e.key);
    }
  }

  function handleTypeahead(ch: string) {
    typeaheadBuffer += ch.toLowerCase();
    if (typeaheadTimer) clearTimeout(typeaheadTimer);
    typeaheadTimer = setTimeout(() => {
      typeaheadBuffer = "";
      typeaheadTimer = null;
    }, 600);
    const start = highlightedIndex >= 0 ? highlightedIndex : 0;
    for (let offset = 0; offset < flat.length; offset++) {
      const i = (start + offset) % flat.length;
      const opt = flat[i];
      if (opt.disabled) continue;
      if (opt.label.toLowerCase().startsWith(typeaheadBuffer)) {
        highlightedIndex = i;
        scrollHighlightIntoView();
        return;
      }
    }
  }

  function scrollHighlightIntoView() {
    if (!listEl || highlightedIndex < 0) return;
    const el = listEl.querySelector<HTMLElement>(
      `[data-index="${highlightedIndex}"]`,
    );
    el?.scrollIntoView({ block: "nearest" });
  }

  function onOutsidePointer(e: MouseEvent) {
    if (!open) return;
    const target = e.target as Node | null;
    if (
      target &&
      !triggerEl?.contains(target) &&
      !listEl?.contains(target)
    ) {
      closeList();
    }
  }

  // Close on window events so the popover doesn't float over stale
  // trigger positions when the user scrolls or resizes.
  function onWindowEvent() {
    if (open) closeList();
  }

  // Only one listbox should be open at a time; selecting via mouse
  // also commits.
  function onOptionMouseDown(e: MouseEvent, index: number) {
    // preventDefault to stop the trigger from losing focus before
    // we commit — a focus switch would otherwise trip the
    // outside-pointer handler and close us in a racy way.
    e.preventDefault();
    commit(index);
  }

  function onOptionMouseEnter(index: number) {
    if (!flat[index]?.disabled) highlightedIndex = index;
  }

  // Group boundaries: emit a group-label row before the first
  // option of each group. Tracked via a simple scan in the markup.
  const groupBoundaries = $derived.by<Set<number>>(() => {
    const set = new Set<number>();
    let prev: string | undefined = undefined;
    for (let i = 0; i < flat.length; i++) {
      const g = flat[i].groupLabel;
      if (g && g !== prev) set.add(i);
      prev = g;
    }
    return set;
  });
</script>

<svelte:window
  onmousedown={onOutsidePointer}
  onscroll={onWindowEvent}
  onresize={onWindowEvent}
/>

<div class="select-wrap" class:open class:disabled>
  <button
    type="button"
    class="select-trigger"
    {id}
    {disabled}
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label={ariaLabel}
    bind:this={triggerEl}
    onclick={toggle}
    onkeydown={onTriggerKey}
  >
    <span class="select-label" class:placeholder={selectedIndex < 0}>
      {selectedLabel}
    </span>
    <svg class="select-chevron" viewBox="0 0 10 6" width="10" height="6" aria-hidden="true">
      <path d="M1 1l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
  </button>

  {#if open}
    <ul
      class="select-list"
      role="listbox"
      bind:this={listEl}
      tabindex="-1"
      aria-activedescendant={highlightedIndex >= 0 ? `opt-${highlightedIndex}` : undefined}
    >
      {#each flat as opt, i (i)}
        {#if groupBoundaries.has(i)}
          <li class="select-group" role="presentation">{opt.groupLabel}</li>
        {/if}
        <li
          id={`opt-${i}`}
          class="select-option"
          class:selected={value === opt.value}
          class:highlighted={highlightedIndex === i}
          class:disabled={opt.disabled}
          class:in-group={!!opt.groupLabel}
          role="option"
          aria-selected={value === opt.value}
          aria-disabled={opt.disabled}
          data-index={i}
          onmousedown={(e) => onOptionMouseDown(e, i)}
          onmouseenter={() => onOptionMouseEnter(i)}
        >
          {opt.label}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .select-wrap {
    position: relative;
    width: 100%;
  }

  /* Trigger styled to match the native <select> closed state pixel-for-pixel
     so the visual footprint is identical whether the field is focused or
     not. Same CSS vars feed both, so skin changes flow through. */
  .select-trigger {
    width: 100%;
    height: 30px;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
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
  }

  .select-trigger:hover:not(:disabled) {
    background: var(--bg-hover);
  }
  .select-trigger:focus-visible {
    background: var(--bg-input-focus);
    border-color: var(--accent);
  }
  .select-wrap.open .select-trigger {
    background: var(--bg-input-focus);
    border-color: var(--accent);
  }
  .select-trigger:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .select-label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .select-label.placeholder {
    color: var(--fg-tertiary);
  }

  .select-chevron {
    flex-shrink: 0;
    color: var(--fg-secondary);
    transition: transform 0.15s ease;
  }
  .select-wrap.open .select-chevron {
    transform: rotate(180deg);
  }

  /* Popover. Sits directly below the trigger; width matches.
     z-index above terminal/session overlays.

     Uses --option-bg / --option-fg / --option-group-fg — the
     existing skin-defined variables for dropdown popups. Every
     built-in and documented custom skin sets these to solid
     colors (they used to back the OS's native <option> popup
     fallback); using them here gives the custom listbox an
     opaque, skin-appropriate surface automatically, with no
     translucent-panel see-through. */
  .select-list {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    z-index: 120;
    max-height: 240px;
    overflow-y: auto;
    margin: 0;
    padding: 4px;
    list-style: none;
    background: var(--option-bg);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-floating, var(--shadow-panel));
  }

  .select-option {
    padding: 6px 8px;
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    color: var(--option-fg, var(--fg-primary));
    border-radius: var(--radius-sm);
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .select-option.in-group {
    padding-left: 18px;
  }
  .select-option.highlighted {
    background: var(--bg-hover);
  }
  .select-option.selected {
    color: var(--accent);
    font-weight: 500;
  }
  .select-option.selected.highlighted {
    background: var(--accent-muted, var(--bg-hover));
    color: var(--accent);
  }
  .select-option.disabled {
    color: var(--fg-tertiary);
    cursor: not-allowed;
  }

  .select-group {
    padding: 8px 8px 4px 8px;
    font-family: var(--font-ui);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--option-group-fg, var(--fg-tertiary));
    pointer-events: none;
  }
  .select-group:first-child {
    padding-top: 4px;
  }

  @media (prefers-reduced-motion: reduce) {
    .select-trigger,
    .select-chevron {
      transition: none;
    }
  }
</style>
