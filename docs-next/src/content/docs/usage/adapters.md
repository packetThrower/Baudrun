---
title: USB-serial adapters
description: 'Which USB-serial adapters work with Baudrun, on which platform, and through which driver.'
editUrl: https://github.com/packetThrower/Baudrun/edit/main/docs/ADAPTERS.md
---

Which adapters work with Baudrun, on which OS, through which path.
The short answer for most users: if you bought a mainstream USB-serial
cable in the last five years, it works on every OS without any setup.
This page is the long version, aimed at anyone staring at a "Connect
failed" message on industrial or rebranded gear.

## Support matrix

Legend: **✓** works out of the box, **✗** not supported through that
path, **driver** means a user-installed vendor driver is required.
"Direct USB" is Baudrun's vendored libusb-backed backend
(`src-tauri/src/usbserial/`). When that column is ✓, Baudrun opens
the device itself without any driver install regardless of what the
OS does.

| Chipset / variant | Linux (mainline kernel module) | macOS 11+ (Apple-bundled DEXT) | Windows | Direct USB |
|---|---|---|---|---|
| **FTDI** (stock VIDs `0403:*`, plus ~25 rebrand VIDs) | `ftdi_sio` ✓ | `AppleUSBFTDI` ✓ (97 PIDs covered) | FTDI VCP driver | Planned (parity) |
| **Prolific PL2303** (modern PIDs `067b:2303`, `067b:2304`, `067b:a100`, `067b:e1f1`, `0557:2008` ATEN) | `pl2303` ✓ | `AppleUSBPLCOM` ✓ | Prolific driver | Planned (parity) |
| **Prolific PL2303HXA** (legacy chip rev: TRENDnet TU-S9 and similar) | `pl2303` ✓ | `AppleUSBPLCOM` ✓ | Legacy Prolific driver on x64 only — no working ARM64 path | Planned |
| **Silicon Labs CP2102 / CP2102N / CP2104** (stock `10c4:ea60`) | `cp210x` ✓ | `AppleUSBSLCOM` ✓ | SiLabs VCP driver | ✓ |
| **Silicon Labs CP2105** dual-UART (stock `10c4:ea70`) | `cp210x` ✓ | `AppleUSBSLCOM` ✓ | SiLabs VCP driver | ✓ (first UART only; second planned) |
| **Silicon Labs CP2108** quad-UART (stock `10c4:ea71`) | `cp210x` ✓ | ✗ (not in Apple's id_table) | SiLabs VCP driver | ✓ (first UART only) |
| **CP210x vendor rebrands** (e.g. Siemens RUGGEDCOM `0908:01ff`) | Needs manual `echo VID PID > /sys/bus/usb-serial/drivers/cp210x/new_id` | ✗ (not in Apple's id_table) | Vendor-supplied driver | ✓ |
| **WCH CH340G / CH340B / CH340C** (stock `1a86:7523`) | `ch341` ✓ | `AppleUSBCHCOM` ✓ | WCH driver | Planned |
| **WCH CH9102 / CH343** (stock `1a86:55d4`) | `ch341` ✓ | `AppleUSBCHCOM` ✓ | WCH driver | Planned |
| **CH34x other PIDs / rebrands** | Manual sysfs bind | ✗ (not in Apple's id_table) | WCH driver | Planned |
| **CDC-ACM** (USB-C consoles, Arduino, RuggedCom RST2228, many SoC debug ports) | `cdc_acm` ✓ | `AppleUSBCDCACMData` ✓ | `usbser.sys` ✓ | N/A (every OS covers this natively) |

## What the columns actually mean

**Linux (mainline kernel module).** The `cp210x`, `ftdi_sio`, `pl2303`,
`ch341`, and `cdc_acm` modules are all in the upstream kernel and every
distro that matters (Ubuntu 22.04+, Debian 12+, Fedora 40+, Arch,
openSUSE Tumbleweed) ships them loaded. Plug in a stock-VID device,
`/dev/ttyUSB*` or `/dev/ttyACM*` appears. No user action required.

*Permissions:* `/dev/tty*` device nodes are owned by `root:dialout`
(Debian/Ubuntu) or `root:uucp` (some others). Opening them as a regular
user traditionally means adding yourself to `dialout` and logging out:

```sh
sudo usermod -aG dialout $USER
# log out and back in
```

Baudrun's `.deb`, `.rpm`, and `.pkg.tar.zst` packages skip that dance
by shipping a udev rule
([`/usr/lib/udev/rules.d/60-baudrun-serial.rules`](https://github.com/packetThrower/Baudrun/blob/main/packaging/linux/60-baudrun-serial.rules))
that tags USB-backed TTYs and known USB-serial VIDs with
`TAG+="uaccess"`. systemd-logind turns that into an ACL entry for
whoever is currently logged in at the console, so Baudrun can open the
port with no group membership and no logout. The package post-install
reloads udev so the rule applies immediately, with no re-plug needed
for already-attached devices.

The AppImage doesn't install system files, so the dialout path is the
only option there. If you want the udev-rule convenience on an
AppImage-based install, grab the rule file from the repo and drop it
in place yourself:

```sh
sudo curl -fsSL -o /usr/lib/udev/rules.d/60-baudrun-serial.rules \
  https://raw.githubusercontent.com/packetThrower/Baudrun/main/packaging/linux/60-baudrun-serial.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=tty --subsystem-match=usb --action=change
```

The rebrand cases are the exception: the kernel's id_table for each
driver is an explicit list of `(VID, PID)` pairs. Siemens's RUGGEDCOM
under `0908:01FF` isn't in that list, so `cp210x` doesn't claim the
device on hotplug. You can force a one-time bind with:

```sh
sudo modprobe cp210x
echo 0908 01ff | sudo tee /sys/bus/usb-serial/drivers/cp210x/new_id
```

Or you can let Baudrun's direct-USB backend handle it. The libusb
path picks up the device without any sysfs setup.

**macOS 11+ (Apple-bundled DEXT).** Apple ships four USB-serial
DriverKit extensions in `/System/Library/DriverExtensions/`:

- `com.apple.DriverKit-AppleUSBFTDI.dext` covers 97 VID:PID pairs
- `com.apple.DriverKit-AppleUSBPLCOM.dext` covers 5 pairs (Prolific) — matches by VID:PID only, with no `bcdDevice` constraint, so the legacy PL2303HXA chip rev (TRENDnet TU-S9 etc.) is bound out of the box. The "Prolific stops supporting older chips" story is a Windows driver issue, not a macOS one.
- `com.apple.DriverKit-AppleUSBSLCOM.dext` covers 2 pairs (CP210x)
- `com.apple.DriverKit-AppleUSBCHCOM.dext` covers 2 pairs (CH340)

The catch is that each DEXT matches only the specific VID:PID values
hard-coded in its `Info.plist`. `AppleUSBSLCOM` covers stock SiLabs
CP2102 (`10c4:ea60`) and CP2105 (`10c4:ea70`), but not CP2108 or
rebrands. `AppleUSBCHCOM` covers the two most common CH34x PIDs and
misses the rest. `AppleUSBPLCOM` covers all PL2303 chip revisions
including the legacy HXA — that chip's well-known rejection issue
is purely a Windows-driver story.

Anything outside those lists falls through to `AppleUSBHostCompositeDevice`,
the generic composite-device shim that acknowledges the device exists
but doesn't provide a `/dev/cu.*` node. That's when you'd see a
"driver not loaded" banner before the direct-USB backend was wired
in; now the in-tree `usbserial` module opens those devices directly.

Older macOS versions (pre-Big Sur) don't have these DEXTs at all; you'd
install SiLabs VCP, FTDI VCP, and Prolific drivers the way everyone did
in 2015. Baudrun supports macOS 11+ only, so that path isn't officially
on the radar, but the library works there.

**Windows.** No Microsoft-bundled equivalent of the Apple DEXTs for
FTDI / Prolific / SiLabs / WCH. The CDC-ACM class driver (`usbser.sys`)
is built in and covers USB-C consoles and CDC-ACM devices generally,
but the vendor-specific chipsets each need their own driver. The install
flow is well-trodden: every vendor ships a signed driver package that
installs via double-click and the device appears as `COMn`.

Because shipping a signed userspace USB driver on Windows is its own
cottage industry, the direct-USB backend is a no-op there. It falls
through to the `serialport` crate, which opens the `COMn` that the
vendor driver created.

*Windows 11 ARM (Snapdragon laptops, Apple Silicon VMs):* the
ARM64-vs-x64 driver story matters. Vendor drivers for x64 Windows
do not load under emulation because Windows refuses to load
non-ARM64 kernel modules in an ARM64 kernel. Per-vendor state:

- **SiLabs CP210x:** ARM64 driver ships via Windows Update. Plug in,
  it works. Lowest friction option.
- **FTDI:** ARM64 driver exists (VCP 2.12.36.20+) but the auto-installer
  isn't ARM64-compatible. Manual install: download the ARM64 driver
  package from ftdichip.com, unzip, point Device Manager at the folder.
  Once installed it works as well as it does on x64.
- **WCH CH340/CH343:** WCH ships ARM64 drivers; quality varies by chip
  generation. CH343 is better supported than CH340.
- **Prolific PL2303 (modern chips — REV_05+):** Prolific driver
  v4.6.0.0+ ships ARM64 binaries; v6.5.0.0 (Feb 2026) is current.
- **Prolific PL2303HXA (legacy: TU-S9 etc.):** no working path. The
  modern Prolific driver rejects HXA, the legacy driver is x64-only
  and unsigned for ARM64, and the patched community drivers
  (theAmberLion/Prolific, daniel-marschall) are also x64-only.
  Replace the cable with FTDI or CP210x.

**Direct USB** is the in-tree `src-tauri/src/usbserial/` module
(originally vendored from
[`packetThrower/usbserial-go`](https://github.com/packetThrower/usbserial-go)
and ported to Rust on top of `rusb`). No vendor driver needed; no
kext / DEXT / `.sys`. It shows up in the port picker as a separate
entry labeled `USB · VID:PID · product name`. When both paths exist
for the same device (e.g. Linux with `cp210x` already bound), the OS
path wins in the UI; the library doesn't try to claim already-bound
devices.

Current status: **CP210x only**, including vendor rebrands. CH340 is
next; FTDI and PL2303 are planned for API parity but those chipsets
already work through the OS on everything that matters.

## Picking a known-good cable

If you're buying a USB-to-serial cable and want it to work across
Linux, macOS, and Windows without any driver chase, in rough order of
preference:

1. **FTDI-based cables** (FT232R, FT232H, FT2232H). Apple covers
   essentially everything under the FTDI VID and all common rebrands
   (97 PIDs). Stable silicon, sensible drivers, widely cloned but
   the real ones are stable.
2. **Silicon Labs CP210x cables**, specifically CP2102 or CP2104
   (`10c4:ea60`). Works on modern macOS out of the box. Avoid CP2108
   (quad-UART) if you want it to work on macOS without Baudrun's
   direct-USB path.
3. **USB-C consoles on newer gear** (HPE/Aruba, Cisco, some RuggedCom
   switches). CDC-ACM, zero-driver on every OS.

What to avoid if you can:

- **CH340-based cables** on Windows without a pre-existing driver
  install. They're cheap and everywhere (every no-name ESP32 dev
  board, most "3-pack USB serial cables" on Amazon), but the driver
  flow on Windows is less polished than FTDI's.
- **Pre-2016 Prolific PL2303HXA cables** on Windows. Prolific's
  current Windows driver rejects them as counterfeit even when they're
  genuine. The legacy x64 driver works but isn't signed for ARM64,
  so Windows 11 ARM has no driver path at all. macOS and Linux both
  drive these cables fine via their bundled drivers.
