# Advanced features

Reference for the features beyond basic connect-and-type. Each section
describes what the feature does, where it's configured, and notable
behavior. For the full JSON schema of every profile field referenced,
see [PROFILES.md](PROFILES.md).

## Send Break

- Session-header **Break** button.
- Sends a 300 ms break condition (TX line held low).
- Implemented via the `serialport` crate's `set_break()` /
  `clear_break()` pair (with a `sleep(duration)` between) — backed
  by `tcsendbreak` on Unix, `SetCommBreak` / `ClearCommBreak` on
  Windows. Direct-USB CP210x uses the SET_BREAK control transfer.
- Disabled while the session is in the reconnecting state.
- No keyboard shortcut: common modifier combinations collide with
  real terminal control characters.

## Control-line policies (DTR / RTS)

- Four profile fields: `dtrOnConnect`, `rtsOnConnect`,
  `dtrOnDisconnect`, `rtsOnDisconnect`.
- Valid values: `default` (leave the OS default state alone),
  `assert` (line high), `deassert` (line low). Empty string is
  treated as `default`.
- On-connect policies apply immediately after port open. On-
  disconnect policies apply immediately before port close.
- Live mid-session control: **DTR** and **RTS** pills in the session
  header flip the line and reflect the current state via the pill
  highlight.

## Hex view (RX)

- Per-profile toggle.
- Renders incoming bytes as a 16-byte-per-line hex dump with an
  ASCII sidebar.
- Mutually exclusive with the syntax highlighter — toggling either
  mode resets the other's buffer.
- Scrollback is not shared with plain view; switching modes mid-
  session clears the viewport.

## Hex send (TX)

- Session-header **Hex** button opens a modal input.
- Parser accepts any of, or a mix of:
  - Space-separated: `02 FF AA 55`
  - Compact: `02FFAA55`
  - `0x`-prefixed: `0x02 0xFF 0xAA 0x55`
  - Comma-separated: `02,FF,AA,55`
- Post-normalization (whitespace, commas, and `0x` prefixes
  stripped), length must be even and content must be pure hex
  `[0-9a-fA-F]`. Validation failures render an inline error and
  leave the modal open.
- **Enter** submits; **Escape** or a backdrop click cancels.
- On successful send the input clears but the modal stays open,
  supporting a sequence of sends without repeated button clicks.
- Bytes go through the same `api.sendBytes` path as typed input.

## File transfer (XMODEM / YMODEM)

- Session-header **Send File** button opens a modal.
- Protocol options:
  - **YMODEM** — 1024-byte blocks with CRC-16 and a header block
    carrying filename and size. Receivers that speak `rb`, `loady`
    on U-Boot, or most modern MCU bootloader "Receive YMODEM" menus.
  - **XMODEM-1K** — 1024-byte blocks with CRC-16, no filename
    metadata. Sometimes called XMODEM-CRC with 1K blocks or YAM.
  - **XMODEM-CRC** — 128-byte blocks with CRC-16. Receiver
    initiates with `C`.
  - **XMODEM** — 128-byte blocks with an 8-bit checksum. Receiver
    initiates with `NAK`. Legacy; present in some older ROMs and
    boot loaders.
- The transfer runs entirely in the Go backend. While in progress,
  the Session's RX dispatch is redirected to the protocol state
  machine, so incoming bytes don't appear in the terminal viewport
  or trigger syntax highlighting.
- Progress events stream to the frontend after each block ACK and
  drive a live progress bar in the modal.
- **Cancel** aborts the transfer mid-flight by sending `CAN CAN CAN
  CAN CAN` to the receiver, which every XMODEM-family receiver
  honors as "stop now."
- **Timeout behavior:** the initial handshake waits up to 60 s for
  the receiver to send `C` or `NAK`. Per-block ACK waits are 10 s
  with up to 10 retries per block. A stuck receiver eventually
  surfaces as a retry-exhaustion error rather than a hang.
- **ZMODEM is not implemented** — its state machine is an order of
  magnitude larger than XMODEM/YMODEM and most embedded bootloader
  targets don't speak it. Add-on request.

**Sequence for a typical firmware upload:**

1. Interrupt the target's boot loader so it's at a prompt.
2. Tell the boot loader to start its receiver (`loady`, `rx`, menu
   selection, etc.). It starts sending `C` (or `NAK` for plain
   XMODEM) to the serial line.
