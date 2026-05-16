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
| [`baudrun-test-pack-good.json`](baudrun-test-pack-good.json) | Well-formed highlight rule pack. Exercises every field a pack can have — `id`, `name`, `description`, `source`, plus rules with `pattern`, `color`, `group`, and `ignoreCase` (the last on at least one rule so the optional path is hit). Import should succeed silently and the pack appears under **Settings → Highlighting → Imported packs**. |
| [`baudrun-test-pack-bad-id.json`](baudrun-test-pack-bad-id.json) | Minimal valid JSON, but the `id` collides with a bundled pack. Kept deliberately bare-bones — its job is the collision branch, not field coverage. Import should reject with a clear "id already exists" error; the built-in pack must not be overwritten. |
| [`baudrun-test-pack-bad-json.json`](baudrun-test-pack-bad-json.json) | Malformed JSON (intentional syntax error). Import should reject with a parse error pointing at the line number; no partial state written to disk. **Do not "fix" the syntax** — the file's whole purpose is to be broken. |
| [`baudrun-test-skin.json`](baudrun-test-skin.json) | Well-formed app skin. Exercises every field — `vars` (the base CSS-var map), `darkVars` (overlay applied in dark mode), `lightVars` (empty here because `supportsLight: false`), plus `id` / `name` / `source` / `description` / `supportsLight`. Import should succeed and the skin appears under **Settings → Appearance → Imported skins**. |

The two "good" fixtures (`pack-good`, `skin`) double as quick schema
spot-checks — if a future refactor adds a non-optional field to
either struct, the import should fail loudly on these on the next
run.

## Manual test recipe

1. Settings → Highlighting → ⋯ → **Import pack…**
2. Pick one of the JSON files above.
3. Observe the expected outcome from the table.
4. Repeat for the skin file under Settings → Appearance.

Automating these is on the roadmap; for now they're a quick manual
spot-check before tagging a release. The transfer protocols are
fully automated — see [`scripts/transfer-tests/`](../../scripts/transfer-tests/).
