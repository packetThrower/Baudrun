# Seriesly

A cross-platform (macOS + Windows) serial terminal for network devices —
profile-based like SSH, with a built-in xterm terminal and native-feeling UI.

Built for connecting to switch consoles, router CLIs, and other serial-attached
network gear without the ritual of remembering baud rates, fiddling with
`screen /dev/cu.usbserial-...`, or opening three different apps.

## Features

### Profiles
- Named connection settings per device: port, baud, data bits, parity, stop
  bits, flow control, line ending, local echo.
- Persisted as JSON at `~/Library/Application Support/Seriesly/profiles.json`
  (macOS) or `%APPDATA%\Seriesly\profiles.json` (Windows) — hand-editable,
  iCloud-syncable, diff-friendly.
- Sensible defaults for network gear (CR line ending, 9600 8N1).

### Serial I/O
- Port auto-detection enumerates `/dev/cu.*` on macOS and `COM*` on Windows,
  surfacing USB metadata (VID/PID, product, serial number, **chipset family**)
  when available.
- Works out of the box with CDC-ACM USB-C consoles (HPE/Aruba, newer Cisco,
  RuggedCom RST2228) — no driver needed.
- Control-line policies per profile: explicit `DTR on connect`, `RTS on
  connect`, `DTR on disconnect`, `RTS on disconnect` (useful for RS-485
  direction, Arduino DTR-reset, firmwares that gate session state on DTR).
- Live DTR/RTS toggle pills in the session header for in-session control.

### Driver detection (macOS + Windows)
- When a USB-serial adapter is plugged in but its vendor driver isn't loaded,
  Seriesly detects the chipset and shows a yellow banner above the port
  dropdown with a one-click link to the vendor's driver download.
- Chipset coverage: CP210x (SiLabs), FTDI, Prolific PL2303, WCH CH340/CH341,
  Microchip MCP2221, Cypress, ATEN, ARM mbed CDC-ACM, MosChip/ASIX, Magic
  Control, Moxa UPort, Brainboxes.
- Special cases handled: Siemens RUGGEDCOM RST2228 (CP210x reprogrammed with
  Siemens VID), counterfeit-Prolific drivers that refuse older but genuine
  chips (TRENDnet TU-S9), manufacturer-string fallback for rebrands.
- macOS: reads IOKit via `ioreg`. Windows: queries `Get-PnpDevice` over
  PowerShell.
- Dismissible per-device (× button, session-scoped) and disable-able
  globally via Settings → Advanced.

### Terminal
- xterm.js-backed, full ANSI/VT100 support, 10,000-line scrollback.
- Line-ending translation on Enter (CR, LF, CRLF) per profile.
- Local echo toggle.
- **Hex view** — alternate output mode showing bytes as a 16-per-line hex
  dump with ASCII sidebar. Useful for binary protocols (Modbus RTU, firmware
  bootloaders) or debugging line endings.
- **Line timestamps** — prefix each line with `[HH:MM:SS.mmm]` for
  correlating events in logs.
- **Session logging** — toggle per profile to record raw incoming bytes to a
  timestamped file under `~/Library/Application Support/Seriesly/logs/`
  (configurable directory).

### Themes (terminal colors)
- Ten built-in themes: Seriesly, Dracula, Solarized Dark/Light, Nord,
  One Dark, Monokai, Gruvbox Dark, Tomorrow Night, **Colorblind Safe**.
- The Colorblind Safe theme uses Bang Wong's palette (Nature Methods, 2011)
  — red and green slots are vermillion and bluish-green, perpendicular to
  the protan/deutan confusion axis, so `up` vs `down` output stays
  distinguishable for ~6% of men with red-green colorblindness.
