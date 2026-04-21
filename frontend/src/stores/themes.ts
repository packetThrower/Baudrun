import { writable, derived, get } from "svelte/store";
import { api, type Theme, type Settings } from "../lib/api";

export const themes = writable<Theme[]>([]);
export const settings = writable<Settings>({
  defaultThemeId: "baudrun",
  fontSize: 13,
});

export async function loadThemes() {
  const list = await api.listThemes();
  themes.set(list ?? []);
}

export async function loadSettings() {
  const s = await api.getSettings();
  settings.set(s);
}

export async function importTheme(): Promise<Theme | null> {
  try {
    const t = await api.importTheme();
    await loadThemes();
    return t;
  } catch (e) {
    const msg = String(e);
    if (msg.includes("cancelled")) return null;
    throw e;
  }
}

export async function deleteTheme(id: string): Promise<void> {
  await api.deleteTheme(id);
  const s = get(settings);
  if (s.defaultThemeId === id) {
    await api.updateSettings({ ...s, defaultThemeId: "baudrun" });
    settings.set({ ...s, defaultThemeId: "baudrun" });
  }
  await loadThemes();
}

export async function setDefaultTheme(id: string): Promise<void> {
  const s = get(settings);
  const updated = await api.updateSettings({ ...s, defaultThemeId: id });
  settings.set(updated);
}

export function resolveTheme(id: string, all: Theme[]): Theme | undefined {
  return all.find((t) => t.id === id);
}

// Derived store that gives the effective theme for a given profile themeID.
// Empty themeID means inherit from global default.
export function effectiveTheme(profileThemeID: string): Theme | undefined {
  const all = get(themes);
  const s = get(settings);
  const id = profileThemeID || s.defaultThemeId || "baudrun";
  return all.find((t) => t.id === id) ?? all.find((t) => t.id === "baudrun") ?? all[0];
}