3. Open Send File in Baudrun, pick the matching protocol, choose
   the binary.
4. Click Send. The modal shows progress; the boot loader's status
   updates on the terminal are suspended during the transfer.
5. On completion, use the boot loader's flash/write command to
   commit the uploaded payload to non-volatile storage.

## Line timestamps

- Per-profile toggle.
- Prefixes each newly committed line with `[HH:MM:SS.mmm]`.
- Applied at line commit time; enabling mid-session does not
  retroactively timestamp existing scrollback.
- Only visible in plain view — hex view has no per-line concept to
  prefix.

## Config-directory relocation

- Settings → Advanced → **Config Directory**.
- Changes where the app reads profiles, themes, skins, and settings
  from. The default is platform-idiomatic
  (`~/Library/Application Support/Baudrun` on macOS,
  `%APPDATA%\Baudrun` on Windows, `$XDG_CONFIG_HOME/Baudrun` on
  Linux); relocation lets users who keep their dotfiles in a single
  tree (e.g. `~/dotfiles/baudrun/`) store Baudrun's config
  alongside.
- Implementation: a single-line redirect file
  (`config_dir_override`) lives at the platform default location
  and contains the absolute path of the custom directory. On startup,
  `appdata.SupportDir()` reads it; if absent, falls back to the
  default. Deleting the file resets to default.
- **Takes effect on next launch** — stores are loaded once at
  startup. The Settings UI shows a "Restart Baudrun to apply"
  status message.
- **Existing files are not migrated.** Moving the config pointer is
  separate from moving the files themselves; copy profiles.json /
  themes / skins / settings.json over manually if you want to
  preserve them. A fresh start at the new location is equally valid.

## Session logging

- Per-profile toggle (`logEnabled`).
- Destination:
  `<logdir>/<profile-slug>_<YYYY-MM-DD_HHMMSS>.log`.
- Default `<logdir>` is `<support>/logs/`; overridable in
  Settings → Advanced → Session Log Directory.
- One file per session. No built-in rotation or rollover.
- Writes raw incoming bytes only — no framing, no timestamps, no
  captured keystrokes. Device echo of user input appears if the
  device itself echoes.
- The writer is closed cleanly on disconnect.

## Auto-reconnect

- Per-profile toggle (`autoReconnect`, default `true`).
- Triggered when the session's read pump exits with an error —
  typically "port disappeared" after a USB-serial adapter
  re-enumerates.
- Runs in the Go backend, independent of UI state, so it keeps
  working while the session is suspended (the user won't see the
  "reconnecting" chrome because the terminal is hidden, but the
  status bar still updates and the session is already reconnected
  on resume).
- Poll interval: 1 s. Timeout: 30 s.
- Reopens with the config snapshotted at Connect time, not the
  current profile state. Edits to the profile during reconnect
  don't retarget the next attempt.
- The `<Terminal>` component stays mounted across the gap, so
  scrollback survives.
- Session state gains a `reconnecting` status between `connected`
  and `idle`. Session-header dot pulses amber; sub-label shows
  `reconnecting…`.
- Break, DTR, and RTS buttons are disabled while reconnecting.
- User-clicked Disconnect cancels the retry loop before closing.
- Timeout without a successful reopen emits a standard disconnect
  event with `reason: "reconnect timeout"`.

## Paste safety

Two profile-level flags that alter behavior when an `onData`
callback is detected as a paste rather than typed input.

**Paste detection heuristic:** input is treated as a paste if it is
at least 20 characters long OR contains `\r` / `\n`. Typed input
never crosses lines in a single callback and typing bursts top out
well below 20 characters.

**`pasteWarnMultiline`** (profile field, default `true`)

- Only applies to pastes that contain a line break.
- Prompts via `window.confirm` with the line count plus a truncated
  (80-char) preview of the first line.
- On cancel, the paste is discarded and a "Paste cancelled" status
  message is shown.

**`pasteSlow`** (profile field, default `true`) + **`pasteCharDelayMs`** (profile field, default `10`, valid range 0-500)

- When enabled, pasted bytes are sent one at a time with
  `pasteCharDelayMs` between each.
- Local echo (if the profile has it on) is applied per-byte.
- The flag composes with `pasteWarnMultiline`: the confirmation
  runs first, then the slow send begins on approval.
