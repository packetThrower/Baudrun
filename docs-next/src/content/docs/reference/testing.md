---
title: Testing
description: 'How Baudrun is tested — the unit suite, the wire-level transfer harness, and how to run them locally.'
editUrl: https://github.com/packetThrower/Baudrun/edit/main/docs-next/src/content/docs/reference/testing.md
---

Baudrun ships two layers of automated tests plus a manual playbook
for the parts that need a human at the keyboard.

| Layer | What it covers | Where |
|---|---|---|
| Unit tests | Pure-data invariants: JSON round-trips, parser edge cases, protocol checksums / CRCs, hex input validation, highlight runtime. | `cargo test` in the repo root. |
| Wire-level transfer tests | Real XMODEM / YMODEM / Send-Hex code paths against a baud-paced virtual pty pair. Byte-diff against deterministic fixtures. Unix-only. | `scripts/transfer-tests/` |
| Manual playbook | Things that need eyes on the UI: import dialogs, paste safety prompts, scrollback retention, transfer cancel UX. | `scripts/virtual-serial/TESTING.md` in the source tree. |

---

## Unit tests

Standard Cargo. Runs on macOS, Windows, and Linux in CI on every
push.

```sh
cargo test
```

The tests live alongside the code they exercise (`#[cfg(test)] mod
tests { ... }` at the bottom of each module). No external fixtures,
no I/O, no temp files. Fast — the whole suite finishes in seconds.

---

## Wire-level transfer tests

Drive Baudrun's actual send paths against a virtual serial bridge
and byte-diff the output. The harness exists because the production
app is the wrong shape for protocol regression testing — gpui UI,
profile picker, port enumeration — but the protocol code itself
(under `src/data/transfer.rs`) is fully decoupled.

The harness lifts `transfer.rs` verbatim via `#[path]` so it
exercises the same XMODEM / YMODEM state machines the app ships.
Refactors there are picked up automatically at the harness's next
build — there's no parallel copy to keep in sync.

### Setup (one-time)

```sh
# Build the bridge and the harness.
(cd scripts/virtual-serial && cargo build --release)
(cd scripts/transfer-tests && cargo build --release)

# Generate the deterministic fixture set under test/transfers/.
./scripts/transfer-tests/regen-fixtures.sh
```

You'll also need `lrzsz` for the XMODEM / YMODEM receiver
subprocess:

```sh
brew install lrzsz                # macOS — installs lrb / lrx
sudo apt install lrzsz            # Debian / Ubuntu — installs rb / rx
```

The harness searches both name conventions.

### Running

```sh
./scripts/transfer-tests/target/release/transfer-tests              # all 11 cases
./scripts/transfer-tests/target/release/transfer-tests --quick      # skip 1 MiB T9
./scripts/transfer-tests/target/release/transfer-tests --tests hex  # subset
```

### Results

Last verified **2026-05-15** on macOS arm64:

| Test | What | Baud | Wall | Result |
|---|---|---:|---:|:---:|
| T1 | hex ASCII (`Hello`) | 9 600 | 0.01 s | ok |
| T3 | hex binary / non-printable | 9 600 | 0.01 s | ok |
| T5 | hex 1 KiB random | 9 600 | 1.35 s | ok |
| T11 | YMODEM 512 B over slow link | 9 600 | 5.92 s | ok |
| T6 | YMODEM 4 KiB | 115 200 | 3.69 s | ok |
| T9 | YMODEM 1 MiB | 115 200 | 126.00 s | ok |
| T7c | XMODEM classic (128 B / checksum) | 115 200 | 1.56 s | ok |
| T7C | XMODEM-CRC (128 B / CRC-16) | 115 200 | 1.56 s | ok |
| T7k | XMODEM-1K (1024 B / CRC-16) | 115 200 | 1.55 s | ok |
| T8 | XMODEM single-block (SUB padding) | 115 200 | 1.08 s | ok |
| T10 | YMODEM cancel mid-transfer | 115 200 | 1.18 s | ok |

**11 / 11 passed, 143.9 s wall.** T9 dominates the run time. The
other ten cases finish in well under 18 seconds combined.

### What each test ID exercises

The IDs (T1 … T11) match the section numbering in
[`scripts/virtual-serial/TESTING.md`](https://github.com/packetThrower/Baudrun/blob/main/scripts/virtual-serial/TESTING.md),
which spells out the manual version of each test. T2 (hex input
format equivalence) and T4 (hex input validation) are UI-layer
concerns and aren't automated — `parse_hex_string` is unit-tested
separately if that coverage matters to you.

| ID | Category | Notes |
|---|---|---|
| T1 | hex round-trip | "Hello" — printable ASCII baseline. |
| T3 | hex round-trip | `00 01 02 ff fe 7f` — control bytes + 0xff + DEL. |
| T5 | hex round-trip | 1 KiB random — exercises the rate-limited write path at the 9600-baud floor. |
| T6 | YMODEM | 4 KiB at 115200 — the "happy path" file transfer. |
| T9 | YMODEM | 1 MiB at 115200 — many-block stress, retry-loop sensitivity. |
| T11 | YMODEM | 512 B at 9600 — slow-link timing margins, lrzsz's own per-block timeouts. |
| T7c | XMODEM classic | 4 KiB with 128-byte blocks + 8-bit checksum. Receiver-initiated with `NAK`. |
| T7C | XMODEM-CRC | 4 KiB with 128-byte blocks + CRC-16. Receiver-initiated with `'C'`. |
| T7k | XMODEM-1K | 4 KiB with 1024-byte blocks + CRC-16. Header byte (`STX` vs `SOH`) signals block size. |
| T8 | XMODEM single-block | 6 B (`"hello\n"`) — exercises the SUB-padding path for the last block. |
| T10 | cancel mid-transfer | 1 MiB YMODEM with cancel tripped 150 ms in. Asserts `TransferError::Cancelled`, verifies the bridge survives. |

### Why this layer exists

Unit tests catch invariant violations (the checksum of a known
block must be 0x5A); the wire layer catches integration
regressions (the protocol state machine actually drives the
state-machine partner on the other side of the wire to
completion). Both layers find things the other misses. The
manual playbook then covers UI-level concerns no harness can
sanely automate — does the cancel button actually look pressed
during a 1 MiB transfer, does the import-pack dialog give a
useful error on malformed JSON.

For the full design — why no threads in the read path, why
`#[path]` over a library extraction, why deterministic seed —
see the harness README in the source tree.
