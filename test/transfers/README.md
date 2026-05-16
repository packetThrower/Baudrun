# test/transfers/

Deterministic payloads for the [`transfer-tests`](../../scripts/transfer-tests/)
harness and the manual [`TESTING.md`](../../scripts/virtual-serial/TESTING.md)
playbook. Every file is generated from a fixed Python RNG seed (42)
so two runs of the regen script produce bit-identical output and
two contributors see the same source bytes for assertion comparison.

## Files

| File | Size | Purpose |
|---|---:|---|
| [`hex-1k.txt`](hex-1k.txt) | 2 KiB | 2048-char hex string (1024 random bytes encoded). Paste into Send Hex… for **T5**. |
| [`payload-4k.bin`](payload-4k.bin) | 4 KiB | 4096 random bytes. Source for **T6** (YMODEM 4 KiB) and **T7** (XMODEM variants). |
| [`tiny.txt`](tiny.txt) | 6 B | Literal `hello\n`. Used by **T8** (single-block XMODEM, exercises SUB-padding path). |
| [`payload-1m.bin`](payload-1m.bin) | 1 MiB | 1048576 random bytes. Source for **T9** (1 MiB YMODEM, ~91 s wall) and **T10** (cancel mid-transfer rehearsal). |
| [`payload-512.bin`](payload-512.bin) | 512 B | 512 random bytes. Source for **T11** (YMODEM over a 9600-baud slow link). |

T1 (hex ASCII "Hello") and T3 (hex non-printable `00 01 02 ff fe 7f`)
don't need a fixture file — the harness hard-codes those inputs at
the call site because the entire payload fits on a single line in
the test source.

## Regen

```sh
./scripts/transfer-tests/regen-fixtures.sh
```

Reproducibility note: Python's `random.Random(seed)` is the Mersenne
Twister — its byte stream is stable across all Python 3.x versions
but is NOT cryptographically random. Don't use these files for
anything where unpredictability matters; they're round-trip payloads,
not security fixtures.

## Why deterministic

The earlier version of TESTING.md asked the user to `head -c N
/dev/urandom > /tmp/...` per test, which meant every run compared
against different source bytes. Useful for fuzzing the protocols
with varied inputs, useless for assertion-based regression
testing — a "diff matched" pass tells you nothing about whether
bit 42 of byte 4095 round-tripped correctly when the source byte
itself changed between runs.