- A progress pill appears top-right of the terminal while a slow
  paste is in flight (`PASTE  142/500 bytes  Esc to cancel`),
  themed against the active palette.
- Pressing `Escape` while the pill is visible aborts the paste;
  any bytes already sent stay with the device, the remainder is
  discarded, and a `Paste aborted after N/M bytes` status is
  logged. The `Escape` keystroke itself is swallowed so the
  device doesn't see a stray 0x1B mid-stream.

## Backspace key mapping

- Profile field `backspaceKey`: `"del"` (0x7F) or `"bs"` (0x08).
  Empty string is treated as `"del"`.
- Default `"del"` matches VT100 / xterm / every modern OS; wrong
  value typically surfaces as `^H` echoed on screen when Backspace
  is pressed.
- xterm always emits 0x7F from the Backspace key. The swap happens
  in the Terminal component's input handler before `sendBytes`.

## Copy on select

- Global setting (Settings → Advanced), default `false`.
- Hooks xterm's `onSelectionChange` event. On every selection
  update, the current selection text is written to the clipboard
  via `navigator.clipboard.writeText`.
- Empty selections (plain clicks) are skipped so clicking into the
  terminal doesn't clobber the clipboard.
- Because the event fires continuously during a drag, the clipboard
  always reflects the live selection.

## Session suspend / resume

- **Suspend** session-header button: leaves the port open and the
  `<Terminal>` component mounted, and routes the UI back to the
  profile view.
- **Resume** is triggered by returning to the suspended profile's
  terminal view. A `refit()` call on the xterm instance re-syncs
  the viewport to current dimensions.
- Because the component never unmounts, scrollback and cursor
  position are preserved across suspend.
- Navigating away *without* suspending (clicking another profile,
  opening Settings, or creating a new profile) triggers an automatic
  Disconnect.

## Scrollback

- Global setting (`scrollbackLines`, default `10000`). Applied via
  xterm.js's constructor `scrollback` option.
- Settings UI ships five presets (1k / 5k / 10k / 50k / 100k) with
  approximate memory cost annotated inline. Custom values can be set
  by editing `settings.json` directly and are preserved, surfaced in
  the dropdown as `N lines (custom)` so they don't silently reset.
- Changing the setting tears down and rebuilds the `<Terminal>`
  component — xterm doesn't resize the ring buffer in place. The
  rebuild snapshots the existing buffer as plain text and writes it
  back into the fresh instance. Tradeoffs on the existing scrollback:
  plain text survives, ANSI color attributes are flattened, current
  selection is lost. New output after the rebuild is colored as
  normal. Same mechanism the font-size live-update uses.
- The buffer is strictly in-memory. Independent of, and not
  affected by, the Session logging feature — if you need permanent
  history, enable `logEnabled` on the profile.

**Memory cost (approximate, at a 200-column terminal):**

| Scrollback | Memory |
|---|---|
| 1,000 lines | ~0.4 MB |
| 5,000 lines | ~2 MB |
| 10,000 lines (default) | ~4 MB |
| 50,000 lines | ~20 MB |
| 100,000 lines | ~40 MB |
| 500,000 lines | ~200 MB |

xterm.js stores each cell as an object (character + 24-bit fg/bg +
attributes), so the per-line cost scales with column count.
Narrower terminals cost proportionally less.

Past ~100k lines, two things start to degrade: the WebKit garbage
collector spends visibly more time on cleanup cycles, and the
buffer-snapshot step inside the recreate path (font-size or
scrollback change) takes noticeably longer because it linearly
reads every cell. In-terminal search, if/when wired in, would also
scale linearly with line count.

## Light / dark appearance

- Global setting (`appearance`): `auto` / `light` / `dark`.
- `auto` follows the OS preference via
  `matchMedia('(prefers-color-scheme: dark)')` and updates live when
  the OS flips.
- Only swaps the CSS palette. The window's own
  `NSVisualEffectView` material on macOS is pinned to the dark
  system appearance at startup — Tauri v2's runtime appearance
  setters don't reliably swap NSAppearance live on macOS, so the
  vibrancy material cannot change after launch.
- Dark-only skins (CRT, Cyberpunk) ignore the preference and pin
  their palette to dark.

## Syntax highlighting

- Per-profile toggle (`highlight`, default `true`) plus a per-profile
  pack-override list. Settings → Syntax Highlighting picks the global
  default packs; profiles override under their own collapsible
  Syntax Highlighting card.
