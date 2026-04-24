# Requirements

Minimums for running a released build, and the extras needed to build
from source. Split into two top-level sections.

## Running a released build

These are the floors for end users downloading from the Releases page.

### macOS
- **macOS 11 Big Sur or later.** Per-arch builds: `arm64` for Apple
  Silicon, `amd64` for Intel. Pick the artifact matching your CPU.
- **Gatekeeper warning on first launch** — the app is ad-hoc signed
  but not notarized. Right-click → Open to bypass. If macOS still
  refuses with "damaged" or "unidentified developer," strip the
  quarantine flag: `xattr -cr Baudrun.app`. The .dmg installer
  handles signatures correctly out of the box; the .zip works once
  unpacked with Finder or `ditto -x -k`.
- **USB-to-serial drivers** where applicable — see the chipset table
  in [README.md](https://github.com/packetThrower/Baudrun/blob/main/README.md#usb-to-serial-adapter-drivers).

The 11.0 floor is set by Tauri v2's webview baseline (Apple Silicon's
own minimum) and `bundle.macOS.minimumSystemVersion` in
`src-tauri/tauri.conf.json`.

### Windows
- **Windows 10 21H2 or later**, or Windows 11. amd64 and arm64 builds
  shipped separately; pick the matching artifact.
- **[Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)**
  — already installed on Windows 11 and most recent Windows 10
  builds. Baudrun surfaces a pointer to the installer if it's
  missing.
