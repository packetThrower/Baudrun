# Baudrun: alacritty + gpui prototype

A research spike. Self-contained Cargo project that's *not* part of
the main `src-tauri` workspace. Goal: prove (or disprove) that
`alacritty_terminal` for VT parsing + `gpui` for rendering can match
or beat the current xterm.js + Tauri stack on Windows specifically.

## Why this exists

Even after v0.9.6-beta.3's Windows fixes (DOM renderer default,
async serial commands, CSS-injection backstop), per-keystroke
latency on Windows has noticeable lag. The remaining bottleneck
appears to be the WebView2 IPC channel itself: every keystroke
crosses the JS / Rust boundary and the reverse-direction echo
crosses it again. A native stack moves all of this in-process,
removing the IPC entirely.

Zed (the editor) is the existence proof for this combination —
its embedded terminal pane uses `alacritty_terminal` + `gpui`,
runs at 120fps, and feels native on every platform Zed ships to.

## Scope of the experiment

**In scope** (the questions we're trying to answer):

1. Does `alacritty_terminal` work as an embeddable VT parser when
   driven by serial bytes (instead of a PTY)?
2. Does `gpui` produce smooth typing on Windows in a real-world
   build? Specifically: the per-keystroke-echo latency that's
   the user-visible bottleneck today.
3. How much UI scaffolding effort is required to reach feature
   parity with Baudrun's current chrome (profile sidebar, settings,
   highlight packs, theme picker, hex view, send-file, etc.)?

**Out of scope** (initially):

- Theme system, skins, syntax highlighting — defer until basic
  terminal rendering is proven.
- Multi-window — prove single-window first.
- Auto-updater, profile persistence, settings — focus on the
  rendering path.
- macOS / Linux parity — Windows is where the perf pain is, so
  that's where the first measurements happen.

## Status

Initial scaffold. Cargo.toml lists the deps; `src/main.rs` is a
hello-world stub. Run with:

```bash
cd prototype
cargo run
```

…and verify a window opens with some text in it. That's the bar
for the first checkpoint — if even getting `gpui` to compile and
draw a window is painful, that's signal enough about the cost of
this rewrite.

## Notes on dependency state

- `alacritty_terminal` is published on crates.io. Stable, used by
  Zed and Wezterm. Pinned at `0.26`.
- `gpui` is now externally published on crates.io as `gpui = "0.2"`.
  Standalone — no longer needs a git pin to the Zed monorepo.

## Build prerequisites

- **macOS**: gpui compiles its Metal shaders during build, which
  requires the `metal` shader compiler. That binary ships with
  **full Xcode** (App Store, ~8 GB), not the slimmer Command Line
  Tools. If `cargo build` fails with
  `xcrun: error: unable to find utility "metal"`, install Xcode
  from the App Store and run `sudo xcode-select -s /Applications/Xcode.app`
  to point xcrun at it.
- **Windows**: WebView2 isn't involved here — gpui draws via
  Direct2D / DirectWrite under the hood. No special toolchain
  beyond a current Rust + MSVC build environment.
- **Linux**: gpui requires a current GTK / wayland setup; specifics
  depend on the version of gpui pinned. Check gpui's README on
  crates.io for the current list.

## Decision criteria

After 1-2 weeks of focused effort on the spike, the question is:
**did getting "Windows typing latency feels native" come within
sight, or is the rest of the rewrite (chrome, persistence, etc.)
clearly going to cost months?**

If the answer is "in sight," promote the prototype to a real
branch and start the proper migration. If "months of chrome
work," archive this branch and ship more incremental Windows
perf fixes on the existing stack.
