import { writable } from "svelte/store";

// Session-scoped set of dismissed driver-banner keys.
// Key format: "vid:pid:serialNumber". Clears on app restart.
export const dismissedDrivers = writable<Set<string>>(new Set());

export function dismissDriver(key: string) {
  dismissedDrivers.update((s) => {
    const next = new Set(s);
    next.add(key);
    return next;
  });
}

export function driverKey(d: {
  vid: string;
  pid: string;
  serialNumber?: string;
}): string {
  return `${d.vid}:${d.pid}:${d.serialNumber ?? ""}`;
}
