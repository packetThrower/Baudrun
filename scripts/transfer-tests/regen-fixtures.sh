#!/usr/bin/env bash
# Regenerate the deterministic transfer-test fixtures.
#
# Inputs live in `test/transfers/` (gitignored — see /.gitignore).
# All binary payloads are produced from a fixed RNG seed (42) so the
# automation harness can do byte-exact diff comparisons, and so two
# contributors regenerating fixtures get identical bytes.
#
# Files produced:
#   hex-1k.txt        2048-char hex string (1024 random bytes encoded).
#                     Paste into Send Hex… for T5.
#   payload-4k.bin    4 KiB random binary. Source for T6 (YMODEM) and
#                     T7 (XMODEM variants).
#   tiny.txt          Literal "hello\n" (6 bytes). T8 small-file /
#                     single-block XMODEM, exercises the SUB-padding path.
#   payload-1m.bin    1 MiB random binary. T9 large YMODEM, T10 cancel
#                     mid-transfer rehearsal.
#   payload-512.bin   512 random bytes. T11 transfer over a slow link.
#
# Reproducibility note: Python's `random.Random(seed)` is a Mersenne
# Twister — its byte stream is stable across Python 3.x versions but
# is NOT the OS urandom. Don't use these files for anything where
# unpredictability matters (they're just round-trip payloads).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="$REPO_ROOT/test/transfers"
mkdir -p "$OUT_DIR"

python3 - "$OUT_DIR" <<'PY'
import os, random, sys

out = sys.argv[1]
r = random.Random(42)

def randbytes(n):
    return bytes(r.randrange(256) for _ in range(n))

with open(f"{out}/hex-1k.txt", "w") as f:
    f.write(randbytes(1024).hex())

with open(f"{out}/payload-4k.bin", "wb") as f:
    f.write(randbytes(4096))

with open(f"{out}/tiny.txt", "w") as f:
    f.write("hello\n")

with open(f"{out}/payload-1m.bin", "wb") as f:
    f.write(randbytes(1024 * 1024))

with open(f"{out}/payload-512.bin", "wb") as f:
    f.write(randbytes(512))

for name in sorted(os.listdir(out)):
    p = os.path.join(out, name)
    print(f"{os.path.getsize(p):>10}  {name}")
PY

echo
echo "Fixtures written to $OUT_DIR"
