import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

export type Profile = {
  id: string;
  name: string;
  portName: string;
  baudRate: number;
  dataBits: number;
  parity: string;
  stopBits: string;
  flowControl: string;
  lineEnding: string;
  localEcho: boolean;
  highlight: boolean;
  themeId: string;
  dtrOnConnect: string;
  rtsOnConnect: string;
  dtrOnDisconnect: string;
  rtsOnDisconnect: string;
  hexView: boolean;
  timestamps: boolean;
  logEnabled: boolean;
  autoReconnect: boolean;
  backspaceKey: string;
  pasteWarnMultiline: boolean;
  pasteSlow: boolean;
  pasteCharDelayMs?: number;
  /** Per-profile override for global enabledHighlightPresets in Settings.
   * undefined = inherit; defined (even an empty array) = override. */
  enabledHighlightPresets?: string[];
  createdAt: string;
  updatedAt: string;
};

export type PortInfo = {
  name: string;
  isUSB: boolean;
  vid?: string;
  pid?: string;
  serialNumber?: string;
  product?: string;
  chipset?: string;
};

export type USBSerialCandidate = {
  vid: string;
  pid: string;
  chipset: string;
  manufacturer?: string;
  product?: string;
  serialNumber?: string;
  driverURL?: string;
  reason?: string;
};

export type Theme = {
  id: string;
  name: string;
  source: string;
  background: string;
  foreground: string;
  cursor: string;
  cursorAccent?: string;
  selection: string;
  selectionForeground?: string;
  black: string;
  red: string;
  green: string;
  yellow: string;
  blue: string;
  magenta: string;
  cyan: string;
  white: string;
  brightBlack: string;
  brightRed: string;
  brightGreen: string;
  brightYellow: string;
  brightBlue: string;
  brightMagenta: string;
  brightCyan: string;
  brightWhite: string;
};

export type Settings = {
  defaultThemeId: string;
  fontSize?: number;
  logDir?: string;
  disableDriverDetection?: boolean;
  skinId?: string;
  appearance?: string;
  copyOnSelect?: boolean;
  screenReaderMode?: boolean;
  scrollbackLines?: number;
  shortcuts?: Record<string, string>;
  disableUpdateCheck?: boolean;
  includePrereleaseUpdates?: boolean;
  dismissedUpdateVersion?: string;
  enabledHighlightPresets?: string[];
};

export type HighlightColor =
  | "red"
  | "green"
  | "yellow"
  | "blue"
  | "magenta"
  | "cyan"
  | "dim";

export type HighlightRule = {
  pattern: string;
  color: HighlightColor | string;
  ignoreCase?: boolean;
  group?: string;
};

export type HighlightPack = {
  id: string;
  name: string;
  description?: string;
  /** "builtin" or "user" — set by the backend store at load time. */
  source: string;
  rules: HighlightRule[];
};

export type ControlLines = {
  dtr: boolean;
  rts: boolean;
};

export type Skin = {
  id: string;
  name: string;
  source: string;
  description?: string;
  vars: Record<string, string>;
  darkVars?: Record<string, string>;
  lightVars?: Record<string, string>;
  supportsLight: boolean;
};

export const EVT_DATA = "serial:data";
export const EVT_DISCONNECT = "serial:disconnect";
export const EVT_RECONNECTING = "serial:reconnecting";
export const EVT_RECONNECTED = "serial:reconnected";
export const EVT_TRANSFER_PROGRESS = "transfer:progress";
export const EVT_TRANSFER_COMPLETE = "transfer:complete";
export const EVT_TRANSFER_ERROR = "transfer:error";

export type TransferProgress = { sent: number; total: number };
export type TransferProtocol = "xmodem" | "xmodem-crc" | "xmodem-1k" | "ymodem";

