# AGENTS.md

Guidance for AI agents and human contributors. Keep changes consistent with what's
already here — match the surrounding code, and prefer deleting over adding.
[CONTRIBUTING.md](CONTRIBUTING.md) is the long form; this is what to hold in your head.

## What this is

Baudrun — a serial terminal for network engineers, on macOS / Windows / Linux.
Rust + [GPUI](https://www.gpui.rs) + gpui-component, with `alacritty_terminal` as the
VT parser and grid (gpui does the rendering). One binary crate at the repo root
(package `baudrun`, binary `Baudrun`), GPL-3.0-or-later. macOS is the primary
dev/test platform; Windows and Linux are verified less often, so keep every change
cross-platform and gate platform code with `#[cfg(...)]`.

## Layout

- `src/main.rs` — app entry, menubar / dock, window setup.
- `src/app_view/` — the main window: sidebar, profile editor, session header,
  transfer UI, chrome.
- `src/settings_view/` — the standalone Settings window (theme preview, shortcuts).
- `src/terminal_view.rs`, `terminal_grid.rs`, `term_bridge.rs` — the alacritty →
  gpui terminal: renderer, cell grid, colour/attribute translation.
- `src/serial_io.rs` — serial read/write threads + write channel.
- `src/settings_bus.rs`, `profiles_bus.rs`, `skin_tokens.rs` — app-scoped state
  entities and change broadcast.
- `src/data/` — the pure data layer, **no UI deps**: profiles, settings, themes
  (incl. `.itermcolors`), skins, highlight packs, XMODEM/YMODEM (`transfer.rs`),
  port enumeration + chipset detection (`serial/`), libusb CP210x backend (`usbserial/`).
- `resources/` — compiled-in skins, themes, highlight packs, icons, Info.plist.
- `locales/` — UI strings (`en.yml` is the source; see `locales/README.md`).
- `scripts/virtual-serial`, `scripts/transfer-tests` — dev tools, each its own
  Cargo workspace (not part of the root build).
- `docs-next/` — Astro Starlight docs site (deploys via `docs.yml`).

## Build, run, and the gate

```
cargo run                                # dev launch (loopback if no port)
cargo run -- /dev/cu.usbserial-XXX       # attached to a real port
```

Before every commit, run what CI runs (`.github/workflows/ci.yml`) — these must pass:

```
cargo fmt --check                        # also in each scripts/*/ dir
cargo clippy --all-targets -- -D warnings
cargo test
cargo deny check
```

`cargo check` alone is **not** the gate — clippy is, and any warning fails the build.
Touching `src/data/transfer.rs` or protocol code: also run the wire-level transfer
suite (`scripts/transfer-tests`, Unix only; CI doesn't run it). GUI changes need a
real `cargo run` — regressions there don't show in tests.

Say "optimized build" for `cargo build --release`; "release" means a product release.

## Dependencies

- gpui, gpui_platform, gpui-component and gpui-component-assets move together, in
  one commit. The assets rev must match gpui-component's or icons break.
  Dependabot's standalone gpui PRs are always wrong — close them.
- `deny.toml` ignores carry a reason and a "revisit at each gpui pull" horizon;
  drop an ignore once `cargo deny` reports it as not encountered.
- Don't add a dependency for what a few lines do.

## Code conventions

- **Surgical diffs.** Every changed line traces to the change. Don't reformat or
  "improve" adjacent code; leave no orphaned `dead_code` / `unused`.
- **No speculative abstraction** — no one-impl traits, factories, or config nobody sets.
- **`src/data/` stays UI-free** so it can be tested without gpui.
- **User-facing strings go through i18n** (`locales/en.yml`), not literals.
- Comments explain *why*; this codebase documents its non-obvious choices at the
  point of use (see the dependency comments in `Cargo.toml`). Keep that up.

## Commits, changelog, docs

Conventional commits with a scope: `fix(serial): …`, `build(deps): …`,
`docs(changelog): …`. Body explains why. One topic per PR.

User-facing changes go in `CHANGELOG.md` under `[Unreleased]` in the same PR;
dev-tooling changes stay out of it. If a change alters behaviour described in
`docs-next/`, update that page in the same PR.

## Releases

Stable = move `[Unreleased]` to a `## [x.y.z]` section in `CHANGELOG.md` and push tag
`vX.Y.Z`; pre-release = `vX.Y.Z-beta.N` / `-rc.N`. The version comes from the tag —
`release.yml` patches `Cargo.toml` at build time, so don't bump it by hand.
`after_release.yml` submits stable releases to winget automatically; the Homebrew tap
and Scoop bucket pick up new releases on their own 6-hourly pollers.
