# Testing Baudrun hex send and file transfer

A cookbook for exercising Baudrun's **Send Hex…** and **Send File…**
features against the `virtual-serial` bridge, without physical USB-serial
hardware. Covers prereqs, per-test steps, expected output, and how to
verify byte-for-byte correctness.

Unix only (macOS + Linux) — `virtual-serial` uses pty primitives that
Windows doesn't expose. Windows dev machines should pair com0com ports
and swap `/tmp/baudrun-*` for the paired COM paths throughout.

---

## Prereqs

### Tools

| Tool | What it's for | Install (macOS) | Install (Debian/Ubuntu) |
|---|---|---|---|
| `virtual-serial` | Bridges two pty endpoints with baud-rate pacing (this repo) | `go run ./scripts/virtual-serial …` | Same |
| `lrzsz` | `rb` / `rx` receivers for YMODEM / XMODEM | `brew install lrzsz` | `sudo apt install lrzsz` |
| `xxd`, `od`, `cat`, `cmp`, `diff` | Byte inspection + file comparison | Ships with macOS | Ships with every Unix |

### Baudrun profile pointing at the virtual port

Baudrun's port dropdown only lists enumerated hardware serial devices,
so you need to inject a profile whose `portName` points at the symlink
the bridge will create (`/tmp/baudrun-a`). Two options:

**Option A — edit the profile JSON directly (easiest).**

1. Close Baudrun.
2. Open the profile store:
   - macOS: `~/Library/Application Support/Baudrun/profiles.json`
   - Linux: `~/.config/Baudrun/profiles.json`
3. Add a profile (copy an existing one, tweak `id` to any unique
   string, set `portName`):

   ```json
   {
     "id": "virtual-test",
     "name": "Virtual test",
     "portName": "/tmp/baudrun-a",
     "baudRate": 9600,
     "dataBits": 8,
     "parity": "none",
     "stopBits": "1",
     "flowControl": "none",
     "lineEnding": "cr",
     "autoReconnect": true,
     "pasteWarnMultiline": true,
     "pasteSlow": true,
     "pasteCharDelayMs": 10,
     "backspaceKey": "del",
     "dtrOnConnect": "default",
     "rtsOnConnect": "default",
     "dtrOnDisconnect": "default",
     "rtsOnDisconnect": "default",
     "createdAt": "2026-04-22T00:00:00Z",
     "updatedAt": "2026-04-22T00:00:00Z"
   }
   ```
4. Reopen Baudrun. The profile appears in the sidebar; its port
   will read `/tmp/baudrun-a (not connected)` until the bridge is
   up, then becomes selectable for Connect.

**Option B — reuse an existing profile.** If you have a profile for a
physical adapter that's currently unplugged, set its `portName` to
`/tmp/baudrun-a` via the same JSON edit. Change it back later for
regular use.

### Terminal layout

Most tests use two or three terminals plus the Baudrun window:

| Terminal | Purpose |
|---|---|
| **Bridge** | `virtual-serial` process — leave running for the whole test session |
| **Receiver** | The tool consuming `/tmp/baudrun-b` for a given test (`xxd`, `rb`, `rx`, etc.) |
| **Baudrun window** | Where you drive **Send Hex…** / **Send File…** |
| **Scratch** (optional) | For one-off `diff` / `cmp` / file-creation commands |

Open them all at the start, keep them visible.

### Starting the bridge

At the repo root, leave this running for the whole session:

```sh
go run ./scripts/virtual-serial -baud 9600 -link-a /tmp/baudrun-a -link-b /tmp/baudrun-b
```

You should see:

```
Endpoint A: /dev/ttysNNN (→ /tmp/baudrun-a)
Endpoint B: /dev/ttysMMM (→ /tmp/baudrun-b)
Throttle:   9600 baud, 10 bits/byte → 1.042ms per byte
Ctrl+C to quit.
```

For faster file transfer tests you can bump `-baud` up to 115200 or
higher. Just restart the bridge between baud changes.

---

## Hex send tests

### T1 — basic ASCII

