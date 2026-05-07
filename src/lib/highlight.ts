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

function plainLines(
  block: string,
  timestamps: boolean,
  prefixSource: () => string = timestampPrefix,
): string {
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
    out.push(leading + prefixSource() + rest + (isLast ? "" : "\n"));
  }
  return out.join("");
}

/// Pull cursor-home / erase-line bytes out of a line so a prefix
/// (timestamp, highlight) doesn't get written BEFORE them. We split
/// at the LAST redraw, not just the first, because the partial
/// "-- More --" prompt from a paged CLI is often still sitting in
/// the highlighter's buffer when the spacebar-reply bytes arrive —
/// they get concatenated and the redraw ends up mid-"line" (no `\n`
/// between them).
///
/// Handles:
///   - bare `\r` (cursor to col 0; new content overwrites in place)
///   - `\r\x1b[K`, `\r\x1b[0K`, `\r\x1b[1K`, `\r\x1b[2K` (erase in line)
///   - bare `\b+` (cursor back, no erase — rare)
///   - `\b+ +\b+` (Cisco IOS pager pattern: backspace over the
///     `--More--` prompt, write spaces, backspace again to land the
///     cursor at the start). Without this branch, splitRedrawPrefix
///     missed the erase entirely on Cisco's pager and our timestamp
///     prefix landed at col 8 (end of `--More--`); the device's
///     subsequent backspace-spaces-backspace then partially overwrote
///     it, leaving the visible `--More-- [09:5...` artifact.
function splitRedrawPrefix(raw: string): { leading: string; rest: string } {
  let lastSplit = 0;
  let i = 0;
  while (i < raw.length) {
    const ch = raw[i];
    if (ch === "\r") {
      let end = i + 1;
      const match = raw.slice(end).match(/^\x1b\[[0-2]?K/);
      if (match) {
        end += match[0].length;
      }
      lastSplit = end;
      i = end;
      continue;
    }
    if (ch === "\b") {
      // Consume a run of backspaces. Look ahead for the spaces +
      // backspaces tail — present in the Cisco pattern, absent for
      // a plain "move cursor left" sequence. Either form is treated
      // as a redraw (the cursor is now somewhere left of where it
      // started; whatever follows is the actual content we want to
      // see, prefixed by our timestamp).
      let end = i;
      while (end < raw.length && raw[end] === "\b") end++;
      if (end < raw.length && raw[end] === " ") {
        let probe = end;
        while (probe < raw.length && raw[probe] === " ") probe++;
        if (probe < raw.length && raw[probe] === "\b") {
          while (probe < raw.length && raw[probe] === "\b") probe++;
          end = probe;
        }
      }
      lastSplit = end;
      i = end;
      continue;
    }
    i += 1;
  }
  return {
    leading: raw.slice(0, lastSplit),
    rest: raw.slice(lastSplit),
  };
}

/// Format a Date as the bracketed dim-grey timestamp prefix used in
/// front of every timestamped line. Pulled out so replay() can
/// pre-compute a per-line prefix from a stored arrival time without
/// going through `new Date()` (which would emit "now" instead of
/// the original arrival time, drifting every replay forward in time).
function formatTimestamp(d: Date): string {
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  const ms = String(d.getMilliseconds()).padStart(3, "0");
  return `\x1b[90m[${hh}:${mm}:${ss}.${ms}]\x1b[0m `;
}

/// Default prefix source — current wall-clock time, formatted as the
/// bracketed dim-grey prefix. This is what the live feed path wants
/// (lines arrive ~now, timestamps reflect that). The replay path
/// passes a custom source backed by stored arrival times.
function timestampPrefix(): string {
  return formatTimestamp(new Date());
}

export function highlightLines(
  block: string,
  timestamps = false,
  prefixSource: () => string = timestampPrefix,
): string {
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
    const prefix = timestamps ? prefixSource() : "";
    out.push(leading + prefix + highlighted + (cr ? "\r" : "") + (isLast ? "" : "\n"));
  }
  return out.join("");
}

export class TerminalHighlighter {
  private buffer = "";
  private flushTimer: number | null = null;
  private flushMs = 60;

  // Raw mirror of every byte we've seen, with device-emitted SGR
  // escapes preserved but no highlighter additions. Used by replay()
  // to re-render scrollback when the user toggles highlight on/off,
  // toggles a pack in Settings, or flips timestamps. Stored as
  // per-line entries plus an in-progress tail because replay needs
  // each line's original arrival time — replaying with `new Date()`
  // would re-stamp every historical line as "now", which is the
  // bug live testing turned up. Capped by line count so the bookkeeping
  // arrays stay bounded over long-running sessions.
  private rawLines: { text: string; ts: number }[] = [];
  private rawTail = "";
  private rawTailTs: number | null = null;
  private readonly maxRawLines: number;

