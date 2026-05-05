---
title: Profiles
description: 'Authoring connection profiles for switches, routers, and firewalls.'
editUrl: https://github.com/packetThrower/Baudrun/edit/main/docs/PROFILES.md
---

Profiles are named serial-connection settings: port, baud, framing,
and the various per-session toggles. Baudrun stores every profile in
a single JSON file, which makes them easy to hand-edit, version-
control, or generate programmatically from inventory data.

## File location

All profiles live in one file:

- **macOS**: `~/Library/Application Support/Baudrun/profiles.json`
- **Windows**: `%APPDATA%\Baudrun\profiles.json`
- **Linux**: `$XDG_CONFIG_HOME/Baudrun/profiles.json` (usually
  `~/.config/Baudrun/profiles.json`)

The file is a JSON array. Baudrun loads it once at startup; changes
made while the app is running are not picked up until the next
launch. Use the app UI for interactive edits, hand-edit only while
the app is closed.

## JSON schema

```json
[
  {
    "id": "8c6b3d4f-...",
    "name": "Core-SW-01 (Aruba)",
    "portName": "/dev/cu.usbserial-AU04CDLV",
    "baudRate": 9600,
    "dataBits": 8,
    "parity": "none",
    "stopBits": "1",
    "flowControl": "none",
    "lineEnding": "cr",
    "localEcho": false,
    "highlight": true,
    "themeId": "",
    "dtrOnConnect": "default",
    "rtsOnConnect": "default",
    "dtrOnDisconnect": "default",
    "rtsOnDisconnect": "default",
    "hexView": false,
    "timestamps": false,
    "logEnabled": false,
    "autoReconnect": false,
    "backspaceKey": "del",
    "createdAt": "2026-04-17T10:15:22Z",
    "updatedAt": "2026-04-17T10:15:22Z"
  }
]
```

### Identity + port

| Field      | Type   | Required | Notes                                                                                                                                    |
| ---------- | ------ | -------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `id`       | string | yes\*    | UUID. Generated automatically when the app creates a profile. Required for the in-app update path. Hand-written profiles can set any unique string. |
| `name`     | string | **yes**  | Display name shown in the sidebar. Must be non-empty.                                                                                    |
| `portName` | string | **yes**  | OS device path. macOS `/dev/cu.usbserial-...`, Linux `/dev/ttyUSB0` or `/dev/ttyACM0`, Windows `COM7`. Must match what the app enumerates. |

\* `id` is only required when round-tripping through the app UI. A
hand-added profile with a blank `id` will be loaded but can't be
updated through the form until it gets one.

### Framing

| Field         | Type   | Required | Valid values                                         | Default  |
| ------------- | ------ | -------- | ---------------------------------------------------- | -------- |
| `baudRate`    | int    | **yes**  | Any positive integer. Common: `9600`, `19200`, `38400`, `57600`, `115200`. | _none_ |
| `dataBits`    | int    | **yes**  | `5`, `6`, `7`, `8`                                   | `8`      |
| `parity`      | string | **yes**  | `"none"`, `"odd"`, `"even"`, `"mark"`, `"space"`     | `"none"` |
| `stopBits`    | string | **yes**  | `"1"`, `"1.5"`, `"2"` (strings, not numbers)         | `"1"`    |
| `flowControl` | string | **yes**  | `"none"`, `"rtscts"`, `"xonxoff"`                    | `"none"` |

Shorthand "8N1" = `dataBits: 8, parity: "none", stopBits: "1"`.
Network gear is overwhelmingly 9600 8N1.

### Terminal behavior

