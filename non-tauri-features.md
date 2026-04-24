# Features That Don't Fit Tauri v2 Cleanly

Tracking document for features from the Wails v2 Baudrun that either
require custom Rust work outside the Tauri plugin ecosystem, or don't
have a direct Tauri equivalent. Populated during the Wails → Tauri v2
migration on branch `tauri-v2-migration`.

The goal is to keep Phase 1–9 focused on the happy path; anything
flagged here becomes a post-migration follow-up.

## Open items

### libusb-direct CP210x fallback (serial layer)

The Go version uses [`packetThrower/usbserial-go`](https://github.com/packetThrower/usbserial-go)
to talk to CP210x adapters directly over libusb when no vendor driver
is installed — common on macOS without the SiLabs VCP driver and on
rebranded devices whose VID doesn't bind to the kernel's id table.

No off-the-shelf Rust crate covers the CP210x control-transfer
protocol. Porting [`usbserial-go/cp210x`](https://github.com/packetThrower/usbserial-go/tree/main/cp210x)
to Rust on top of `rusb` is a ~300 LOC sub-project (baud-rate /
framing / flow-control / DTR-RTS / break control-transfer sequences,
bulk-transfer read+write endpoints).

Impact: users on macOS without the SiLabs driver currently can't talk
to a CP210x adapter from the Tauri build. On Linux and Windows the
kernel drivers cover CP210x so there's no regression there.

Tracked on branch `tauri-v2-migration`:
- `src-tauri/src/serial/direct.rs` — stub module, `list_direct_usb()`
  returns empty.
- `src-tauri/src/serial/session.rs` — `DirectUsbUnsupported` error
  fires when a profile's `port_name` starts with `usb:`.

Follow-up: add `rusb = "0.9"` to `Cargo.toml`, implement a `NativePort`
peer that wires `rusb::DeviceHandle` through the same `PortBackend`
trait, and replace the stubs.

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
