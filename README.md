# Seriesly

A macOS serial terminal for network devices — profile-based like SSH, with a
built-in xterm terminal and native-feeling UI.

Built for connecting to switch consoles, router CLIs, and other serial-attached
network gear without the ritual of remembering baud rates, fiddling with
`screen /dev/cu.usbserial-...`, or opening three different apps.

## Features

### Profiles
- Named connection settings per device: port, baud, data bits, parity, stop
  bits, flow control, line ending, local echo.
- Persisted as JSON at `~/Library/Application Support/Seriesly/profiles.json` —
  hand-editable, iCloud-syncable, diff-friendly.
- Sensible defaults for network gear (CR line ending, 9600 8N1).

### Serial I/O
- Port auto-detection scans `/dev/cu.*` and surfaces USB metadata (VID/PID,
  product, serial number) when available — rescan button in the profile form.
- Works out of the box with CDC-ACM USB-C consoles (HPE/Aruba, newer Cisco,
  RuggedCom RST2228) — no driver needed.

### Terminal
- xterm.js-backed, full ANSI/VT100 support, 10,000 line scrollback.
- Line-ending translation on Enter (CR, LF, CRLF) per profile.
- Local echo toggle.

### Themes
- Nine built-in themes: Seriesly, Dracula, Solarized Dark/Light, Nord, One Dark,
  Monokai, Gruvbox Dark, Tomorrow Night.
- Import any `.itermcolors` file from iTerm2's color scheme ecosystem
  ([iterm2colorschemes.com](https://iterm2colorschemes.com/) has hundreds).
- Per-profile theme override, with a global default that every profile
  inherits unless it sets its own.
- Custom themes persisted to `~/Library/Application Support/Seriesly/themes/`.

### Syntax highlighting
Universal pattern-based colorization applied to incoming text. Toggle per profile.

| Color | Matches |
|-------|---------|
| Cyan | IPv4 (`192.168.1.1/24`), IPv6, MAC addresses |
| Magenta | MAC addresses (colon, dash, Cisco-dotted) |
| Blue | Interface names — `GigabitEthernet0/1`, `Gi1/0/24`, `ge-0/0/1`, `Vlan100` |
| Green | `up`, `online`, `active`, `established`, `enabled`, `OK`, `FULL` |
| Red | `down`, `failed`, `err-disabled`, `error`, `denied`, `timeout`, `critical` |
| Yellow | `warning`, `degraded`, `init`, `learning`, `blocking` |
| Dim gray | Timestamps (`HH:MM:SS`), dates (`YYYY-MM-DD`) |

Device-supplied ANSI colors (e.g. Aruba CX, Junos) pass through untouched —
highlighting only fills in uncolored text.

### Suspend / Resume
- **Suspend** a connected session to return to the profile list without closing
  the serial port. Green dot + "Session suspended" badge show it's still alive.
- **Resume** picks up where you left off — **full backlog preserved** because
  xterm stays mounted in the background while bytes keep streaming in.
- Navigating away from the terminal view (clicking another profile, creating a
  new one, opening Settings) auto-disconnects by default — Suspend is the
  explicit opt-in to keep a session alive.

### macOS native feel
- Hidden-inset titlebar with traffic lights, vibrancy, SF fonts.
- Sidebar master-detail layout (profiles → editor/terminal).
- Dark appearance, translucent window.

## Requirements

- macOS 11 or later.
- For USB-to-serial adapters: the driver for your chipset.
  - **SiLabs CP210x** (Cisco console cables, cheap adapters) — driver from
    [silabs.com](https://www.silabs.com/developers/usb-to-uart-bridge-vcp-drivers).
  - **FTDI** — built into macOS 11+, no install.
  - **Prolific PL2303** — [Prolific driver](https://www.prolific.com.tw); watch
    for counterfeit chips that don't work with the genuine driver.
  - **WCH CH340/CH341** — [WCH driver](https://www.wch-ic.com).
- **USB-C console ports** (HPE/Aruba, newer Cisco, RuggedCom RST2228) use
  CDC-ACM and need no driver.

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
├── main.go                        # Wails entrypoint, macOS window options
├── app.go                         # Wails-bound App struct (API surface)
├── internal/
│   ├── appdata/                   # ~/Library/Application Support/Seriesly helper
│   ├── profiles/                  # JSON-backed profile store
│   ├── serial/                    # go.bug.st/serial wrapper, read pump
│   ├── settings/                  # global settings (default theme, font size)
│   └── themes/                    # built-in themes + .itermcolors parser
├── frontend/
│   └── src/
│       ├── App.svelte             # sidebar + main layout
│       ├── lib/
│       │   ├── Sidebar.svelte     # profile list + settings button
│       │   ├── ProfileForm.svelte # profile editor + connect/suspend flow
│       │   ├── Terminal.svelte    # xterm.js wrapper, stays mounted per-session
│       │   ├── Settings.svelte    # default theme + font size + theme library
│       │   ├── highlight.ts       # line-buffered ANSI-aware colorizer
│       │   └── api.ts             # thin Wails bindings wrapper
│       └── stores/                # Svelte stores (profiles, themes, session)
```

**Data flow.** Bytes from the serial port flow as base64-encoded Wails events
(`serial:data`) to preserve binary fidelity, are decoded in `api.onData`, fed
through the highlighter for pattern-based colorization, and written to the
xterm instance. Keystrokes go the other way via `api.sendBytes`, with
line-ending translation applied on the frontend.

**Serial lifecycle.** Opening a port starts a goroutine-driven read pump with
a 100ms read timeout; closing the port waits for the pump to exit via a
`sync.WaitGroup` so the OS-level FD is guaranteed released before `Disconnect`
returns.

**Terminal persistence.** The `<Terminal>` component stays mounted as long as
there's an active session, even when the UI is showing the profile form or
settings. CSS toggles visibility; xterm keeps buffering incoming bytes. A
`refit()` call on resume re-syncs the viewport dimensions.

## License

TBD
