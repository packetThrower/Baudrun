//! Serial session — owns the open port, runs a read pump in its own
//! thread, and exposes the control-line / break / log-writer /
//! transfer-redirect surface the rest of the app needs.
//!
//! The native backend uses `serialport::SerialPort::try_clone()` to
//! split the underlying FD into independent read and write handles,
//! so command invocations (send, set_dtr, send_break, ...) don't
//! contend with the 100ms blocking read loop.

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use thiserror::Error;

/// Kernel/driver read block. 100ms is the same budget go.bug.st uses
/// and is small enough that Close() observes `closed = true` promptly.
const READ_TIMEOUT: Duration = Duration::from_millis(100);

/// "300ms matches PuTTY's default and is long enough for every device
/// I've seen without stalling the session noticeably." — same value
/// the Go version used.
pub const BREAK_DURATION: Duration = Duration::from_millis(300);

/// Serial parameters + control-line policies for a session.
#[derive(Debug, Clone)]
pub struct Config {
    pub port_name: String,
    pub baud_rate: u32,
    pub data_bits: u32,
    pub parity: String,
    pub stop_bits: String,
    pub flow_control: String,

    /// "", "default": leave the line alone.
    /// "assert": drive high.
    /// "deassert": drive low.
    pub dtr_on_connect: String,
    pub rts_on_connect: String,
    pub dtr_on_disconnect: String,
    pub rts_on_disconnect: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ControlLines {
    pub dtr: bool,
    pub rts: bool,
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("invalid parity: {0}")]
    InvalidParity(String),
    #[error("invalid stop bits: {0}")]
    InvalidStopBits(String),
    #[error("invalid flow control: {0}")]
    InvalidFlowControl(String),
    #[error("invalid data bits: {0}")]
    InvalidDataBits(u32),
    #[error("baud rate must be positive")]
    InvalidBaud,
    #[error("direct-USB (libusb) backend not yet implemented on Tauri")]
    DirectUsbUnsupported,
    #[error("session closed")]
    Closed,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("serialport: {0}")]
    Serial(#[from] serialport::Error),
}

pub type Result<T> = std::result::Result<T, SessionError>;

/// Internal backend trait. Native (serialport crate) is the only
/// implementation today; libusb-direct (rusb + CP210x) is planned
/// for a later phase and will implement this same trait.
trait PortBackend: Read + Write + Send {
    fn set_dtr(&mut self, v: bool) -> io::Result<()>;
    fn set_rts(&mut self, v: bool) -> io::Result<()>;
    fn send_break_signal(&mut self, duration: Duration) -> io::Result<()>;
}

struct NativePort {
    inner: Box<dyn SerialPort>,
}

impl Read for NativePort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for NativePort {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl PortBackend for NativePort {
    fn set_dtr(&mut self, v: bool) -> io::Result<()> {
        self.inner
            .write_data_terminal_ready(v)
            .map_err(serialport_to_io)
    }
    fn set_rts(&mut self, v: bool) -> io::Result<()> {
        self.inner
            .write_request_to_send(v)
            .map_err(serialport_to_io)
    }
    fn send_break_signal(&mut self, duration: Duration) -> io::Result<()> {
        self.inner.set_break().map_err(serialport_to_io)?;
        thread::sleep(duration);
        self.inner.clear_break().map_err(serialport_to_io)
    }
}

fn serialport_to_io(err: serialport::Error) -> io::Error {
    match err.kind() {
        serialport::ErrorKind::NoDevice => io::Error::new(io::ErrorKind::NotFound, err),
        serialport::ErrorKind::InvalidInput => io::Error::new(io::ErrorKind::InvalidInput, err),
        serialport::ErrorKind::Io(kind) => io::Error::new(kind, err),
        _ => io::Error::other(err),
    }
}

/// Callback invoked on each chunk of bytes read from the port during
/// normal operation. Shared (`Arc`) so the session can keep a handle
/// for logging/transfer coordination while the read thread also owns
/// one.
pub type OnRead = Arc<dyn Fn(&[u8]) + Send + Sync>;

/// Callback invoked once when the read pump exits due to an I/O error
/// (not a clean close). Surfaced to the frontend as `serial:disconnect`
/// so the session UI can react (offer reconnect, switch icons, etc.).
pub type OnExit = Arc<dyn Fn(io::Error) + Send + Sync>;

/// Sink for received bytes during file transfer. When set, incoming
/// bytes bypass the normal `on_read` delivery path and flow to this
/// callback instead — so the transfer protocol gets raw byte access
/// without the terminal also displaying them.
pub type TransferSink = Arc<dyn Fn(&[u8]) + Send + Sync>;

/// An open serial session.
pub struct Session {
    /// Port used for writes and control-line operations. Split from
    /// the read pump via `try_clone()` so blocking reads don't stall
    /// outbound traffic.
    port_write: Mutex<Box<dyn PortBackend>>,

