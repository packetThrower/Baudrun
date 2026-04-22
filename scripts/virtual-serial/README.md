# virtual-serial

A baud-throttled virtual serial pair for dev testing Baudrun without
physical hardware. Creates two pty endpoints wired together, enforcing
realistic inter-byte timing so code paths that care about actual
baud-rate pacing (paste safety, file-transfer progress, slow-link
behaviour) can be exercised end-to-end.

For a ready-made playbook covering **Send Hex…** and **Send File…**
with prereqs, step-by-step tests, and verification commands, see
[`TESTING.md`](TESTING.md).

Unix only (macOS and Linux). Windows users: pair two virtual COM ports
via [com0com](https://sourceforge.net/projects/com0com/) and talk to
real hardware for the baud-sensitive tests — `pty` doesn't exist on
Windows.

## Usage

From the repo root:

```sh
go run ./scripts/virtual-serial -baud 9600
```

Typical output:

```
Endpoint A: /dev/ttys007
Endpoint B: /dev/ttys008
Throttle:   9600 baud, 10 bits/byte → 1.041ms per byte
Ctrl+C to quit.
```

Point Baudrun at `Endpoint A`, run any test tool against `Endpoint B`
(or the other way around). Traffic in both directions is throttled.

### Flags

| Flag | Default | What it does |
|---|---|---|
| `-baud` | `9600` | Target baud rate. Byte pacing is `bits / baud` seconds each. |
| `-bits` | `10` | Bits per byte including start + parity + stop (8N1 = 10, 8N2 = 11). |
| `-link-a` | *(none)* | Create a stable symlink pointing at endpoint A (e.g. `-link-a /tmp/baudrun-a`). Convenient so profiles can hardcode a path. |
| `-link-b` | *(none)* | Same, for endpoint B. |

## Test recipes

### Hex send

Verify every byte Baudrun sends via **Send Hex…** arrives at the other
end byte-for-byte, at the expected rate:

```sh
# Terminal 1
go run ./scripts/virtual-serial -baud 9600 -link-a /tmp/baudrun-a -link-b /tmp/baudrun-b

# Terminal 2 — hex-dump anything that comes out of endpoint B
xxd < /tmp/baudrun-b
```

In Baudrun: create a profile pointing at `/dev/ttys007` (or the symlink
`/tmp/baudrun-a`), connect, open **Send Hex…**, send a payload, watch
it appear in `xxd`. At 9600 baud a 16-byte paste should take ~16ms to
complete — visibly instantaneous but the timing is real.

### YMODEM file transfer

```sh
# Terminal 1 — throttle at a realistic serial-console rate
go run ./scripts/virtual-serial -baud 115200 -link-a /tmp/baudrun-a -link-b /tmp/baudrun-b

# Terminal 2 — receive into the current directory
rb < /tmp/baudrun-b > /tmp/baudrun-b
```

In Baudrun, connect to `/tmp/baudrun-a` and use **Send File…** with
YMODEM selected. When the transfer finishes, `diff` the sent source
file against the received copy — they should be byte-identical.

For XMODEM variants, swap `rb` for `rx` (XMODEM checksum), `rx -c`
(XMODEM-CRC), or `rx -k` (XMODEM-1K).

### Paste safety under realistic pacing

```sh
go run ./scripts/virtual-serial -baud 9600
cat < /dev/ttys008   # or whatever endpoint B printed
```

Copy a multi-line block, paste into a connected Baudrun session, and
observe that the slow-paste delay combined with the pty throttle
produces actual serial-like timing rather than instantaneous
instantaneous delivery. The multi-line confirmation dialog should fire;
after confirm, `cat` receives bytes at the paced rate.

### Auto-reconnect rehearsal

Kill the `virtual-serial` process while Baudrun has a session open.
Both pty paths disappear; Baudrun's Go-side read pump exits with an
error; if the profile has `autoReconnect: true` (the default) the
reconnect loop starts polling. Relaunch `virtual-serial` with the same
`-link-a` / `-link-b` paths within 30 seconds and Baudrun will reopen
the session transparently.

## Accuracy caveats

- `time.Sleep` on stock OS schedulers is accurate to ~100µs on macOS
  and ~1ms on Linux without PREEMPT_RT. At 9600 baud (~1ms per byte)
  the pacing is solid; at 115200 (~87µs per byte) it's a coarse
  approximation. Fine for dev-loop testing, not fine for timing-
  critical protocol conformance work.
- Ptys don't model framing errors, parity errors, control lines
  (DTR/RTS), or Break signaling. For those you still need real
  hardware.
- Both directions share one process; under heavy concurrent traffic
  Go's goroutine scheduler may add a handful of microseconds of
  skew. Irrelevant for anything but exotic latency measurement.
