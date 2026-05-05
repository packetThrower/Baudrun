---
title: Advanced settings
description: 'Reference for everything beyond basic connect-and-type: hex view, session logs, syntax highlighting, file transfer.'
editUrl: https://github.com/packetThrower/Baudrun/edit/main/docs/ADVANCED.md
---

Reference for the features beyond basic connect-and-type. Each section
describes what the feature does, where it's configured, and notable
behavior. For the full JSON schema of every profile field referenced,
see [PROFILES.md](/Baudrun/usage/profiles/).

## Send Break

- Session-header **Break** button.
- Sends a 300 ms break condition (TX line held low).
- Implemented via the `serialport` crate's `set_break()` and
  `clear_break()` pair (with a `sleep(duration)` between), backed
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
- Mutually exclusive with the syntax highlighter. Toggling either
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
  - **YMODEM.** 1024-byte blocks with CRC-16 and a header block
    carrying filename and size. Receivers that speak `rb`, `loady`
    on U-Boot, or most modern MCU bootloader "Receive YMODEM" menus.
  - **XMODEM-1K.** 1024-byte blocks with CRC-16, no filename
    metadata. Sometimes called XMODEM-CRC with 1K blocks or YAM.
  - **XMODEM-CRC.** 128-byte blocks with CRC-16. Receiver
    initiates with `C`.
  - **XMODEM.** 128-byte blocks with an 8-bit checksum. Receiver
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
- **ZMODEM is not implemented.** Its state machine is an order of
  magnitude larger than XMODEM/YMODEM and most embedded bootloader
  targets don't speak it. Open an issue if you need it.

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
- Only visible in plain view; hex view has no per-line concept to
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
- **Takes effect on next launch.** Stores are loaded once at
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
- Writes raw incoming bytes only. No framing, no timestamps, no
  captured keystrokes. Device echo of user input appears if the
  device itself echoes.
- The writer is closed cleanly on disconnect.

## Auto-reconnect

- Per-profile toggle (`autoReconnect`, default `true`).
- Triggered when the session's read pump exits with an error,
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

## Multi-window

Every profile can be torn off into its own window, and each window
holds an independent serial session. Useful for parallel work on two
or more devices side-by-side without juggling tabs.

### Gestures

- **Right-click** any profile in the sidebar → "Open profile in new
  window".
- **Drag** any profile out of the sidebar and release the mouse
  outside the source window.

Both gestures route through the same handler, so behavior is
identical regardless of which one you reach for.

### Session migration on tear-off

If the profile you're tearing off is *currently connected* in the
source window, the live session moves with it:

- The serial port stays open the entire time. No mid-session bytes
  are lost, no DTR/RTS pulse fires, and there is no need to re-
  authenticate the device-side shell.
- The visible xterm scrollback rides along too (serialized via
  `@xterm/addon-serialize` and replayed in the new window's terminal
  on mount), so you don't lose what was on screen.
- The source window's session UI clears. The connection now belongs
  to the new window.
- An in-flight XMODEM/YMODEM transfer blocks migration with a
  `transfer in progress: wait for it to finish or cancel before
  migrating` error. Wait for the transfer or click Cancel transfer
  first.

If the dragged profile *isn't* currently connected, the new window
just opens with that profile selected and disconnected (same as
right-click → "Open" with a non-active profile).

### Per-window sessions

Each window keeps its own session in the backend, keyed by Tauri
window label (`main` for the original, `win-<uuid>` for spawned
ones). Settings, profiles, themes, skins, and highlight packs
remain shared across all windows; only the *connection state* is
per-window. Closing a window disconnects only that window's session,
not the others.

A single physical port can still only be opened once at the OS
level. If window A is connected to `/dev/cu.usbserial-1234` and you
try to connect window B to the same port, the OS returns busy.
Migrate the session instead of trying to open the port twice.

### Cross-platform behavior

- macOS: spawned windows match the main window's overlay-titlebar +
  traffic-light layout, so they don't look out of place against the
  active skin.
- Windows + Linux: spawned windows get the OS's default decorated
  chrome (titlebar with title + min/max/close). The drag-out
  gesture works the same: Tauri queries the OS cursor position
  directly rather than trusting browser-event coordinates, which
  side-steps the WebKitGTK / WebView2 quirks around `dragend`.

## Software updates

Settings → Advanced → Updates controls the launch-time check
against GitHub for a newer Baudrun release. Default behavior:

- Check for updates on launch is **on**. The check hits the GitHub
  Releases API once per app start; failures (offline, rate limit)
  fall through silently.
- Include pre-releases is **off**. Stable users see only stable
  releases as updates; flip the toggle to surface alphas / betas
  / RCs too.

When an update is available a one-line toast appears in the
bottom-right of the footer: `Update available: vX.Y.Z` linking to
the release notes, with an `×` to dismiss for that exact version
(it'll re-show next time something newer is published). Pre-release
toasts open the release page in your browser so you can read the
notes and download manually. Stable toasts get an additional
**Install** button that uses Tauri's signed-updater plugin:

1. Downloads the platform-specific bundle from the GitHub release.
2. Verifies the bundle's minisign signature against the public key
   embedded in `tauri.conf.json`. Bundles signed with any other
   key are rejected before they touch disk.
3. Replaces the installed binary in place and relaunches into the
   new version.

The signing keypair is held outside the repo. Per-platform mechanics:

- **macOS.** Auto-update unpacks `Baudrun.app.tar.gz` next to the
  installed `.app` and replaces it. Builds are ad-hoc codesigned
  (no paid Apple Developer account yet), so each downloaded update
  triggers Gatekeeper's "right-click → Open" prompt the first time
  on each user's machine.
- **Windows.** Downloads the NSIS `setup.exe` and runs it in
  silent mode.
- **Linux.** AppImage updater. `.deb`, `.rpm`, and Arch users update
  through their distro's package manager (or download a fresh
  package from the release page); the in-app updater leaves those
  alone.

