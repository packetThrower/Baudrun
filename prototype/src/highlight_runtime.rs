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

use regex::{Regex, RegexBuilder};

use crate::data::highlight::HighlightRule;

/// SGR reset — closes every colour run we open.
const ANSI_RESET: &str = "\x1b[0m";

/// Map a rule's named colour to its SGR open sequence. Unknown
/// values fall back to `dim` (90), matching the Tauri reader.
fn ansi_open(color: &str) -> &'static str {
    match color {
        "red" => "\x1b[31m",
        "green" => "\x1b[32m",
        "yellow" => "\x1b[33m",
        "blue" => "\x1b[34m",
        "magenta" => "\x1b[35m",
        "cyan" => "\x1b[36m",
        "dim" | _ => "\x1b[90m",
    }
}

#[derive(Debug)]
struct CompiledRule {
    re: Regex,
    open: &'static str,
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
            match RegexBuilder::new(&rule.pattern)
                .case_insensitive(rule.ignore_case)
                .build()
            {
                Ok(re) => compiled.push(CompiledRule {
                    re,
                    open: ansi_open(&rule.color),
                }),
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
        for rule in &self.rules {
            for m in rule.re.find_iter(line) {
                let (start, end) = (m.start(), m.end());
                if end == start {
                    continue;
                }
                let overlaps = matches
                    .iter()
                    .any(|(s, e, _)| !(end <= *s || start >= *e));
                if !overlaps {
                    matches.push((start, end, rule.open));
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

/// Line-buffered wrapper around `HighlightEngine` for streaming
/// terminal bytes. Bytes accumulate in `pending`; complete lines
/// (delimited by `\n`) are emitted coloured; the tail past the
/// last newline stays pending until either the next chunk
/// completes it or `flush_partial` is called.
pub struct HighlightBuffer {
    engine: HighlightEngine,
    /// Bytes since the last newline (or last partial flush).
    pending: Vec<u8>,
}

impl HighlightBuffer {
    pub fn new(engine: HighlightEngine) -> Self {
        Self {
            engine,
            pending: Vec::new(),
        }
    }

    /// Append `bytes`, emit every complete line coloured, keep the
    /// trailing partial in the buffer. The returned `Vec` is what
    /// downstream stages (timestamps, terminal feed) should treat
    /// as the next chunk.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.pending.extend_from_slice(bytes);
        let mut out = Vec::with_capacity(self.pending.len());
        // Drain whole lines from the front of `pending`.
        loop {
            let Some(nl_idx) = self.pending.iter().position(|&b| b == b'\n')
            else {
                break;
            };
            // `..=nl_idx` includes the newline so the cursor moves
            // onto its own row downstream (terminal still sees the
            // \n it expects). The optional `\r` immediately before
            // it gets included naturally too.
            let line: Vec<u8> = self.pending.drain(..=nl_idx).collect();
            // Trim trailing \n and optional \r before regex match;
            // the line content the rules are written against
            // doesn't include the newline.
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
            let highlighted = self.engine.apply(&body_str);
            out.extend_from_slice(highlighted.as_bytes());
            // Re-append whatever line ending the device sent.
            out.extend_from_slice(&line[body_end..]);
        }
        out
    }

    /// Drop the partial-line buffer back through the highlighter
    /// and return the result. Used on the idle-flush timer so
    /// prompts (no trailing newline) still appear without forcing
    /// the user to type a newline first. The match result is
    /// best-effort — many rules anchored to `^…$` won't match a
    /// prompt, but substring rules (`Router#`, IP regex) still
    /// catch fine.
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
