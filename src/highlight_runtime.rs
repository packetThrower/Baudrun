//! Runtime side of the highlight system. The data layer
//! ([`crate::data::highlight`]) holds the parsed rule packs; this
//! module compiles them and applies them to incoming terminal
//! bytes.
//!
//! Strategy mirrors the Tauri highlighter (see
//! `src/lib/highlight.ts`): line-buffer incoming bytes; when a
//! newline arrives, run each compiled rule against the line, wrap
//! matching ranges in ANSI colour escapes, and emit. Partial
//! tails (typical for prompts with no trailing newline) are
//! flushed raw on an idle timer so the prompt still appears
//! responsively — at the cost of those bytes not matching any
//! highlight rules.
//!
//! ReDoS protection: unlike the Tauri version we don't need
//! per-rule wall-clock budgets because Rust's `regex` crate is
//! RE2-based and runs in linear time relative to input — there is
//! no catastrophic backtracking to defend against.

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use regex::{Regex, RegexBuilder};

use crate::data::highlight::HighlightRule;

/// SGR reset — closes every colour run we open.
const ANSI_RESET: &str = "\x1b[0m";

/// Detect the `\b(?:foo|bar|baz)\b` (or `\b(foo|bar|baz)\b`)
/// shape and return the keyword list. Returns `None` if any
/// alternative contains regex metacharacters or the surrounding
/// shape doesn't match — caller falls back to the regex engine.
///
/// Many bundled rules use exactly this shape for "match any of
/// these keywords" — e.g. baudrun-default's state-good rule
/// (`\b(?:up|online|active|established|...)\b`), cisco-ios's
/// STP roles, OSPF / BGP state lists. Routing them through an
/// Aho–Corasick automaton instead of the regex DFA is a big win
/// when the keyword set is large.
fn extract_word_alternation(pattern: &str) -> Option<Vec<String>> {
    let inner = pattern.strip_prefix("\\b")?.strip_suffix("\\b")?;
    let alternatives = inner
        .strip_prefix("(?:")
        .or_else(|| inner.strip_prefix('('))
        .and_then(|s| s.strip_suffix(')'))?;
    let mut words = Vec::new();
    for alt in alternatives.split('|') {
        if alt.is_empty() {
            return None;
        }
        // Reject anything that looks like regex metacharacters
        // — the alternation has to be plain literals for the
        // Aho–Corasick path to be safe. `-` is allowed since it
        // shows up in interface keywords (`err-disabled`,
        // `Port-channel`, etc.) and isn't a metachar outside of
        // character classes.
        if alt.bytes().any(|b| {
            matches!(
                b,
                b'\\' | b'|' | b'(' | b')' | b'[' | b']' | b'{' | b'}'
                    | b'?' | b'*' | b'+' | b'.' | b'^' | b'$'
            )
        }) {
            return None;
        }
        words.push(alt.to_string());
    }
    if words.is_empty() {
        return None;
    }
    Some(words)
}

/// Map a rule's named colour to its SGR open sequence. Standard
/// 8-colour ANSI plus the bright variants (SGR 90–97) for a
/// 14-colour palette. `dim` is an alias for bright-black (the
/// Tauri build calls it that). Unknown / future names fall
/// back to dim so a pack with a colour we don't recognise
/// still renders something rather than nothing.
fn ansi_open(color: &str) -> &'static str {
    match color {
        // Standard 8 (foreground 30–37 — black is omitted; nobody
        // wants invisible-on-default-bg highlights).
        "red" => "\x1b[31m",
        "green" => "\x1b[32m",
        "yellow" => "\x1b[33m",
        "blue" => "\x1b[34m",
        "magenta" => "\x1b[35m",
        "cyan" => "\x1b[36m",
        "white" => "\x1b[37m",
        // Bright variants (foreground 90–97). `dim` is the
        // Tauri-shipped name for bright-black; both spellings map
        // to the same code.
        "dim" | "bright_black" => "\x1b[90m",
        "bright_red" => "\x1b[91m",
        "bright_green" => "\x1b[92m",
        "bright_yellow" => "\x1b[93m",
        "bright_blue" => "\x1b[94m",
        "bright_magenta" => "\x1b[95m",
        "bright_cyan" => "\x1b[96m",
        "bright_white" => "\x1b[97m",
        _ => "\x1b[90m",
    }
}