Pre-release tags (`vX.Y.Z-alpha.N`, `-beta.N`, `-rc.N`) ship signed
updater bundles too but **don't** update the
`/releases/latest/download/latest.json` manifest the auto-updater
reads from. So existing stable installs aren't auto-jumped onto an
alpha; pre-release-channel users continue to install pre-releases
manually.

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
  component, since xterm doesn't resize the ring buffer in place. The
  rebuild snapshots the existing buffer as plain text and writes it
  back into the fresh instance. Tradeoffs on the existing scrollback:
  plain text survives, ANSI color attributes are flattened, current
  selection is lost. New output after the rebuild is colored as
  normal. Same mechanism the font-size live-update uses.
- The buffer is strictly in-memory. Independent of, and not
  affected by, the Session logging feature. If you need permanent
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
  system appearance at startup. Tauri v2's runtime appearance
  setters don't reliably swap NSAppearance live on macOS, so the
  vibrancy material cannot change after launch.
- Dark-only skins (CRT, Cyberpunk) ignore the preference and pin
  their palette to dark.

## Syntax highlighting

> Authoring guide: see [**Authoring → Syntax highlighting**](/Baudrun/authoring/highlighting/) for the full schema, the rule playground, and how to write your own packs. This section covers the in-app feature behavior only.

- Per-profile toggle (`highlight`, default `true`) plus a per-profile
  pack-override list. Settings → Syntax Highlighting picks the global
  default packs; profiles override under their own collapsible
  Syntax Highlighting card.
- Rules ship as **packs**: JSON files with a list of `{pattern, color,
  ignoreCase?, group?}` entries. Built-in packs:

  | Pack | Covers |
  | ---- | ------ |
  | **Baudrun default** | Vendor-neutral: IPv4/IPv6, MACs, interface names, `up`/`down`/`error`/`warning` keywords, timestamps, dates, VLANs |
  | **Cisco IOS / IOS XE / IOS XR** | `line protocol`, log mnemonics (`%LINK-3-UPDOWN`), STP roles (`DESG`/`ROOT`/`ALTN`), OSPF/BGP states, AS numbers, ACL `permit`/`deny` |
  | **Juniper Junos** | Chassis status (`Online`/`Empty`), BGP/OSPF/IS-IS states, `[edit ...]` banners, commit messages, set/delete diff lines |
  | **Aruba AOS-CX** | VSX/VSF status, LAG/MCLAG, STP role+state, daemon names in event logs, ACL actions |
  | **Arista EOS** | MLAG peer state, VXLAN/EVPN fabric keywords, `Et1/1` short-form interfaces, `Aboot`/EOS version banners, log facility (`%BGP-5-ADJCHANGE`), config-section headers |
  | **MikroTik RouterOS** | `/export` section paths (`/ip firewall filter`, `/interface vlan`), `k=v` parameter syntax, firewall chain + action semantics (accept/drop/reject), connection states, RouterOS-style interface names (`ether1`, `wlan1`, `wg0`) |

- The "User overrides" pack lives at
  `$SUPPORT_DIR/highlight-rules.json` and is editable on disk.
  Rules there layer on top of bundled packs.
- Available colors: `red`, `green`, `yellow`, `blue`, `magenta`,
  `cyan`, `dim`. First match wins on overlap; rules within a pack
  are tried in array order, packs in load order.
- Mutually exclusive with hex view. Toggling either mode resets
  the other's line buffer.
- Device-supplied ANSI CSI colors pass through unchanged. The
  highlighter only applies color to text that arrived uncolored.

### Importing shared packs

Settings → Syntax Highlighting → **Import pack…** copies a JSON file
into `$SUPPORT_DIR/highlight/<id>.json`. The imported pack auto-enables,
shows a **Remove** button next to its entry, and otherwise behaves
identically to the bundled ones. Imports with an `id` that collides
with a bundled pack or the reserved `user` scratchpad are rejected.

Two starter packs are documented with a copy-button code block plus
a download link:

- [Minimal example](https://github.com/packetThrower/Baudrun/blob/main/docs/examples/highlight-pack.example.md):
  near-empty skeleton showing the schema. Copy, rename `id` and `name`,
  add rules, import.
- [Syslog / journald](https://github.com/packetThrower/Baudrun/blob/main/docs/examples/syslog.example.md):
  practical starter for generic syslog / journald output (severity
  keywords, systemd unit states, sshd accepted/denied lines,
  `[OK]`/`[FAILED]` markers, daemon tags, PIDs).

### Playground

Want to try a regex before saving it? Open the
[**rule playground**](/Baudrun/playground.html), paste or drop a real
capture, edit the pack JSON, and see the colors apply live. Everything
runs in your browser; the file you drop never leaves your machine.

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
> guidance, see [USB-serial adapters](/Baudrun/usage/adapters/). This section
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

- **Siemens RUGGEDCOM RST2228.** CP210x reprogrammed with Siemens
  VID:PID (`0908:01FF`). Mapped via a known-rebrand table back to
  the SiLabs driver.
- **Counterfeit-Prolific.** Older genuine Prolific PL2303 chips
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
systems that can be mixed freely. See [SKINS.md](/Baudrun/authoring/skins/) and
[THEMES.md](/Baudrun/authoring/themes/) for the reference.
