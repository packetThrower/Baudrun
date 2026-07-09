---
title: Translations
description: 'Translate Baudrun into your language: one YAML file plus one line, no Rust required.'
editUrl: https://github.com/packetThrower/Baudrun/edit/main/locales/README.md
---

Baudrun's interface can run in any language. Each language is a
single YAML file under [`locales/`](https://github.com/packetThrower/Baudrun/tree/main/locales)
named for its locale code — `en.yml`, `zh-CN.yml`, and whatever you
add. English is both the source of truth and the fallback, so a
partial translation is safe: any string you don't translate simply
shows in English.

You don't need to be a programmer. Native fluency — ideally from
someone who actually consoles into gear in that language — matters
far more than any tooling. A machine can produce 300 plausible
strings; only a speaker knows whether "flow control" or
"assert/deassert" reads right to an engineer.

## The short version

1. Copy `locales/en.yml` to `locales/<code>.yml` (BCP-47 code:
   `de`, `fr`, `ja`, or language-Region where the script matters —
   `zh-CN`, `zh-TW`, `pt-BR`).
2. Translate the **values**, never the **keys**.
3. Keep every `%{name}` placeholder (they're filled in at runtime).
4. Leave technical tokens alone — `HEX`, `DTR`, `RTS`, `CR`/`LF`,
   `XON/XOFF`, `USB`, `RS-485`, product names, hex examples,
   keyboard combos, shell snippets.
5. Add one line to `SUPPORTED` in
   [`src/i18n.rs`](https://github.com/packetThrower/Baudrun/blob/main/src/i18n.rs)
   with your code and the language's name in its own script:
   `("de", "Deutsch")`. The language picker and OS auto-detection
   pick it up automatically.
6. `cargo run`, then Settings → Appearance → Language → your
   language. It switches live.

Open a PR with the `<code>.yml` file and the one-line `SUPPORTED`
edit — that's the whole contribution.

## Full guide

The complete step-by-step (YAML quoting rules, the placeholder
mechanics, the Traditional-Chinese resolver note, and the
consistency tips) lives next to the files themselves, in
[`locales/README.md`](https://github.com/packetThrower/Baudrun/blob/main/locales/README.md).
That's the canonical reference — this page is the signpost.

## What is and isn't translated

Only the **chrome** — sidebar, session header, profile editor,
settings, notifications, tooltips, menus, dialogs. **Terminal
output is never translated**: the bytes your device emits pass
through untouched, as they must. Wire-format identifiers (serial
enum values like `cr`/`none`/`del`, profile JSON field names,
theme/skin IDs) aren't display text and stay in code, never in the
locale files.
