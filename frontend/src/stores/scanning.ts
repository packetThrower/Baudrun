import { writable } from "svelte/store";

// True while the profile form is enumerating serial ports and
// querying USB driver status. Surfaced as a status-bar indicator
// so users get feedback during the ~1-2s Windows PowerShell scan
// (and the faster macOS ioreg / Linux sysfs scans, which usually
// flash by too quickly to notice).
export const portScanning = writable<boolean>(false);
