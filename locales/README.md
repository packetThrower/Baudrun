# Translating Baudrun

Baudrun's interface can run in any language. Each language is one
YAML file in this directory, named for its locale code:

- `en.yml` — English (the source of truth **and** the fallback)
- `zh-CN.yml` — Simplified Chinese
- add your own: `de.yml`, `fr.yml`, `ja.yml`, `zh-TW.yml`, …

You don't need to be a programmer to add a language — it's one file
plus one line. Native fluency matters far more than Rust here; a
translation from someone who actually uses console gear in that
language is worth more than any machine output.

## Add a language in 8 steps

1. **Copy the English file** to your locale code:
   `cp en.yml <code>.yml` (e.g. `de.yml`). Use the BCP-47 code —
   two letters for most languages (`de`, `fr`, `ja`), or
   language-Region where the script differs (`zh-CN` Simplified,
   `zh-TW` Traditional, `pt-BR` Brazilian Portuguese).

2. **Translate the values, never the keys.** Only change the text
   to the *right* of each colon:
   ```yaml
   welcome:
     pick_profile: "Pick a profile from the sidebar…"   # ← translate this
   ```
   Leave `welcome:` and `pick_profile:` exactly as they are — the
   app looks strings up by those keys.

3. **Keep every `%{name}` placeholder.** These are filled in at
   runtime (a port name, a count, an error). Keep the name spelled
   the same; you may move it to wherever it reads naturally:
   ```yaml
   connected: "Connected to %{port} @ %{baud}"
   # German, placeholder reordered — fine:
   connected: "Mit %{port} @ %{baud} verbunden"
   ```

4. **Leave technical tokens untranslated.** These are the same in
   every language and translating them would confuse a network
   engineer: `HEX`, `DTR`, `RTS`, `CR`, `LF`, `CRLF`, `XON/XOFF`,
   `RTS/CTS`, `USB`, `UART`, `RS-485`, `Baudrun`, hex byte examples
   like `02 FF AA 55`, keyboard combos (`⌘K`, `Ctrl+Shift`), and
   shell/path snippets (`sudo chmod 666 …`, `.deb`, `settings.json`).
   Product names (Cisco, PuTTY, GitHub) stay too.

5. **Stay valid YAML.** Double-quote any value that contains a
   colon, `%`, `#`, a straight `"`, or leading/trailing spaces;
   escape an internal `"` as `\"` and a backslash as `\\` (the
   line-ending options have literal backslashes like `CR (\\r)`).
   When unsure, quote it — quoting is always safe.

6. **Partial is fine.** Any key you leave out (or delete) falls
   back to the English value automatically. Ship what you've done;
   fill the rest in later. You never have to translate all ~300 at
   once.

7. **Register the language.** Add one line to the `SUPPORTED` list
   in [`../src/i18n.rs`](../src/i18n.rs), with the locale code and
   the language's name *in its own script* (its endonym, so a user
   already in the wrong language can still find theirs):
   ```rust
   pub const SUPPORTED: &[(&str, &str)] =
       &[("en", "English"), ("zh-CN", "简体中文"), ("de", "Deutsch")];
   ```
   The Settings → Appearance → Language picker reads this list, so
   your language appears automatically. OS-locale auto-detection
   also starts working for it (e.g. a `de-DE` system boots into
   German). Traditional Chinese is pre-wired in the resolver —
   adding `("zh-TW", "繁體中文")` + `zh-TW.yml` is all it takes.

8. **Try it.** `cargo run`, then Settings → Appearance → Language →
   your language. It switches live, no restart. Missing keys show
   English, never a raw `some.key` string.

## Tips

- **Be consistent.** Pick one term per concept and use it
  everywhere — a device's "baud rate" should read the same in
  every string. Match how your OS and the gear's own docs render
  networking terms.
- **Keys are grouped by area** (`welcome.*`, `chrome.*` = sidebar/
  header, `editor.*` = profile form, `settings.*`, `terminal.*`,
  `opts.*` = dropdown options). The comments in `en.yml` say what
  each area is.
- **What's *not* here, on purpose:** serial values (`none`, `cr`,
  `del`, …), profile JSON field names, and theme/skin IDs are
  wire-format identifiers, not display text — they live in code and
  are never translated. Terminal output (bytes from the device) is
  never translated either; only the chrome around it.

Open a PR with your `<code>.yml` and the one-line `SUPPORTED` edit.
That's the whole contribution — thank you.
