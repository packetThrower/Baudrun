# Install

The fastest path on macOS and Windows is a package manager — auto-update
on `brew upgrade` / `scoop update`, no Gatekeeper or SmartScreen friction
on first launch, and a pre-release channel alongside stable. Linux users
grab the matching `.deb` / `.rpm` / `.AppImage` / `.pkg.tar.zst` straight
from GitHub.

System requirements (OS floors, runtime dependencies) live in
[Requirements](REQUIREMENTS.md). Building from source is also covered there.

## macOS — Homebrew

The tap [`packetThrower/tap`](https://github.com/packetThrower/homebrew-tap)
ships two casks: `baudrun` (stable) and `baudrun@alpha` (pre-release). They
install side-by-side as `Baudrun.app` and `Baudrun Alpha.app`, so you can
keep stable as your daily driver and run alpha on the side to verify
upcoming changes against your gear.

```sh
brew tap packetThrower/tap
brew install --cask baudrun           # stable
brew install --cask baudrun@alpha     # pre-release
```

The cask strips the macOS quarantine xattr on install, so the app launches
without the right-click → Open dance. Baudrun ships ad-hoc signed but not
notarized — the cask handles that for you so direct downloads stay the
only path that hits Gatekeeper.

Per-arch DMG, picked automatically based on `arch`:

| CPU | DMG |
|---|---|
| Apple Silicon (M1+) | `Baudrun_<version>_aarch64.dmg` |
| Intel | `Baudrun_<version>_x64.dmg` |

Update with `brew upgrade --cask baudrun` (or `baudrun@alpha`). The tap's
auto-bump workflow polls upstream every 6 hours, so a new tag is normally
installable within a quarter day.

## Windows — Scoop

The bucket [`packetThrower/scoop-bucket`](https://github.com/packetThrower/scoop-bucket)
ships two manifests: `baudrun` (stable) and `baudrun-prerelease`. They
install side-by-side with separate Start menu entries (`Baudrun` and
`Baudrun Alpha`) and separate `PATH` shims (`Baudrun` / `baudrun-alpha`).

```powershell
# Scoop needs git to fetch + update buckets. If `git --version`
# already prints something, skip this line.
scoop install git

scoop bucket add packetThrower https://github.com/packetThrower/scoop-bucket
scoop install baudrun                 # stable
scoop install baudrun-prerelease      # pre-release
```

If you skip the `scoop install git` line and try to add the bucket
directly, Scoop fails fast with `ERROR Git is required for buckets.
Run 'scoop install git' and try again.` — same fix.

Both manifests use the per-arch NSIS setup (`_x64-setup.exe` /
`_arm64-setup.exe`), picked by Scoop based on host architecture. Scoop
runs the installer in `/S` silent mode so you don't see the SmartScreen
prompt.

Update with `scoop update baudrun` (or `baudrun-prerelease`). Same 6h
auto-bump cadence as the Homebrew tap.

### WebView2 runtime

Scoop won't install WebView2 for you. It's already present on Windows 11
and most Windows 10 builds from 21H2 onward; if Baudrun launches with a
"WebView2 missing" message, install the **Evergreen Bootstrapper** from
[microsoft.com/edge/webview2](https://developer.microsoft.com/microsoft-edge/webview2/).

## Linux

No package-manager bucket equivalent for Linux — `apt` / `dnf` / `pacman`
each work against their own repo formats and packetThrower doesn't run an
APT/DNF mirror. The release artifacts install cleanly into each distro's
native package format:

=== "Debian / Ubuntu"

    ```sh
    curl -LO https://github.com/packetThrower/Baudrun/releases/latest/download/Baudrun_<version>_amd64.deb
    sudo apt install ./Baudrun_<version>_amd64.deb
    ```

    The `.deb` declares its GTK / WebKit2GTK / libusb dependencies so
    `apt` pulls them in automatically. A udev rule
    (`/usr/lib/udev/rules.d/60-baudrun-serial.rules`) is also installed
    so you don't need `dialout` / `plugdev` group membership to open
    serial adapters.

=== "Fedora / RHEL"

    ```sh
    curl -LO https://github.com/packetThrower/Baudrun/releases/latest/download/Baudrun-<version>-1.x86_64.rpm
    sudo dnf install ./Baudrun-<version>-1.x86_64.rpm
    ```

    Same dependency declarations and udev rule as the `.deb`.

=== "Arch"

    ```sh
    curl -LO https://github.com/packetThrower/Baudrun/releases/latest/download/baudrun-<version>-1-x86_64.pkg.tar.zst
    sudo pacman -U baudrun-<version>-1-x86_64.pkg.tar.zst
    ```

    The Arch package isn't on the AUR yet (`baudrun-bin` PKGBUILD lives
    in the repo for future submission).

=== "AppImage"

    ```sh
    curl -LO https://github.com/packetThrower/Baudrun/releases/latest/download/Baudrun_<version>_amd64.AppImage
    chmod +x Baudrun_<version>_amd64.AppImage
    ./Baudrun_<version>_amd64.AppImage
    ```

    Works on any glibc-based distro with FUSE (`libfuse2` on Ubuntu).
    AppImages don't run install hooks, so the udev rule isn't applied
    automatically — add yourself to the `dialout` group manually
    (`sudo usermod -aG dialout $USER`) or apply the rule by hand.

Substitute `<version>` with the tag you want (e.g. `0.9.2` for the
current stable). For ARM64 hosts use the matching `arm64` / `aarch64`
artifact. The full per-platform artifact table is on the
[Releases](https://github.com/packetThrower/Baudrun/releases) page and in
the [README](https://github.com/packetThrower/Baudrun/blob/main/README.md#releases).

## Direct download (any OS)

If you'd rather not go through a package manager, every release on
[GitHub Releases](https://github.com/packetThrower/Baudrun/releases) ships
the same artifacts the package managers consume:

| Platform | Artifact | Notes |
|---|---|---|
| macOS arm64 | `Baudrun_<version>_aarch64.dmg` | Apple Silicon |
| macOS amd64 | `Baudrun_<version>_x64.dmg` | Intel Macs |
| Windows x64 | `Baudrun_<version>_x64-setup.exe` | NSIS installer |
| Windows arm64 | `Baudrun_<version>_arm64-setup.exe` | Native ARM |
| Linux | `.deb` / `.rpm` / `.pkg.tar.zst` / `.AppImage` | per arch |

First-launch friction lives here — the brew/scoop paths sidestep both:

- **macOS Gatekeeper.** Direct DMGs are ad-hoc signed but not notarized.
  Right-click → Open on first launch, or `xattr -cr Baudrun.app` to
  strip quarantine.
- **Windows SmartScreen.** The NSIS installer is unsigned. Click "More
  info" → "Run anyway".

The auto-updater inside Baudrun handles signature verification on its
own (minisign keypair embedded in the binary), so once you're past the
first launch, updates are seamless regardless of how you installed.

## Pre-release channel

Pre-release tags (`vX.Y.Z-alpha.N`, `-beta.N`, `-rc.N`) trigger the same
release workflow as stable but publish under GitHub's "Pre-release" badge
and don't displace the "Latest release" pointer. Both Homebrew and Scoop
expose a separate manifest for that channel — installs land side-by-side
with stable so you can run both:

| Channel | macOS install | Windows install |
|---|---|---|
| Stable | `brew install --cask baudrun` | `scoop install baudrun` |
| Pre-release | `brew install --cask baudrun@alpha` | `scoop install baudrun-prerelease` |

Linux users grab a pre-release tag's artifact directly from the
[Releases](https://github.com/packetThrower/Baudrun/releases) page (no
"latest/download/" shortcut for pre-release because that pointer always
tracks stable).

The in-app updater can be configured to follow the pre-release channel
too — Settings → Advanced → Updates → "Include pre-releases" makes the
launch update check consider pre-release tags. This works regardless of
how you installed.

## Update

| Install path | Update command |
|---|---|
| Homebrew | `brew upgrade --cask baudrun` (or `baudrun@alpha`) |
| Scoop | `scoop update baudrun` (or `baudrun-prerelease`) |
| `.deb` | `sudo apt install ./Baudrun_<new>_amd64.deb` |
| `.rpm` | `sudo dnf install ./Baudrun-<new>-1.x86_64.rpm` |
| `.pkg.tar.zst` | `sudo pacman -U baudrun-<new>-1-x86_64.pkg.tar.zst` |
| Direct download | use the in-app updater toast on launch |

## Uninstall

| Install path | Uninstall command |
|---|---|
| Homebrew | `brew uninstall --cask baudrun baudrun@alpha` |
| Scoop | `scoop uninstall baudrun baudrun-prerelease` |
| Linux package | `sudo apt remove baudrun` / `sudo dnf remove baudrun` / `sudo pacman -R baudrun` |
| Direct download | drag `Baudrun.app` to Trash, run the uninstaller `Baudrun_<version>_x64-setup.exe /S` with `--uninstall`, or just delete the AppImage |

`brew uninstall --zap --cask baudrun` clears profiles, settings, themes,
skins, and the WebKit cache. The non-zap uninstall leaves them in
`~/Library/Application Support/Baudrun` so reinstalling is seamless.
