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

  // Popover position, computed from the trigger's viewport rect
  // on open. The popover is portaled to document.body so it
  // escapes every ancestor stacking context (panels with
  // backdrop-filter, transforms, filters, etc. each create their
  // own context that z-index can't climb out of) — hence the
  // `position: fixed` + explicit coords. `placement` flips the
  // popover above the trigger when there isn't room below.
  type PopoverPos = {
    top: number;
    left: number;
    width: number;
    maxHeight: number;
    placement: "bottom" | "top";
  };
  let pos = $state<PopoverPos>({
    top: 0,
    left: 0,
    width: 0,
    maxHeight: 240,
    placement: "bottom",
  });

  function computePos() {
    if (!triggerEl) return;
    const r = triggerEl.getBoundingClientRect();
    const gap = 4;
    const desired = 240;
    const below = window.innerHeight - r.bottom - gap - 8;
    const above = r.top - gap - 8;
    const placement: "bottom" | "top" =
      below >= Math.min(desired, 120) || below >= above ? "bottom" : "top";
    const avail = placement === "bottom" ? below : above;
    const maxHeight = Math.max(120, Math.min(desired, avail));
    const top =
      placement === "bottom" ? r.bottom + gap : r.top - gap - maxHeight;
    pos = {
      top,
      left: r.left,
      width: r.width,
      maxHeight,
      placement,
    };
  }

  // Action: move a node to document.body on mount, restore on
  // destroy. Used on the popover so z-index can actually reach
  // above other stacked content regardless of where the <Select>
  // sits in the DOM tree.
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return {
      destroy() {
        if (node.parentNode === document.body) {
          document.body.removeChild(node);
        }
      },
    };
  }

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
    computePos();
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
  // trigger positions when the user scrolls or resizes. Scroll
  // listener needs capture-phase handling (see $effect below) —
  // scroll events from a scrollable ancestor (.scroll in Settings /
  // ProfileForm) don't bubble to window, so the svelte:window
  // handler alone would miss them and the popover would hang
  // around while the trigger scrolls out from under it.
  function onWindowEvent() {
    if (open) closeList();
  }

  $effect(() => {
    const handler = (e: Event) => {
      if (!open) return;
      // Scrolling inside the popover's own list (long option set
      // with a scrollbar) is fine — only ancestor scrolls should
      // close us, because those move the trigger out from under
      // the popover. Check the target before closing.
      const target = e.target as Node | null;
      if (target && listEl?.contains(target)) return;
      closeList();
    };
    window.addEventListener("scroll", handler, true);
    return () => window.removeEventListener("scroll", handler, true);
  });

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

</div>

{#if open}
  <ul
    class="select-list"
    class:placement-top={pos.placement === "top"}
    role="listbox"
    bind:this={listEl}
    tabindex="-1"
    aria-activedescendant={highlightedIndex >= 0 ? `opt-${highlightedIndex}` : undefined}
    use:portal
    style:top="{pos.top}px"
    style:left="{pos.left}px"
    style:width="{pos.width}px"
    style:max-height="{pos.maxHeight}px"
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

  /* Popover. Portaled to document.body so z-index can actually
     reach above adjacent stacked content (panels with
     backdrop-filter create their own stacking contexts, and
     z-index inside one of those can't climb out). position:
     fixed + explicit top/left/width set via inline styles from
     the trigger's viewport rect at open time; :global so the
     scoped styles still apply once the node lives under <body>.

     Uses --option-bg / --option-fg / --option-group-fg — the
     existing skin-defined variables for dropdown popups. Every
     built-in and documented custom skin sets these to solid
     colors (they used to back the OS's native <option> popup
     fallback); using them here gives the custom listbox an
     opaque, skin-appropriate surface automatically. */
  :global(.select-list) {
    position: fixed;
    z-index: 9999;
    overflow-y: auto;
    margin: 0;
    padding: 4px;
    list-style: none;
    background: var(--option-bg);
    color: var(--option-fg, var(--fg-primary));
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-floating, 0 10px 30px rgba(0, 0, 0, 0.4));
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
  }

  /* :global on every rule inside the portaled popover — the
     listbox lives under <body> after the portal action moves it,
     outside this component's scoped-style reach. Scoped rules
     would emit a hashed selector that no longer matches. */
  :global(.select-option) {
    padding: 6px 8px;
    color: var(--option-fg, var(--fg-primary));
    border-radius: var(--radius-sm);
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  :global(.select-option.in-group) {
    padding-left: 18px;
  }
  :global(.select-option.highlighted) {
    background: var(--bg-hover);
  }
  :global(.select-option.selected) {
    color: var(--accent);
    font-weight: 500;
  }
  :global(.select-option.selected.highlighted) {
    background: var(--accent-muted, var(--bg-hover));
    color: var(--accent);
  }
  :global(.select-option.disabled) {
    color: var(--fg-tertiary);
    cursor: not-allowed;
  }

  :global(.select-group) {
    padding: 8px 8px 4px 8px;
    font-family: var(--font-ui);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--option-group-fg, var(--fg-tertiary));
    pointer-events: none;
  }
  :global(.select-group:first-child) {
    padding-top: 4px;
  }

  @media (prefers-reduced-motion: reduce) {
    .select-trigger,
    .select-chevron {
      transition: none;
    }
  }
</style>
