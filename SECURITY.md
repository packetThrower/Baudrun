# Security policy

## Supported versions

Only the most recent tagged release receives security updates. Tags
follow SemVer (`vX.Y.Z` or `vX.Y.Z-{alpha,beta}.N` for pre-releases);
grab the latest from the
[Releases page](https://github.com/packetThrower/Baudrun/releases).

## Reporting a vulnerability

**Please don't file public issues for security problems.** Use
GitHub's private vulnerability reporting instead:

1. Go to the repository's **Security** tab.
2. Click **Report a vulnerability**.
3. Fill in the details.

GitHub routes the report privately to the maintainers. The advisory
stays hidden until a fix ships.

If you can't use GitHub's flow, the maintainer's email is in
[`Cargo.toml`](Cargo.toml)'s `package.authors` field.

## What to expect

- **Acknowledgement** within a few business days.
- **Initial assessment** (confirmed / not-a-bug / won't-fix-and-why)
  within two weeks.
- **Fix timeline** scaled to severity. Baudrun doesn't run a paid
  SLA; fixes ship when they're ready and tested.
- **Credit** in the release notes if you'd like it. Say so in your
  report.

## Scope

In scope:

- The Baudrun desktop app (single-crate Rust, `gpui` UI,
  `alacritty_terminal` VT parser, `serialport` for I/O).
- The GitHub Actions workflows that build the release artifacts
  (`.github/workflows/{ci,release,docs}.yml`).
- Configuration file handling — the JSON parsers + on-disk paths
  for profiles, themes, skins, highlight packs, and settings.
- The `cargo-packager` bundle metadata + Linux postinst script
  (`packaging/linux/`).
- The bundled Homebrew tap (`packetThrower/homebrew-tap`) and the
  cask DSL it ships.

Out of scope (report to the respective upstream):

- Vulnerabilities in `gpui` itself — report to
  [zed-industries/zed](https://github.com/zed-industries/zed).
- Vulnerabilities in `alacritty_terminal`, `serialport`, `rusb`,
  or any other third-party crate — report upstream.
- Vulnerabilities in USB-serial chipset vendor drivers — those are
  vendor issues (SiLabs, FTDI, Prolific, etc.).
- Physical-access attacks on the machine running Baudrun. A serial
  terminal is a trust-the-operator tool by design.

## Hardening notes

Baudrun is currently unsigned on all platforms; verifying a release
means checking the GitHub Actions run that produced it. Code-signing
+ notarization on macOS and Windows are on the near-term roadmap
(see [TODO.md](TODO.md)). The Homebrew cask runs
`xattr -dr com.apple.quarantine` on install to dodge the Gatekeeper
prompt — that workaround goes away once notarization ships.

### What the app touches

- **Local serial ports** via `serialport` + (on macOS) direct libusb
  fallback via the vendored `rusb`.
- **The OS app-support directory** for profiles, themes, skins,
  highlight packs, and settings JSON. Path is
  `~/Library/Application Support/Baudrun` on macOS,
  `%APPDATA%\Baudrun` on Windows, `~/.config/Baudrun` (or
  `$XDG_CONFIG_HOME/Baudrun`) on Linux.
- **No outbound network connections.** If you observe network
  activity from the app, that is itself worth a security report.

### Session logs may contain device-side secrets

When per-profile session logging is enabled (Settings → Advanced →
"Log session output"), every byte received from the serial device is
written to a sanitised `.log` file under
`<config-dir>/logs/<profile>_<timestamp>.log`. The sanitiser strips
ANSI / OSC / CSI escapes but **does not redact secrets**. User-typed
input is typically not echoed by network devices, but the device
itself may print sensitive material in plain text — common cases
include `show running-config` dumps, SNMP communities, IPSec
pre-shared keys, TACACS / RADIUS exchanges, and login banners.

Logs persist indefinitely at default user-readable permissions. If
you share, back up, or screenshot these files, treat them as
credential material. Disable session logging when working on
production gear unless you specifically need the transcript.

### Skin / theme / highlight-pack imports

User imports go through `Store::import` paths that slugify the
declared `id` field before it becomes a filename — a malicious JSON
declaring `"id": "../../foo"` cannot escape its imports directory.
Validate any third-party JSON you import the same way you would any
other downloaded file: read it first, don't blindly trust authors
you don't know.
