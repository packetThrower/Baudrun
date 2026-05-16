# test/

Fixture data for Baudrun's automated and manual tests. Two subdirs:

| Path | What | Used by |
|---|---|---|
| [`transfers/`](transfers/) | Deterministic binary + hex payloads for the XMODEM / YMODEM / Send-Hex round-trip harness. | [`scripts/transfer-tests/`](../scripts/transfer-tests/) |
| [`json/`](json/) | Hand-crafted JSON files — both well-formed and intentionally broken — for exercising Settings → Import (highlight packs, skins). | Manual UI tests |

Everything here is tracked in git so a fresh clone can run the full
test suite without setup. The transfer fixtures are generated from a
fixed Python RNG seed, so regeneration via
[`scripts/transfer-tests/regen-fixtures.sh`](../scripts/transfer-tests/regen-fixtures.sh)
produces bit-identical output and the committed copies don't
gradually drift from what the script emits.

## Regenerating

```sh
./scripts/transfer-tests/regen-fixtures.sh
```

Run that any time `payload-*.bin`, `hex-1k.txt`, or `tiny.txt` are
missing or you've intentionally bumped the RNG seed. The script is
idempotent — repeated runs produce the same bytes.

The `json/` fixtures are hand-written, not generated; tweak them
directly when adding new Settings → Import test cases.
