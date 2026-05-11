//! Session-log sanitizer — wraps an `io::Write` to strip ANSI CSI /
//! OSC escape sequences and apply in-line terminal mutations (BS,
//! CR-overwrite, CR-heavy line endings) so a `.log` file matches
//! what the user sees in the terminal emulator instead of the raw
//! bytes that came over the wire.
//!
//! Implementation is a tiny line-buffered terminal model:
//!  - **BS** (`0x08`) pops the last byte of the current line — this
//!    covers both "user typed a character and hit backspace" and
//!    the `BS SP BS` "overwrite to erase" pattern many CLIs use
//!    while cycling through tab-complete suggestions (Aruba, JunOS,
//!    HP/Aruba AOS-CX).
//!  - **CR alone** clears the line buffer — terminals return the
//!    cursor to column 0 on CR and any subsequent write overwrites.
//!    The `\r\e[K` prompt-redraw combo collapses cleanly this way.
//!  - **CR + LF** (or a run of CRs followed by LF, as emitted by
//!    Cisco IOS / Cisco Small Business firmware with its signature
//!    `\r\r\n`) emits the current line + a single `\n`.
//!  - **LF alone** emits the current line + `\n`.
//!  - CSI (`ESC[…final`), OSC (`ESC]…BEL/ST`), and short two-byte
//!    escapes (`ESC 7`, `ESC c`, …) are stripped entirely.
//!  - BEL is stripped.
//!  - TAB, UTF-8, everything else passes through.
//!
//! Limitations: cursor movement escapes (`ESC[H`, `ESC[A/B/C/D`)
//! are dropped, not honored. For a fully accurate render you'd
//! need a real terminal model — see `vte` or `alacritty_terminal`
//! crates. This sanitizer is "good enough for the common session-log
//! case" and orders of magnitude simpler.

use std::io::{self, Write};

pub struct SanitizingLogWriter<W: Write> {
    inner: W,
    state: State,
    /// Bytes accumulated for the current line (not yet flushed).
    line: Vec<u8>,
    /// `true` once we've seen one or more CR bytes and are waiting
    /// to classify what follows.
    pending_cr: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Normal,
    /// Just saw ESC (0x1B); waiting to classify.
    Esc,
    /// Saw ESC `[` — inside a CSI. Consume parameter + intermediate
    /// bytes until a final byte in 0x40..=0x7E.
    Csi,
    /// Saw ESC `]` — inside an OSC. Terminated by BEL (0x07) or ST
    /// (`ESC \`).
    Osc,
    /// Inside OSC, just saw ESC — might be ST.
    OscEsc,
}

impl<W: Write> SanitizingLogWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            state: State::Normal,
            line: Vec::with_capacity(256),
            pending_cr: false,
        }
    }

    /// If a CR is pending, cursor is at column 0 — a subsequent
    /// byte means the existing line content is about to be
    /// overwritten. Clear the buffer.
    fn commit_cr_overwrite(&mut self) {
        if self.pending_cr {
            self.line.clear();
            self.pending_cr = false;
        }
    }
}

impl<W: Write> Write for SanitizingLogWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for &b in buf {
            match self.state {
                State::Normal => match b {
                    0x1B => {
                        // `\r\e[K` is the prompt-redraw combo: treat
                        // the pending CR as the intended overwrite,
                        // clear the line, then enter escape parsing.
                        self.commit_cr_overwrite();
                        self.state = State::Esc;
                    }
                    b'\r' => {
                        // Accumulate — Cisco `\r\r\n` runs are common.
                        self.pending_cr = true;
                    }
                    b'\n' => {
                        // LF (with or without preceding CRs) finishes
                        // the line. Emit buffered content + a single
                        // LF.
                        self.inner.write_all(&self.line)?;
                        self.inner.write_all(b"\n")?;
                        self.line.clear();
                        self.pending_cr = false;
                    }
                    0x08 => {
                        // Backspace: pop the last byte of the line.
                        // At column 0 (pending_cr) the real terminal
                        // no-ops, so do the same here.
                        if !self.pending_cr {
                            self.line.pop();
                        }
                    }
                    0x07 => {
                        // BEL has no place in a text log. Don't let
                        // it commit a pending CR either.
                    }
                    _ => {
                        self.commit_cr_overwrite();
                        self.line.push(b);
                    }
                },
                State::Esc => match b {
                    b'[' => self.state = State::Csi,
                    b']' => self.state = State::Osc,
                    // Two-byte escapes (ESC 7, ESC c, ESC =, …) —
                    // swallow and return to Normal.
                    _ => self.state = State::Normal,
                },
                State::Csi => {
                    // Consume parameter (0x30..=0x3F) + intermediate
                    // (0x20..=0x2F) bytes until the final byte
                    // (0x40..=0x7E).
                    if (0x40..=0x7E).contains(&b) {
                        self.state = State::Normal;
                    }
                }
                State::Osc => match b {
                    0x07 => self.state = State::Normal, // BEL terminator
                    0x1B => self.state = State::OscEsc,
                    _ => {}
                },
                State::OscEsc => {
                    // `ESC \` (ST) ends the OSC; anything else is a
                    // malformed sequence — return to Osc consumption.
                    self.state = if b == b'\\' {
                        State::Normal
                    } else {
                        State::Osc
                    };
                }
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // Emit whatever's in the buffer (without adding a trailing
        // LF — the caller didn't ask for one). Subsequent writes
        // just extend a fresh buffer; output stream stays coherent.
        if !self.line.is_empty() {
            self.inner.write_all(&self.line)?;
            self.line.clear();
        }
        self.pending_cr = false;
        self.inner.flush()
    }
}

