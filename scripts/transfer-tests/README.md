# transfer-tests

Headless test harness that drives Baudrun's hex / XMODEM / YMODEM
**send** paths against the [`virtual-serial`](../virtual-serial/) bridge
and byte-diffs the received output against deterministic fixtures.
One command, one verdict per test — useful for catching protocol
regressions before they reach a release without ever launching the
Baudrun UI.

The harness lifts `src/data/transfer.rs` verbatim via `#[path]` (see
the top of `src/main.rs`) so it exercises the real protocol code,
not a parallel reimplementation. Refactors to the source file are
picked up automatically on the next `cargo build`.

Unix only (macOS + Linux). The bridge produces pty endpoints under
`/tmp/`, which Windows doesn't have.

## Prereqs

1. **Bridge built.** From the repo root:
   ```sh
   (cd scripts/virtual-serial && cargo build --release)
   ```
2. **Fixtures generated** (one-time, regenerable):
   ```sh
   ./scripts/transfer-tests/regen-fixtures.sh
   ```
3. **lrzsz installed** for `rb` / `rx` (or `lrb` / `lrx` on macOS):
   ```sh
   brew install lrzsz                # macOS
   sudo apt install lrzsz            # Debian / Ubuntu
   ```
   The harness searches both name conventions.

## Build + run

From the repo root:

```sh
(cd scripts/transfer-tests && cargo build --release)
./scripts/transfer-tests/target/release/transfer-tests
```

Sample output:

```
running 11 test(s)
  T1 hex ASCII (Hello)                        ok  (  0.01s,  9600 baud) 5 bytes round-tripped
  T3 hex binary / non-printable               ok  (  0.01s,  9600 baud) 6 bytes round-tripped
  T5 hex 1 KiB                                ok  (  1.35s,  9600 baud) 1024 bytes round-tripped
  T11 YMODEM 512 B over slow link             ok  (  5.92s,  9600 baud) ok
  T6 YMODEM 4 KiB                             ok  (  3.69s, 115200 baud) ok
  T9 YMODEM 1 MiB (~91s)                      ok  (126.00s, 115200 baud) ok
  T7c XMODEM classic (128/checksum)           ok  (  1.56s, 115200 baud) ok
  T7C XMODEM-CRC (128/CRC-16)                 ok  (  1.56s, 115200 baud) ok
  T7k XMODEM-1K (1024/CRC-16)                 ok  (  1.55s, 115200 baud) ok
  T8 XMODEM single-block (SUB padding)        ok  (  1.08s, 115200 baud) ok
  T10 cancel mid-transfer                     ok  (  1.18s, 115200 baud) ok

result: ok. 11/11 passed (143.9s wall)
```

T9 dominates the wall time. Skip it with `--quick` for a fast loop.

## Flags

| Flag | Default | What it does |
|---|---|---|
| `--tests LIST` | `all` | Comma-separated subset of `all,hex,ymodem,xmodem,cancel`. |
| `--quick` | off | Skip T9 (1 MiB transfer, ~91s wall). |
| `--bridge PATH` | auto | Path to the `virtual-serial` binary. Auto-discovered relative to this binary or the repo root. |
| `--verbose, -v` | off | Pass the bridge subprocess's stderr through to your terminal — useful when a test wedges and you need to see the per-throttle byte rate or any error chatter. |
| `-h, --help` | — | Show usage. |

Exit code: `0` if every selected test passes, `1` if any fail, `2`
on a setup error (missing fixture, missing bridge, bad flag).

## Tests

Map between test IDs and the [TESTING.md](../virtual-serial/TESTING.md)
playbook entries:

| Test | TESTING.md | Group | Coverage |
|---|---|---|---|
| `T1` | T1 | hex | "Hello" round-trip at 9600 baud. |
| `T3` | T3 | hex | Non-printable bytes (`00 01 02 ff fe 7f`). |
| `T5` | T5 | hex | 1 KiB random hex via `parse_hex_string` + write path. |
| `T11` | T11 | ymodem | 512 B YMODEM at 9600 baud — slow-link path. |
| `T6` | T6 | ymodem | 4 KiB YMODEM at 115200 baud. |
| `T9` | T9 | ymodem | 1 MiB YMODEM — long-running, many-block stress. |
| `T7c` | T7 (classic) | xmodem | 4 KiB XMODEM classic (SOH/checksum). |
| `T7C` | T7 (CRC) | xmodem | 4 KiB XMODEM-CRC (SOH/CRC-16). |
| `T7k` | T7 (1K) | xmodem | 4 KiB XMODEM-1K (STX/CRC-16, 1024-byte blocks). |
| `T8` | T8 | xmodem | 6 B XMODEM classic — exercises SUB padding. |
| `T10` | T10 | cancel | Start 1 MiB YMODEM, trip the cancel flag 150 ms in, assert `TransferError::Cancelled` returned. |

T2 (hex input-format equivalence) and T4 (hex input validation) are
not automated here — they're UI-layer concerns. `parse_hex_string`
itself can be unit-tested separately if you want T2/T4 coverage.

## How the moving parts fit together

```
                ┌────────────────────┐
                │  transfer-tests    │
                │  (this harness)    │
                │                    │
                │  writes / reads  ───────► /tmp/baudrun-a
                │  (port A — half/  │              ▲
                │   full-duplex)    │              │
                └────────────────────┘              │
                                                    │ pty
                              virtual-serial bridge ┼──┐
                              (baud-paced throttle) │  │
                                                    │  │
                ┌────────────────────┐              │  │
                │  rb / rx           ◄──────────────┘  │
                │  (lrzsz receiver)  ───────► /tmp/baudrun-b
                └────────────────────┘
                                          ▲
                                          │
                                  /tmp/baudrun-rx/
                                  (received files)
```

The harness manages the bridge subprocess for the lifetime of the
run, restarting it between baud-rate groups (T1, T3, T5, T11 at
9600; everything else at 115200). For the file tests it spawns
the `lrzsz` receiver with stdin+stdout pointing at the bridge's
endpoint B, the same way TESTING.md's `< /tmp/baudrun-b > /tmp/baudrun-b`
shell redirect does.

For the hex tests the harness owns both endpoints — port A for
the write, port B for the read — and there's no subprocess. The
read path uses `libc::poll` directly rather than a forwarder
thread; see `src/port_reader.rs` for the (interesting) reason why.

## When tests fail

* **`bridge start: ...`** — the `virtual-serial` binary isn't where
  the harness expected. Build it (see prereqs) or pass `--bridge`.
* **`missing fixture: ...`** — run `./scripts/transfer-tests/regen-fixtures.sh`
  from the repo root.
* **`none of ["rb", "lrb"] on $PATH`** — install lrzsz.
* **`send_xmodem: transfer timeout` / `send_ymodem: transfer timeout`** —
  the receiver subprocess never sent the initial handshake byte
  (`C` for CRC, `NAK` for classic). Usually means `lrx`/`lrb` couldn't
  open the bridge endpoint. Run with `-v` to see bridge chatter.
* **Byte mismatch on a hex test** — the wire is corrupting bytes.
  Almost always points at virtual-serial or the slave-fd termios.
  See `src/port_reader.rs` for the off-by-one regression we hit
  during bring-up.
