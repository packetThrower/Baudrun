//! Session-log sanitizer — wraps an `io::Write` to strip ANSI CSI /
//! OSC escape sequences and normalize CR-heavy line endings into
//! plain LF-separated text, so a `.log` file matches what the user
//! sees in the terminal instead of the raw wire output.
//!
//! Device-specific quirks this handles:
//!   - Cisco IOS / Cisco Small Business firmware emits `\r\r\n` at
//!     every line break, which text editors render as extra blank
//!     lines.
//!   - `\rESC[K<text>` ("carriage return, clear line, overwrite")
//!     is a very common prompt-redraw sequence. The raw bytes look
//!     like `^M^M\e[Kprompt` in a .log.
//!   - Many devices send `ESC[0m` / `ESC[?25h` / etc. for color +
//!     cursor attributes that a plain-text log can't display.

use std::io::{self, Write};

/// A [`Write`] adapter that filters terminal control sequences on
/// the way through. See module docs for the specific transformations.
pub struct SanitizingLogWriter<W: Write> {
    inner: W,
    state: State,
    /// `true` when we've consumed one or more CR bytes and are
    /// waiting to decide whether to emit a newline (depends on the
    /// next byte).
    pending_cr: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Normal,
    /// Just saw ESC (0x1B); waiting to classify the sequence.
    Esc,
    /// Saw ESC `[` — inside a CSI. Consume parameter + intermediate
    /// bytes until a final byte in 0x40..=0x7E.
    Csi,
    /// Saw ESC `]` — inside an OSC. Terminated by BEL (0x07) or ST
    /// (ESC `\`).
    Osc,
    /// Inside OSC, just saw ESC — might be ST.
    OscEsc,
}

impl<W: Write> SanitizingLogWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            state: State::Normal,
            pending_cr: false,
        }
    }

    fn flush_pending_cr(&mut self, out: &mut Vec<u8>) {
        if self.pending_cr {
            out.push(b'\n');
            self.pending_cr = false;
        }
    }
}

impl<W: Write> Write for SanitizingLogWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut out = Vec::with_capacity(buf.len());
        for &b in buf {
            match self.state {
                State::Normal => match b {
                    0x1B => {
                        // If we're in the middle of a CR run and the
                        // next thing is an ESC, it's almost always
                        // part of a "carriage return + clear line +
                        // overwrite" redraw (`\r\x1b[K`). Swallow
                        // the CRs — the incoming sequence will paint
                        // the new line contents in place.
                        self.pending_cr = false;
                        self.state = State::Esc;
                    }
                    b'\r' => {
                        // Accumulate; decide on the next byte.
                        self.pending_cr = true;
                    }
                    b'\n' => {
                        // CR+LF, CR CR+LF, or plain LF — all collapse
                        // to a single newline.
                        out.push(b'\n');
                        self.pending_cr = false;
                    }
                    0x07 => {
                        // BEL has no place in a text log.
                        self.flush_pending_cr(&mut out);
                    }
                    _ => {
                        // A run of CRs NOT followed by LF / ESC / BEL
                        // is a real line break (e.g. Mac-classic line
                        // endings, or a device that emits `\r` alone
                        // to separate fields).
                        self.flush_pending_cr(&mut out);
                        out.push(b);
                    }
                },
                State::Esc => {
                    match b {
                        b'[' => self.state = State::Csi,
                        b']' => self.state = State::Osc,
                        // Two-byte escape sequences (ESC c, ESC 7,
                        // ESC 8, …). Swallow.
                        _ => self.state = State::Normal,
                    }
                }
                State::Csi => {
                    // CSI: parameter bytes (0x30..=0x3F) then
                    // intermediate (0x20..=0x2F), terminated by a
                    // final byte (0x40..=0x7E). We don't care about
                    // the distinctions — just wait for the final.
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
                    if b == b'\\' {
                        // ST (ESC \) — OSC done.
                        self.state = State::Normal;
                    } else {
                        // Stray ESC inside OSC — bail to Normal and
                        // re-process this byte as if fresh. Avoids
                        // eating real content if a device misbehaves.
                        self.state = State::Normal;
                        // Re-process by falling through? Rust doesn't
                        // support; simulate via continuing the loop.
                        // Mark it as Normal handling:
                        match b {
                            0x1B => self.state = State::Esc,
                            b'\r' => self.pending_cr = true,
                            b'\n' => {
                                out.push(b'\n');
                                self.pending_cr = false;
                            }
                            0x07 => self.flush_pending_cr(&mut out),
                            _ => {
                                self.flush_pending_cr(&mut out);
                                out.push(b);
                            }
                        }
                    }
                }
            }
        }
        self.inner.write_all(&out)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.pending_cr {
            self.inner.write_all(b"\n")?;
            self.pending_cr = false;
        }
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
        // "foo\r\r\nbar\r\r\nbaz" → three lines separated by LF
        let input = b"foo\r\r\nbar\r\r\nbaz";
        assert_eq!(sanitize(input), b"foo\nbar\nbaz");
    }

    #[test]
    fn cr_before_esc_is_swallowed() {
        // Classic Cisco prompt redraw: existing-line \r\r ESC[K replacement
        let input = b"before\r\r\x1b[Kafter";
        assert_eq!(sanitize(input), b"beforeafter");
    }

    #[test]
    fn normal_crlf_pairs_collapse_to_lf() {
        let input = b"a\r\nb\r\nc";
        assert_eq!(sanitize(input), b"a\nb\nc");
    }

    #[test]
    fn lone_cr_becomes_newline() {
        // Mac-classic line ending, or field separator.
        let input = b"x\ry";
        assert_eq!(sanitize(input), b"x\ny");
    }

    #[test]
    fn bel_is_stripped() {
        let input = b"ding\x07 dong";
        assert_eq!(sanitize(input), b"ding dong");
    }

    #[test]
    fn osc_is_stripped_through_bel_terminator() {
        // OSC 0 ; "title" BEL  — xterm window-title update
        let input = b"hello\x1b]0;my title\x07there";
        assert_eq!(sanitize(input), b"hellothere");
    }

    #[test]
    fn osc_is_stripped_through_st_terminator() {
        // OSC ... ESC \
        let input = b"hello\x1b]0;my title\x1b\\there";
        assert_eq!(sanitize(input), b"hellothere");
    }

    #[test]
    fn two_byte_esc_sequences_are_stripped() {
        // ESC 7 is "save cursor" — no following params.
        let input = b"foo\x1b7bar";
        assert_eq!(sanitize(input), b"foobar");
    }

    #[test]
    fn preserves_utf8_and_tab() {
        let input = "héllo\tworld\n".as_bytes();
        assert_eq!(sanitize(input), input);
    }
}
