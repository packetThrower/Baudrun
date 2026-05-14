---
title: Requirements
description: 'Minimum OS versions for running a release, plus the toolchain needed to build from source.'
editUrl: https://github.com/packetThrower/Baudrun/edit/main/docs/REQUIREMENTS.md
---

Minimums for running a released build, and the extras needed to build
from source. Split into two top-level sections.

## Running a released build

These are the floors for end users downloading from the Releases page.

### macOS
- **macOS 11 Big Sur or later.** Per-arch builds: `arm64` for Apple
  Silicon, `amd64` for Intel. Pick the artifact matching your CPU.
- **Gatekeeper warning on first launch.** The app is ad-hoc signed
  but not notarized. Right-click → Open to bypass. If macOS still
  refuses with "damaged" or "unidentified developer," strip the
  quarantine flag: `xattr -cr Baudrun.app`. The `.dmg` installer
  handles signatures correctly out of the box; the `.zip` works once
  unpacked with Finder or `ditto -x -k`.
- **USB-to-serial drivers** where applicable. See the chipset table
  in [README.md](https://github.com/packetThrower/Baudrun/blob/main/README.md#usb-to-serial-adapter-drivers).

### Windows
- **Windows 10 21H2 or later**, or Windows 11. `amd64` and `arm64`
  builds shipped separately; pick the matching artifact.
- **SmartScreen warning on first launch.** Unsigned, so click "More
  info" → "Run anyway". Code signing is on the near-term roadmap
  (see [TODO.md](https://github.com/packetThrower/Baudrun/blob/main/TODO.md)).
- Windows 10 1803–21H1 will technically launch the binary, but
  nothing older than 21H2 is tested or supported here.
- Every Windows release ships the NSIS `-setup.exe` installer plus
  a portable `.zip` of the bare `Baudrun.exe`. Stable releases on
  **amd64** additionally produce an `.msi` for corporate silent-
  deploy workflows. The `.msi` is gated on two conditions:
  - **Stable tag only** — WiX requires a numeric-only pre-release
    identifier and rejects alphanumeric ones (`-alpha.N`, `-beta.N`,
    `-rc.N`).
  - **amd64 only** — WiX Toolset's `candle.exe` doesn't run
    reliably on Windows-on-ARM. arm64 users get the NSIS
    installer or the portable `.zip`; the small minority who
    need MSI on arm64 can either run the amd64 `.msi` under
    emulation or wait for WiX's arm64 support to mature.

### Linux
- **Vulkan-capable GPU** with current Mesa drivers — the same floor
  [Zed](https://zed.dev) ships against. The renderer goes straight
  to Vulkan; there's no software fallback. Practically every
  distribution-shipped GPU stack from 2022 onward qualifies.
- Floor for the bundled packages and their dependency declarations:
  - **Ubuntu 22.04 Jammy+**
  - **Debian 12 Bookworm+**
  - **Fedora 38+**
  - **Arch** (rolling)
  - **openSUSE Tumbleweed** (rolling)
- `.deb` / `.rpm` / `.pkg.tar.zst` declare `libusb-1.0` as a package
  dep, so `apt` / `dnf` / `pacman` pull it in. They also install a
  udev rule (`/usr/lib/udev/rules.d/60-baudrun-serial.rules`) that
  grants the console user ACL access to `/dev/ttyUSB*` via
  systemd-logind's `uaccess` tag, with no need to add yourself to
  the `dialout` group.
- **AppImage** has the same runtime requirement plus **FUSE**
  (`libfuse2` on Ubuntu, `fuse2` on most others). Run with
  `--appimage-extract-and-run` if FUSE is unavailable. The AppImage
  doesn't run the post-install udev hook the package installers do,
  so AppImage users still need group / udev setup of their own.
- **Wayland** and **X11** are both supported; Wayland is preferred
  on GNOME / KDE / Sway / Hyprland for HiDPI scaling and IME
  handling. The window uses a client-side title bar so it stays
  draggable under GNOME Mutter (which doesn't ship server-side
  decorations).

## Building from source

### Toolchain

| Tool | Minimum | Why |
|---|---|---|
| Rust | stable (1.77+) | `rust-version` in `Cargo.toml`. |
| `pkg-config` | any | Used to resolve `libusb` on macOS / Linux. |

```bash
# Install Rust (if you don't already have it)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Or via Homebrew: brew install rustup-init && rustup-init -y

# Clone + build
git clone git@github.com:packetThrower/Baudrun.git
cd Baudrun
cargo build --release         # optimized binary at target/release/Baudrun
cargo run                     # dev launch (loopback mode if no port)
```

No Node, no JavaScript build step, no webview runtime.

### macOS host
- **Xcode Command Line Tools** (`xcode-select --install`) for the
  Apple toolchain.
- **libusb + pkg-config** (`brew install libusb pkg-config`) for the
  link against the vendored `rusb` (CP210x direct backend on
  macOS).
- Each release ships a native per-arch binary
  (`cargo build --release --target aarch64-apple-darwin` on Apple
  Silicon, `--target x86_64-apple-darwin` on Intel). Universal
  artifacts aren't part of the build flow — Homebrew's `libusb` is
  per-arch only and the dual-arch split sidesteps a hand-`lipo`'d
  dylib.
- **SDK version matters for window chrome.** The release workflow
  pins the `macos-26` runner (arm64) so binaries link against the
  Tahoe SDK and pick up the rounded-corner / vibrancy treatment.
  The Intel slot uses `macos-15-intel`. Building on an older macOS
  still works, but the resulting binary falls back to legacy window
  chrome when run on macOS 26.

### Windows host
- **Visual Studio Build Tools 2022** (MSVC + Windows SDK) for the
  `link.exe` toolchain that the gpui DirectX backend links through.
- **Native ARM for arm64 builds.** Cross-compiling to Windows-on-ARM
  from x86 Windows isn't reliable; use a native ARM host (Surface
  Pro X, Copilot+ PC, the `windows-11-arm` GitHub runner).
- No `libusb` install needed — Windows uses the Win32 serial API
  directly.

### Linux host
- Build deps:
  ```
  libusb-1.0-0-dev libudev-dev pkg-config
  ```
  Plus the usual `build-essential` / `gcc` toolchain for cgo-free
  Rust linking.
- **Cross-compiling Linux from macOS or Windows is not supported.**
  Use a Linux host, a Linux VM, or CI.
- Extra tools only needed for packaging:
  - **`cargo-packager`** drives the `.dmg` / NSIS / `.deb` / `.rpm`
    / `.AppImage` builds (`cargo install cargo-packager` or use the
    binstall release from CI).
  - **`fpm`** drives `.pkg.tar.zst` output
    (`sudo gem install fpm`). `cargo-packager` doesn't target
    pacman.
  - **`libarchive-tools`** provides `bsdtar`, which fpm shells out
    to when building `.pkg.tar.zst`.

## Why these floors?

- **macOS 11**: Apple Silicon's own minimum and gpui's effective
  baseline. The bundle's `minimumSystemVersion` reflects this.
- **Windows 10 21H2**: the last consumer-supported Windows 10 tail
  for Microsoft's long-tail updates. gpui's DirectX backend runs on
  10 1809+, but 21H2 is the tested floor.
- **Linux Vulkan floor**: gpui renders through Vulkan with no
  software fallback. Practically any distro shipped from 2022
  onward with current Mesa qualifies; the package declarations pick
  a conservative version floor below which we can't validate.
- **Rust stable**: `rust-version = "1.77"` is set in `Cargo.toml`;
  newer is fine.

## Upstream references

- **gpui**: https://www.gpui.rs/
- **alacritty_terminal**: https://github.com/alacritty/alacritty/tree/master/alacritty_terminal
