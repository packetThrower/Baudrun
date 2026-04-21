# Security policy

## Supported versions

Only the most recent tagged release receives security updates. Tags
follow CalVer (`YYYY.MM.DD-patch`); grab the latest from the
[Releases page](https://github.com/packetThrower/Seriesly/releases).

## Reporting a vulnerability

**Please don't file public issues for security problems.** Use
GitHub's private vulnerability reporting instead:

1. Go to the repository's **Security** tab.
2. Click **Report a vulnerability**.
3. Fill in the details.

GitHub routes the report privately to the maintainers. The advisory
stays hidden until a fix ships.

If you can't use GitHub's flow, the maintainer's email is in the
project's `wails.json` under `author.email`.

## What to expect

- **Acknowledgement** within a few business days.
- **Initial assessment** (confirmed / not-a-bug / won't-fix-and-why)
  within two weeks.
- **Fix timeline** scaled to severity. Seriesly doesn't run a paid
  SLA; fixes ship when they're ready and tested.
- **Credit** in the release notes if you'd like it. Say so in your
  report.

## Scope

In scope:

- The Seriesly desktop app (Go backend, Svelte frontend, Wails
  runtime integration).
- The GitHub Actions workflows that build the release artifacts.
- Configuration file handling (profiles, themes, skins, settings
  JSON parsers).

Out of scope (report to the respective upstream):

- Vulnerabilities in Wails itself — report to
  [wailsapp/wails](https://github.com/wailsapp/wails).
- Vulnerabilities in `go.bug.st/serial`, xterm.js, or any other
  third-party dependency — report upstream.
- Vulnerabilities in USB-serial chipset vendor drivers — those are
  vendor issues (SiLabs, FTDI, Prolific, etc.).
- Physical-access attacks on the machine running Seriesly. A serial
  terminal is a trust-the-operator tool by design.

## Hardening notes

Seriesly is unsigned on all platforms today; verifying a release
means checking the GitHub Actions run that produced it. Signed
releases are on the roadmap (see [TODO.md](TODO.md)).

The app does not make outbound network connections. It talks to
local serial ports and writes files under the user's config
directory. If you observe network activity from the app, that is
itself worth a security report.