Verifies bytes entered into **Send Hex…** arrive byte-for-byte and in
the right order.

**Receiver** (16-column hex dump):

```sh
xxd -c 16 < /tmp/baudrun-b
```

**In Baudrun:**

1. Connect the virtual profile.
2. ⋯ → **Send Hex…**
3. Enter `48 65 6c 6c 6f` (hex for "Hello").
4. Click **Send**.

**Expected in the receiver:**

```
00000000: 4865 6c6c 6f                             Hello
```

Baudrun status bar: `Sent 5 bytes`.

Rerun `xxd` (Ctrl-C, re-invoke) between sub-tests so each batch starts
from offset `0`.

### T2 — input format equivalence

All three input forms below should produce byte-identical output:

- `41 42 43` (space-separated)
- `414243` (compact)
- `0x41 0x42 0x43` (`0x`-prefixed)

Expected hex in each case:

```
00000000: 4142 43                                  ABC
```

### T3 — binary / non-printable bytes

```
Input: 00 01 02 ff fe 7f
```

Expected:

```
00000000: 0001 02ff fe7f                           ......
```

ASCII column shows `.` for every non-printable byte, as xxd does for
any byte outside 0x20–0x7e.

### T4 — input validation

Verifies invalid hex is rejected before sending.

| Input | Expected behavior |
|---|---|
| `xyz` | Dialog shows `Invalid: non-hex characters`; nothing hits the receiver. |
| `abc` | `Invalid: odd number of hex digits`; nothing sent. |
| *(empty)* | `Invalid: empty`; nothing sent. |
| `0x` (just the prefix) | Same — empty after stripping. |

### T5 — large payload

Generates 1 KiB of random bytes as hex, sends it through **Send Hex…**
in one go. Exercises the modal input + the rate-limited write path.

```sh
# Generate a hex string and copy it to clipboard
python3 -c "import os; print(os.urandom(1024).hex())" | pbcopy   # macOS
# or: xclip -selection clipboard on Linux
```

In Baudrun: open **Send Hex…**, paste, Send. At 9600 baud the transfer
takes ~1.1 seconds (1024 × 10 bits ÷ 9600 ≈ 1.07 s) — slow enough to
see live progress.

Verify in the receiver that exactly 1024 bytes arrived:

```sh
# Alternative receiver that counts bytes
wc -c < /tmp/baudrun-b   # only meaningful after the sender closes — not useful here
# Better: pipe through tee to capture while live-viewing
tee /tmp/hex-capture.bin < /tmp/baudrun-b | xxd -c 16
# Ctrl-C when output stops growing, then:
wc -c /tmp/hex-capture.bin   # should be 1024
```

---

## File send tests

These use `lrzsz` as the receiver. The shape is always:

1. Create a source file.
2. Launch the receiver on `/tmp/baudrun-b`.
3. In Baudrun, **Send File…** → pick the protocol → pick the source.
4. Wait for completion.
5. `diff` / `cmp` source vs received.

Bump the bridge baud rate for file tests so they don't crawl:

```sh
# Ctrl-C the running bridge, then:
go run ./scripts/virtual-serial -baud 115200 -link-a /tmp/baudrun-a -link-b /tmp/baudrun-b
```

Also update the Baudrun profile's `baudRate` to `115200` (edit the
JSON or bump it in the profile editor) so the two ends agree — the
bridge emulates whatever rate it was started with, and Baudrun's
termios is what actually determines what goes on the virtual wire.

Make a receiver landing dir:

```sh
mkdir -p /tmp/baudrun-rx
```

### T6 — YMODEM, single file round-trip

YMODEM carries the filename and size in the first block, so the
receiver writes to the original filename automatically.

**Source:**

```sh
head -c 4096 /dev/urandom > /tmp/test-payload.bin
```

**Receiver:**

```sh
cd /tmp/baudrun-rx && rb -v < /tmp/baudrun-b > /tmp/baudrun-b
```

`rb` will print `Receiving: test-payload.bin` once Baudrun sends the
header block, then `Bytes received: 4096/4096 BPS:…` on success.