- **SmartScreen warning on first launch** — unsigned, so click "More
  info" → "Run anyway". Code signing is tracked in
  [TODO.md](https://github.com/packetThrower/Baudrun/blob/main/TODO.md).
- Windows 10 1803–21H1 will technically run WebView2, but nothing
  older than 21H2 is tested or supported here.
- Pre-release builds (`v*-alpha.N`, `v*-beta.N`, `v*-rc.N`) ship
  the NSIS `.exe` installer only. Stable releases additionally
  produce an `.msi` for corporate silent-deploy workflows; WiX
  rejects alphanumeric pre-release identifiers, so MSI is held to
  stable tags.

### Linux
- **GTK3 ≥ 3.22** plus **WebKit2GTK 4.1** (`libwebkit2gtk-4.1-0`).
  WebKit2GTK 4.0 was deprecated upstream in 2024 and isn't supported.
- Concretely, this means:
  - **Ubuntu 24.04 Noble+**
  - **Debian 13 Trixie+**
  - **Fedora 40+**
  - **Arch** (rolling)
  - **openSUSE Tumbleweed** (rolling)
- `.deb` / `.rpm` / `.pkg.tar.zst` declare GTK3 + WebKit2GTK-4.1 +
  libusb-1.0 as package deps, so `apt` / `dnf` / `pacman` pull them
  in. They also install a udev rule
  (`/usr/lib/udev/rules.d/60-baudrun-serial.rules`) that grants the
  console user ACL access to `/dev/ttyUSB*` via systemd-logind's
  `uaccess` tag — no need to add yourself to the `dialout` group.
- **AppImage** has the same runtime requirement plus **FUSE**
  (`libfuse2` on Ubuntu, `fuse2` on most others). Run with
  `--appimage-extract-and-run` if FUSE is unavailable. The AppImage
  doesn't run the post-install udev hook the package installers do,
  so AppImage users still need group / udev setup of their own.

## Building from source

### Common toolchain

| Tool | Minimum | Why |
|---|---|---|
| Rust | stable (1.77+) | `rust-version` in `src-tauri/Cargo.toml`. |
| Node.js | 20 LTS | Used by Vite 8 + Svelte 5. CI uses Node 20. |
| `@tauri-apps/cli` | v2 | Pulled in as a dev-dep via `npm install`; no separate global install. |

```bash
# Install Rust (if you don't already have it)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Or via Homebrew: brew install rustup-init && rustup-init -y

# Pull in the Tauri CLI + frontend deps
npm install
```

### macOS host
- **Xcode Command Line Tools** (`xcode-select --install`) for the
  Apple toolchain against WebKit.
- **libusb + pkg-config** (`brew install libusb pkg-config`) for the
  link against `rusb` inside the vendored `src-tauri/src/usbserial/`
  CP210x backend.
- Producing a **universal** artifact is not in the build flow —
  each release ships a native per-arch binary instead
  (`npm run tauri build -- --target aarch64-apple-darwin` on Apple
  Silicon, `--target x86_64-apple-darwin` on Intel). Homebrew's
  libusb is per-arch only, so a universal binary would need a
  hand-`lipo`'d libusb dylib; splitting sidesteps that.
- **SDK version matters for window chrome.** The release workflow
  pins the `macos-26` runner (arm64) so binaries link against the
  Tahoe SDK and pick up macOS 26 window corners. The Intel slot
  uses `macos-15-intel`. Building on an older macOS still works,
  but the resulting binary falls back to legacy window chrome when
  run on macOS 26 hosts.

### Windows host
- **Visual Studio Build Tools 2022** (MSVC + Windows SDK) — the
  `link.exe` toolchain Tauri / `wry` / `webview2-com-sys` need.
- **Native ARM for arm64 builds.** Like Wails before it, Tauri v2
  doesn't cleanly cross-compile to Windows-on-ARM from x86 Windows;
  use a native ARM host (Surface Pro X, Copilot+ PC, the
  `windows-11-arm` GitHub runner).

### Linux host
- Runtime deps + their `-dev` headers for the link step:
  ```
  libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev \
    libayatana-appindicator3-dev librsvg2-dev \
    libusb-1.0-0-dev libudev-dev pkg-config
  ```
- **`xdg-utils`** is bundled into the AppImage so `plugin-opener`
  works on target hosts without it. Install on the build host with
  `sudo apt install xdg-utils` (omitted from the runtime list above
  because it's a build-time copy-into-bundle, not a link-time dep).
- **Cross-compiling Linux from macOS or Windows is not supported.**
  Use a Linux host, a Linux VM, or CI.
- Extra tools only needed for packaging:
  - **`fpm`** — drives `.pkg.tar.zst` output
    (`sudo gem install fpm`). Tauri's bundler handles `.deb`, `.rpm`,
    and `.AppImage` natively but doesn't target pacman.
  - **`libarchive-tools`** — provides `bsdtar`, which fpm shells out
    to when building `.pkg.tar.zst`.
  - **`file`** + **`libfuse2t64`** — needed by Tauri's AppImage
    bundler (`appimagetool` is downloaded on-demand by the
    bundler).

## Why these floors?

- **macOS 11**: Apple Silicon's own minimum and Tauri v2 / WebKit's
  effective baseline. The bundle's `minimumSystemVersion` reflects
  this.
- **Windows 10 21H2**: the last consumer-supported Windows 10 tail
  for Microsoft's long-tail updates. WebView2 technically runs on
  1809+, but 21H2 is the tested floor.
- **WebKit2GTK 4.1**: upstream deprecated the 4.0 series in 2024 and
  Ubuntu 24.04 stopped shipping it. Targeting 4.1 keeps us aligned
  with the long-term Linux ecosystem at the cost of dropping pre-
  24.04 distros.
- **Rust stable / Node 20**: both current stable / LTS. We pin
  `rust-version = "1.77"` in `Cargo.toml` because that's the floor
  Tauri v2's `tauri-build` macro requires; nothing in our own code
  needs anything newer.

## Upstream references

- **Tauri v2 prerequisites** — https://tauri.app/start/prerequisites/
- **Microsoft Edge WebView2 supported OSes** — https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution
- **Svelte 5 runtime requirements** — https://svelte.dev/
- **WebKit2GTK releases** — https://webkitgtk.org/releases