- Import any `.itermcolors` file from iTerm2's color scheme ecosystem
  ([iterm2colorschemes.com](https://iterm2colorschemes.com/) has hundreds).
- Per-profile theme override, with a global default that every profile
  inherits unless it sets its own.
- Custom themes persisted to `~/Library/Application Support/Seriesly/themes/`.

### Skins (app chrome)
- Three built-in skins change the whole app's look:
  - **Seriesly** — translucent sidebar, flush-edge layout, uppercase iOS-style
    row labels (the default).
  - **macOS 26 (Liquid Glass)** — sidebar and main panel become floating
    rounded bubbles with backdrop blur, sentence-case labels, brighter
    accents, bigger continuous radii.
  - **High Contrast** — accessibility-focused: solid black surfaces, pure
    white foreground, bright visible borders on every input / panel /
    divider, WCAG-AAA accent colors, no translucency or blur.
- Import custom skin JSON files (flat map of `--css-var: value` pairs).
  Persisted to `~/Library/Application Support/Seriesly/skins/`.
- Skins are distinct from themes — themes recolor the terminal viewport;
  skins change the app chrome surrounding it. You can mix (e.g. Liquid Glass
  skin + Solarized Dark theme).

**Caveats:** native `<select>` popups always render in the OS's native
style (Chromium delegates that); window chrome and vibrancy are set at
app launch and don't swap live.

### Syntax highlighting
Universal pattern-based colorization applied to incoming text. Toggle per
profile.

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
- **Suspend** a connected session to return to the profile list without
  closing the serial port. Green dot + "Session suspended" badge show it's
  still alive.
- **Resume** picks up where you left off — **full backlog preserved** because
  xterm stays mounted in the background while bytes keep streaming in.
- Navigating away from the terminal view (clicking another profile, creating
  a new one, opening Settings) auto-disconnects by default — Suspend is the
  explicit opt-in to keep a session alive.

## Requirements

### macOS
- macOS 11 or later.

### Windows
- Windows 10 21H2+ or Windows 11.
- [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)
  — already installed on Windows 11 and recent Windows 10. The app will
  complain if it's missing.

### USB-to-serial adapter drivers

Required when using an adapter rather than a device's built-in USB console:

| Chipset | Driver |
|---|---|
| **SiLabs CP210x** (Cisco console cables, many industrial adapters) | [silabs.com VCP](https://www.silabs.com/developers/usb-to-uart-bridge-vcp-drivers) |
| **FTDI** (higher-quality adapters) | Built in on macOS 11+ and Windows 10+ |
| **Prolific PL2303** | [prolific.com.tw](https://www.prolific.com.tw); watch for counterfeit / deprecated-chip caveats |
| **WCH CH340/CH341** (cheap clones, Arduino knockoffs) | [wch-ic.com](https://www.wch-ic.com) |
| **USB-C consoles** (HPE/Aruba, newer Cisco, RuggedCom RST2228) | None — CDC-ACM is built into macOS and Windows |

Seriesly will detect known chipsets and point you at the right download when
a driver is missing.

## Running

Download the latest `.app` bundle (macOS) or `.exe` (Windows) from the
releases page, or build from source. On macOS, drag `Seriesly.app` into
`/Applications`.

The app is currently unsigned on both platforms. First-launch friction:
- **macOS**: right-click → Open to bypass Gatekeeper.
- **Windows**: SmartScreen will warn; click "More info" → "Run anyway".

Code signing and notarization are planned — see `TODO.md`.

## Building from source

Prerequisites:

- Go 1.23+
- Node 20+
- [Wails v2](https://wails.io/docs/gettingstarted/installation) (`go install github.com/wailsapp/wails/v2/cmd/wails@latest`)

```bash
git clone git@github.com:otec-it/Seriesly.git
cd Seriesly
wails build                               # production build for host OS
wails build -platform windows/amd64       # cross-compile to Windows from macOS
wails build -platform darwin/universal    # universal macOS binary
wails dev                                 # hot-reload dev mode
```

CI (`.github/workflows/ci.yml`) runs native Go checks on `macos-latest` and
`windows-latest` on each push to `main`. Tagged pushes matching CalVer
`20*.*.*-*` fire `.github/workflows/release.yml`, which produces a GitHub
Release with both platform binaries attached.

## Architecture

```
Seriesly/
├── main.go                        # Wails entrypoint, macOS window options
├── app.go                         # Wails-bound App struct (API surface)
├── internal/
│   ├── appdata/                   # per-OS app-data directory helper
│   ├── profiles/                  # JSON-backed profile store
│   ├── serial/                    # go.bug.st/serial wrapper, read pump,
│   │                              # chipset detection (ioreg / Get-PnpDevice)
│   ├── settings/                  # global settings (default theme, skin,
│   │                              # font size, log dir, driver-detect toggle)
│   ├── skins/                     # app-chrome skins (CSS-var JSON)
│   └── themes/                    # terminal themes + .itermcolors parser
├── frontend/
│   └── src/
│       ├── App.svelte             # sidebar + main layout, session lifecycle
│       ├── style.css              # CSS custom-property surface (skin root)
│       ├── lib/
│       │   ├── Sidebar.svelte     # profile list + settings button
│       │   ├── ProfileForm.svelte # profile editor + connect/suspend flow
│       │   ├── Terminal.svelte    # xterm.js wrapper, stays mounted per-session
│       │   ├── Settings.svelte    # skins, default theme, log dir, detection toggle
│       │   ├── highlight.ts       # line-buffered ANSI-aware colorizer
│       │   ├── hexdump.ts         # 16-byte-per-line hex+ASCII formatter
│       │   └── api.ts             # thin Wails bindings wrapper
│       └── stores/                # Svelte stores (profiles, themes, skins,
│                                  # settings, session, dismissed-drivers)
├── build/
│   ├── appicon.png                # source icon (Wails generates .icns/.ico)
│   └── make-icon.sh               # ImageMagick script to regenerate
└── .github/workflows/
    ├── ci.yml                     # native Go + frontend checks
    └── release.yml                # tag-triggered macOS + Windows release
```

**Data flow.** Bytes from the serial port flow as base64-encoded Wails events
(`serial:data`) to preserve binary fidelity, are decoded in `api.onData`,
fed through either the highlighter (with optional per-line timestamp
prefixing) or the hex-dump formatter depending on per-profile settings, and
written to the xterm instance. If session logging is enabled, a separate
log-file sink in the Go backend receives a raw copy of every byte. Keystrokes
go the other way via `api.sendBytes`, with line-ending translation applied
on the frontend.

**Serial lifecycle.** Opening a port starts a goroutine-driven read pump with
a 100ms read timeout; closing the port waits for the pump to exit via a
`sync.WaitGroup` so the OS-level FD is guaranteed released before
`Disconnect` returns. On-connect and on-disconnect control-line policies
(DTR/RTS assert/deassert) are applied at the right bookends.

**Terminal persistence.** The `<Terminal>` component stays mounted as long
as there's an active session, even when the UI is showing the profile form
or settings. CSS toggles visibility; xterm keeps buffering incoming bytes
into its scrollback. A `refit()` call on resume re-syncs the viewport
dimensions.

**Skin application.** On app mount, the skin applier sets every variable
from the active skin's JSON onto `document.documentElement`, tracking which
properties it has written. Switching skins first unsets all tracked
properties (so sparser skins don't inherit stale values from richer ones)
then applies the new set. Live-swap, no reload.

## License

TBD
