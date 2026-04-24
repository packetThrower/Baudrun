// Line-buffered syntax highlighter for serial terminal output.
//
// Strategy: buffer incoming text; when a chunk contains a newline, flush
// everything up to the last newline with pattern-matched ANSI color codes
// inserted. The unterminated tail (typically a prompt) is flushed un-highlighted
// after a short idle timeout so prompts still appear responsively.
//
// Rules are tried in order; the first match wins for any character range.
// Regions already inside an existing ANSI escape sequence are left alone so
// we don't interfere with device-supplied colors.

const GREEN = "\x1b[32m";
const RED = "\x1b[31m";
const YELLOW = "\x1b[33m";
const BLUE = "\x1b[34m";
const MAGENTA = "\x1b[35m";
const CYAN = "\x1b[36m";
const DIM = "\x1b[90m";
const RESET = "\x1b[0m";

interface Rule {
  re: RegExp;
  open: string;
  close: string;
}

const RULES: Rule[] = [
  // Timestamps HH:MM:SS[.ms] — first so they don't get caught by IPv6
  { re: /\b\d{2}:\d{2}:\d{2}(?:\.\d+)?\b/g, open: DIM, close: RESET },
  // Dates YYYY-MM-DD
  { re: /\b\d{4}-\d{2}-\d{2}\b/g, open: DIM, close: RESET },

  // IPv4 + optional CIDR
  { re: /\b(?:\d{1,3}\.){3}\d{1,3}(?:\/\d{1,2})?\b/g, open: CYAN, close: RESET },

  // MAC addresses (colon/dash) — before IPv6
  {
    re: /\b(?:[0-9a-fA-F]{2}[:-]){5}[0-9a-fA-F]{2}\b/g,
    open: MAGENTA,
    close: RESET,
  },
  // MAC Cisco-dotted (aabb.ccdd.eeff)
  {
    re: /\b[0-9a-fA-F]{4}\.[0-9a-fA-F]{4}\.[0-9a-fA-F]{4}\b/g,
    open: MAGENTA,
    close: RESET,
  },

  // IPv6 — 3+ groups of hex separated by colons
  {
    re: /\b(?:[0-9a-fA-F]{1,4}:){2,7}[0-9a-fA-F]{1,4}\b/g,
    open: CYAN,
    close: RESET,
  },

  // Cisco-style full interface names
  {
    re: /\b(?:GigabitEthernet|FastEthernet|TenGigabitEthernet|TwentyFiveGigE|HundredGigE|FortyGigE|Ethernet|Serial|Loopback|Vlan|Port-channel|Tunnel|Management|Bundle-Ether|Null)\s?\d+(?:\/\d+)*(?:\.\d+)?\b/g,
    open: BLUE,
    close: RESET,
  },
  // Cisco abbreviated (Gi1/0/24, Fa0/1, Te1/1)
  {
    re: /\b(?:Gi|Fa|Te|Twe|Hun|Fo|Eth|Se|Lo|Vl|Po|Tu|Mg|Bu|Nu)\d+\/\d+(?:\/\d+)*(?:\.\d+)?\b/g,
    open: BLUE,
    close: RESET,
  },
  // Juniper (ge-0/0/1, xe-0/1/0, ae0, etc.)
  {
    re: /\b(?:ge|xe|et|fe|me|xl|em|lo|fxp|irb|lt|st|mt|ae|pp|bme|vcp|vme|vt|sp|reth)-\d+\/\d+\/\d+(?:\.\d+)?\b/g,
    open: BLUE,
    close: RESET,
  },

  // GOOD states (case-insensitive)
  {
    re: /\b(?:up|online|active|established|connected|enabled|forwarding|FULL|passed|OK|success)\b/gi,
    open: GREEN,
    close: RESET,
  },
  // BAD states
  {
    re: /\b(?:down|offline|inactive|failed|err-?disabled|error|denied|disabled|rejected|timeout|lost|critical|fatal|alert|unreachable)\b/gi,
    open: RED,
    close: RESET,
  },
  // WARN states
  {
    re: /\b(?:warning|warn|degraded|partial|init|learning|listening|blocking|pending)\b/gi,
    open: YELLOW,
    close: RESET,
  },

  // VLAN IDs
  { re: /\bVLAN\s?\d+\b/gi, open: YELLOW, close: RESET },
];

