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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Read timeout for the blocking `port.read` call.
///
/// Only matters now as a safety upper bound: the read loop only
/// calls `read()` after `bytes_to_read()` reports a non-zero count,
/// so the read should always satisfy from the driver's cached
/// buffer and return immediately. The previous "shorter timeout =
/// less queue contention" approach didn't work — measured on
/// Windows ARM, 1ms was actually *worse* than 50ms for the first
/// keystroke (1.6s vs 0.8s) because more frequent ReadFile IRPs
/// just meant more chances to be in front of a pending WriteFile.
const READ_TIMEOUT: Duration = Duration::from_millis(50);

/// How long to sleep between `bytes_to_read` polls when the device
/// buffer is empty. This is the lower bound on read latency: any
/// byte coming from the device waits at most this long before we
/// notice. 1ms is well under perception threshold and well under
/// the per-byte time at any baud rate we care about (1ms = 1 byte
/// at 9600, 10 bytes at 115200, 12 bytes at 1Mbaud).
const POLL_INTERVAL: Duration = Duration::from_millis(1);

/// Per-read buffer size. 4 KiB matches what the main app uses and is
/// well above any reasonable single-burst from a 9600-baud-class
/// link, so the read loop only allocates the `Vec` it ships across
/// the channel.
const READ_BUF: usize = 4096;

/// Sink for received bytes during file transfer. When set, the read
/// loop forwards inbound bytes to this callback instead of pushing
/// them on `read_rx` — so the protocol state machine sees raw bytes
/// without the terminal also rendering them. Cleared via
/// `clear_transfer_sink` when the transfer finishes (success, error,
/// or cancel) so normal terminal flow resumes.
pub type TransferSink = Box<dyn FnMut(&[u8]) + Send>;

/// The two ends of a live serial session, as seen from the gpui side.
pub struct SerialChannels {
    /// Async-receivable stream of bytes coming FROM the device.
    /// Drain this in a foreground task and pipe into `feed_bytes`.
    pub read_rx: flume::Receiver<Vec<u8>>,
    /// Synchronously-sendable sink of bytes going TO the device.
    /// Pushed from the keyboard handler — `send` is non-blocking
    /// (unbounded channel) so it never stalls a keystroke.
    pub write_tx: flume::Sender<Vec<u8>>,
    /// Slot the read loop checks each iteration. When `Some(_)`,
    /// inbound bytes are funnelled to the sink instead of `read_rx`.
    /// Held in a `Mutex` so the gpui thread can swap it in/out while
    /// the read thread is mid-loop.
    pub transfer_sink: Arc<std::sync::Mutex<Option<TransferSink>>>,
    /// Token for shutting down the OS threads. Hold for the
    /// session lifetime; call `Disconnect::shutdown` on teardown
    /// to flag the threads to exit quickly. JoinHandles inside
    /// then drop without `join` so the UI doesn't block on a
    /// stuck `port.write_all`; the threads themselves wind down
    /// in <100ms once the flag is set, releasing the port file
    /// descriptor (otherwise the next `open` races and fails with
    /// "Unable to acquire exclusive lock" on macOS).
    pub disconnect: Disconnect,
}

impl SerialChannels {
    /// Install a transfer sink. Callers swap one in for the duration
    /// of an XMODEM/YMODEM transfer; the read loop forwards every
    /// inbound chunk to it instead of `read_rx`.
    pub fn set_transfer_sink(&self, sink: TransferSink) {
        *self.transfer_sink.lock().unwrap() = Some(sink);
    }

    /// Remove any installed transfer sink. After this returns,
    /// inbound bytes flow back to `read_rx` and the terminal pane.
    pub fn clear_transfer_sink(&self) {
        *self.transfer_sink.lock().unwrap() = None;
    }
}

/// `std::io::Write` adapter that pushes outbound bytes through the
/// serial session's write channel. The transfer protocol state
/// machines need a `Write` implementor; the prototype's write side
/// is a flume sender, so we adapt rather than refactor the threads.
pub struct ChannelWriter {
    tx: flume::Sender<Vec<u8>>,
}

impl ChannelWriter {
    pub fn new(tx: flume::Sender<Vec<u8>>) -> Self {
        Self { tx }
    }
}

impl io::Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.tx
            .send(buf.to_vec())
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "serial port closed"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Token for shutting down a serial session's OS threads.
/// Created by `open`; calling `shutdown` flips the shared
/// `Arc<AtomicBool>` the threads check each iteration, then drops
/// the `JoinHandle`s (which detaches them — the threads still wind
/// down on their own within ~POLL_INTERVAL of the flag being
/// set). We deliberately don't `join` because either thread could
/// be stuck on a blocking syscall (`port.write_all` waiting on
/// RTS/CTS, an unresponsive disconnect ioctl), and joining them
/// would freeze the UI.
pub struct Disconnect {
    shutdown: Arc<AtomicBool>,
    read_thread: std::thread::JoinHandle<()>,
    write_thread: std::thread::JoinHandle<()>,
}

