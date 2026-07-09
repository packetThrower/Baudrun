# Contributing to Baudrun

Thanks for the interest. Baudrun is a small project but the
contributor surface is real: it runs on three desktop OSes, ships
in five package formats, and integrates with real serial hardware.
This page covers the local dev loop, what's expected from a PR, and
the project's conventions so you don't have to reverse-engineer
them from `git log`.

## Prerequisites

Rust stable. The crate targets stable Rust and tracks the current
release; no nightly features.

System libraries (for `libusb` + `libudev` discovery):

| Platform | One-liner |
|---|---|
| macOS | `brew install libusb pkg-config` |
| Debian / Ubuntu | `sudo apt install libusb-1.0-0-dev libudev-dev pkg-config` |
| Fedora | `sudo dnf install libusb1-devel systemd-devel pkgconf-pkg-config` |
| Arch | `sudo pacman -S libusb pkgconf` |
| Windows | nothing extra — gpui's DirectX backend ships with Windows 10+ |

For the transfer-test harness you'll also want
[`lrzsz`](https://www.ohse.de/uwe/software/lrzsz.html):

```sh
brew install lrzsz                # macOS — installs lrb / lrx / lsb / lsx
sudo apt install lrzsz            # Debian / Ubuntu — installs rb / rx / sb / sx
```

The harness searches both name conventions, so either works.

## Building and running

```sh
git clone git@github.com:packetThrower/Baudrun.git
cd Baudrun
cargo run                                # dev launch (loopback if no port)
cargo run -- /dev/cu.usbserial-XXX       # attached to a real port
cargo build --release                    # optimized build at target/release/Baudrun
```

The single-crate root means `cargo` does the right thing without
configuration; no workspace setup, no feature flags to remember.

## Running tests

**Unit tests** (fast, no I/O):

```sh
cargo test
```

**Wire-level transfer tests** (Unix only, ~144 s for the full
suite or ~18 s with `--quick`):

```sh
# one-time setup
(cd scripts/virtual-serial && cargo build --release)
(cd scripts/transfer-tests && cargo build --release)

# regenerate fixtures if test/transfers/ is missing files
./scripts/transfer-tests/regen-fixtures.sh

# run
./scripts/transfer-tests/target/release/transfer-tests
```

What the harness covers and how it works:
[scripts/transfer-tests/README.md](scripts/transfer-tests/README.md).
The matching manual playbook (for poking the UI rather than the wire):
[scripts/virtual-serial/TESTING.md](scripts/virtual-serial/TESTING.md).

If you touch `src/data/transfer.rs` or any of the protocol code, run
the transfer suite locally before opening the PR — CI doesn't
exercise it yet.

## Commit style

Conventional Commits with a scope. The scope names a directory, a
feature area, or a build target — whatever's most descriptive.

```
feat(scope):    new user-facing feature
fix(scope):     bug fix
refactor(scope):no behaviour change
docs(scope):    documentation only
test(scope):    test additions or fixes
build(scope):   build / packaging changes
chore(scope):   misc maintenance
```

The body, when present, explains **why** rather than what — the diff
already shows what. Examples from `git log`:

- `feat(skins,themes): add Tokyo Night skin + matching theme`
- `feat(sidebar): collapsible icon strip with status-coloured profile glyphs`
- `feat(virtual-serial): port the Go test rig to Rust`
- `test(transfers): automated wire-level harness for send paths`
- `docs(sitemap): emit lastmod so Google has a freshness signal`

User-facing changes go in
[CHANGELOG.md](CHANGELOG.md) under `[Unreleased]`. Dev-tool
changes (new scripts, refactors of internal modules, CI tweaks)
stay out of CHANGELOG — `git log` is enough.

## Pull requests

- One topic per PR. A skin + a protocol fix is two PRs.
- Keep the diff minimal. Don't reformat unrelated files.
- If the change is user-visible, update CHANGELOG.md in the same PR.
- If the change touches user-visible behaviour described in
  [docs-next/](docs-next/), update the page in the same PR. Stale
  docs are worse than missing docs.
- Squash commits when the history is exploratory; keep them split
  when each commit stands on its own.

CI (`.github/workflows/ci.yml`) runs `cargo check / clippy / test`
on macOS, Windows, and Linux. Treat clippy warnings as build errors
— the project keeps a clean lint baseline.

## Translations

Adding a language is one of the easiest ways to contribute and
needs no Rust — copy `locales/en.yml` to `locales/<code>.yml`,
translate the values, and add one line to `SUPPORTED` in
`src/i18n.rs`. Native fluency (ideally from someone who consoles
into gear in that language) matters far more than tooling; partial
translations are fine — untranslated strings fall back to English.
Full step-by-step in [`locales/README.md`](locales/README.md).

## AI-assisted contributions

Baudrun is co-developed with Claude (see [AI-USAGE.md](AI-USAGE.md)
for the division-of-labour history). AI-assisted PRs are welcome
under the same standards as hand-written ones — same commit style,
same tests, same review bar. Reviewers will spot-check that the
author understands what the change does; a PR description
explaining the trade-offs is more valuable than a transcript.

## Where to find things

- **Code layout, build instructions, install:** [README.md](README.md).
- **Reference docs (user-facing):** the Astro/Starlight site at
  [packetthrower.github.io/Baudrun](https://packetthrower.github.io/Baudrun/),
  sources under [docs-next/](docs-next/).
- **What's coming:** [TODO.md](TODO.md) — durable to-dos, not a
  detailed roadmap.
- **Security policy:** [SECURITY.md](SECURITY.md).
- **License:** GPL-3.0-or-later. See [LICENSE](LICENSE).