const ESC_RE = /\x1b\[[0-9;]*[A-Za-z]/g;

interface Segment {
  text: string;
  isEscape: boolean;
}

// Split a line into alternating plain-text and escape-sequence segments so we
// only apply highlighting to plain text and leave device-set colors intact.
function segmentize(line: string): Segment[] {
  const segments: Segment[] = [];
  ESC_RE.lastIndex = 0;
  let pos = 0;
  let m;
  while ((m = ESC_RE.exec(line)) !== null) {
    if (m.index > pos) {
      segments.push({ text: line.slice(pos, m.index), isEscape: false });
    }
    segments.push({ text: m[0], isEscape: true });
    pos = m.index + m[0].length;
  }
  if (pos < line.length) {
    segments.push({ text: line.slice(pos), isEscape: false });
  }
  return segments;
}

interface Match {
  start: number;
  end: number;
  open: string;
  close: string;
}

function highlightText(text: string): string {
  if (text.length === 0) return text;
  const matches: Match[] = [];
  for (const rule of RULES) {
    rule.re.lastIndex = 0;
    let m;
    while ((m = rule.re.exec(text)) !== null) {
      const start = m.index;
      const end = start + m[0].length;
      if (end === start) {
        rule.re.lastIndex = end + 1;
        continue;
      }
      const overlaps = matches.some(
        (e) => !(end <= e.start || start >= e.end),
      );
      if (!overlaps) {
        matches.push({ start, end, open: rule.open, close: rule.close });
      }
    }
  }
  if (matches.length === 0) return text;
  matches.sort((a, b) => a.start - b.start);

  let out = "";
  let pos = 0;
  for (const mt of matches) {
    out += text.slice(pos, mt.start);
    out += mt.open + text.slice(mt.start, mt.end) + mt.close;
    pos = mt.end;
  }
  out += text.slice(pos);
  return out;
}

function highlightLine(line: string): string {
  const segs = segmentize(line);
  return segs
    .map((s) => (s.isEscape ? s.text : highlightText(s.text)))
    .join("");
}

function plainLines(block: string, timestamps: boolean): string {
  if (!timestamps) return block;
  const parts = block.split("\n");
  const out: string[] = [];
  for (let i = 0; i < parts.length; i++) {
    const isLast = i === parts.length - 1;
    const raw = parts[i];
    if (raw === "" && isLast) break;
    out.push(timestampPrefix() + raw + (isLast ? "" : "\n"));
  }
  return out.join("");
}

function timestampPrefix(): string {
  const d = new Date();
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  const ms = String(d.getMilliseconds()).padStart(3, "0");
  return `\x1b[90m[${hh}:${mm}:${ss}.${ms}]\x1b[0m `;
}

export function highlightLines(block: string, timestamps = false): string {
  // block contains one or more complete lines ending in \n (with optional \r).
  // Split on \n, highlight each stripped line, rejoin preserving line endings.
  const parts = block.split("\n");
  const out: string[] = [];
  for (let i = 0; i < parts.length; i++) {
    const isLast = i === parts.length - 1;
    const raw = parts[i];
    if (raw === "" && isLast) {
      // trailing newline; nothing to append
      break;
    }
    const cr = raw.endsWith("\r");
    const bare = cr ? raw.slice(0, -1) : raw;
    const highlighted = highlightLine(bare);
    const prefix = timestamps ? timestampPrefix() : "";
    out.push(prefix + highlighted + (cr ? "\r" : "") + (isLast ? "" : "\n"));
  }
  return out.join("");
}

export class TerminalHighlighter {
  private buffer = "";
  private flushTimer: number | null = null;
  private flushMs = 60;

  constructor(private writeCb: (text: string) => void) {}

  feed(text: string, highlight: boolean, timestamps = false) {
    if (!highlight && !timestamps) {
      this.flushNow();
      this.writeCb(text);
      return;
    }
    this.buffer += text;
    const lastNL = this.buffer.lastIndexOf("\n");
    if (lastNL >= 0) {
      const complete = this.buffer.slice(0, lastNL + 1);
      this.buffer = this.buffer.slice(lastNL + 1);
      const processed = highlight
        ? highlightLines(complete, timestamps)
        : plainLines(complete, timestamps);
      this.writeCb(processed);
    }
    this.scheduleFlush();
  }

  private scheduleFlush() {
    if (this.flushTimer !== null) {
      window.clearTimeout(this.flushTimer);
      this.flushTimer = null;
    }
    if (this.buffer.length === 0) return;
    this.flushTimer = window.setTimeout(() => {
      this.flushNow();
    }, this.flushMs);
  }

  private flushNow() {
    if (this.flushTimer !== null) {
      window.clearTimeout(this.flushTimer);
      this.flushTimer = null;
    }
    if (this.buffer.length > 0) {
      this.writeCb(this.buffer);
      this.buffer = "";
    }
  }

  reset() {
    this.flushNow();
  }

  dispose() {
    this.flushNow();
  }
}
