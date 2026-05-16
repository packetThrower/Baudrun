# test/json/

JSON fixtures for **Settings → Import** smoke tests. Each pack /
skin file pair exercises one branch of the import-validation path
in `src/data/skins.rs` and `src/data/highlight.rs`. The schemas
themselves live alongside their consumers in
[`docs-next/public/examples/`](../../docs-next/public/examples/) and
the [authoring docs](https://packetthrower.github.io/Baudrun/authoring/themes/);
this directory is just the test corpus.

## Files

| File | What it tests |
|---|---|
| [`baudrun-test-pack-good.json`](baudrun-test-pack-good.json) | Well-formed highlight rule pack with three rules. Import should succeed silently and the pack should appear in **Settings → Highlighting → Imported packs**. |
| [`baudrun-test-pack-bad-id.json`](baudrun-test-pack-bad-id.json) | Valid JSON, but the `id` field collides with a built-in pack. Import should reject with a clear "id already exists" error and the built-in pack must not be overwritten. |
| [`baudrun-test-pack-bad-json.json`](baudrun-test-pack-bad-json.json) | Malformed JSON (intentional syntax error). Import should reject with a parse error pointing at the line number, no partial state written to disk. |
| [`baudrun-test-skin.json`](baudrun-test-skin.json) | Well-formed app skin pack. Import should succeed and the skin should appear in **Settings → Appearance → Imported skins**, immediately selectable. |

## Manual test recipe

1. Settings → Highlighting → ⋯ → **Import pack…**
2. Pick one of the JSON files above.
3. Observe the expected outcome from the table.
4. Repeat for the skin file under Settings → Appearance.

Automating these is on the roadmap; for now they're a quick manual
spot-check before tagging a release. The transfer protocols are
fully automated — see [`scripts/transfer-tests/`](../../scripts/transfer-tests/).