impl Disconnect {
    /// Signal the OS threads to exit. Returns immediately; the
    /// threads will check the flag on their next loop iteration
    /// (worst case ~SHUTDOWN_POLL_INTERVAL away) and drop their
    /// port handles, releasing the file descriptor so a subsequent
    /// `open` can succeed.
    pub fn shutdown(self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

/// What to do with one of the modem-control lines (DTR or RTS) at
/// connect or disconnect. Mirrors the Tauri Profile field shape:
/// the empty string and "default" both mean "leave the line in
/// whatever state the OS / driver opened it"; "assert" / "deassert"
/// drive it high / low respectively.
///
/// These knobs only matter for specific adapters or devices —
/// RS-485 direction control, Arduino DTR-reset on connect,
/// firmwares that key off DTR for session lifecycle. Most network
/// gear runs fine with `Default`.
#[derive(Debug, Clone, Copy, Default)]
pub enum LinePolicy {
    #[default]
    Default,
    Assert,
    Deassert,
}

impl LinePolicy {
    /// Map the string form stored on `Profile` (one of "", "default",
    /// "assert", "deassert") into the typed enum. Unknown values
    /// degrade to `Default` rather than erroring — the store-level
    /// validator already rejects bad values, and a runtime fallback
    /// here keeps a freshly-deserialised profile usable even if the
    /// schema drifts later.
    pub fn from_str(s: &str) -> Self {
        match s {
            "assert" => Self::Assert,
            "deassert" => Self::Deassert,
            _ => Self::Default,
        }
    }
}

/// Full set of modem-control-line policies for one session, both
/// sides (open + close). Defaulting to all-`Default` makes the
/// "no profile" / loopback path a one-line call.
#[derive(Debug, Clone, Copy, Default)]
pub struct LinePolicies {
    pub dtr_on_connect: LinePolicy,
    pub rts_on_connect: LinePolicy,
    pub dtr_on_disconnect: LinePolicy,
    pub rts_on_disconnect: LinePolicy,
}

/// Apply a `LinePolicy` to the DTR line. `Default` is a no-op;
/// errors propagate so the caller can decide whether to abort the
/// open (currently we log and continue — a control-line refusal
/// shouldn't tank an otherwise-working session).
fn apply_dtr(port: &mut dyn serialport::SerialPort, policy: LinePolicy) -> serialport::Result<()> {
    match policy {
        LinePolicy::Default => Ok(()),
        LinePolicy::Assert => port.write_data_terminal_ready(true),
        LinePolicy::Deassert => port.write_data_terminal_ready(false),
    }
}

/// Apply a `LinePolicy` to the RTS line. See `apply_dtr` for the
/// semantics; the only difference is which line gets driven.
fn apply_rts(port: &mut dyn serialport::SerialPort, policy: LinePolicy) -> serialport::Result<()> {
    match policy {
        LinePolicy::Default => Ok(()),
        LinePolicy::Assert => port.write_request_to_send(true),
        LinePolicy::Deassert => port.write_request_to_send(false),
    }
}

/// Open a serial port at `port_path` with `baud` 8N1 and start the
/// read + write threads. Returns the channels the gpui side reads
/// from / writes to. `policies` carries the four DTR/RTS knobs from
/// the active profile — connect-time policies are applied right
/// after open, disconnect-time policies are handed to the write
/// thread which applies them right before exit.
///
/// 8N1 (8 data bits, no parity, 1 stop bit) is hardcoded because
/// it's the universal default for serial-console network gear; a
/// real settings panel will eventually parameterize this.
///
/// If opening or cloning fails, the caller gets the error and can
/// fall back to loopback mode. Connect-time control-line writes
/// only log on failure — losing a DTR pulse shouldn't fail the
/// open if the port itself works.
pub fn open(
    port_path: &str,
    baud: u32,
    policies: LinePolicies,
) -> serialport::Result<SerialChannels> {
    let mut read_port = serialport::new(port_path, baud).timeout(READ_TIMEOUT).open()?;
    if let Err(e) = apply_dtr(&mut *read_port, policies.dtr_on_connect) {
        log::warn!("serial: dtr_on_connect failed: {e}");
    }
    if let Err(e) = apply_rts(&mut *read_port, policies.rts_on_connect) {
        log::warn!("serial: rts_on_connect failed: {e}");
    }
    // `try_clone` is the standard way to get a second handle pointing
    // at the same OS-level port — the read and write threads need
    // independent ownership so neither has to lock the other out.
    let write_port = read_port.try_clone()?;

    let (read_tx, read_rx) = flume::unbounded::<Vec<u8>>();
    let (write_tx, write_rx) = flume::unbounded::<Vec<u8>>();
    let shutdown = Arc::new(AtomicBool::new(false));
    let transfer_sink: Arc<std::sync::Mutex<Option<TransferSink>>> =
        Arc::new(std::sync::Mutex::new(None));

    let read_label = format!("serial-read({port_path})");
    let read_shutdown = shutdown.clone();
    let read_sink = transfer_sink.clone();
    let read_thread = std::thread::Builder::new()
        .name(read_label)
        .spawn(move || read_loop(read_port, read_tx, read_sink, read_shutdown))
        .expect("spawn serial read thread");

    let write_label = format!("serial-write({port_path})");
    let write_shutdown = shutdown.clone();
    let write_thread = std::thread::Builder::new()
        .name(write_label)
        .spawn(move || write_loop(write_port, write_rx, policies, write_shutdown))
        .expect("spawn serial write thread");

    Ok(SerialChannels {
        read_rx,
        write_tx,
        transfer_sink,
        disconnect: Disconnect {
            shutdown,
            read_thread,
            write_thread,
        },
    })
}

fn read_loop(
    mut port: Box<dyn serialport::SerialPort>,
    tx: flume::Sender<Vec<u8>>,
    transfer_sink: Arc<std::sync::Mutex<Option<TransferSink>>>,
    shutdown: Arc<AtomicBool>,
) {
    let mut buf = [0u8; READ_BUF];
    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        // `bytes_to_read()` is `ClearCommError` under the hood — a
        // non-blocking status query against the driver's cached
        // buffer state. Crucially it does NOT issue an IRP to the
        // device, so it doesn't compete with WriteFile for the NT
        // I/O queue. That's the entire point of polling here vs.
        // calling `read()` directly: a blocking ReadFile holds the
        // device queue and writes from the other thread sit behind
        // it for hundreds of ms.
        match port.bytes_to_read() {
            Ok(0) => {
                std::thread::sleep(POLL_INTERVAL);
                continue;
            }
            Ok(_) => match port.read(&mut buf) {
                Ok(0) => continue,
                Ok(n) => {
                    // While a transfer is active, divert bytes to
                    // its sink so the protocol state machine sees
                    // raw input. The lock is held only across the
                    // sink call; gpui-side install/clear happens
                    // between iterations either way.
                    let mut sink_guard = transfer_sink.lock().unwrap();
                    if let Some(sink) = sink_guard.as_mut() {
                        sink(&buf[..n]);
                    } else {
                        drop(sink_guard);
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
                // Shouldn't fire — bytes_to_read said data was
                // available — but handle it for symmetry.
                Err(e) if e.kind() == io::ErrorKind::TimedOut => continue,
                Err(e) => {
                    log::error!("serial read error: {e}");
                    break;
                }
            },
            Err(e) => {
                log::error!("serial bytes_to_read error: {e}");
                break;
            }
        }
    }
}

fn write_loop(
    mut port: Box<dyn serialport::SerialPort>,
    rx: flume::Receiver<Vec<u8>>,
    policies: LinePolicies,
    shutdown: Arc<AtomicBool>,
) {
    // Use `recv_timeout` instead of `recv` so the loop wakes
    // periodically and can check `shutdown` even when no bytes
    // have been queued. Without this, a stray sender clone (e.g.
    // a slow-paste task whose drop hasn't happened yet) would keep
    // `recv` blocked forever and the port handle would never be
    // released.
    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        match rx.recv_timeout(SHUTDOWN_POLL_INTERVAL) {
            Ok(bytes) => {
                // `write_all` because POSIX `write` is allowed to
                // short-write; we want every byte through. No
                // `flush()` afterwards: on Unix that calls
                // `tcdrain` which blocks until the OS tx buffer
                // drains — adding tens of ms of latency on every
                // keystroke, exactly what this prototype is trying
                // to avoid.
                if let Err(e) = port.write_all(&bytes) {
                    log::error!("serial write error: {e}");
                    break;
                }
            }
            Err(flume::RecvTimeoutError::Timeout) => continue,
            Err(flume::RecvTimeoutError::Disconnected) => break,
        }
    }
    // Apply disconnect-time DTR/RTS policies before the port handle
    // drops. The write thread is the natural place for this — it
    // exits when the AppView clears its sender (the disconnect
    // signal), and it owns a port handle that's still alive at that
    // moment. The read thread will exit on its next iteration when
    // its tx channel is closed; nothing to do there.
    if let Err(e) = apply_dtr(&mut *port, policies.dtr_on_disconnect) {
        log::warn!("serial: dtr_on_disconnect failed: {e}");
    }
    if let Err(e) = apply_rts(&mut *port, policies.rts_on_disconnect) {
        log::warn!("serial: rts_on_disconnect failed: {e}");
    }
}

/// How often `write_loop` wakes from `recv_timeout` to re-check
/// the shutdown flag. 50ms is well below the typical reconnect
/// gap (a button click + form re-render is hundreds of ms) and
/// short enough that even a snappy disconnect→connect releases
/// the port within one loop iteration.
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(50);