/// Compiled rule. Two flavours:
///
///  * `Literals` — the source pattern was a `\b(?:foo|bar|baz)\b`
///    style alternation of plain literals, decomposed into a
///    single Aho–Corasick automaton over the keyword set. Word
///    boundaries are checked by the caller because Aho–Corasick
///    has no notion of `\b`. ~10–20× faster than running the
///    equivalent regex on long lines.
///  * `Regex` — anything else: real regex with metacharacters,
///    capture groups, anchors, etc. Still fast (the `regex` crate
///    is RE2-based), just slower than the literal-set path.
#[derive(Debug)]
enum CompiledRule {
    Literals { ac: AhoCorasick, open: &'static str },
    Regex { re: Regex, open: &'static str },
}

impl CompiledRule {
    /// Append `(start, end)` byte spans matched by this rule to
    /// `out`. Both flavours emit non-overlapping leftmost-first
    /// matches; word-boundary trimming for the literals path
    /// is applied here so the caller doesn't have to know which
    /// flavour it's looking at.
    fn find_into(&self, line: &str, out: &mut Vec<(usize, usize, &'static str)>) {
        match self {
            CompiledRule::Regex { re, open } => {
                for m in re.find_iter(line) {
                    if m.start() != m.end() {
                        out.push((m.start(), m.end(), open));
                    }
                }
            }
            CompiledRule::Literals { ac, open } => {
                let bytes = line.as_bytes();
                for m in ac.find_iter(line) {
                    let (s, e) = (m.start(), m.end());
                    let pre_ok = s == 0 || !is_word_byte(bytes[s - 1]);
                    let post_ok = e == bytes.len() || !is_word_byte(bytes[e]);
                    if pre_ok && post_ok && s != e {
                        out.push((s, e, open));
                    }
                }
            }
        }
    }
}

#[inline]
fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Compiled set of highlight rules + the per-line applier.
#[derive(Debug, Default)]
pub struct HighlightEngine {
    rules: Vec<CompiledRule>,
}

impl HighlightEngine {
    /// Compile a flat list of rules. Patterns that fail to compile
    /// (bad regex syntax, unsupported features) are silently
    /// dropped — same behaviour Tauri's `setActiveRules` has —
    /// so a single malformed user rule doesn't disable the rest.
    /// The order of `rules` determines first-match-wins precedence
    /// for overlapping ranges on a line.
    pub fn from_rules(rules: &[HighlightRule]) -> Self {
        let mut compiled = Vec::with_capacity(rules.len());
        for rule in rules {
            let open = ansi_open(&rule.color);
            // Try the fast literal-alternation path first. Many
            // bundled rules (`up|down|...`, `tagged|untagged|...`,
            // STP / OSPF / BGP states) match this shape and run
            // an order of magnitude faster through Aho–Corasick.
            if let Some(words) = extract_word_alternation(&rule.pattern) {
                match AhoCorasickBuilder::new()
                    .ascii_case_insensitive(rule.ignore_case)
                    .match_kind(MatchKind::LeftmostFirst)
                    .build(&words)
                {
                    Ok(ac) => {
                        compiled.push(CompiledRule::Literals { ac, open });
                        continue;
                    }
                    Err(err) => {
                        log::warn!(
                            "highlight: aho-corasick build failed for {:?}, \
                             falling back to regex: {err}",
                            rule.pattern
                        );
                    }
                }
            }
            // Generic regex path.
            match RegexBuilder::new(&rule.pattern)
                .case_insensitive(rule.ignore_case)
                .build()
            {
                Ok(re) => compiled.push(CompiledRule::Regex { re, open }),
                Err(err) => {
                    log::warn!(
                        "highlight: dropping rule with invalid pattern {:?}: {err}",
                        rule.pattern
                    );
                }
            }
        }
        Self { rules: compiled }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Run each compiled rule against `line` and return the same
    /// text with ANSI colour escapes wrapped around matches.
    /// `line` should NOT include a trailing newline — the buffer
    /// caller strips and re-appends those.
    ///
    /// First-match-wins for overlapping ranges (`Cisco IOS` then
    /// `IOS` would yield one Cisco-coloured run, not two stacked).
    /// Matches are byte-indexed; non-UTF-8 input goes through
    /// `from_utf8_lossy` upstream so we always have a `&str`.
    pub fn apply(&self, line: &str) -> String {
        if self.rules.is_empty() || line.is_empty() {
            return line.to_string();
        }
        // Skip lines that contain ANSI CSI escapes — the device
        // already coloured those, and our naive splice would land
        // colour resets in the middle of a vendor sequence and
        // visually break the screen. Fancier handling would
        // segment around escapes (Tauri does this); for now we
        // pessimistically skip mixed-content lines.
        if line.contains('\x1b') {
            return line.to_string();
        }

        let mut matches: Vec<(usize, usize, &'static str)> = Vec::new();
        let mut staging: Vec<(usize, usize, &'static str)> = Vec::new();
        for rule in &self.rules {
            staging.clear();
            rule.find_into(line, &mut staging);
            for &(start, end, open) in &staging {
                let overlaps = matches
                    .iter()
                    .any(|(s, e, _)| !(end <= *s || start >= *e));
                if !overlaps {
                    matches.push((start, end, open));
                }
            }
        }
        if matches.is_empty() {
            return line.to_string();
        }
        matches.sort_by_key(|m| m.0);
        let mut out = String::with_capacity(line.len() + matches.len() * 16);
        let mut pos = 0;
        for (start, end, open) in matches {
            out.push_str(&line[pos..start]);
            out.push_str(open);
            out.push_str(&line[start..end]);
            out.push_str(ANSI_RESET);
            pos = end;
        }
        out.push_str(&line[pos..]);
        out
    }
}

/// Line-buffered wrapper around `HighlightEngine`. Bytes
/// accumulate in `pending`; complete lines (delimited by `\n`)
/// get drained, run through the engine, and emitted coloured.
/// The trailing tail (a chunk that hasn't seen its newline yet)
/// stays in the buffer until either the next chunk completes it
/// or `flush_partial` is called from the caller's idle timer.
///
/// Buffering matters because serial bytes commonly arrive
/// one-at-a-time when the device's UART is sending interactively
/// — a stateless approach would never see enough characters at
/// once for a regex like `Router#` or an IP address to match.
/// The caller is responsible for keeping the idle-flush delay
/// short enough that typing latency stays imperceptible
/// (~30ms is fine; 100ms is visibly laggy).
pub struct HighlightBuffer {
    engine: HighlightEngine,
    /// Bytes since the last `\n` we drained. May contain CR /
    /// other control bytes; the engine apply step segments by
    /// `\n` only.
    pending: Vec<u8>,
}

impl HighlightBuffer {
    pub fn new(engine: HighlightEngine) -> Self {
        Self {
            engine,
            pending: Vec::new(),
        }
    }

    /// Append `bytes`; emit every complete (`\n`-terminated) line
    /// through the engine; keep the trailing partial in
    /// `pending`. The returned `Vec` is the next chunk for the
    /// downstream pipeline.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.pending.extend_from_slice(bytes);
        let mut out = Vec::with_capacity(self.pending.len());
        while let Some(nl) = self.pending.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=nl).collect();
            let body_end = line
                .len()
                .saturating_sub(1) // drop \n
                .saturating_sub(if line.len() >= 2 && line[line.len() - 2] == b'\r' {
                    1
                } else {
                    0
                });
            let body = &line[..body_end];
            let body_str = std::str::from_utf8(body)
                .map(std::borrow::Cow::Borrowed)
                .unwrap_or_else(|_| String::from_utf8_lossy(body));
            out.extend_from_slice(self.engine.apply(&body_str).as_bytes());
            // Re-append the line ending byte(s) the device sent.
            out.extend_from_slice(&line[body_end..]);
        }
        out
    }

    /// Drop the partial-line buffer through the engine and
    /// return the result. Called by the caller's idle-flush
    /// timer so prompts (no trailing newline) appear without the
    /// user having to type a newline first.
    pub fn flush_partial(&mut self) -> Vec<u8> {
        if self.pending.is_empty() {
            return Vec::new();
        }
        let line = std::mem::take(&mut self.pending);
        let body_str = std::str::from_utf8(&line)
            .map(std::borrow::Cow::Borrowed)
            .unwrap_or_else(|_| String::from_utf8_lossy(&line));
        self.engine.apply(&body_str).into_bytes()
    }
}
