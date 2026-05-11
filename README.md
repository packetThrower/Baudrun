<p align="center">
  <img src="build/appicon.png" alt="Baudrun" width="128">
</p>

# Baudrun

[![CI](https://img.shields.io/github/actions/workflow/status/packetThrower/Baudrun/ci.yml?branch=main&style=flat-square&logo=github&label=CI)](https://github.com/packetThrower/Baudrun/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/packetThrower/Baudrun?style=flat-square&logo=github&label=release&include_prereleases)](https://github.com/packetThrower/Baudrun/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/packetThrower/Baudrun/total?style=flat-square&logo=github&label=downloads)](https://github.com/packetThrower/Baudrun/releases)
[![Rust](https://img.shields.io/badge/Rust-stable-CE422B?style=flat-square&logo=rust&logoColor=white)](Cargo.toml)
[![License: GPL v3+](https://img.shields.io/badge/license-GPLv3%2B-blue?style=flat-square)](LICENSE)

**Minimum OS Versions** &nbsp;
[![macOS 11+](https://img.shields.io/badge/macOS-11%2B-333?style=flat-square&logo=apple&logoColor=white)](https://packetthrower.github.io/Baudrun/reference/requirements/)
[![Apple Silicon](https://img.shields.io/badge/Apple%20Silicon-arm64-333?style=flat-square&logo=apple&logoColor=white)](https://packetthrower.github.io/Baudrun/reference/requirements/)
[![Intel](https://img.shields.io/badge/Intel-x86__64-333?style=flat-square&logo=apple&logoColor=white)](https://packetthrower.github.io/Baudrun/reference/requirements/)
[![Windows 10 21H2+](https://img.shields.io/badge/Windows%2010%2021H2%2B-x64%20%2F%20arm64-0078D4?style=flat-square&logo=windows&logoColor=white)](https://packetthrower.github.io/Baudrun/reference/requirements/)
[![Linux](https://img.shields.io/badge/Linux-amd64%20%2F%20arm64-FCC624?style=flat-square&logo=linux&logoColor=black)](https://packetthrower.github.io/Baudrun/reference/requirements/)

A cross-platform serial terminal for network devices. Built for switch consoles,
router CLIs, and other serial-attached gear. Profile-based: each device gets a
named profile storing port, baud rate, framing, flow control, line ending, and
optional send-on-connect sequences. One click connect; no `screen /dev/cu.usbserial-…`
from memory, no hunting for which baud rate a specific switch wants, no juggling
a different terminal app per adapter chipset.

Developed in close collaboration with Claude (Anthropic). See
[AI-USAGE.md](AI-USAGE.md) for how that split works.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs-next/public/screenshots/macos-dark-baudrun.png">
    <source media="(prefers-color-scheme: light)" srcset="docs-next/public/screenshots/macos-light-baudrun.png">
    <img src="docs-next/public/screenshots/macos-dark-baudrun.png" alt="Baudrun in its default skin" width="720">
  </picture>
</p>

> **Status — active rewrite.** The `main` branch and all shipping
> Releases use a Tauri v2 + Svelte 5 + xterm.js stack. The
> [`experiments/alacritty-gpui`](https://github.com/packetThrower/Baudrun/tree/experiments/alacritty-gpui)
> branch is replacing the entire stack with `alacritty_terminal` for VT
> parsing and Zed's [`gpui`](https://www.gpui.rs/) for rendering — same
> features, native window, no embedded browser. The Tauri version stays
> the shipping artifact until the rewrite ships. Install instructions
> below still produce the Tauri build; the "Building from source"
> section covers the gpui rewrite.

## Documentation

Full reference docs live at **[packetthrower.github.io/Baudrun](https://packetthrower.github.io/Baudrun/)**
(Astro / Starlight). The site covers profiles, themes, skins, highlighting rule
authoring, the regex playground, file-transfer protocols, and accessibility.
Markdown sources under [`docs-next/`](docs-next/).

For authoring your own skins / themes / highlight packs, sample JSON lives in
[`docs/examples/`](docs/examples/).

## Highlights

- **Profile-based connections** — named settings per device (port, baud,
  framing, flow control, line ending, control-line policies), stored as plain
  JSON. See [Profiles](https://packetthrower.github.io/Baudrun/usage/profiles/).
- **USB chipset detection** — VID/PID lookup identifies CP210x, FTDI, PL2303,
  CH340, MCP2221, and more, and points at the vendor driver when one's missing.
  CDC-ACM USB-C consoles (HPE/Aruba, newer Cisco, RuggedCom RST2228) work out
  of the box.
- **Auto-reconnect** — USB adapter drops recover transparently with the
  terminal buffer preserved across the gap.
- **Send Break** — 300 ms TX-low pulse for Cisco ROMMON, Juniper diagnostic
  mode, and bootloader interrupts.
- **File transfer** — XMODEM / XMODEM-CRC / XMODEM-1K / YMODEM for firmware
  uploads to embedded bootloaders.
- **Paste safety** — multi-line confirmation prompt + configurable slow paste
  so UARTs with small buffers don't drop bytes.
- **Suspend / Resume** — step away from a connected session without tearing
  down the serial port; the full backlog is preserved on return.
- **Multi-window** — right-click a profile to spawn a new window with that
  profile selected, or migrate a live session to a new window (port + scrollback
  + DTR/RTS state all follow). Hold parallel sessions to different devices
  side by side.
- **Vendor-aware syntax highlighting** — bundled rule packs for Cisco IOS,
  Juniper Junos, Aruba AOS-CX, Arista EOS, and MikroTik RouterOS, plus a
  vendor-neutral default. Author your own and test against captures via the
  browser-based [rule playground](https://packetthrower.github.io/Baudrun/playground.html).
- **13 terminal themes + 14 app skins.** Themes include Dracula, Solarized,
  Gruvbox, Nord, OneDark, Tomorrow, Brogrammer, CRT Phosphor, Synthwave, and
  **Colorblind Safe** (Bang Wong's palette from Nature Methods 2011 —
  perpendicular to the protan/deutan confusion axis). Skins include
  macOS 26 (Liquid Glass), Windows 11 (Fluent), GNOME (Adwaita), KDE (Breeze),
  CRT, Cyberpunk, Blueprint, E-Ink, and High Contrast. Skins and themes are
  independent.
- **Accessibility** — `prefers-reduced-motion` respected, keyboard zoom,
  configurable shortcuts, ARIA labels on every icon-only control. Details at
  [Accessibility](https://packetthrower.github.io/Baudrun/reference/accessibility/).
- **Relocatable config directory** — keep profiles, themes, skins, and
  settings alongside your dotfiles; pick the target in Settings → Advanced.

## Install (shipping Tauri build)

Package managers track the latest stable tag and handle the Gatekeeper /
SmartScreen first-launch friction for you. Pre-release channels are available
side-by-side with stable on either platform.

```sh
# macOS — Homebrew
brew tap packetThrower/tap
brew install --cask baudrun                 # stable
brew install --cask baudrun@alpha           # pre-release

# Windows — Scoop
scoop install git                           # if you don't already have git
scoop bucket add packetThrower https://github.com/packetThrower/scoop-bucket
scoop install baudrun                       # stable
scoop install baudrun-prerelease            # pre-release
```

The taps also hold related tools. Repos:
[packetThrower/homebrew-tap](https://github.com/packetThrower/homebrew-tap),
[packetThrower/scoop-bucket](https://github.com/packetThrower/scoop-bucket).

Linux users grab the matching `.deb` / `.rpm` / `.AppImage` /
`.pkg.tar.zst` directly from the
[Releases page](https://github.com/packetThrower/Baudrun/releases). The package
formats ship a udev rule for `/dev/ttyUSB*` ACL access without the dialout-group
dance. Arch users can install the `.pkg.tar.zst` directly via `pacman -U`.

Manual install: download from
[Releases](https://github.com/packetThrower/Baudrun/releases), drag `Baudrun.app`
to `/Applications` on macOS or run the NSIS installer on Windows. Builds are
ad-hoc signed on macOS (right-click → Open the first time, or
`xattr -cr Baudrun.app`) and unsigned on Windows (SmartScreen → "More info" →
"Run anyway"). Apple Developer signing + notarization and Windows code signing
are planned — see [TODO.md](TODO.md).

## Building from source

Two build paths depending on the branch you're on.

### `main` — Tauri v2 + Svelte 5 build

Prerequisites: Rust stable, Node 20+, and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for your
platform.

```bash
git clone git@github.com:packetThrower/Baudrun.git
cd Baudrun
npm install                              # pulls Tauri CLI + frontend deps
npm run tauri dev                        # hot-reload dev (Rust + Vite)
npm run tauri build                      # production bundle for host arch
```

System libraries on Linux: `gtk3-dev`, `webkit2gtk-4.1-dev`, `libsoup-3.0-dev`,
`libayatana-appindicator3-dev`, `librsvg2-dev`, `libusb-1.0-0-dev`, `libudev-dev`.

### `experiments/alacritty-gpui` — alacritty + gpui rewrite

Single-crate Rust project at the repo root. No Node, no webview.

```bash
git clone -b experiments/alacritty-gpui git@github.com:packetThrower/Baudrun.git
cd Baudrun
cargo run                                # dev launch (loopback mode if no port)
cargo run -- /dev/cu.usbserial-XXX       # dev launch attached to a port
cargo build --release                    # optimized binary at target/release/Baudrun
```

System libraries:

- **macOS**: `brew install libusb pkg-config`
- **Debian / Ubuntu**: `sudo apt install libusb-1.0-0-dev libudev-dev pkg-config`
- **Fedora**: `sudo dnf install libusb1-devel systemd-devel pkgconf-pkg-config`
- **Arch**: `sudo pacman -S libusb pkgconf`
- **Windows**: nothing extra; the gpui DirectX backend ships with Windows 10+.

The rewrite is on the
[Phase 8 / Phase 9 punch list in TODO.md](TODO.md); not yet shipping. Drivers
for code-signing, notarization, and an auto-updater land with Phase 9.

## Project layout

```
Baudrun/
├── Cargo.toml                # baudrun crate (binary name: Baudrun)
├── src/
│   ├── main.rs               # app entry + macOS menubar / dock setup
│   ├── app_view.rs           # window-level UI: sidebar, editor, session header
│   ├── terminal_view.rs      # alacritty_terminal bridge + grid renderer
│   ├── terminal_grid.rs      # the cell grid the renderer paints from
│   ├── term_bridge.rs        # alacritty → gpui colour / attribute translation
│   ├── settings_view.rs      # standalone Settings window (tabs, panes, filter)
│   ├── settings_bus.rs       # App-scoped settings entity + change broadcast
│   ├── skin_tokens.rs        # active-skin token cache (read by every render)
│   ├── highlight_runtime.rs  # line-buffered regex highlighter
│   ├── serial_io.rs          # serial-port read/write threads + write channel
│   └── data/                 # pure-Rust data layer (no UI deps)
│       ├── profiles.rs       # JSON-backed profile store + validation
│       ├── settings.rs       # global settings store
│       ├── themes/           # theme store + .itermcolors plist parser
│       ├── skins.rs          # skin store + CSS-var validation
│       ├── highlight.rs      # rule-pack store
│       ├── sanitize.rs       # session-log sanitizer
│       ├── transfer.rs       # XMODEM / YMODEM state machines
│       ├── serial/           # port enumeration + chipset detection
│       ├── usbserial/        # libusb-direct backend (CP210x)
│       └── appdata.rs        # OS config-directory resolution
├── resources/                # bundled at compile time
│   ├── Info.plist            # macOS bundle metadata (Phase 9 packaging)
│   ├── icons/                # .icns / .ico / .png set
│   ├── builtin_skins.json    # 14 built-in app skins
│   ├── builtin_themes.json   # 13 built-in terminal themes
│   └── highlight/            # bundled vendor rule packs
├── build/                    # icon source + Windows installer assets
├── packaging/                # Linux udev rule + .desktop file + Arch PKGBUILD
├── scripts/virtual-serial/   # Go test rig for XMODEM/YMODEM smoke tests
├── docs/examples/            # sample skin / theme / highlight-pack JSON
├── docs-next/                # Astro/Starlight docs site source
└── .github/workflows/        # CI + release + docs deploy
```

CI (`.github/workflows/ci.yml`) and release (`.github/workflows/release.yml`)
still target the Tauri build; Phase 9 rewrites them for the gpui crate. The
docs workflow (`docs.yml`) already targets the Astro site and is
gpui-agnostic.

## License

[GNU General Public License v3.0 or later](LICENSE). Forks are welcome;
derivative works must stay open under the same license. Commercial use is
permitted but can't close the source.
