import { writable } from "svelte/store";

export type SessionState =
  | { status: "idle" }
  | { status: "connecting"; profileID: string }
  | { status: "connected"; profileID: string }
  | { status: "error"; message: string };

export const session = writable<SessionState>({ status: "idle" });