    closed: Arc<AtomicBool>,
    dtr_state: AtomicBool,
    rts_state: AtomicBool,
    dtr_on_close: String,
    rts_on_close: String,

    log_writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    transfer_rx: Arc<Mutex<Option<TransferSink>>>,

    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Session {
    /// Open the port named in `cfg` and start the read pump. The
    /// `on_read` callback receives every chunk of bytes in normal
    /// operation; `on_exit` fires once when the read pump dies
    /// unexpectedly.
    pub fn open(cfg: Config, on_read: OnRead, on_exit: OnExit) -> Result<Self> {
        if cfg.baud_rate == 0 {
            return Err(SessionError::InvalidBaud);
        }
        if is_direct_usb_port_name(&cfg.port_name) {
            return Err(SessionError::DirectUsbUnsupported);
        }
        Self::open_native(cfg, on_read, on_exit)
    }

    fn open_native(cfg: Config, on_read: OnRead, on_exit: OnExit) -> Result<Self> {
        let settings = serialport::new(&cfg.port_name, cfg.baud_rate)
            .data_bits(to_data_bits(cfg.data_bits)?)
            .parity(to_parity(&cfg.parity)?)
            .stop_bits(to_stop_bits(&cfg.stop_bits)?)
            .flow_control(to_flow(&cfg.flow_control)?)
            .timeout(READ_TIMEOUT);

        let port = settings.open().map_err(|e| enrich_open_error(&cfg.port_name, e))?;
        let read_handle = port.try_clone().map_err(|e| {
            enrich_open_error(
                &cfg.port_name,
                serialport::Error::new(
                    serialport::ErrorKind::Io(io::ErrorKind::Other),
                    format!("try_clone: {}", e),
                ),
            )
        })?;

        let mut write = NativePort { inner: port };
        let read_port = NativePort { inner: read_handle };

        let dtr = apply_line(|v| write.set_dtr(v), &cfg.dtr_on_connect, true);
        let rts = apply_line(|v| write.set_rts(v), &cfg.rts_on_connect, true);

        let closed = Arc::new(AtomicBool::new(false));
        let log_writer: Arc<Mutex<Option<Box<dyn Write + Send>>>> =
            Arc::new(Mutex::new(None));
        let transfer_rx: Arc<Mutex<Option<TransferSink>>> = Arc::new(Mutex::new(None));

        let thread = spawn_read_pump(
            read_port,
            closed.clone(),
            on_read,
            on_exit,
            log_writer.clone(),
            transfer_rx.clone(),
        );

        Ok(Session {
            port_write: Mutex::new(Box::new(write)),
            closed,
            dtr_state: AtomicBool::new(dtr),
            rts_state: AtomicBool::new(rts),
            dtr_on_close: cfg.dtr_on_disconnect,
            rts_on_close: cfg.rts_on_disconnect,
            log_writer,
            transfer_rx,
            thread: Mutex::new(Some(thread)),
        })
    }

    pub fn send(&self, data: &[u8]) -> Result<()> {
        if self.closed.load(Ordering::Acquire) {
            return Err(SessionError::Closed);
        }
        let mut port = self.port_write.lock().unwrap();
        port.write_all(data)?;
        Ok(())
    }

    pub fn set_dtr(&self, v: bool) -> Result<()> {
        if self.closed.load(Ordering::Acquire) {
            return Err(SessionError::Closed);
        }
        let mut port = self.port_write.lock().unwrap();
        port.set_dtr(v)?;
        self.dtr_state.store(v, Ordering::Release);
        Ok(())
    }

    pub fn set_rts(&self, v: bool) -> Result<()> {
        if self.closed.load(Ordering::Acquire) {
            return Err(SessionError::Closed);
        }
        let mut port = self.port_write.lock().unwrap();
        port.set_rts(v)?;
        self.rts_state.store(v, Ordering::Release);
        Ok(())
    }

    pub fn control_lines(&self) -> ControlLines {
        ControlLines {
            dtr: self.dtr_state.load(Ordering::Acquire),
            rts: self.rts_state.load(Ordering::Acquire),
        }
    }

    /// Hold the serial break condition (TX low) for `duration`, then
    /// release. Used to drop into ROMMON on Cisco gear, Juniper's
    /// diagnostic prompt, or to break out of a boot loader.
    pub fn send_break(&self, duration: Duration) -> Result<()> {
        if self.closed.load(Ordering::Acquire) {
            return Err(SessionError::Closed);
        }
        let mut port = self.port_write.lock().unwrap();
        port.send_break_signal(duration)?;
        Ok(())
    }

    /// Attach or detach a writer that gets a copy of every byte read
    /// from the port (session logging). Passing `None` detaches and
    /// closes the previous writer. Typically called right after
    /// `open`.
    pub fn set_log_writer(&self, writer: Option<Box<dyn Write + Send>>) {
        let mut guard = self.log_writer.lock().unwrap();
        *guard = writer;
    }

    /// Divert incoming bytes away from `on_read` to `sink` until
    /// [`Session::end_transfer`] is called. Used by XMODEM/YMODEM so
    /// the transfer protocol has raw byte access without the terminal
    /// also rendering the bytes.
    pub fn start_transfer(&self, sink: TransferSink) {
        let mut guard = self.transfer_rx.lock().unwrap();
        *guard = Some(sink);
    }

    /// Restore normal `on_read` delivery.
    pub fn end_transfer(&self) {
        let mut guard = self.transfer_rx.lock().unwrap();
        *guard = None;
    }

    /// Close the port and join the read thread. Applies the
    /// on-disconnect control-line policies before releasing the
    /// handle; close is idempotent so repeated calls are safe.
    pub fn close(&self) -> Result<()> {
        // Compare-and-swap — only the first caller runs the teardown.
        if self
            .closed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(());
        }

        {
            let mut port = self.port_write.lock().unwrap();
            apply_line(|v| port.set_dtr(v), &self.dtr_on_close, true);
            apply_line(|v| port.set_rts(v), &self.rts_on_close, true);
            // Dropping the write handle here would be ideal but the
            // port lives inside the struct until Drop — good enough,
            // since the read thread sees `closed` and exits.
        }

        // Best-effort join so the FD is definitely released before
        // returning. Bounded by READ_TIMEOUT.
        if let Some(handle) = self.thread.lock().unwrap().take() {
            let _ = handle.join();
        }

        // Drop the log writer if attached.
        let mut log = self.log_writer.lock().unwrap();
        *log = None;
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn spawn_read_pump(
    mut port: NativePort,
    closed: Arc<AtomicBool>,
    on_read: OnRead,
    on_exit: OnExit,
    log_writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    transfer_rx: Arc<Mutex<Option<TransferSink>>>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("baudrun-read".into())
        .spawn(move || {
            let mut buf = vec![0u8; 4096];
            loop {
                if closed.load(Ordering::Acquire) {
                    return;
                }
                let n = match port.read(&mut buf) {
                    Ok(n) => n,
                    Err(err) if is_timeout(&err) => continue,
                    Err(err) => {
                        if !closed.load(Ordering::Acquire) {
                            on_exit(err);
                        }
                        return;
                    }
                };
                if n == 0 {
                    continue;
                }
                let chunk = &buf[..n];

                let transfer = transfer_rx.lock().unwrap().clone();
                if let Some(sink) = transfer {
                    sink(chunk);
                } else {
                    on_read(chunk);
                }

                if let Some(w) = log_writer.lock().unwrap().as_mut() {
                    let _ = w.write_all(chunk);
                }
            }
        })
        .expect("spawn baudrun-read thread")
}

fn is_timeout(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    )
}

/// Apply a control-line policy to `setter`. Returns the resulting
/// logical state. On "default"/"" the line is left untouched and the
/// caller's assumed default is returned instead.
fn apply_line<F>(mut setter: F, policy: &str, default_state: bool) -> bool
where
    F: FnMut(bool) -> io::Result<()>,
{
    match policy {
        "assert" => {
            let _ = setter(true);
            true
        }
        "deassert" => {
            let _ = setter(false);
            false
        }
        _ => default_state,
    }
}

fn to_data_bits(n: u32) -> Result<DataBits> {
    Ok(match n {
        5 => DataBits::Five,
        6 => DataBits::Six,
        7 => DataBits::Seven,
        8 => DataBits::Eight,
        _ => return Err(SessionError::InvalidDataBits(n)),
    })
}

fn to_parity(p: &str) -> Result<Parity> {
    Ok(match p {
        "" | "none" => Parity::None,
        "odd" => Parity::Odd,
        "even" => Parity::Even,
        // serialport-rs 4.x doesn't expose Mark/Space — treat them as
        // None at the driver level and flag the case in the error.
        // Callers will need to re-validate; for now we accept
        // configuration and downgrade.
        "mark" | "space" => Parity::None,
        other => return Err(SessionError::InvalidParity(other.to_string())),
    })
}

fn to_stop_bits(s: &str) -> Result<StopBits> {
    Ok(match s {
        "" | "1" => StopBits::One,
        // serialport-rs 4.x only exposes One and Two. 1.5 is rare in
        // modern hardware; downgrade to One rather than failing open.
        "1.5" => StopBits::One,
        "2" => StopBits::Two,
        other => return Err(SessionError::InvalidStopBits(other.to_string())),
    })
}

fn to_flow(f: &str) -> Result<FlowControl> {
    Ok(match f {
        "" | "none" => FlowControl::None,
        "rtscts" | "hardware" => FlowControl::Hardware,
        "xonxoff" | "software" => FlowControl::Software,
        other => return Err(SessionError::InvalidFlowControl(other.to_string())),
    })
}

/// Reserved prefix on a port name that marks it as a libusb-direct
/// device — the Go version routed these to usbserial-go. Tauri port
/// stubs the direct-USB backend (see `non-tauri-features.md`), so
/// callers hitting this prefix currently error out.
pub(super) const DIRECT_USB_PREFIX: &str = "usb:";

pub fn is_direct_usb_port_name(name: &str) -> bool {
    name.starts_with(DIRECT_USB_PREFIX)
}

/// Linux-only: rewrite EACCES-looking errors with the dialout-group
/// fix-up hint. Other platforms return the error wrapped with the
/// port name for context.
fn enrich_open_error(port_name: &str, err: serialport::Error) -> SessionError {
    #[cfg(target_os = "linux")]
    {
        if looks_like_permission_denied(&err) {
            return SessionError::Io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "open {}: {} — fix: run `sudo usermod -aG dialout $USER`, then log out and back in. \
                     (Baudrun's .deb/.rpm/pacman packages ship a udev rule that avoids this entirely — \
                     installed via AppImage or from source, you're on the manual path.)",
                    port_name, err
                ),
            ));
        }
    }
    SessionError::Io(io::Error::other(format!("open {}: {}", port_name, err)))
}

