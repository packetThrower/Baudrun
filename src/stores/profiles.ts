import { writable, get } from "svelte/store";
import { api, type Profile } from "../lib/api";

export const profiles = writable<Profile[]>([]);
export const selectedProfileID = writable<string | null>(null);

export async function loadProfiles() {
  const list = await api.listProfiles();
  profiles.set(list ?? []);
  const current = get(selectedProfileID);
  if (current && !list.some((p) => p.id === current)) {
    selectedProfileID.set(list[0]?.id ?? null);
  } else if (!current && list.length > 0) {
    selectedProfileID.set(list[0].id);
  }
}

export async function createProfile(p: Profile): Promise<Profile> {
  const created = await api.createProfile(p);
  profiles.update((list) => [...list, created]);
  selectedProfileID.set(created.id);
  return created;
}

export async function updateProfile(p: Profile): Promise<Profile> {
  const updated = await api.updateProfile(p);
  profiles.update((list) => list.map((x) => (x.id === updated.id ? updated : x)));
  return updated;
}

export async function deleteProfile(id: string): Promise<void> {
  await api.deleteProfile(id);
  profiles.update((list) => list.filter((p) => p.id !== id));
  const current = get(selectedProfileID);
  if (current === id) {
    const remaining = get(profiles);
    selectedProfileID.set(remaining[0]?.id ?? null);
  }
}
