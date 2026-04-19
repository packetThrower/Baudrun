# Advanced features

A task-oriented tour of the features that don't fit into "open a port
and type." For the full schema behind each profile field referenced
below, see [PROFILES.md](PROFILES.md).

## Getting a device into ROMMON / bootloader

The serial "break" signal is how most network gear, MCUs, and boot
loaders drop into their low-level prompts.

**Steps:**
1. Open a session with your normal profile.
2. Reboot the target device (power cycle, `reload`, reset button).
3. During early boot, click **Break** in the session header — sends
   a 300 ms break pulse.
4. The device drops into its low-level prompt (`rommon 1>`,
   `loader>`, `=>`, etc.).

**Break windows by vendor:**

| Device / boot stage          | When to send Break                                                |
| ---------------------------- | ----------------------------------------------------------------- |
| Cisco IOS (ROMMON)           | Within ~60 s of reload. Fine to tap Break repeatedly.             |
| Juniper JunOS                | 2-3 s into boot, around the "Hit [Enter] to boot immediately" line. |
| U-Boot / ARM boot loaders    | "Press any key to stop autoboot" window — Break works, Enter too. |
| MCU bootloaders (SAM-BA etc.) | Often combined with specific DTR/RTS sequences; see below.        |

Some MCU bootloaders also require DTR or RTS in a specific state
before they accept input. Either set the profile's **DTR/RTS on
connect** policy to `assert` or `deassert`, or flip the live DTR/RTS
pills in the session header mid-session.

## Password recovery

Textbook Cisco password recovery leans on Break + the config-register:

1. Session → **Break** during boot → `rommon 1>` prompt.
2. `confreg 0x2142` at ROMMON — tells IOS to ignore the startup-config.
3. `reset` — device reboots.
4. Device comes up with no config. `enable` gets you to privileged
   mode without a password.
5. `copy startup-config running-config` — load the real config in
   without it being active yet.
6. `configure terminal` → `enable secret <new-password>` → `exit`.
7. `config-register 0x2102` (restore normal boot behavior).
8. `write memory` to save.

Same pattern with different keywords on other vendors; the Break tool
is what unlocks step 1.

## Debugging binary protocols

Modbus RTU, firmware bootloader handshakes, custom embedded protocols
— anything non-ASCII needs both directions visible in hex to
interpret.

**Setup:**
1. Enable **Hex view** (profile → Advanced). Incoming bytes render as
   a 16-per-line hex dump with an ASCII sidebar.
2. Enable **Line timestamps** (profile → Advanced). Binary protocols
   are timing-sensitive; timestamps surface inter-packet gaps.
3. Use the **Hex** session-header button to send raw bytes. The
   parser is forgiving: `02 FF AA 55`, `02FFAA55`, and `0x02 0xFF 0xAA 0x55`
   all produce the same four bytes.

**Tips:**
- Enable **Session logging** too so the full raw stream is on disk
  for post-mortem scripting.
- Pair with a terminal **theme** that has a distinct background so
  the hex dump doesn't blur into the app chrome — Solarized Dark,
  Dracula, or Monokai all work.
- For Modbus: function code is byte 2 of the payload; CRC is the last
  two bytes (little-endian). Timestamps help confirm the 3.5-char
  inter-frame gap is present.

## Pasting config safely

Pasting a config blob into the wrong session is a classic incident.
Two profile-level tools:

**Confirm multi-line pastes** — prompts with line count + first-line
preview before sending anything that contains a line break. Typed
input never triggers it (typing can't cross lines in a single burst).

**Slow paste** — sends pasted bytes one at a time with a configurable
delay (default 10 ms, tunable 0-500 ms). Mandatory for:
- MCUs with small UART FIFOs (8/16-byte buffers drop anything fast).
- Older Cisco IOS trains on slow CPUs.
- Any device at 115200 without flow control.

Rule of thumb: if your `conf t` paste has ever produced garbled
output, you needed slow paste.

## Keeping a session alive across adapter drops

Cheap USB-serial adapters (CH340, CP210x clones) re-enumerate under
EMI, bad cables, or sleepy hubs. Enable **Auto-reconnect** on the
profile. Behavior:

- On disconnect, the app polls for the port name to reappear (1 s
  interval, 30 s timeout).
- xterm stays mounted, so scrollback survives the gap.
- Session header dot pulses amber with a "reconnecting…" label.
- As soon as the port reappears, the app reopens with the same
  config — including your DTR/RTS policies.
- User-clicked Disconnect cancels the retry cleanly.

Combine with **Session logging** for unattended captures: an
overnight RS-485 bus tap records continuously even if the adapter
blips every few hours.

## Navigating away without losing the session

**Suspend** (session header) keeps the serial port open and xterm
running but returns you to the profile list. Useful for:
- Comparing config between two switches (switch A suspended, open B).
- Checking settings without tearing down the session.
- Editing the current profile's theme or auto-reconnect setting
  mid-conversation.

**Resume** by clicking back into the suspended profile. The
`<Terminal>` component never unmounts, so the full backlog and
cursor position are still there.

Auto-disconnect kicks in if you navigate away *without* suspending
(new profile, Settings button) — an explicit Suspend is the opt-in
to stay connected.

## RS-485 direction and odd control-line needs

RS-485 half-duplex uses RTS (or sometimes DTR) to toggle between TX
and RX mode. Profile policies handle the common shapes:

- Always-asserted RX (typical for a bus monitor): `rtsOnConnect:
  "assert"`, `rtsOnDisconnect: "deassert"`.
- TX-only controller: `rtsOnConnect: "assert"` to hold the driver
  enable while sending.
- Live bus work: use the live DTR/RTS pills in the session header to
  flip direction mid-session.

Some MCU dev boards tie their reset line to DTR. To avoid auto-reset
on connect: `dtrOnConnect: "deassert"`. Common for Arduino Uno R3
and clones when you want to attach a serial monitor without losing
state.

## Session logging for post-mortem work

**Enable:** profile → Advanced → "Record session to file."

**Location:** `<support>/logs/<profile-slug>_<YYYY-MM-DD_HHMMSS>.log`
by default. Override via Settings → Advanced → Session Log Directory.

**Format:** raw bytes, no timestamps or framing. Makes post-mortem
grep trivial; pair with the terminal's own timestamp prefix if you
want timing info in the log.

**Rotation:** none — each session produces one file. Rotate manually
or with your existing log tooling if you're recording 24/7.

**What's recorded:** incoming bytes only. Your keystrokes aren't in
the log. If you need a full transcript, combine with local echo so
your input appears on the RX stream as the device echoes it back.

## USB-serial driver troubleshooting

Seriesly shows a yellow banner above the port dropdown when it
detects a known USB-serial chipset plugged in but without its vendor
driver available to the OS.

**Detected chipsets:** CP210x (SiLabs), FTDI, Prolific PL2303, WCH
CH340/341, Microchip MCP2221, Cypress, ATEN, ARM mbed CDC-ACM,
MosChip/ASIX, Magic Control, Moxa UPort, Brainboxes. Plus special
cases: Siemens RUGGEDCOM RST2228 (CP210x with Siemens VID) and
counterfeit-Prolific detection (genuine old chips that Prolific's
modern driver refuses; TRENDnet TU-S9 is the classic).

**Banner flow:** click the driver-URL link to open the vendor
download page. Install, reboot if asked, unplug/replug the adapter,
click **Refresh**. Banner goes away when the driver shows up.

**Dismissing:** × button on the banner dismisses for the current
session. Globally disable via Settings → Advanced → "Detect
un-drivered USB adapters" if you've manually installed a driver the
detector doesn't recognize.

**Platform details:**
- macOS reads the IOKit registry via `ioreg -p IOUSB -l`. More
  reliable than `system_profiler` on recent macOS releases.
- Windows queries `Get-PnpDevice` through PowerShell.
- Linux: detection is a no-op (kernel drivers are built-in; if
  the port shows up it works).

## Copy-on-select for quick clipboard capture

Settings → Advanced → "Copy terminal selection to clipboard
automatically." PuTTY-style: drag-select and release → selection is
on the clipboard. No Cmd/Ctrl+C needed.

Off by default so users don't get surprise clipboard writes.
Empty selections (plain clicks) are ignored.

## Backspace / Delete mapping

Older Cisco IOS trains, some Foundry switches, and a handful of
industrial gear expect Backspace to send BS (0x08) instead of the
VT100/xterm default DEL (0x7f). Wrong setting shows as `^H` echoed
on screen when you hit Backspace.

Profile → "Backspace sends" → pick DEL or BS. Default is DEL, which
matches every modern OS and router.

## Window / view management

- **Clear** (session header) — clears the terminal viewport. Scrollback
  is preserved.
- **Hex view** and **plain view** don't share scrollback — toggling
  clears what's on screen.
- **Line timestamps** are prefixed at line commit time, so enabling
  mid-session timestamps only future lines, not backlog.
- **Suspend** — see [Navigating away without losing the session](#navigating-away-without-losing-the-session).

## Seeing what the device is sending (syntax highlighting)

Seriesly auto-colors common network-gear patterns on incoming text:

| Color       | Pattern                                                         |
| ----------- | --------------------------------------------------------------- |
| Cyan        | IPv4 (with/without CIDR), IPv6                                  |
| Magenta     | MAC addresses (colon, dash, or Cisco-dotted)                    |
| Blue        | Interface names (`GigabitEthernet0/1`, `Gi1/0/24`, `ge-0/0/1`, `Vlan100`) |
| Green       | `up`, `online`, `active`, `established`, `enabled`, `OK`, `FULL` |
| Red         | `down`, `failed`, `err-disabled`, `error`, `denied`, `timeout`, `critical` |
| Yellow      | `warning`, `degraded`, `init`, `learning`, `blocking`           |
| Dim gray    | Timestamps and dates                                            |

Device-supplied ANSI colors (Aruba CX, Junos, etc.) pass through
unchanged — the highlighter only fills in text the device left
uncolored. Toggle per profile if you're on a device whose output
collides with a pattern.
