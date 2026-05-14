# Highlight pack — syslog / journald

Practical starter for generic syslog, journald, and systemd output.
Rules cover:

- RFC 5424 severity keywords (`emerg` / `crit` / `err` red, `notice`
  yellow, `debug` dim)
- `[ OK ]` / `[FAILED]` / `[WARN]` markers from sysv-style boot scripts
  and many init systems
- sshd events — "Accepted publickey" green, "Failed password" /
  "Invalid user" red
- systemd unit lifecycle ("Started …" / "Reached target" green,
  "Stopped" / "Failed to start" red)
- Daemon tags (`sshd:`, `kernel:`, `systemd:`, `cron:`, …) blue
- PIDs `[1234]` dim
- RFC 3164 priority prefix `<NN>` dim

Import via **Settings → Syntax Highlighting → Import pack…**, or drop
the file directly into
`$SUPPORT_DIR/highlight/syslog-example.json`. Stack with the
**Baudrun default** pack to also pick up IPs, MACs, and the generic
status keywords.

The [**rule playground**](../playground.html) is the easiest way to
tweak — drop a real `journalctl` or `/var/log/syslog` capture into
the right pane and watch the colors apply live as you edit the JSON.

## Download

<a href="syslog.example.json" download="syslog.example.json">
Download <code>syslog.example.json</code>
</a>

Or copy from the block below.

## Pack contents

```json
--8<-- "examples/syslog.example.json"
```
