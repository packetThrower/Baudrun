// Helpers for displaying port identifiers in the UI. Port names
// come in two shapes:
//
//   - OS-native: "/dev/cu.usbmodem01", "COM3", etc. Already human-
//     readable; show as-is.
//   - Libusb-direct: "usb:VID:PID[:serial]". Internal plumbing the
//     backend uses to re-find a device on open. Not something to
//     put in a dropdown or header subtitle verbatim — compress to
//     "USB · VID:PID".
//
// formatPortName is the single entry point; the select-label
// builder in ProfileForm also calls it.

/** Pretty-print a port identifier for display. */
export function formatPortName(name: string, vid = "", pid = ""): string {
  if (!name.startsWith("usb:")) return name;
  if (vid && pid) return `USB · ${vid}:${pid}`;
  const tail = name.slice(4); // "VID:PID" or "VID:PID:serial"
  const parts = tail.split(":");
  if (parts.length >= 2) return `USB · ${parts[0]}:${parts[1]}`;
  return `USB · ${tail}`;
}
