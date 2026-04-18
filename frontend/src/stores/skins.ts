import { writable, get } from "svelte/store";
import { api, type Skin } from "../lib/api";

export const skins = writable<Skin[]>([]);
export const activeSkinID = writable<string>("seriesly");

// Track which CSS custom-property names we've written, so we can reliably
// clear them before applying a new skin. Without this, switching from a
// richer skin back to a sparser one would leave stale values set on the
// document root.
let managedProps = new Set<string>();

export async function loadSkins() {
  const list = await api.listSkins();
  skins.set(list ?? []);
}

export function applySkin(skin: Skin | undefined) {
  const root = document.documentElement.style;

  // Clear anything we previously set but which this skin doesn't override.
  for (const k of managedProps) {
    root.removeProperty(k);
  }
  managedProps = new Set();

  if (!skin || !skin.vars) return;

  for (const [k, v] of Object.entries(skin.vars)) {
    if (!k.startsWith("--")) continue;
    root.setProperty(k, v);
    managedProps.add(k);
  }
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