- Rules ship as **packs** — JSON files with a list of `{pattern, color,
  ignoreCase?, group?}` entries. Built-in packs:

  | Pack | Covers |
  | ---- | ------ |
  | **Baudrun default** | Vendor-neutral: IPv4/IPv6, MACs, interface names, `up`/`down`/`error`/`warning` keywords, timestamps, dates, VLANs |
  | **Cisco IOS / IOS XE / IOS XR** | `line protocol`, log mnemonics (`%LINK-3-UPDOWN`), STP roles (`DESG`/`ROOT`/`ALTN`), OSPF/BGP states, AS numbers, ACL `permit`/`deny` |
  | **Juniper Junos** | Chassis status (`Online`/`Empty`), BGP/OSPF/IS-IS states, `[edit ...]` banners, commit messages, set/delete diff lines |
  | **Aruba AOS-CX** | VSX/VSF status, LAG/MCLAG, STP role+state, daemon names in event logs, ACL actions |

- The "User overrides" pack lives at
  `$SUPPORT_DIR/highlight-rules.json` and is editable on disk —
  rules there layer on top of bundled packs.
- Available colors: `red`, `green`, `yellow`, `blue`, `magenta`,
  `cyan`, `dim`. First match wins on overlap; rules within a pack
  are tried in array order, packs in load order.
- Mutually exclusive with hex view — toggling either mode resets
  the other's line buffer.
- Device-supplied ANSI CSI colors pass through unchanged. The
  highlighter only applies color to text that arrived uncolored.

### Playground

Want to try a regex before saving it? Open the
[**rule playground**](playground.html) — paste or drop a real capture,
edit the pack JSON, see the colors apply live. Everything runs in your
browser; the file you drop never leaves your machine.

## Theme preview

- Settings → Installed Themes → each row has a **Preview** button.
- Opens a modal containing a read-only xterm instance that renders
  a canned RuggedCom-style output sample (prompts, interface
  status, MAC addresses, IPs, timestamps, warnings, errors).
- The sample is passed through the highlighter, so the preview
  shows the combined palette + highlighter behavior, not just the
  raw theme colors.
- The preview terminal focuses itself after the initial write so
  the cursor renders filled rather than the unfocused outline
  state.

## USB-serial driver detection

> For the full per-chipset × per-OS support matrix and cable-buying
> guidance, see [USB-serial adapters](ADAPTERS.md). This section
> covers the in-app banner behaviour only.

- Global setting (Settings → Advanced): "Detect un-drivered USB
  adapters." Default on.
- Shows a yellow banner above the port dropdown when a USB-serial
  chipset is plugged in but no corresponding serial port is
  enumerated by the OS **and** Baudrun's vendored libusb-direct
  backend (`src-tauri/src/usbserial/`) can't open it either.
- Banner includes a link to the vendor driver download and a
  Refresh action.
- Dismissal via × is session-scoped; the banner re-shows on the
  next app launch for the same adapter.

**Chipset coverage:** CP210x (SiLabs), FTDI, Prolific PL2303, WCH
CH340 / CH341, Microchip MCP2221, Cypress, ATEN, ARM mbed CDC-ACM,
MosChip / ASIX, Magic Control, Moxa UPort, Brainboxes.

**Special cases:**

- **Siemens RUGGEDCOM RST2228** — CP210x reprogrammed with Siemens
  VID:PID (`0908:01FF`). Mapped via a known-rebrand table back to
  the SiLabs driver.
- **Counterfeit-Prolific** — older genuine Prolific PL2303 chips
  that Prolific's current driver refuses as counterfeit (TRENDnet
  TU-S9 is the canonical example). Detected via manufacturer-string
  heuristic; banner points at the legacy Prolific driver.

**Platform detection mechanism:**

| Platform | Source                                                  |
| -------- | ------------------------------------------------------- |
| macOS    | IOKit registry via `ioreg -p IOUSB -l -w 0`             |
| Windows  | `Get-PnpDevice` invoked through PowerShell              |
| Linux    | No-op. Kernel modules for all listed chipsets are built in, so an adapter either shows up as a tty or doesn't. |

## Skin and theme systems

App chrome (skins) and terminal color scheme (themes) are separate
systems that can be mixed freely. See [SKINS.md](SKINS.md) and
[THEMES.md](THEMES.md) for the reference.
