//! Serial port I/O for checkpoint #5.
//!
//! `serialport-rs` is a synchronous library — its `Read` blocks until
//! bytes arrive (or the timeout fires) and its `Write` blocks until
//! the OS accepts the bytes. Both sides are pumped on dedicated OS
//! threads so neither blocks the gpui main loop:
//!
//!   * Read thread: blocking `port.read(&mut buf)` with a short
//!     timeout, then ships any bytes through the read channel. The
//!     timeout (vs. blocking forever) lets the thread drop cleanly
//!     once both ends of the channel are closed.
//!   * Write thread: blocks on the write channel; whenever
//!     `TerminalView::handle_key_down` pushes encoded keystroke
//!     bytes, this thread drains them and writes to the port.
//!
//! Splitting read and write across two threads (instead of one
//! select-style loop) keeps each side simple — no need for
//! non-blocking I/O or a poll/epoll wrapper. The cost is one extra
//! thread per session, which is fine for a one-port-at-a-time tool.
//!
//! The gpui-side glue (drain `read_rx` async, `view.feed_bytes`)
//! lives in `main.rs` so this module stays parser-agnostic — same
//! shape as the future "promote to the main tree" world where the
//! serial layer doesn't know about TerminalView at all.

use std::io::{self, Read, Write};
use std::time::Duration;

/// Read timeout for the blocking `port.read` call. Short enough that
/// a closed channel gets noticed promptly when the gpui app shuts
/// down; long enough that we're not burning CPU when the device is
/// idle. 50ms is roughly two frames at 60fps — invisible to a human.
const READ_TIMEOUT: Duration = Duration::from_millis(50);

/// Per-read buffer size. 4 KiB matches what the main app uses and is
/// well above any reasonable single-burst from a 9600-baud-class
/// link, so the read loop only allocates the `Vec` it ships across
/// the channel.
const READ_BUF: usize = 4096;

/// The two ends of a live serial session, as seen from the gpui side.
pub struct SerialChannels {
    /// Async-receivable stream of bytes coming FROM the device.
    /// Drain this in a foreground task and pipe into `feed_bytes`.
    pub read_rx: flume::Receiver<Vec<u8>>,
    /// Synchronously-sendable sink of bytes going TO the device.
    /// Pushed from the keyboard handler — `send` is non-blocking
    /// (unbounded channel) so it never stalls a keystroke.
    pub write_tx: flume::Sender<Vec<u8>>,
}

/// Open a serial port at `port_path` with `baud` 8N1 and start the
/// read + write threads. Returns the channels the gpui side reads
/// from / writes to. If opening or cloning fails, the caller gets
/// the error and can fall back to loopback mode.
///
/// 8N1 (8 data bits, no parity, 1 stop bit) is hardcoded because
/// it's the universal default for serial-console network gear; a
/// real settings panel will eventually parameterize this.
pub fn open(port_path: &str, baud: u32) -> serialport::Result<SerialChannels> {
    let read_port = serialport::new(port_path, baud).timeout(READ_TIMEOUT).open()?;
    // `try_clone` is the standard way to get a second handle pointing
    // at the same OS-level port — the read and write threads need
    // independent ownership so neither has to lock the other out.
    let write_port = read_port.try_clone()?;

    let (read_tx, read_rx) = flume::unbounded::<Vec<u8>>();
    let (write_tx, write_rx) = flume::unbounded::<Vec<u8>>();

    let read_label = format!("serial-read({port_path})");
    std::thread::Builder::new()
        .name(read_label)
        .spawn(move || read_loop(read_port, read_tx))
        .expect("spawn serial read thread");

    let write_label = format!("serial-write({port_path})");
    std::thread::Builder::new()
        .name(write_label)
        .spawn(move || write_loop(write_port, write_rx))
        .expect("spawn serial write thread");

    Ok(SerialChannels { read_rx, write_tx })
}

fn read_loop(mut port: Box<dyn serialport::SerialPort>, tx: flume::Sender<Vec<u8>>) {
    let mut buf = [0u8; READ_BUF];
    loop {
        match port.read(&mut buf) {
            Ok(0) => continue,
            Ok(n) => {
                // The receiver-dropped case = the gpui side has shut
                // down. Stop reading; the OS thread exits cleanly.
                if tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
            // The timeout fires every `READ_TIMEOUT` whenever there's
            // nothing on the wire. Loop and try again.
            Err(e) if e.kind() == io::ErrorKind::TimedOut => continue,
            Err(e) => {
                log::error!("serial read error: {e}");
                break;
            }
        }
    }
}

fn write_loop(mut port: Box<dyn serialport::SerialPort>, rx: flume::Receiver<Vec<u8>>) {
    while let Ok(bytes) = rx.recv() {
        // `write_all` because POSIX `write` is allowed to short-write;
        // we want every byte through. No `flush()` afterwards: on
        // Unix that calls `tcdrain` which blocks until the OS tx
        // buffer drains — adding tens of ms of latency on every
        // keystroke, exactly what this prototype is trying to avoid.
        if let Err(e) = port.write_all(&bytes) {
            log::error!("serial write error: {e}");
            break;
        }
    }
}