export const api = {
  listProfiles: () => invoke<Profile[]>("list_profiles"),
  createProfile: (p: Profile) => invoke<Profile>("create_profile", { profile: p }),
  updateProfile: (p: Profile) => invoke<Profile>("update_profile", { profile: p }),
  deleteProfile: (id: string) => invoke<void>("delete_profile", { id }),
  defaultProfile: () => invoke<Profile>("default_profile"),

  listPorts: () => invoke<PortInfo[]>("list_ports"),
  listMissingDrivers: () => invoke<USBSerialCandidate[]>("list_missing_drivers"),
  connect: (profileId: string) => invoke<void>("connect", { profileId }),
  disconnect: () => invoke<void>("disconnect"),
  activeProfileID: () => invoke<string>("active_profile_id"),
  setRTS: (v: boolean) => invoke<void>("set_rts", { v }),
  setDTR: (v: boolean) => invoke<void>("set_dtr", { v }),
  sendBreak: () => invoke<void>("send_break"),
  pickSendFile: () => invoke<string>("pick_send_file"),
  sendFile: (protocol: TransferProtocol, path: string) =>
    invoke<void>("send_file", { protocol, path }),
  cancelTransfer: () => invoke<void>("cancel_transfer"),

  listThemes: () => invoke<Theme[]>("list_themes"),
  importTheme: () => invoke<Theme>("import_theme"),
  deleteTheme: (id: string) => invoke<void>("delete_theme", { id }),

  listSkins: () => invoke<Skin[]>("list_skins"),
  importSkin: () => invoke<Skin>("import_skin"),
  deleteSkin: (id: string) => invoke<void>("delete_skin", { id }),

  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (s: Settings) => invoke<Settings>("update_settings", { settings: s }),
  pickLogDirectory: () => invoke<string>("pick_log_directory"),
  defaultLogDirectory: () => invoke<string>("default_log_directory"),
  getConfigDirectory: () => invoke<string>("get_config_directory"),
  getDefaultConfigDirectory: () => invoke<string>("get_default_config_directory"),
  pickConfigDirectory: () => invoke<string>("pick_config_directory"),
  setConfigDirectory: (dir: string) => invoke<void>("set_config_directory", { dir }),
  openPath: (path: string) => invoke<void>("open_path", { path }),

  getControlLines: () => invoke<ControlLines>("get_control_lines"),

  setTrafficLightsInset: (x: number, y: number) =>
    invoke<void>("set_traffic_lights_inset", { x, y }),
  openProfileWindow: (profileId: string, profileName?: string) =>
    invoke<string>("open_profile_window", { profileId, profileName }),
  cursorOutsideWindow: () =>
    invoke<boolean>("cursor_outside_window"),
  migrateSession: (targetLabel: string, terminalSnapshot?: string) =>
    invoke<void>("migrate_session", { targetLabel, terminalSnapshot }),
  takePendingTerminalSnapshot: () =>
    invoke<string | null>("take_pending_terminal_snapshot"),
  takePendingProfileId: () =>
    invoke<string | null>("take_pending_profile_id"),
  /** Open or close the Settings window (singleton). Returns the new
   *  state: `true` = window is now open, `false` = window is now
   *  closed. Backend persists size/position to settings.json on
   *  CloseRequested so reopens land in the same place. */
  toggleSettingsWindow: () =>
    invoke<boolean>("toggle_settings_window"),

  listHighlightPacks: () => invoke<HighlightPack[]>("list_highlight_packs"),
  updateUserHighlightPack: (pack: HighlightPack) =>
    invoke<HighlightPack>("update_user_highlight_pack", { pack }),
  importHighlightPack: () =>
    invoke<HighlightPack>("import_user_highlight_pack"),
  deleteHighlightPack: (id: string) =>
    invoke<void>("delete_user_highlight_pack", { id }),

  sendBytes(bytes: Uint8Array): Promise<void> {
    return invoke<void>("send", { data: base64Encode(bytes) });
  },

  sendString(s: string): Promise<void> {
    return invoke<void>("send", { data: base64Encode(new TextEncoder().encode(s)) });
  },

  onData(handler: (bytes: Uint8Array) => void): () => void {
    return subscribe<string>(EVT_DATA, (payload) => handler(base64Decode(payload)));
  },

  onDisconnect(handler: (reason: string) => void): () => void {
    return subscribe<string>(EVT_DISCONNECT, handler);
  },

  onReconnecting(handler: (portName: string) => void): () => void {
    return subscribe<string>(EVT_RECONNECTING, handler);
  },

  onReconnected(handler: (profileID: string) => void): () => void {
    return subscribe<string>(EVT_RECONNECTED, handler);
  },

  onTransferProgress(handler: (p: TransferProgress) => void): () => void {
    return subscribe<TransferProgress>(EVT_TRANSFER_PROGRESS, handler);
  },

  onTransferComplete(handler: (filename: string) => void): () => void {
    return subscribe<string>(EVT_TRANSFER_COMPLETE, handler);
  },

  onTransferError(handler: (reason: string) => void): () => void {
    return subscribe<string>(EVT_TRANSFER_ERROR, handler);
  },
};

