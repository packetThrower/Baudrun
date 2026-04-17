import * as App from "../../wailsjs/go/main/App.js";
import { EventsOn, EventsOff } from "../../wailsjs/runtime/runtime.js";
import type { profiles, serial } from "../../wailsjs/go/models";

export type Profile = profiles.Profile;
export type PortInfo = serial.PortInfo;

export const EVT_DATA = "serial:data";
export const EVT_DISCONNECT = "serial:disconnect";

export const api = {
  listProfiles: App.ListProfiles,
  createProfile: App.CreateProfile,
  updateProfile: App.UpdateProfile,
  deleteProfile: App.DeleteProfile,
  defaultProfile: App.DefaultProfile,

  listPorts: App.ListPorts,
  connect: App.Connect,
  disconnect: App.Disconnect,
  activeProfileID: App.ActiveProfileID,
  setRTS: App.SetRTS,
  setDTR: App.SetDTR,

  sendBytes(bytes: Uint8Array): Promise<void> {
    return App.Send(base64Encode(bytes));
  },

  sendString(s: string): Promise<void> {
    return App.Send(base64Encode(new TextEncoder().encode(s)));
  },

  onData(handler: (bytes: Uint8Array) => void): () => void {
    const cb = (payload: string) => handler(base64Decode(payload));
    EventsOn(EVT_DATA, cb);
    return () => EventsOff(EVT_DATA);
  },

  onDisconnect(handler: (reason: string) => void): () => void {
    EventsOn(EVT_DISCONNECT, handler);
    return () => EventsOff(EVT_DISCONNECT);
  },
};

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
  { value: "cr", label: "CR (\\r)" },
  { value: "lf", label: "LF (\\n)" },
  { value: "crlf", label: "CRLF (\\r\\n)" },
];