impl<W: Write> Drop for SanitizingLogWriter<W> {
    fn drop(&mut self) {
        let _ = Write::flush(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sanitize(bytes: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut w = SanitizingLogWriter::new(&mut buf);
            w.write_all(bytes).unwrap();
            w.flush().unwrap();
        }
        buf
    }

    #[test]
    fn strips_csi_sequences() {
        let input = b"\x1b[KHello\x1b[0m World\x1b[2J";
        assert_eq!(sanitize(input), b"Hello World");
    }

    #[test]
    fn collapses_cisco_line_endings() {
        let input = b"foo\r\r\nbar\r\r\nbaz";
        assert_eq!(sanitize(input), b"foo\nbar\nbaz");
    }

    #[test]
    fn cr_esc_k_is_prompt_redraw() {
        // `before \r \r \e[K after` — classic CLI prompt redraw.
        let input = b"before\r\r\x1b[Kafter";
        assert_eq!(sanitize(input), b"after");
    }

    #[test]
    fn normal_crlf_pairs_collapse_to_lf() {
        let input = b"a\r\nb\r\nc";
        assert_eq!(sanitize(input), b"a\nb\nc");
    }

    #[test]
    fn lone_cr_overwrites_line() {
        // Progress-bar style: each write-over collapses via CR.
        let input = b"  0%\r 50%\r100%\n";
        assert_eq!(sanitize(input), b"100%\n");
    }

    #[test]
    fn backspace_pops_previous_byte() {
        let input = b"ab\x08c";
        assert_eq!(sanitize(input), b"ac");
    }

    #[test]
    fn backspace_space_backspace_erase() {
        // The `BS SP BS` pattern terminals use to visually erase a
        // single glyph. Here we erase "b" from "ab" and get "a".
        let input = b"ab\x08 \x08";
        assert_eq!(sanitize(input), b"a");
    }

    #[test]
    fn aruba_tab_cycle_ends_on_final_suggestion() {
        // Real byte pattern captured from an Aruba AOS-CX session —
        // user typed "run", cycled tab-complete through "route",
        // "for==" back to "route".
        let input = b"sho ip run\x08 \x08\x08 \x08\x08 \x08route\x08 \x08\x08 \x08\x08 \x08\x08 \x08\x08 \x08for==\x08 \x08\x08 \x08\x08 \x08\x08 \x08\x08 \x08route";
        assert_eq!(sanitize(input), b"sho ip route");
    }

    #[test]
    fn bel_is_stripped() {
        let input = b"ding\x07 dong";
        assert_eq!(sanitize(input), b"ding dong");
    }

    #[test]
    fn osc_is_stripped_through_bel_terminator() {
        let input = b"hello\x1b]0;my title\x07there";
        assert_eq!(sanitize(input), b"hellothere");
    }

    #[test]
    fn osc_is_stripped_through_st_terminator() {
        let input = b"hello\x1b]0;my title\x1b\\there";
        assert_eq!(sanitize(input), b"hellothere");
    }

    #[test]
    fn two_byte_esc_sequences_are_stripped() {
        let input = b"foo\x1b7bar";
        assert_eq!(sanitize(input), b"foobar");
    }

    #[test]
    fn preserves_utf8_and_tab() {
        let input = "héllo\tworld\n".as_bytes();
        assert_eq!(sanitize(input), input);
    }

    #[test]
    fn inline_color_escape_does_not_clear_line() {
        // ESC without a preceding CR must not drop prior content.
        let input = b"prompt> \x1b[32mok\x1b[0m\n";
        assert_eq!(sanitize(input), b"prompt> ok\n");
    }
}
