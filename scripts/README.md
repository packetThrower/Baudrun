# scripts/

Developer tooling that isn't part of the shipped Baudrun binary.
Each subdir is a standalone Cargo workspace, isolated from the main
crate's build graph so contributors don't pay the gpui / wgpu /
alacritty_terminal compile cost for what's a few hundred lines of
test plumbing.

| Subdir | What |
|---|---|
| [`virtual-serial/`](virtual-serial/) | Baud-throttled virtual serial pair. Creates two pty endpoints wired together with accurate inter-byte timing, so code paths that care about real baud-rate pacing (paste safety, file-transfer progress, slow-link behaviour) can be exercised end-to-end without a USB-serial adapter. |
| [`transfer-tests/`](transfer-tests/) | Headless test harness driving Baudrun's hex / XMODEM / YMODEM **send** paths against the `virtual-serial` bridge. Reads `src/data/transfer.rs` directly via `#[path]` so the harness shares the real protocol code, not a parallel reimplementation. |
| `transfer-tests/regen-fixtures.sh` | Regenerates the deterministic test payloads under [`test/transfers/`](../test/transfers/) from a fixed RNG seed. |

Unix only (macOS + Linux). Both tools depend on pty primitives that
Windows doesn't expose; Windows dev machines should pair com0com
ports and talk to real hardware for baud-sensitive tests.

## Quick start

From the repo root:

```sh
# build both tools (one-time)
(cd scripts/virtual-serial  && cargo build --release)
(cd scripts/transfer-tests  && cargo build --release)

# (re)generate fixtures (one-time; idempotent thereafter)
./scripts/transfer-tests/regen-fixtures.sh

# run the automated suite
./scripts/transfer-tests/target/release/transfer-tests
```

Each subdir has its own README with full flag reference and per-tool
detail. The manual TESTING.md playbook (for poking the UI rather
than the wire) lives at
[`scripts/virtual-serial/TESTING.md`](virtual-serial/TESTING.md).
