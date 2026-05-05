---
title: Syntax highlighting
description: Authoring custom syntax-highlighting rule packs for Baudrun's terminal viewport.
editUrl: https://github.com/packetThrower/Baudrun/edit/main/docs-next/src/content/docs/authoring/highlighting.md
---

Baudrun runs every byte received from the wire through a regex-based highlighter before it lands in the terminal. The highlighter is **vendor-aware**, with built-in packs for Cisco IOS / IOS XE / IOS XR, Juniper Junos, Aruba AOS-CX, Arista EOS, and MikroTik RouterOS. It is also **extensible**: you can author your own packs as a JSON file and drop them in.

This page is the authoring guide. For the per-feature reference (per-profile overrides, mutual exclusion with hex view, ANSI passthrough behavior) see [Advanced settings → Syntax highlighting](/Baudrun/usage/advanced/#syntax-highlighting).

## Try a rule before saving it

The [**rule playground**](/Baudrun/playground.html) is a self-contained HTML page that runs the same highlighter Baudrun uses, against text you paste or drop. Edit the pack JSON, see the colors apply live, iterate. Everything runs in your browser; the file you drop never leaves your machine.

This is the fastest way to develop a new pack. Land your regexes here, then export the JSON and import it into the app.

## Pack schema

A pack is a JSON file with four top-level fields:

```json
{
  "id": "my-lab",
  "name": "My Lab",
  "description": "Example highlight pack for lab use.",
  "rules": [
    { "pattern": "\\blab-[a-z0-9-]+\\b", "color": "magenta", "group": "lab-hostnames" },
    { "pattern": "\\b(?:PASS|OK|SUCCESS)\\b", "color": "green", "group": "test-ok" },
    { "pattern": "\\b(?:FAIL|ERROR|ABORT)\\b", "color": "red", "group": "test-fail" },
    { "pattern": "\\bTODO\\b", "color": "yellow", "group": "todo" }
  ]
}
```

| Field | Type | Notes |
| --- | --- | --- |
| `id` | string | Becomes the filename under `$SUPPORT_DIR/highlight/<id>.json`. Alphanumeric / hyphen / underscore only. Must not collide with a bundled pack id or the reserved `user`. |
| `name` | string | Display name in the Settings → Syntax Highlighting list. |
| `description` | string | Optional one-line summary shown beneath the name. |
| `rules` | array | One or more rule objects (below). |

Each rule object:

| Field | Type | Notes |
| --- | --- | --- |
| `pattern` | string | A JavaScript regex. Anchors and lookarounds work. Use `\\b` for word boundaries; remember to double-escape backslashes in JSON. |
| `color` | string | One of: `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `dim`. |
| `ignoreCase` | boolean | Optional. Defaults to `false`. |
| `group` | string | Optional label used by the playground's "matches by group" view. Has no runtime effect inside Baudrun. |

**Match precedence**: rules within a pack are tried in array order; first match wins on overlap. Across packs, load order is the order they appear in the Settings list (top to bottom).

**ANSI passthrough**: device-supplied ANSI CSI colors pass through unchanged. The highlighter only applies color to text that arrived uncolored.

## Built-in packs

| Pack | Covers |
| --- | --- |
| **Baudrun default** | Vendor-neutral: IPv4/IPv6, MACs, interface names, `up`/`down`/`error`/`warning` keywords, timestamps, dates, VLANs |
| **Cisco IOS / IOS XE / IOS XR** | `line protocol`, log mnemonics (`%LINK-3-UPDOWN`), STP roles (`DESG`/`ROOT`/`ALTN`), OSPF/BGP states, AS numbers, ACL `permit`/`deny` |
| **Juniper Junos** | Chassis status (`Online`/`Empty`), BGP/OSPF/IS-IS states, `[edit ...]` banners, commit messages, set/delete diff lines |
| **Aruba AOS-CX** | VSX/VSF status, LAG/MCLAG, STP role+state, daemon names in event logs, ACL actions |
| **Arista EOS** | MLAG peer state, VXLAN/EVPN fabric keywords, `Et1/1` short-form interfaces, `Aboot`/EOS version banners, log facility (`%BGP-5-ADJCHANGE`), config-section headers |
| **MikroTik RouterOS** | `/export` section paths (`/ip firewall filter`, `/interface vlan`), `k=v` parameter syntax, firewall chain + action semantics (accept/drop/reject), connection states, RouterOS-style interface names (`ether1`, `wlan1`, `wg0`) |

Built-in packs are read-only. To layer one-off rules on top of a built-in pack, use the **User overrides** scratchpad. It lives at `$SUPPORT_DIR/highlight-rules.json` and is editable on disk.

## Importing your pack

1. Author the JSON file (use the playground while iterating).
2. Open **Settings → Syntax Highlighting → Import pack…**
3. Pick the file. Baudrun copies it to `$SUPPORT_DIR/highlight/<id>.json` and auto-enables it.

Imports with an `id` that collides with a bundled pack or the reserved `user` scratchpad are rejected with an error message; pick a different id and re-import.

The imported pack shows up in the Syntax Highlighting list with a **Remove** button next to its entry. Removing deletes the file and disables it; the original file you imported from is unchanged.

## Per-profile overrides

By default every profile inherits the global Syntax Highlighting selection from Settings. To override for a single profile (say, you only want the MikroTik pack on for one specific RouterOS device), open the profile's edit form and switch to the **Highlighting** tab; there is a per-profile toggle and pack list there.

## Starter packs

- [**Minimal example**](https://github.com/packetThrower/Baudrun/blob/main/docs/examples/highlight-pack.example.md): near-empty skeleton showing the schema. Copy, rename `id` and `name`, add rules, import.
- [**Syslog / journald**](https://github.com/packetThrower/Baudrun/blob/main/docs/examples/syslog.example.md): practical starter for generic syslog / journald output (severity keywords, systemd unit states, sshd accepted/denied lines, `[OK]` / `[FAILED]` markers, daemon tags, PIDs).