| Field          | Type    | Valid values                     | Default | Purpose                                                                                       |
| -------------- | ------- | -------------------------------- | ------- | --------------------------------------------------------------------------------------------- |
| `lineEnding`   | string  | `"cr"`, `"lf"`, `"crlf"`         | `"cr"`  | Byte(s) the Enter key sends. Most network gear wants CR; Linux consoles want LF; legacy/Windows sometimes CRLF. |
| `localEcho`    | boolean | `true` / `false`                 | `false` | Echo typed characters locally. Enable when the device doesn't echo.                           |
| `highlight`    | boolean | `true` / `false`                 | `true`  | Run the line-buffered regex colorizer over incoming text.                                     |
| `backspaceKey` | string  | `"del"`, `"bs"`                  | `"del"` | What the Backspace key sends. DEL (0x7f) matches VT100/xterm; BS (0x08) for some older Cisco/Foundry gear. Wrong setting surfaces as `^H` echoed on screen. |
| `hexView`      | boolean | `true` / `false`                 | `false` | Render incoming bytes as a hex dump (16 per line, ASCII sidebar). Binary protocols, firmware loaders. |
| `timestamps`   | boolean | `true` / `false`                 | `false` | Prefix each line with `[HH:MM:SS.mmm]`.                                                       |
| `themeId`      | string  | any theme ID or `""`             | `""`    | Per-profile theme override. Empty = use the global default theme from Settings.               |

### Control lines (DTR/RTS)

Four policies for when the app asserts/deasserts the DTR and RTS
lines on connect/disconnect. All four accept the same enum:

| Value         | Meaning                                                                          |
| ------------- | -------------------------------------------------------------------------------- |
| `"default"`   | Leave the line in the OS default state (both asserted on Unix; RTS asserted on Windows). |
| `""`          | Same as `"default"`. Treated as unset.                                           |
| `"assert"`    | Force the line high at the bookend.                                              |
| `"deassert"`  | Force the line low at the bookend.                                               |

| Field             | Applies at    |
| ----------------- | ------------- |
| `dtrOnConnect`    | Port open     |
| `rtsOnConnect`    | Port open     |
| `dtrOnDisconnect` | Port close    |
| `rtsOnDisconnect` | Port close    |

Common uses:
- **RS-485 direction.** Some adapters use RTS to toggle TX/RX; pin it.
- **Arduino DTR reset.** Deassert DTR on connect to avoid auto-reset
  on port open.
- **Device state gating.** Some firmwares require a specific DTR
  state to accept input.

Live DTR/RTS toggle pills in the session header let you flip the
lines mid-session regardless of the connect-time policy.

### Session features

| Field           | Type    | Default | Purpose                                                                                                    |
| --------------- | ------- | ------- | ---------------------------------------------------------------------------------------------------------- |
| `logEnabled`    | boolean | `false` | Record raw incoming bytes to a timestamped file. Directory configured in Settings → Advanced (defaults to `<support>/logs/`). |
| `autoReconnect` | boolean | `false` | On disconnect (port disappeared), poll for the port to reappear (1s interval, 30s timeout) and reopen with the same config. Preserves xterm backlog across the gap. |

### Timestamps

| Field       | Type             | Notes                                                                     |
| ----------- | ---------------- | ------------------------------------------------------------------------- |
| `createdAt` | RFC3339 string   | Managed by the app. Preserved on update. Don't touch in hand-edits unless you're forging history. |
| `updatedAt` | RFC3339 string   | Managed by the app. Bumped on every UI edit.                              |

## Examples

### Standard network switch (Cisco, Aruba, etc.)

```json
{
  "name": "Edge-SW-02",
  "portName": "/dev/cu.usbserial-AU04CDLV",
  "baudRate": 9600,
  "dataBits": 8,
  "parity": "none",
  "stopBits": "1",
  "flowControl": "none",
  "lineEnding": "cr",
  "highlight": true,
  "backspaceKey": "del"
}
```

### RuggedCom with USB-C console

USB-C consoles (RuggedCom RST2228, newer Cisco, HPE/Aruba) are
CDC-ACM, so the port name looks different (`cu.usbmodem...` on
macOS). Otherwise the same:

```json
{
  "name": "RST2228-Plant-A",
  "portName": "/dev/cu.usbmodem00000000001A1",
  "baudRate": 57600,
  "dataBits": 8,
  "parity": "none",
  "stopBits": "1",
  "flowControl": "none",
  "lineEnding": "crlf"
}
```

### Arduino with DTR-reset avoidance

Deassert DTR on connect so the port open doesn't reset the board:

```json
{
  "name": "Uno-Prototype",
  "portName": "/dev/cu.usbmodem14201",
  "baudRate": 115200,
  "dataBits": 8,
  "parity": "none",
  "stopBits": "1",
  "flowControl": "none",
  "lineEnding": "lf",
  "localEcho": true,
  "dtrOnConnect": "deassert",
  "autoReconnect": true
}
```

### Modbus RTU debug

Hex view for binary protocol inspection, RTS/CTS for half-duplex
adapters, auto-reconnect for flaky USB:

```json
{
  "name": "Modbus-PLC",
  "portName": "/dev/cu.usbserial-0001",
  "baudRate": 19200,
  "dataBits": 8,
  "parity": "even",
  "stopBits": "1",
  "flowControl": "rtscts",
  "lineEnding": "cr",
  "hexView": true,
  "timestamps": true,
  "autoReconnect": true
}
```

## Bulk provisioning

Because the file is a plain JSON array, generating many profiles at
once is a scripting job. Two common approaches:

### From inventory CSV with jq

Given `switches.csv` with columns `name,port,baud`:

```bash
tail -n +2 switches.csv | \
  jq -Rs 'split("\n") | map(select(length > 0)) |
    map(split(",")) |
    map({
      name: .[0],
      portName: .[1],
      baudRate: (.[2] | tonumber),
      dataBits: 8,
      parity: "none",
      stopBits: "1",
      flowControl: "none",
      lineEnding: "cr",
      highlight: true,
      backspaceKey: "del",
      dtrOnConnect: "default",
      rtsOnConnect: "default",
      dtrOnDisconnect: "default",
      rtsOnDisconnect: "default"
    })' > profiles.json
```

### From Python

```python
import json, uuid
from datetime import datetime, timezone

inventory = [
    ("Core-01", "/dev/cu.usbserial-A1", 9600),
    ("Core-02", "/dev/cu.usbserial-A2", 9600),
    ("Edge-01", "/dev/cu.usbserial-B1", 115200),
]

now = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")

profiles = [
    {
        "id": str(uuid.uuid4()),
        "name": name,
        "portName": port,
        "baudRate": baud,
        "dataBits": 8,
        "parity": "none",
        "stopBits": "1",
        "flowControl": "none",
        "lineEnding": "cr",
        "localEcho": False,
        "highlight": True,
        "backspaceKey": "del",
        "dtrOnConnect": "default",
        "rtsOnConnect": "default",
        "dtrOnDisconnect": "default",
        "rtsOnDisconnect": "default",
        "hexView": False,
        "timestamps": False,
        "logEnabled": False,
        "autoReconnect": False,
        "createdAt": now,
        "updatedAt": now,
    }
    for name, port, baud in inventory
]

with open("profiles.json", "w") as f:
    json.dump(profiles, f, indent=2)
```

Close the app, drop `profiles.json` into the config dir, relaunch.

## Hand-editing etiquette

- **Quit Baudrun first.** The app reads once at startup and
  rewrites the whole file on every save. Hand-edits during a running
  session will be clobbered on the next UI edit.
- **Validation is strict.** The app refuses to load a profile whose
  baud rate ≤ 0, data bits outside 5-8, or parity/stopBits/flowControl/
  lineEnding outside the enum. Fix typos before relaunching.
- **IDs must be unique** within the file. Duplicates cause ambiguity
  on update and may get deduped unpredictably.
- **Don't bump `createdAt`.** Not dangerous, but future sorting
  features might rely on it.

## Version control

A natural workflow for shared environments: keep `profiles.json` in
a private git repo, symlink it into `<support>/profiles.json` on
each workstation. Hand-edits become commits; diffs show who changed
what when.

The file has no secrets (passwords aren't stored, since this is a
serial terminal, not SSH), so a shared team repo works without
additional encryption.

## Sharing single profiles

There's no per-profile file format today. Workarounds:

- Copy a single object out of the JSON array and paste into a gist
  / email. Recipient merges it into their own `profiles.json`.
- Keep a shared JSON snippets file alongside `profiles.json` and
  import by hand.

A profiles export/import UI is a candidate feature if this becomes
a real workflow. File an issue with the use case.