// Tauri's listen() is async but callers (Svelte onMount) want a sync
// unsubscribe. Buffer cancellation so disposal before the listener
// is wired still detaches once it arrives.
//
// Uses `getCurrentWebviewWindow().listen` (not the top-level `listen`
// from @tauri-apps/api/event) — the global form receives EVERY event
// regardless of which window it was emitted to, which causes
// cross-window leaks under multi-window mode (one window's serial:data
// would print into every other window's terminal). The window-scoped
// form only receives events targeted at this webview's label, plus
// any global emit() events, which is what we want.
function subscribe<T>(event: string, handler: (payload: T) => void): () => void {
  let unlisten: (() => void) | null = null;
  let cancelled = false;
  getCurrentWebviewWindow()
    .listen<T>(event, (e) => handler(e.payload))
    .then((un) => {
      if (cancelled) un();
      else unlisten = un;
    });
  return () => {
    cancelled = true;
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
  };
}

function base64Encode(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

function base64Decode(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

export function themeToXterm(t: Theme) {
  return {
    background: t.background,
    foreground: t.foreground,
    cursor: t.cursor,
    cursorAccent: t.cursorAccent || t.background,
    selectionBackground: t.selection,
    selectionForeground: t.selectionForeground || undefined,
    black: t.black,
    red: t.red,
    green: t.green,
    yellow: t.yellow,
    blue: t.blue,
    magenta: t.magenta,
    cyan: t.cyan,
    white: t.white,
    brightBlack: t.brightBlack,
    brightRed: t.brightRed,
    brightGreen: t.brightGreen,
    brightYellow: t.brightYellow,
    brightBlue: t.brightBlue,
    brightMagenta: t.brightMagenta,
    brightCyan: t.brightCyan,
    brightWhite: t.brightWhite,
  };
}

export const BAUD_RATES = [
  300, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800,
  921600,
];

export const PARITIES = [
  { value: "none", label: "None" },
  { value: "odd", label: "Odd" },
  { value: "even", label: "Even" },
  { value: "mark", label: "Mark" },
  { value: "space", label: "Space" },
];

export const STOP_BITS = [
  { value: "1", label: "1" },
  { value: "1.5", label: "1.5" },
  { value: "2", label: "2" },
];

export const DATA_BITS = [5, 6, 7, 8];

export const FLOW_CONTROL = [
  { value: "none", label: "None" },
  { value: "rtscts", label: "RTS/CTS" },
  { value: "xonxoff", label: "XON/XOFF" },
];

export const LINE_ENDINGS = [
  { value: "cr", label: "CR (\\r) — switches, routers" },
  { value: "lf", label: "LF (\\n) — Linux consoles" },
  { value: "crlf", label: "CRLF (\\r\\n) — legacy / Windows" },
];

export const LINE_POLICIES = [
  { value: "default", label: "Default (leave as-is)" },
  { value: "assert", label: "Assert (high)" },
  { value: "deassert", label: "Deassert (low)" },
];
