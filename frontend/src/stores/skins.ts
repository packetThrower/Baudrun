import { writable, get } from "svelte/store";
import { api, type Skin } from "../lib/api";

export const skins = writable<Skin[]>([]);
export const activeSkinID = writable<string>("seriesly");

// "auto" follows `prefers-color-scheme`; "light" / "dark" pin.
export type Appearance = "auto" | "light" | "dark";
export const appearance = writable<Appearance>("auto");

// Tracks the system preference via matchMedia; updates whenever the OS flips
// between light and dark while the app is running.
export const systemIsDark = writable<boolean>(
  typeof window !== "undefined"
    ? window.matchMedia("(prefers-color-scheme: dark)").matches
    : true,
);

if (typeof window !== "undefined") {
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  mq.addEventListener("change", (e) => systemIsDark.set(e.matches));
}

// Track which CSS custom-property names we've written, so we can reliably
// clear them before applying a new skin or switching modes.
let managedProps = new Set<string>();

export async function loadSkins() {
  const list = await api.listSkins();
  skins.set(list ?? []);
}

export function effectiveMode(
  pref: Appearance,
  isDark: boolean,
): "light" | "dark" {
  if (pref === "light") return "light";
  if (pref === "dark") return "dark";
  return isDark ? "dark" : "light";
}

export function applySkin(
  skin: Skin | undefined,
  pref: Appearance,
  isDark: boolean,
) {
  const root = document.documentElement.style;

  for (const k of managedProps) root.removeProperty(k);
  managedProps = new Set();

  if (!skin) {
    delete document.documentElement.dataset.skin;
    delete document.documentElement.dataset.mode;
    return;
  }

  // Dark-only skins (CRT, potentially Matrix/Synthwave later) ignore the
  // global appearance preference and always render in their dark palette.
  const mode = skin.supportsLight ? effectiveMode(pref, isDark) : "dark";

  // Expose skin ID and mode as DOM attributes so per-skin CSS selectors
  // can target element-level styling that goes beyond palette swaps
  // (e.g., an XP Luna Start-button look on the Settings footer button).
  document.documentElement.dataset.skin = skin.id;
  document.documentElement.dataset.mode = mode;

  const write = (map: Record<string, string> | undefined) => {
    if (!map) return;
    for (const [k, v] of Object.entries(map)) {
      if (!k.startsWith("--")) continue;
      root.setProperty(k, v);
      managedProps.add(k);
    }
  };

  write(skin.vars);
  write(mode === "dark" ? skin.darkVars : skin.lightVars);
}

export function resolveSkin(id: string, all: Skin[]): Skin | undefined {
  return (
    all.find((s) => s.id === id) ??
    all.find((s) => s.id === "seriesly") ??
    all[0]
  );
}

export async function importSkin(): Promise<Skin | null> {
  try {
    const s = await api.importSkin();
    await loadSkins();
    return s;
  } catch (e) {
    const msg = String(e);
    if (msg.includes("cancelled")) return null;
    throw e;
  }
}

export async function deleteSkin(id: string): Promise<void> {
  await api.deleteSkin(id);
  await loadSkins();
}

export function currentSkin(): Skin | undefined {
  return resolveSkin(get(activeSkinID), get(skins));
}