  constructor(
    private writeCb: (text: string) => void,
    maxRawLines = 25_000,
  ) {
    this.maxRawLines = maxRawLines;
  }

  feed(text: string, highlight: boolean, timestamps = false) {
    // Mirror to rawLines / rawTail regardless of highlight/timestamps
    // so a later toggle has the raw input to replay through the new
    // pipeline. Each completed line carries the arrival time of the
    // newline that closed it; the tail (still-buffering line) keeps
    // its own first-seen ts so it stamps consistently when it
    // eventually completes via a future feed.
    const now = Date.now();
    if (text.length > 0) {
      let combined = this.rawTail + text;
      let cursor = 0;
      let nlIdx;
      const startTs = this.rawTailTs ?? now;
      while ((nlIdx = combined.indexOf("\n", cursor)) !== -1) {
        const line = combined.slice(cursor, nlIdx + 1);
        // Lines emitted in the same feed share `now`. The very first
        // emitted line uses startTs (which respects an already-pending
        // tail's first-seen time). After we cross the tail boundary,
        // subsequent lines were definitely seen in this feed, so they
        // use `now` directly.
        const ts = cursor === 0 ? startTs : now;
        this.rawLines.push({ text: line, ts });
        cursor = nlIdx + 1;
      }
      if (cursor < combined.length) {
        // Remaining bytes form the next tail. If we were already
        // building a tail and never crossed a newline this feed, keep
        // the original first-seen ts; otherwise this is a fresh tail
        // started in this feed.
        this.rawTail = combined.slice(cursor);
        this.rawTailTs = cursor === 0 ? startTs : now;
      } else {
        this.rawTail = "";
        this.rawTailTs = null;
      }
      while (this.rawLines.length > this.maxRawLines) this.rawLines.shift();
    }

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

  /** Re-render the entire raw history through the current highlight
   *  pipeline. Returns the rendered text — caller is responsible for
   *  clearing the terminal and writing it (we don't own the writeCb's
   *  destination). Returns `""` when there's no raw history yet, which
   *  signals "skip replay, leave xterm as-is" — the caller should not
   *  clear in that case. This protects migrated sessions whose
   *  scrollback came from a SerializeAddon snapshot rather than feed():
   *  we have no raw text to replay, but the migrated scrollback is
   *  still legitimate and shouldn't get wiped on a toggle.
   *
   *  When timestamps are enabled, prefixes are produced from each
   *  line's stored arrival time (`ts`) rather than `new Date()`, so
   *  replaying an hour-old session shows the original arrival times
   *  instead of stamping every line "now". The tail (in-progress
   *  partial line) uses `rawTailTs` if it has one, falling back to
   *  the most recent completed line's ts so a user who replays
   *  before the tail completes still sees a sensible prefix. */
  replay(highlight: boolean, timestamps: boolean): string {
    if (!this.hasHistory()) return "";
    // Concatenate completed lines + tail. The parallel `lineTs` array
    // carries the arrival time for each line in the concatenated
    // block, in order; highlightLines / plainLines walk it via the
    // closure below as they emit each line.
    let raw = "";
    const lineTs: number[] = [];
    for (const { text, ts } of this.rawLines) {
      raw += text;
      lineTs.push(ts);
    }
    if (this.rawTail.length > 0) {
      raw += this.rawTail;
      const fallback =
        this.rawLines.length > 0 ? this.rawLines[this.rawLines.length - 1].ts : Date.now();
      lineTs.push(this.rawTailTs ?? fallback);
    }

    if (!highlight && !timestamps) return raw;

    let lineIdx = 0;
    const prefixSource = (): string => {
      const ts = lineTs[lineIdx++] ?? Date.now();
      return formatTimestamp(new Date(ts));
    };

    return highlight
      ? highlightLines(raw, timestamps, prefixSource)
      : plainLines(raw, timestamps, prefixSource);
  }

  /** Whether replay() would produce content. Lets callers decide
   *  whether to clear the terminal before writing the replay. */
  hasHistory(): boolean {
    return this.rawLines.length > 0 || this.rawTail.length > 0;
  }

  /** Drop the raw history. Wired to the session-clear path in
   *  Terminal.svelte so a subsequent highlight toggle doesn't
   *  resurrect content the user explicitly cleared. */
  clearHistory(): void {
    this.rawLines = [];
    this.rawTail = "";
    this.rawTailTs = null;
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