**In Baudrun:**

1. ⋯ → **Send File…**
2. **Protocol:** YMODEM
3. **File:** `/tmp/test-payload.bin`
4. **Send**.

**Expected:**

- Progress bar fills 0 → 4096 over ~0.5 s at 115200 baud.
- Baudrun status: `Sent test-payload.bin`.
- `rb` exits cleanly.

**Verify:**

```sh
diff /tmp/test-payload.bin /tmp/baudrun-rx/test-payload.bin && echo MATCH
```

### T7 — XMODEM variants

Same shape as T6, but XMODEM receivers don't read a filename header, so
you name the destination yourself on the command line.

| Protocol in Baudrun | Receiver command |
|---|---|
| **XMODEM** (128-byte, checksum) | `rx /tmp/baudrun-rx/out.bin < /tmp/baudrun-b > /tmp/baudrun-b` |
| **XMODEM-CRC** (128-byte, CRC-16) | `rx -c /tmp/baudrun-rx/out.bin < /tmp/baudrun-b > /tmp/baudrun-b` |
| **XMODEM-1K** (1024-byte, CRC-16) | `rx -k /tmp/baudrun-rx/out.bin < /tmp/baudrun-b > /tmp/baudrun-b` |

**Padding caveat:** XMODEM pads the last block with `0x1a` (SUB) up to
the block size. A 100-byte source sent over XMODEM lands as a 128-byte
received file. Use `cmp -n` to compare only the first N bytes:

```sh
cmp -n $(wc -c < /tmp/test-payload.bin) \
    /tmp/test-payload.bin /tmp/baudrun-rx/out.bin && echo MATCH
```

### T8 — small file (single-block)

Exercises the padding path and the "one block, immediate EOT" control
flow.

```sh
echo "hello" > /tmp/tiny.txt       # 6 bytes including the trailing newline
```

Send via XMODEM (classic). Verify:

- Received file is exactly 128 bytes.
- First 6 bytes match the source.
- Bytes 7–128 are all `0x1a`.

```sh
wc -c /tmp/baudrun-rx/out.bin                              # 128
cmp -n 6 /tmp/tiny.txt /tmp/baudrun-rx/out.bin && echo HEAD-MATCH
xxd /tmp/baudrun-rx/out.bin | tail -3                      # trailing 1a 1a 1a ...
```

### T9 — large file, many blocks

```sh
head -c 1048576 /dev/urandom > /tmp/big.bin   # exactly 1 MiB
```

Send via YMODEM at 115200 baud. Expected wall time ≈ 1048576 × 10 ÷
115200 ≈ 91 seconds, plus a few seconds of protocol overhead. Watch
the progress bar tick smoothly from 0 → 1048576 — any long stall means
a retry loop is happening (also surfaced in `rb -v` output).

Verify:

```sh
diff /tmp/big.bin /tmp/baudrun-rx/big.bin && echo MATCH
```

### T10 — cancel mid-transfer

While T9 is in flight, click **Cancel transfer** in Baudrun's transfer
modal. Expected:

- Transfer stops within a block.
- Baudrun status: transfer error or cancelled message.
- `rb` exits non-zero (timeout or CAN byte received).
- Partial file may exist in `/tmp/baudrun-rx/` — delete it before the
  next test.
- Running T6 again immediately afterwards should succeed (the bridge
  and Baudrun both recover cleanly).

### T11 — transfer over a slow link

Restart the bridge at 9600 baud and repeat T6 with a small file
(256–512 bytes). Exercises:

- Visible progress bar animation.
- Timing-sensitive timeouts inside `rb` (they're generous enough that
  9600 baud is fine, but it's worth confirming).

---

## Scrollback tests

### T12 — scrollback retention limit

Verifies the scrollback setting actually caps what the terminal keeps.

**Setup:**

Baudrun **Settings → Terminal → Scrollback** = `1,000 lines`.
Restart the bridge at a fast rate so the flood doesn't take forever:

```sh
go run ./scripts/virtual-serial -baud 115200 -link-a /tmp/baudrun-a -link-b /tmp/baudrun-b
```

Connect the virtual profile in Baudrun.

**Flood:**

In a scratch terminal:

```sh
seq 1 2000 > /tmp/baudrun-b
```

2000 newline-terminated numbers stream into endpoint B, the bridge
forwards them to A, Baudrun renders them.

**Expected:**

- Scroll all the way up in Baudrun's terminal pane.
- The oldest visible line is around `1001` — give or take a few
  because xterm can trim up to one display row at the head during
  reflow. Lines `1`–`~1000` have been pushed out of the 1000-line
  buffer.
- The newest line is `2000`.

**Verify in settings.json:**

```sh
grep scrollbackLines "$HOME/Library/Application Support/Baudrun/settings.json"
# "scrollbackLines": 1000
```

### T13 — live setting change, preserve plain-text scrollback

Verifies changing the setting at runtime rebuilds `<Terminal>` without
nuking the existing content.

1. Continue from T12's state (buffer has lines roughly 1001–2000).
2. Open **Settings**, change **Scrollback** to `10,000 lines`.
3. Return to the terminal pane.

**Expected:**

- A brief flicker as `<Terminal>` rebuilds.
- Existing lines 1001–2000 still visible when scrolling up.
- ANSI color attributes on old output may be flattened to the default
  palette (documented tradeoff — the recreate path snapshots as plain
  text).
- New data pushed through the bridge after the change is colored
  normally.
- Flood another batch (`seq 2001 12000 > /tmp/baudrun-b`) and verify
  the buffer now retains ~10,000 lines instead of 1,000.

### T14 — custom (non-preset) value preservation

Confirms that hand-edited values survive a round-trip through the UI
dropdown without being silently rounded to a preset.

1. Close Baudrun.
2. Edit `settings.json`, set `"scrollbackLines": 7777`.
3. Reopen Baudrun.
4. Open **Settings → Scrollback**.

**Expected:**

- Dropdown shows `7,777 lines (custom)` as the selected option on top
  of the five preset rows.
- Changing to a preset and back to the custom value isn't possible
  from the UI (by design — custom values are display-only via the
  dropdown). To preserve a custom value, don't touch the dropdown;
  to set a new custom value, edit the JSON file.

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `rb` / `rx` hangs at startup, no data | Baudrun never pressed Send, or profile isn't connected | Check Baudrun's session status; kill receiver with Ctrl-C and retry after connecting |
| Bytes arrive line-buffered in `cat` (only on Enter) | `virtual-serial` build from before the raw-mode fix | Ctrl-C the bridge and rerun `go run ./scripts/virtual-serial …` (`go run` rebuilds each invocation) |
| Symlinks missing in `/tmp/` | Bridge process exited | `ps aux \| grep virtual-serial` to check, rerun if dead |
| Baudrun: `port busy` or `resource busy` on Connect | Two Baudrun instances sharing the same pty slave, or a leaked child from a previous aborted test | Close Baudrun, kill bridge (`pkill -f virtual-serial`), restart both |
| Transfer succeeds but `diff` reports differences on XMODEM | Forgetting the trailing-pad caveat | Use `cmp -n $(wc -c < src)` (see T7) |
| Baudrun's "Auto-reconnect" kicks in during test | Bridge got killed mid-test, adapter "disappeared" | Expected — relaunch the bridge within 30 s and the session resumes. Intentional dev rehearsal of the reconnect path. |

---

## Coverage notes

These tests exercise the Baudrun-side code path end-to-end: frontend
modal → Wails binding → Go send path → serial port write → pty bridge
→ receiver. They do NOT exercise:

- Real UART framing / parity errors (ptys don't model them).
- Real DTR/RTS/Break line signaling (ptys have no control lines).
- USB device detach/reattach under OS-level load (the bridge quit is
  an approximation, not a bus-level event).
- Windows-specific COM port paths and enumeration.

For those, run the same test recipes against actual hardware: a USB
CP210x or FTDI adapter looped back with a null-modem cable to a
second USB adapter, with the receiver running on the second port's
device path.
