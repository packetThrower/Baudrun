# Seriesly

[![macOS 11+](https://img.shields.io/badge/macOS-11%2B-333?style=flat-square&logo=apple&logoColor=white)](docs/REQUIREMENTS.md#macos)
[![Windows 10 21H2+](https://img.shields.io/badge/Windows-10%2021H2%2B-0078D4?style=flat-square&logo=windows11&logoColor=white)](docs/REQUIREMENTS.md#windows)
[![Ubuntu 24.04+](https://img.shields.io/badge/Ubuntu-24.04%2B-E95420?style=flat-square&logo=ubuntu&logoColor=white)](docs/REQUIREMENTS.md#linux)
[![Debian 13+](https://img.shields.io/badge/Debian-13%2B-A81D33?style=flat-square&logo=debian&logoColor=white)](docs/REQUIREMENTS.md#linux)
[![Fedora 40+](https://img.shields.io/badge/Fedora-40%2B-294172?style=flat-square&logo=fedora&logoColor=white)](docs/REQUIREMENTS.md#linux)

[![Arch](https://img.shields.io/badge/Arch-1793D1?style=flat-square&logo=archlinux&logoColor=white)](docs/REQUIREMENTS.md#linux)
[![openSUSE Tumbleweed](https://img.shields.io/badge/openSUSE-Tumbleweed-73BA25?style=flat-square&logo=opensuse&logoColor=white)](docs/REQUIREMENTS.md#linux)
[![AppImage: libwebkit2gtk-4.1 + FUSE](https://img.shields.io/badge/AppImage-libwebkit2gtk--4.1%20%2B%20FUSE-2166B7?style=flat-square&logo=appimage&logoColor=white)](docs/REQUIREMENTS.md#linux)

A cross-platform (macOS + Windows + Linux) serial terminal for network devices —
profile-based like SSH, with a built-in xterm terminal and native-feeling UI.

Built for connecting to switch consoles, router CLIs, and other serial-attached
network gear without the ritual of remembering baud rates, fiddling with
`screen /dev/cu.usbserial-...`, or opening three different apps.

## Features

- **Profile-based connections** — named settings per device (port, baud,
  framing, flow control, line ending, and more), stored as plain JSON.
  See [docs/PROFILES.md](docs/PROFILES.md) for the full schema and
  bulk-provisioning recipes.
- **Auto-detecting ports + chipset identification** — USB VID/PID lookup
  surfaces the chipset family (CP210x, FTDI, PL2303, CH340, MCP2221,
  and a dozen more) and links to the vendor driver when one's missing.
  Works out of the box with CDC-ACM USB-C consoles (HPE/Aruba, newer
  Cisco, RuggedCom RST2228).
- **Rich terminal** — xterm.js backend with 10k-line scrollback, line-
  ending translation, local echo, hex view / send, line timestamps,
  session logging to file, copy-on-select, custom baud rates, and
  Backspace/Delete mapping.
- **Auto-reconnect** — opt-in per profile; USB adapter drops recover
  transparently with the xterm buffer preserved across the gap.
- **Send Break** — 300 ms TX-low pulse for Cisco ROMMON, Juniper
  diagnostic mode, and boot-loader interrupts.
- **Paste safety** — multi-line confirmation prompt + configurable slow
  paste so UARTs with small buffers don't drop bytes.
- **File transfer** — XMODEM / XMODEM-CRC / XMODEM-1K / YMODEM for
  firmware uploads to embedded bootloaders.
- **Suspend / Resume** — step away from a connected session without
  tearing down the serial port; xterm stays mounted so the full
  backlog is preserved on return.
- **Syntax highlighting for network gear** — auto-color IPs, MACs,
  interface names, status keywords (up/down/err-disabled/warning),
  timestamps. Device-supplied ANSI colors pass through unchanged.

| Color | Matches |
|-------|---------|
| Cyan | IPv4 (with CIDR), IPv6, MAC addresses |
| Magenta | MAC addresses (colon, dash, Cisco-dotted) |
| Blue | Interface names — `GigabitEthernet0/1`, `Gi1/0/24`, `ge-0/0/1`, `Vlan100` |
| Green | `up`, `online`, `active`, `established`, `enabled`, `OK`, `FULL` |
| Red | `down`, `failed`, `err-disabled`, `error`, `denied`, `timeout`, `critical` |
| Yellow | `warning`, `degraded`, `init`, `learning`, `blocking` |
| Dim gray | Timestamps (`HH:MM:SS`), dates (`YYYY-MM-DD`) |

- **13 built-in terminal themes + `.itermcolors` import** — including
  **Colorblind Safe** (Bang Wong's palette from Nature Methods, 2011 —
  perpendicular to the protan/deutan confusion axis so up/down stays
  legible for the ~6% of men with red-green colorblindness), **CRT
  Phosphor**, **Synthwave**, **Brogrammer**, and all the usual
  Dracula/Solarized/Nord/Gruvbox suspects. [docs/THEMES.md](docs/THEMES.md).
- **14 built-in app skins** with light/dark appearance support:

  | Category | Skins |
  |---|---|
  | Default | **Seriesly** |
  | Modern OS | **macOS 26 (Liquid Glass)**, **Windows 11 (Fluent)**, **GNOME (Adwaita)**, **KDE (Breeze)**, **elementary (Pantheon)**, **Xfce (Greybird)** |
  | Retro OS | **macOS Classic**, **Windows XP (Luna)** |
  | Aesthetic | **CRT (Green Phosphor)**, **Cyberpunk (Synthwave)**, **Blueprint**, **E-Ink (Paper)** |
  | Accessibility | **High Contrast** |

  Skins and themes are orthogonal — mix freely (Liquid Glass skin +
  Solarized Dark theme, CRT skin + CRT Phosphor theme, etc.). Author
  your own via [docs/SKINS.md](docs/SKINS.md).
- **Accessibility** — xterm screen-reader mode, `prefers-reduced-motion`
  respected, Cmd/Ctrl +/- terminal zoom, ARIA labels on every icon-only
  control. Details in [docs/ACCESSIBILITY.md](docs/ACCESSIBILITY.md).
- **Relocatable config directory** — keep profiles, themes, skins, and
  settings alongside your dotfiles; Settings → Advanced picks the
  target.

Everything above is documented in depth under [docs/](docs/) — see
[docs/ADVANCED.md](docs/ADVANCED.md) for the feature reference covering
Send Break, hex send/view, file transfer, auto-reconnect, control-line
policies, session logging, driver detection, and config-directory
relocation.

### Documentation
Full reference docs under [docs/](docs/):

| File | Covers |
|---|---|
| [ADVANCED.md](docs/ADVANCED.md) | Every advanced feature — Send Break, hex send/view, file transfer, auto-reconnect, control-line policies, session logging, driver detection, config-directory relocation, and more. Reference, not how-to. |
| [ACCESSIBILITY.md](docs/ACCESSIBILITY.md) | Screen-reader mode, reduced-motion, terminal zoom shortcuts, ARIA-label coverage, and known caveats. |
| [PROFILES.md](docs/PROFILES.md) | Profile JSON schema + bulk provisioning from CSV inventory via jq/Python. |
| [THEMES.md](docs/THEMES.md) | Theme JSON schema, ANSI-slot reference, `.itermcolors` import ecosystem. |
| [SKINS.md](docs/SKINS.md) | Skin CSS-variable reference, light/dark handling, layout tradeoffs. |
| [examples/](docs/examples/) | Annotated `.jsonc` + importable `.json` reference files for authoring your own skin or theme. |

## Requirements

Released builds target recent mainstream OS releases:

- **macOS 11 Big Sur or later** — universal binary, runs natively on
  Intel and Apple Silicon.
- **Windows 10 21H2+ or 11** — needs the [Microsoft Edge WebView2
  Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/),
  preinstalled on Windows 11 and recent 10 builds. amd64 and arm64
  artifacts shipped.
- **Linux with GTK3 + WebKit2GTK 4.1** — Ubuntu 24.04+, Fedora 40+,
  Debian 13+, or current Arch. amd64 and arm64 artifacts shipped in
  `.deb`, `.rpm`, `.pkg.tar.zst`, and `.AppImage` formats.

See [docs/REQUIREMENTS.md](docs/REQUIREMENTS.md) for the detailed
breakdown, including everything you need to build from source.

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
| **Linux amd64** | `seriesly_<version>_amd64.deb`, `seriesly-<version>.x86_64.rpm`, `seriesly-<version>-x86_64.pkg.tar.zst`, `Seriesly-<version>-x86_64.AppImage` | Standard 64-bit x86 desktop Linux. Pick the format your distro uses; AppImage works anywhere with FUSE. |
| **Linux arm64** | `seriesly_<version>_arm64.deb`, `seriesly-<version>.aarch64.rpm`, `seriesly-<version>-aarch64.pkg.tar.zst`, `Seriesly-<version>-aarch64.AppImage` | Raspberry Pi 4 / 5, ARM workstations, Apple Silicon Linux VMs. |

Arch users can install the `.pkg.tar.zst` directly with
`pacman -U`. The [packaging/arch/](packaging/arch/) directory holds
a PKGBUILD for AUR submission (`seriesly-bin`) — not yet published.

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
- [Wails v2.12](https://wails.io/docs/gettingstarted/installation) (`go install github.com/wailsapp/wails/v2/cmd/wails@v2.12.0`)

```bash
git clone git@github.com:packetThrower/Seriesly.git
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

CI (`.github/workflows/ci.yml`) runs native Go checks on `macos-26`,
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
├── build/                         # Wails build inputs (read at wails build)
│   ├── appicon.png                # source icon (Wails generates .icns)
│   ├── make-icon.sh               # ImageMagick script — regenerates the
│   │                              # .png + a multi-resolution Windows .ico
│   ├── darwin/Info.plist          # macOS bundle metadata
│   └── windows/                   # Windows .ico + manifest + installer
├── packaging/                     # downstream packaging metadata
│   ├── linux/seriesly.desktop     # freedesktop entry shipped inside .deb/.rpm/AppImage
│   └── arch/                      # AUR PKGBUILD for seriesly-bin
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

[GNU General Public License v3.0 or later](LICENSE). Forks are
welcome; derivative works must stay open under the same license.
Commercial use is permitted but can't close the source.
