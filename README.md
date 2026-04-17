# Seriesly

A macOS serial terminal for network devices — profile-based like SSH, with a
built-in xterm terminal and native-feeling UI.

Built for connecting to switch consoles, router CLIs, and other serial-attached
network gear without the ritual of remembering baud rates, fiddling with
`screen /dev/cu.usbserial-...`, or opening three different apps.

## Features

- **Profiles** — named connection settings per device (port, baud, data/parity/stop
  bits, flow control, line ending, local echo). Persisted to
  `~/Library/Application Support/Seriesly/profiles.json`.
- **Built-in terminal** — xterm.js with a dark theme tuned for console output.
- **Native macOS feel** — hidden-inset titlebar, vibrancy, SF fonts, sidebar
  layout.
- **Port auto-detection** — scans `/dev/cu.*` and surfaces USB metadata
  (VID/PID, product, serial number) when available.

## Requirements

- macOS 11 or later
- For USB-to-serial adapters: the matching driver for your chipset. Common
  adapters (SiLabs CP210x, Prolific PL2303, WCH CH340) need drivers from their
  vendors. FTDI is built into macOS. USB-C consoles (HPE/Aruba, newer Cisco,
  RuggedCom RST2228) use CDC-ACM and need no driver.

## Running

Download the latest `.app` bundle from the releases page (or build from source —
see below), then drag it into `/Applications`.

Because the app is currently self-signed (not yet notarized), the first time you
launch it you'll need to **right-click → Open** to confirm.

## Building from source

Prerequisites:

- Go 1.23+
- Node 18+
- [Wails v2](https://wails.io/docs/gettingstarted/installation) (`go install github.com/wailsapp/wails/v2/cmd/wails@latest`)

```bash
git clone git@github.com:otec-it/Seriesly.git
cd Seriesly
wails build         # production build → build/bin/Seriesly.app
# or
wails dev           # hot-reload dev mode
```

## Architecture

```
Seriesly/
├── main.go                      # Wails entrypoint, macOS window options
├── app.go                       # Wails-bound App struct (profile + serial API)
├── internal/
│   ├── profiles/                # JSON-backed profile store
│   └── serial/                  # go.bug.st/serial wrapper, read pump
├── frontend/
│   └── src/
│       ├── App.svelte           # sidebar + main layout
│       ├── lib/
│       │   ├── Sidebar.svelte   # profile list
│       │   ├── ProfileForm.svelte
│       │   ├── Terminal.svelte  # xterm.js wrapper
│       │   └── api.ts           # thin Wails bindings wrapper
│       └── stores/              # Svelte stores (profiles, session)
```

Bytes from the serial port flow as base64-encoded Wails events
(`serial:data`) to preserve binary fidelity, are decoded in `api.onData`, and
written straight to the xterm instance. Keystrokes go the other way via
`api.sendBytes`, with line-ending translation (CR/LF/CRLF) applied at the
frontend based on the active profile.

## License

TBD
