# Seriesly

A cross-platform (macOS + Windows + Linux) serial terminal for network devices —
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
  iCloud-syncable, diff-friendly. See [docs/PROFILES.md](docs/PROFILES.md)
  for the full schema, control-line policy reference, and examples of
  bulk-provisioning from CSV inventory.
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
- Twelve built-in themes: Seriesly, Dracula, Solarized Dark/Light, Nord,
  One Dark, Monokai, Gruvbox Dark, Tomorrow Night, **Colorblind Safe**,
  **CRT Phosphor (Green)**, **Synthwave**.
- The Colorblind Safe theme uses Bang Wong's palette (Nature Methods, 2011)
  — red and green slots are vermillion and bluish-green, perpendicular to
  the protan/deutan confusion axis, so `up` vs `down` output stays
  distinguishable for ~6% of men with red-green colorblindness.
- CRT Phosphor is a monochrome-green VT100/IBM-3270 palette (pairs with
  the CRT skin). Synthwave is a neon palette on deep purple (pairs with
  Cyberpunk).
- **Theme preview** — each theme has a Preview button in Settings that
  opens a modal with a canned RuggedCom-style output sample showing the
  palette + highlighter against real network-gear text. See how a theme
  looks without having to switch.
- Import any `.itermcolors` file from iTerm2's color scheme ecosystem
  ([iterm2colorschemes.com](https://iterm2colorschemes.com/) has hundreds).
- Per-profile theme override, with a global default that every profile
  inherits unless it sets its own.
- Custom themes persisted to `~/Library/Application Support/Seriesly/themes/`.
  See [docs/THEMES.md](docs/THEMES.md) for the JSON schema, the ANSI-slot
  reference, and tips on picking `.itermcolors` files from the wider
  ecosystem.

### Skins (app chrome)

Fourteen built-in skins swap the whole app's chrome — colors, typography,
radii, elevation, layout shape. Grouped by inspiration:

| Category | Skins |
|---|---|
| Default | **Seriesly** — translucent sidebar, flush-edge layout, uppercase iOS-style row labels. |
| Modern OS | **macOS 26 (Liquid Glass)** — floating rounded bubbles with backdrop blur. **Windows 11 (Fluent)** — Segoe UI Variable, Mica surfaces, stroke borders. **GNOME (Adwaita)** — Cantarell font, flat surfaces, GNOME blue accent. **KDE (Breeze)** — Breeze palette, tighter type. **elementary (Pantheon)** — rounded everything, strataverse blue. **Xfce (Greybird)** — neutral grey, thin borders. |
| Retro OS | **macOS Classic** — Sierra-era Aqua. **Windows XP (Luna)** — blue start-button and bevels. |
| Aesthetic | **CRT (Green Phosphor)** — green-on-black VT100 look with scanline overlay. **Cyberpunk (Synthwave)** — neon magenta/cyan on deep purple. **Blueprint** — engineering-drawing grid on blue paper. **E-Ink (Paper)** — warm paper-white, low saturation. |
| Accessibility | **High Contrast** — solid black surfaces, pure white foreground, visible borders on every control, WCAG-AAA accents. |

- **Light/Dark appearance** — every skin except CRT and Cyberpunk
  supports both modes. The app's CSS palette flips per-skin based on a
  Settings dropdown (Auto / Light / Dark).
- Import custom skin JSON files (flat map of `--css-var: value` pairs).
  Persisted to `~/Library/Application Support/Seriesly/skins/`. See
  [docs/SKINS.md](docs/SKINS.md) for the full variable reference and
  authoring guide.
- Skins are distinct from themes — themes recolor the terminal viewport;
  skins change the app chrome surrounding it. Mix freely (e.g. Liquid
  Glass skin + Solarized Dark theme, or CRT skin + CRT Phosphor theme).

**Caveats:** native `<select>` popups always render in the OS's native
style (Chromium delegates popup rendering to the OS). The window's own
NSVisualEffectView on macOS is pinned to the dark system appearance —
Wails v2.12's runtime theme setters are empty stubs on macOS, so the
vibrancy material can't flip live. Light mode works in the CSS layer;
translucent light skins layer over dark vibrancy only when the CSS is
opaque enough to hide it.

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

### More

Task-oriented walkthroughs of the power features — Send Break for
ROMMON access, hex send/view for binary protocols, paste safety,
auto-reconnect, RS-485 control-line policies, session logging,
driver troubleshooting — live in
[docs/ADVANCED.md](docs/ADVANCED.md).

## Requirements

### macOS
- macOS 11 or later. Universal binary; runs natively on both Intel and
  Apple Silicon.

### Windows
- Windows 10 21H2+ or Windows 11.
- amd64 (x86_64) and arm64 (Snapdragon X / Surface Pro X / Copilot+ PCs)
  builds shipped; pick the matching artifact for your hardware.
- [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)
  — already installed on Windows 11 and recent Windows 10. The app will
  complain if it's missing.

### Linux
- `libgtk-3-0` and `libwebkit2gtk-4.1-0` at runtime (default on Ubuntu
  24.04+, Fedora 40+, recent Debian; the `.deb` and `.rpm` declare them
  as dependencies so `apt install` / `dnf install` pulls them in).
- amd64 and arm64 builds shipped. `.deb`, `.rpm`, and `.AppImage`
  formats available per arch.

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

## Releases

Tagged pushes (CalVer `YYYY.MM.DD-patch`) produce a GitHub Release with
artifacts for five targets:

| Platform | Artifact | Notes |
|---|---|---|
| **macOS** | `Seriesly-macOS-<version>.zip` (contains `.app`) | **Universal binary** — one `.app` with both Intel (x86_64) and Apple Silicon (arm64) slices fused via `lipo`. macOS picks the matching slice at launch, so the same download runs natively on M1/M2/M3 without Rosetta. Trade-off is roughly 2× the download size of a single-arch build. |
| **Windows amd64** | `Seriesly-Windows-amd64-<version>.zip` (contains `.exe`) | Standard 64-bit x86 Windows 10/11. |
| **Windows arm64** | `Seriesly-Windows-arm64-<version>.zip` (contains `.exe`) | Native Windows on ARM (Surface Pro X, Copilot+ PCs on Snapdragon X). No Prism emulation; runs at native speed. |
| **Linux amd64** | `seriesly_<version>_amd64.deb`, `seriesly-<version>.x86_64.rpm`, `Seriesly-<version>-x86_64.AppImage` | Standard 64-bit x86 desktop Linux. Pick the format your distro uses; AppImage works anywhere with FUSE. |
| **Linux arm64** | `seriesly_<version>_arm64.deb`, `seriesly-<version>.aarch64.rpm`, `Seriesly-<version>-aarch64.AppImage` | Raspberry Pi 4 / 5, ARM workstations, Apple Silicon Linux VMs. |

An Arch Linux package (`seriesly-bin`) pulls the same `.deb` in AUR
form — see [packaging/arch/](packaging/arch/) for the PKGBUILD and
submission notes.

Download, unpack, and run. On macOS, drag `Seriesly.app` into `/Applications`.

The app is currently unsigned on all platforms. First-launch friction:
- **macOS**: right-click → Open to bypass Gatekeeper.
- **Windows**: SmartScreen will warn; click "More info" → "Run anyway".
- **Linux**: `chmod +x Seriesly && ./Seriesly`. You'll need `libwebkit2gtk-4.1`
  and `libgtk-3` installed (default on Ubuntu 24.04+, Fedora 40+, and recent
  Debian).

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

Cross-compiling to Linux from macOS is **not** supported by Wails — Linux
builds have to run on Linux (or in CI). On a Linux host, install
`libgtk-3-dev` + `libwebkit2gtk-4.1-dev` + `pkg-config` first, then
`wails build -platform linux/amd64` (or `linux/arm64`).

CI (`.github/workflows/ci.yml`) runs native Go checks on `macos-latest`,
`windows-latest`, `windows-11-arm`, `ubuntu-latest`, and `ubuntu-24.04-arm`
on each push to `main`. Tagged pushes matching CalVer `20*.*.*-*` fire
`.github/workflows/release.yml`, which produces a GitHub Release with all
five platform artifacts attached.

## Architecture

```
Seriesly/
├── main.go                        # Wails entrypoint + per-OS window options
├── app.go                         # Wails-bound App struct (API surface)
├── internal/
│   ├── appdata/                   # per-OS app-data directory helper
│   ├── profiles/                  # JSON-backed profile store
│   ├── serial/                    # go.bug.st/serial wrapper, read pump,
│   │                              # chipset detection (ioreg / Get-PnpDevice)
│   ├── settings/                  # global settings (default theme, skin,
│   │                              # appearance, font size, log dir, toggles)
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
│       │   ├── PreviewTerminal.svelte # read-only xterm for theme previews
│       │   ├── Settings.svelte    # skin picker, appearance, default theme,
│       │   │                      # theme preview modal, log dir, toggles
│       │   ├── highlight.ts       # line-buffered ANSI-aware colorizer
│       │   ├── hexdump.ts         # 16-byte-per-line hex+ASCII formatter
│       │   └── api.ts             # thin Wails bindings wrapper
│       └── stores/                # Svelte stores (profiles, themes, skins,
│                                  # settings, session, appearance, dismissed-drivers)
├── build/
│   ├── appicon.png                # source icon (Wails generates .icns)
│   ├── make-icon.sh               # ImageMagick script — regenerates the
│   │                              # .png + a multi-resolution Windows .ico
│   ├── windows/icon.ico           # hand-managed .ico (make-icon.sh regenerates)
│   └── linux/seriesly.desktop     # freedesktop entry shipped inside .deb/.rpm/AppImage
└── .github/workflows/
    ├── ci.yml                     # native Go + frontend checks across
    │                              # macOS, Windows (amd64/arm64), Linux (amd64/arm64)
    └── release.yml                # tag-triggered build + GitHub Release;
                                   # emits .app (macOS universal), .exe .zip
                                   # per Windows arch, and .deb/.rpm/.AppImage
                                   # per Linux arch
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
