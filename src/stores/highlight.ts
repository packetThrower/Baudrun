import { writable, get } from "svelte/store";

import { api, type HighlightPack, type HighlightRule } from "../lib/api";
import { setActiveRules } from "../lib/highlight";

/**
 * Every pack the backend reported (bundled presets + the user pack
 * at $SUPPORT_DIR/highlight-rules.json). Sorted by source then id
 * — bundled first, user pack last, so the Settings UI shows the
 * canonical defaults above the editable slot.
 */
export const highlightPacks = writable<HighlightPack[]>([]);

/**
 * Load packs from the backend and update the store. Safe to call
 * any time. Does NOT recompile the active rule set on its own —
 * callers pair this with `applyEnabledHighlightPresets` once the
 * Settings have been read.
 */
export async function loadHighlightPacks(): Promise<void> {
  try {
    const packs = await api.listHighlightPacks();
    highlightPacks.set(packs ?? []);
  } catch (err) {
    console.error("load highlight packs", err);
    highlightPacks.set([]);
  }
}

/**
 * Recompile the active regex set from the currently-loaded packs,
 * picking only the packs whose ids are in `enabled`. Empty `enabled`
 * is "no highlighting" — explicit opt-out, not fallback.
 *
 * Each pack's rules are concatenated in pack-order, then within each
 * pack in rule-order. The matcher in highlight.ts is first-match-
 * wins, so order matters: bundled defaults first, user overrides
 * last (so a user rule wins over a default).
 */
export function applyEnabledHighlightPresets(enabled: string[] | undefined): void {
  const packs = get(highlightPacks);
  const set = new Set(enabled ?? []);
  const rules: HighlightRule[] = [];
  for (const pack of packs) {
    if (!set.has(pack.id)) continue;
    rules.push(...pack.rules);
  }
  setActiveRules(rules);
}
