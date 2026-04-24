// Keyboard shortcut helpers — parsing, matching DOM events, and
// user-facing display formatting.
//
// Shortcut strings are stored and passed around in W3C
// aria-keyshortcuts format: modifier tokens + a key, joined by
// `+`. Matches what the DOM hands us and what assistive tech
// expects on aria-keyshortcuts attributes, so we don't have to
// translate at the DOM boundary.
//
// Examples:
//   "Meta+K"             — Cmd+K
//   "Control+Shift+B"    — Ctrl+Shift+B
//   "Control+Shift+K"    — Ctrl+Shift+K

/** Canonical action names for the shortcut table. */
export type ShortcutAction = "clear" | "break" | "suspend";

/** Parsed form: which modifiers are required and the main key. */
export interface ShortcutSpec {
  ctrl: boolean;
  meta: boolean;
  shift: boolean;
  alt: boolean;
  key: string; // lowercase, e.g. "k", "b", "s", "arrowup"
}

const MODIFIER_TOKENS = new Set(["control", "meta", "shift", "alt"]);

/**
 * Parse a `Control+Shift+K` style spec into the typed form used
 * by match() / format(). Unknown modifier tokens are ignored so a
 * malformed stored value can't crash the shortcut subsystem.
 */
export function parseShortcut(spec: string): ShortcutSpec | null {
  if (!spec) return null;
  const parts = spec.split("+");
  const out: ShortcutSpec = {
    ctrl: false,
    meta: false,
    shift: false,
    alt: false,
    key: "",
  };
  for (const raw of parts) {
    const tok = raw.trim().toLowerCase();
    if (!tok) continue;
    if (MODIFIER_TOKENS.has(tok)) {
      if (tok === "control") out.ctrl = true;
      else if (tok === "meta") out.meta = true;
      else if (tok === "shift") out.shift = true;
      else if (tok === "alt") out.alt = true;
    } else {
      // Last non-modifier token wins; anything with a key parse
      // error above bubbles through unchanged.
      out.key = tok;
    }
  }
  return out.key ? out : null;
}

/**
 * True when a DOM KeyboardEvent matches spec. Modifier state has
 * to match exactly — a shortcut bound to `Meta+K` won't fire for
 * `Meta+Shift+K`, so adding Shift to a Mac user's Clear binding
 * still lets them send actual VT bytes through the passthrough.
 */
export function matchesShortcut(
  event: KeyboardEvent,
  spec: ShortcutSpec | string | null,
): boolean {
  const s = typeof spec === "string" ? parseShortcut(spec) : spec;
  if (!s) return false;
  if (event.ctrlKey !== s.ctrl) return false;
  if (event.metaKey !== s.meta) return false;
  if (event.shiftKey !== s.shift) return false;
  if (event.altKey !== s.alt) return false;
  return event.key.toLowerCase() === s.key;
}

/**
 * Turn a spec into the user-facing label — `⌘K`, `Ctrl+Shift+B`,
 * etc. On macOS we fold modifiers into the standard Apple glyphs
 * + no separator; on everything else we keep `Ctrl+Shift+K`
 * verbose form which is more familiar to Linux and Windows users.
 */
export function formatShortcut(
  spec: ShortcutSpec | string | null,
  isMac: boolean,
): string {
  const s = typeof spec === "string" ? parseShortcut(spec) : spec;
  if (!s) return "";
  if (isMac) {
    let out = "";
    if (s.ctrl) out += "⌃";
    if (s.alt) out += "⌥";
    if (s.shift) out += "⇧";
    if (s.meta) out += "⌘";
    return out + s.key.toUpperCase();
  }
  const parts: string[] = [];
  if (s.ctrl) parts.push("Ctrl");
  if (s.meta) parts.push("Win"); // fallback label, rarely used here
  if (s.alt) parts.push("Alt");
  if (s.shift) parts.push("Shift");
  parts.push(s.key.length === 1 ? s.key.toUpperCase() : s.key);
  return parts.join("+");
}

/**
 * Build a spec string from a live KeyboardEvent, for "click to
 * record" capture widgets. Returns null for pure-modifier presses
 * (Shift by itself, etc.) so the capture loop keeps waiting for a
 * real key.
 */
export function specFromEvent(event: KeyboardEvent): string | null {
  if (
    event.key === "Control" ||
    event.key === "Meta" ||
    event.key === "Shift" ||
    event.key === "Alt"
  ) {
    return null;
  }
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Control");
  if (event.metaKey) parts.push("Meta");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  // Letter keys: uppercase single char. Named keys: keep the DOM
  // identifier as-is ("Enter", "ArrowUp") so display logic can
  // distinguish later.
  const k = event.key;
  parts.push(k.length === 1 ? k.toUpperCase() : k);
  return parts.join("+");
}

/** Single source of truth for what actions can have a shortcut. */
export const SHORTCUT_ACTIONS: ShortcutAction[] = [
  "clear",
  "break",
  "suspend",
];

/** User-visible labels for each action, used in the settings UI. */
export const SHORTCUT_LABELS: Record<ShortcutAction, string> = {
  clear: "Clear terminal",
  break: "Send Break",
  suspend: "Suspend session",
};

/** Platform-appropriate defaults — picked lazily so the backend
 * can stay platform-agnostic. */
export const DEFAULT_SHORTCUTS_MAC: Record<ShortcutAction, string> = {
  clear: "Meta+K",
  break: "Meta+Shift+B",
  suspend: "Meta+Shift+S",
};
export const DEFAULT_SHORTCUTS_OTHER: Record<ShortcutAction, string> = {
  clear: "Control+Shift+K",
  break: "Control+Shift+B",
  suspend: "Control+Shift+S",
};

/**
 * Resolve the effective spec for an action: user override if set,
 * platform default otherwise. Empty string in the override map is
 * treated the same as unset — keeps the "reset to default"
 * affordance in the UI from having to actually delete the map entry.
 */
export function effectiveShortcut(
  action: ShortcutAction,
  overrides: Record<string, string> | undefined,
  isMac: boolean,
): string {
  const override = overrides?.[action];
  if (override && override.trim()) return override;
  return (isMac ? DEFAULT_SHORTCUTS_MAC : DEFAULT_SHORTCUTS_OTHER)[action];
}