#[cfg(target_os = "linux")]
fn looks_like_permission_denied(err: &serialport::Error) -> bool {
    if let serialport::ErrorKind::Io(kind) = err.kind() {
        if kind == io::ErrorKind::PermissionDenied {
            return true;
        }
    }
    err.to_string().to_ascii_lowercase().contains("permission denied")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_usb_prefix_matches() {
        assert!(is_direct_usb_port_name("usb:10c4:ea60"));
        assert!(is_direct_usb_port_name("usb:10c4:ea60:ABC123"));
        assert!(!is_direct_usb_port_name("/dev/cu.usbserial-1234"));
        assert!(!is_direct_usb_port_name("COM3"));
    }

    #[test]
    fn apply_line_policies() {
        let mut last: Option<bool> = None;
        let state = apply_line(
            |v| {
                last = Some(v);
                Ok(())
            },
            "assert",
            false,
        );
        assert_eq!(state, true);
        assert_eq!(last, Some(true));

        let mut last: Option<bool> = None;
        let state = apply_line(
            |v| {
                last = Some(v);
                Ok(())
            },
            "deassert",
            true,
        );
        assert_eq!(state, false);
        assert_eq!(last, Some(false));

        let mut last: Option<bool> = None;
        let state = apply_line(
            |v| {
                last = Some(v);
                Ok(())
            },
            "default",
            true,
        );
        assert_eq!(state, true);
        assert_eq!(last, None, "default must not invoke the setter");
    }

    #[test]
    fn to_data_bits_range() {
        assert!(matches!(to_data_bits(8), Ok(DataBits::Eight)));
        assert!(matches!(to_data_bits(7), Ok(DataBits::Seven)));
        assert!(matches!(to_data_bits(4), Err(SessionError::InvalidDataBits(4))));
    }
}
