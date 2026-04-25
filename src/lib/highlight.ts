// Line-buffered syntax highlighter for serial terminal output.
//
// Strategy: buffer incoming text; when a chunk contains a newline, flush
// everything up to the last newline with pattern-matched ANSI color codes
// inserted. The unterminated tail (typically a prompt) is flushed un-highlighted
// after a short idle timeout so prompts still appear responsively.
//
// Rules come from one or more "highlight packs" the user has enabled in
// Settings → Advanced. Each pack is a JSON document; the bundled set ships
// with the binary (Baudrun default + cisco-ios + junos + aruba-cx today)
// and the user's editable pack lives at $SUPPORT_DIR/highlight-rules.json.
// `setActiveRules()` is called at app start (and whenever the user toggles
// presets) to recompile the union of all enabled packs into the regex
// engine below.
//
// Rules are tried in order; the first match wins for any character range.
// Regions already inside an existing ANSI escape sequence are left alone so
// we don't interfere with device-supplied colors.

import type { HighlightRule } from "./api";

const RESET = "\x1b[0m";
const ANSI_BY_COLOR: Record<string, string> = {
  red: "\x1b[31m",
  green: "\x1b[32m",
  yellow: "\x1b[33m",
  blue: "\x1b[34m",
  magenta: "\x1b[35m",
  cyan: "\x1b[36m",
  dim: "\x1b[90m",
};

interface CompiledRule {
  re: RegExp;
  open: string;
  close: string;
  /** Pattern source — used for the one-time disable warning. */
  source: string;
  /** Set to true after this rule's per-line CPU budget overruns
   * once. ReDoS-prone patterns get permanently skipped for the rest
   * of the session so a malicious or careless user pack can't hold
   * the renderer hostage on every subsequent line. */
  disabled: boolean;
}

let RULES: CompiledRule[] = [];

/** Per-line wall-clock budget for ALL rules combined. Exceed it and
 *  the remainder of the line is rendered uncolored — readability
 *  beats correctness when a regex is misbehaving. */
const PER_LINE_BUDGET_MS = 5;
/** Per-rule budget for a single .exec() loop on one line. A single
 *  pathological pattern can't burn the whole line budget alone. */
const PER_RULE_BUDGET_MS = 2;

/**
 * Replace the active rule set. Called by the runtime when the user
 * toggles preset packs or edits the user pack. Patterns that fail to
 * compile (bad regex syntax, unsupported features) are silently
 * dropped so a single malformed user rule doesn't take the whole
 * highlighter down — see console for the dropped pattern.
 */
export function setActiveRules(rules: HighlightRule[]): void {
  const compiled: CompiledRule[] = [];
  for (const rule of rules) {
    let flags = "g";
    if (rule.ignoreCase) flags += "i";
    let re: RegExp;
    try {
      re = new RegExp(rule.pattern, flags);
    } catch (err) {
      console.warn(
        `highlight: dropping rule with invalid pattern ${JSON.stringify(rule.pattern)}: ${err}`,
      );
      continue;
    }
    const open = ANSI_BY_COLOR[rule.color] ?? ANSI_BY_COLOR.dim;
    compiled.push({
      re,
      open,
      close: RESET,
      source: rule.pattern,
      disabled: false,
    });
  }
  RULES = compiled;
}

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
  // Wall-clock budget across the whole rule set for this line. Once
  // exceeded, the remaining rules are skipped — coloring the line
  // partially is fine; locking the renderer is not.
  const lineStart = performance.now();
  for (const rule of RULES) {
    if (rule.disabled) continue;
    if (performance.now() - lineStart > PER_LINE_BUDGET_MS) break;

    rule.re.lastIndex = 0;
    const ruleStart = performance.now();
    let m;
    while ((m = rule.re.exec(text)) !== null) {
      // Per-rule budget guard: a single pathological pattern (ReDoS
      // catastrophic backtracking — e.g. `(a+)+` on a long no-match
      // tail) can spin inside .exec() for seconds. Re-checking after
      // each successful match catches the case where exec returns at
      // all but each iteration is slow.
      if (performance.now() - ruleStart > PER_RULE_BUDGET_MS) {
        console.warn(
          `highlight: disabling slow rule (>${PER_RULE_BUDGET_MS}ms): ${JSON.stringify(rule.source)} — likely catastrophic backtrack`,
        );
        rule.disabled = true;
        break;
      }
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
    const { leading, rest } = splitRedrawPrefix(raw);
    // The redraw prefix (\r, optionally followed by ESC[K) has to
    // run BEFORE the timestamp — otherwise the timestamp lands at
    // the cursor's previous position (e.g. the tail of a paged
    // "-- More --" prompt), CR then yanks the cursor to col 0 and
    // the new content overwrites from there, leaving timestamp
    // residue visible on the right half of the line.
    out.push(leading + timestampPrefix() + rest + (isLast ? "" : "\n"));
  }
  return out.join("");
}

/// Pull cursor-home / erase-line bytes out of a line so a prefix
/// (timestamp, highlight) doesn't get written BEFORE them. We split
/// at the LAST redraw, not just the first, because the partial
/// "-- More --" prompt from a paged CLI is often still sitting in
/// the highlighter's buffer when the spacebar-reply bytes arrive —
/// they get concatenated and the `\r\x1b[K` ends up mid-"line" (no
/// `\n` between them).
///
/// Handles:
///   - bare `\r` (cursor to col 0; new content overwrites in place)
///   - `\r\x1b[K`, `\r\x1b[0K`, `\r\x1b[1K`, `\r\x1b[2K` (erase in line)
function splitRedrawPrefix(raw: string): { leading: string; rest: string } {
  let lastSplit = 0;
  let i = 0;
  while (i < raw.length) {
    if (raw[i] !== "\r") {
      i += 1;
      continue;
    }
    let end = i + 1;
    const match = raw.slice(end).match(/^\x1b\[[0-2]?K/);
    if (match) {
      end += match[0].length;
    }
    lastSplit = end;
    i = end;
  }
  return {
    leading: raw.slice(0, lastSplit),
    rest: raw.slice(lastSplit),
  };
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
    const { leading, rest } = splitRedrawPrefix(bare);
    const highlighted = highlightLine(rest);
    const prefix = timestamps ? timestampPrefix() : "";
    out.push(leading + prefix + highlighted + (cr ? "\r" : "") + (isLast ? "" : "\n"));
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
