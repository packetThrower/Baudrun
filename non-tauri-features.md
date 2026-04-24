# Features That Don't Fit Tauri v2 Cleanly

Tracking document for features from the Wails v2 Baudrun that either
require custom Rust work outside the Tauri plugin ecosystem, or don't
have a direct Tauri equivalent. Populated during the Wails → Tauri v2
migration on branch `tauri-v2-migration`.

The goal is to keep Phase 1–9 focused on the happy path; anything
flagged here becomes a post-migration follow-up.

## Open items

_Nothing flagged yet. This section fills in as later phases surface
gaps._

## Resolved items

_Items move here once addressed, with a brief note on the chosen
approach._

---

## Notes on specific Wails features and their Tauri equivalents

These are being tracked as "handled" rather than "not-fit":

| Wails feature | Tauri v2 replacement | Notes |
|---|---|---|
| `runtime.OpenFileDialog` / `OpenDirectoryDialog` | `tauri-plugin-dialog` | Supported. |
| `runtime.EventsEmit` / `EventsOn` / `EventsOff` | `Emitter::emit` / `listen()` | Async listen on JS side; wrapped in `subscribe()` helper for sync unsubscribe. |
| `runtime.BrowserOpenURL` | `tauri-plugin-opener` (`openUrl`) | Supported. |
| `runtime.WindowShow` / `WindowUnminimise` | `WebviewWindow::show`/`unminimize`/`set_focus` | Via single-instance plugin callback. |
| `runtime.LogErrorf` | `tauri-plugin-log` + `log` crate | Supported. |
| Wails SingleInstanceLock | `tauri-plugin-single-instance` | Supported. |
| `os.UserConfigDir` per-OS | `app.path().app_config_dir()` | XDG / AppData / App Support. |
| go.bug.st/serial | `serialport` crate | Parity (same libserialport backend family). |
| usbserial-go (libusb-direct) | `rusb` crate | Parity. |
| XMODEM/YMODEM | Custom Rust impl | No mature crate covers both with cancellation + progress. |
| iTerm `.itermcolors` import | `plist` crate | Supported. |
| macOS hidden-titlebar inset | `titleBarStyle: "Overlay"` + `hiddenTitle: true` | Supported via `tauri.conf.json`. |
| Windows toast notifications | `tauri-plugin-notification` | Available if we need it; current app does not emit OS-level toasts. |
| Linux D-Bus notifications | `tauri-plugin-notification` | Same. |
| macOS `open` / Windows `explorer.exe` / Linux `xdg-open` | `tauri-plugin-opener` (`openPath`) | Supported. |
| `//go:embed frontend/dist` | `frontendDist` in `tauri.conf.json` | Supported. |
| `go:build windows` / `darwin` / `!windows` tags | `#[cfg(target_os = "...")]` | Rust equivalent. |
