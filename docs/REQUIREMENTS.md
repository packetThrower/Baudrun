# Requirements

Minimums for running a released build, and the extras needed to build
from source. Split into two top-level sections.

## Running a released build

These are the floors for end users downloading from the Releases page.

### macOS
- **macOS 11 Big Sur or later.** Universal binary (x86_64 + arm64),
  so the same download runs natively on both Intel Macs and Apple
  Silicon.
- **Gatekeeper warning on first launch** — the app is unsigned for
  now. Right-click → Open to bypass.
- **USB-to-serial drivers** where applicable — see the chipset table
  in [README.md](https://github.com/packetThrower/Baudrun/blob/main/README.md#usb-to-serial-adapter-drivers).

Upstream Wails v2 supports macOS 10.13+, but Go 1.23's macOS floor
and Apple Silicon's own 11.0 floor raise the effective minimum to 11.

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

### Linux
- **GTK3 ≥ 3.22** plus **WebKit2GTK 4.1** (`libwebkit2gtk-4.1-0`).
  WebKit2GTK 4.0 was deprecated upstream in 2024 and isn't supported.
- Concretely, this means:
  - **Ubuntu 24.04 Noble+**
  - **Debian 13 Trixie+**
  - **Fedora 40+**
  - **Arch** (rolling)
  - **openSUSE Tumbleweed** (rolling)
- `.deb` / `.rpm` / `.pkg.tar.zst` declare GTK3 and WebKit2GTK-4.1 as
  package deps, so `apt` / `dnf` / `pacman` pull them in.
- **AppImage** has the same runtime requirement plus **FUSE**
  (`libfuse2` on Ubuntu, `fuse2` on most others). Run with
  `--appimage-extract-and-run` if FUSE is unavailable.

## Building from source

### Common toolchain

| Tool | Minimum | Why |
|---|---|---|
| Go | 1.23 | Declared in `go.mod`. |
| Node.js | 20 LTS | Used by Vite 8 + Svelte 5. CI uses Node 20. |
| Wails CLI | v2.12.0 | Pinned in CI via `go install ...@v2.12.0`. |

```bash
go install github.com/wailsapp/wails/v2/cmd/wails@v2.12.0
```

### macOS host
- **Xcode Command Line Tools** (`xcode-select --install`) for the cgo
  toolchain against WKWebView.
- Producing the universal artifact requires either an Apple Silicon
  host with a modern SDK or `lipo` on any host with the arm64 +
  x86_64 SDKs installed. `wails build -platform darwin/universal`
  handles the `lipo` step itself.
- **SDK version matters for window chrome.** The release workflow
  pins the `macos-26` runner so binaries link against the Tahoe SDK
  and pick up macOS 26 window corners. Building on an older macOS
  still works, but the resulting binary falls back to legacy window
  chrome when run on macOS 26 hosts.

### Windows host
- **Visual Studio Build Tools** (MSVC + Windows SDK) or **MSYS2** with
  a C compiler — cgo needs a native toolchain to link against the
  WebView2 loader.
- **Native ARM for arm64 builds.** Wails v2 can't cross-compile to
  Windows on ARM from x86 Windows; use a native ARM host (Surface Pro
  X, Copilot+ PC, `windows-11-arm` GitHub runner).

### Linux host
- Runtime deps, plus their `-dev` headers for cgo linking:
  ```
  libgtk-3-dev libwebkit2gtk-4.1-dev pkg-config build-essential
  ```
- Pass `-tags webkit2_41` to `wails build`, `go vet`, and `go build`
  so pkg-config resolves against the 4.1 series. Without it the
  build still targets 4.0 and fails on modern distros.
- **Cross-compiling Linux from macOS or Windows is not supported by
  Wails.** Use a Linux host or CI.
- Extra tools only needed for packaging:
  - **`fpm`** — drives `.deb`, `.rpm`, and `.pkg.tar.zst` output
    (`sudo gem install fpm`).
  - **`libarchive-tools`** — provides `bsdtar`, which fpm shells out
    to when building `.pkg.tar.zst`.
  - **`appimagetool`** — downloaded on-the-fly in the release
    workflow; for local builds, grab the continuous release from the
    [AppImage repo](https://github.com/AppImage/appimagetool).

## Why these floors?

- **macOS 11**: Apple Silicon's own minimum, plus Go 1.23 dropped
  macOS 10.15 support. Wails upstream claims 10.13+; our effective
  floor is raised by the toolchain.
- **Windows 10 21H2**: the last consumer-supported Windows 10 tail
  for Microsoft's long-tail updates. WebView2 technically runs on
  1809+, but 21H2 is the tested floor.
- **WebKit2GTK 4.1**: upstream deprecated the 4.0 series in 2024 and
  Ubuntu 24.04 stopped shipping it. Targeting 4.1 keeps us aligned
  with the long-term Linux ecosystem at the cost of dropping pre-
  24.04 distros.
- **Go 1.23 / Node 20**: both current stable / LTS. Upstream Wails
  and Svelte minimums are older, but we haven't pinned lower because
  nothing in the code needs the older toolchain.

## Upstream references

- **Wails v2 platform support** — https://wails.io/docs/gettingstarted/installation
- **Microsoft Edge WebView2 supported OSes** — https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution
- **Svelte 5 runtime requirements** — https://svelte.dev/
- **WebKit2GTK releases** — https://webkitgtk.org/releases
