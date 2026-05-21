---
title: Install
description: 'Install Baudrun via Homebrew, Scoop, or direct download. Stable and pre-release channels available.'
editUrl: https://github.com/packetThrower/Baudrun/edit/main/docs/INSTALL.md
---

On macOS and Windows the recommended path is a package manager. Both
Homebrew and Scoop ship Baudrun with auto-update on `brew upgrade` or
`scoop update`, sidestep Gatekeeper and SmartScreen friction on first
launch, and expose a pre-release channel alongside stable. Linux users
grab the matching `.deb`, `.rpm`, `.AppImage`, or `.pkg.tar.zst` from
GitHub.

System requirements (OS floors, runtime dependencies) live in
[Requirements](/Baudrun/reference/requirements/). Building from source is also covered there.

## macOS (Homebrew)

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
without the right-click → Open prompt. Baudrun ships ad-hoc signed but
not notarized; the cask handles that for you so the only path that hits
Gatekeeper is a direct download.

Per-arch DMG, picked automatically based on `arch`:

| CPU | DMG |
|---|---|
| Apple Silicon (M1+) | `Baudrun_<version>_aarch64.dmg` |
| Intel | `Baudrun_<version>_x64.dmg` |

Update with `brew upgrade --cask baudrun` (or `baudrun@alpha`). The tap's
auto-bump workflow polls upstream every 6 hours, so a new tag is normally
installable within a quarter day.

## Windows (winget)

[winget](https://github.com/microsoft/winget-cli) ships with Windows 11
and modern Windows 10 builds — it's Microsoft's first-party package
manager. The
[packetThrower.Baudrun](https://github.com/microsoft/winget-pkgs/tree/master/manifests/p/packetThrower/Baudrun)
manifest points at the per-arch `.msi` installers from the
[Releases page](https://github.com/packetThrower/Baudrun/releases),
so installing is a one-liner:

```powershell
winget install packetThrower.Baudrun
# or, using the registered shorthand:
winget install baudrun
```

**Scope**: x64 only, stable releases only.

- Pre-release tags (`vX.Y.Z-beta.N`) aren't submitted to winget — the
  MSI build itself is gated on stable tags only because WiX's
  `ProductVersion` field rejects alphanumeric pre-release identifiers.
  Pre-release NSIS `-setup.exe` installers are still produced and
  shipped via Scoop's `baudrun-prerelease` channel, or you can grab
  them directly from the Releases page.
- arm64 .msi is also not currently listed in winget. arm64 Windows
  users running `winget install` get the x64 .msi installed via
  Windows 11's x86_64-on-arm64 emulation, which runs the Rust+MSVC
  binary fine in practice. For a native-arm64 install, reach for
  Scoop or the Releases page. Native-arm64 winget inclusion is
  deferred — see [TODO.md](https://github.com/packetThrower/Baudrun/blob/main/TODO.md)
  for the underlying constraint (winget's arm64 validation sandbox
  can't initialize a graphics adapter for the launch test).

Update with `winget upgrade packetThrower.Baudrun`. Manifests are
submitted manually after each stable release, so a new tag is
normally installable within a day of the GitHub Release going live.

## Windows (Scoop)

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
Run 'scoop install git' and try again.` (same fix).

Both manifests use the per-arch NSIS setup (`_x64-setup.exe` /
`_arm64-setup.exe`), picked by Scoop based on host architecture. Scoop
runs the installer in `/S` silent mode so you don't see the SmartScreen
prompt.

Update with `scoop update baudrun` (or `baudrun-prerelease`). Same 6h
auto-bump cadence as the Homebrew tap.

## Linux

There is no package-manager bucket equivalent for Linux. `apt`, `dnf`,
and `pacman` each work against their own repo formats and packetThrower
doesn't run an APT or DNF mirror. The release artifacts install cleanly
into each distro's native package format:

=== "Debian / Ubuntu"

    ```sh
    curl -LO https://github.com/packetThrower/Baudrun/releases/latest/download/Baudrun_<version>_amd64.deb
    sudo apt install ./Baudrun_<version>_amd64.deb
    ```

    The `.deb` declares its libusb dependency so `apt` pulls it in
    automatically. A udev rule
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
    automatically. Add yourself to the `dialout` group manually
    (`sudo usermod -aG dialout $USER`) or apply the rule by hand.

Substitute `<version>` with the tag you want (e.g. `0.9.7` for the
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

First-launch friction lives in this path. The brew and scoop installs
sidestep both items below:

- **macOS Gatekeeper.** Direct DMGs are ad-hoc signed but not notarized.
  Right-click → Open on first launch, or `xattr -cr Baudrun.app` to
  strip quarantine.
- **Windows SmartScreen.** The NSIS installer is unsigned. Click "More
  info" → "Run anyway".

Baudrun's in-app update check is **detection-only** — when a newer
release is published, a small amber dot appears on the sidebar's gear
icon and on **Settings → Updates**, with a "View release" button that
opens the GitHub Releases page in your browser. Downloading and
replacing the bundle is a manual step; code-signing + notarization
are on the near-term roadmap before that becomes an auto-install path.

## Pre-release channel

Pre-release tags (`vX.Y.Z-alpha.N`, `-beta.N`, `-rc.N`) trigger the same
release workflow as stable but publish under GitHub's "Pre-release" badge
and don't displace the "Latest release" pointer. Both Homebrew and Scoop
expose a separate manifest for that channel. Installs land side-by-side
with stable so both can run on the same machine:

| Channel | macOS install | Windows install |
|---|---|---|
| Stable | `brew install --cask baudrun` | `scoop install baudrun` |
| Pre-release | `brew install --cask baudrun@alpha` | `scoop install baudrun-prerelease` |

Linux users grab a pre-release tag's artifact directly from the
[Releases](https://github.com/packetThrower/Baudrun/releases) page. The
"latest/download/" shortcut always tracks stable so it isn't useful for
pre-release downloads.

The in-app update check can also follow the pre-release channel:
**Settings → Updates → "Include pre-releases"** makes the boot-time
check consider pre-release tags. The amber dot then lights up on the
next pre-release just like it does on stable.

## Update

| Install path | Update command |
|---|---|
| Homebrew | `brew upgrade --cask baudrun` (or `baudrun@alpha`) |
| Scoop | `scoop update baudrun` (or `baudrun-prerelease`) |
| `.deb` | `sudo apt install ./Baudrun_<new>_amd64.deb` |
| `.rpm` | `sudo dnf install ./Baudrun-<new>-1.x86_64.rpm` |
| `.pkg.tar.zst` | `sudo pacman -U baudrun-<new>-1-x86_64.pkg.tar.zst` |
| Direct download | watch the amber dot on the sidebar gear icon — opens **Settings → Updates** where "View release" links to the GitHub Releases page for the new artifact |

## Uninstall

| Install path | Uninstall command |
|---|---|
| Homebrew | `brew uninstall --cask baudrun baudrun@alpha` |
| Scoop | `scoop uninstall baudrun baudrun-prerelease` |
| Linux package | `sudo apt remove baudrun` / `sudo dnf remove baudrun` / `sudo pacman -R baudrun` |
| Direct download | drag `Baudrun.app` to Trash, run the uninstaller `Baudrun_<version>_x64-setup.exe /S` with `--uninstall`, or just delete the AppImage |

`brew uninstall --zap --cask baudrun` clears profiles, settings, themes,
skins, and highlight packs. The non-zap uninstall leaves them in
`~/Library/Application Support/Baudrun` so reinstalling is seamless.
