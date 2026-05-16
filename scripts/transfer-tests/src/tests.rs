//! Individual test cases. Each is a function returning [`TestOutcome`].
//!
//! Map between these and TESTING.md's T1–T11 numbering:
//!
//! - T1 (hex ASCII)               → `hex_ascii`
//! - T3 (hex binary / non-printable) → `hex_binary`
//! - T5 (hex 1 KiB)               → `hex_1k`
//! - T6 (YMODEM 4 KiB)            → `ymodem_4k`
//! - T7-classic / T7-crc / T7-1k  → `xmodem_classic` / `xmodem_crc` / `xmodem_1k`
//! - T8 (XMODEM single block)     → `xmodem_tiny`
//! - T9 (YMODEM 1 MiB)            → `ymodem_1m`
//! - T10 (cancel mid-transfer)    → `cancel_midway`
//! - T11 (slow link)              → `ymodem_slow_512`
//!
//! T2 (hex input-format equivalence) and T4 (hex input validation)
//! are UI-layer concerns and aren't covered here — `parse_hex_string`
//! itself can be unit-tested separately if we want T2/T4 coverage,
//! but driving the modal is out of scope for a wire-level harness.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::hex::parse_hex_string;
use crate::port_reader::PollReader;
use crate::transfer::{self, send_xmodem, send_ymodem, Options, XModemVariant};
use crate::{LINK_A, LINK_B, RX_DIR};

pub struct TestOutcome {
    pub id: &'static str,
    pub name: &'static str,
    pub baud: u32,
    pub passed: bool,
    pub detail: String,
    pub duration: Duration,
}

impl TestOutcome {
    fn ok(id: &'static str, name: &'static str, baud: u32, start: Instant, detail: String) -> Self {
        Self { id, name, baud, passed: true, detail, duration: start.elapsed() }
    }
    fn fail(id: &'static str, name: &'static str, baud: u32, start: Instant, detail: String) -> Self {
        Self { id, name, baud, passed: false, detail, duration: start.elapsed() }
    }
}

// -- Hex tests --------------------------------------------------------

pub fn hex_ascii() -> TestOutcome {
    run_hex_case("T1", "hex ASCII (Hello)", "48 65 6c 6c 6f", b"Hello")
}

pub fn hex_binary() -> TestOutcome {
    run_hex_case(
        "T3",
        "hex binary / non-printable",
        "00 01 02 ff fe 7f",
        &[0x00, 0x01, 0x02, 0xff, 0xfe, 0x7f],
    )
}

pub fn hex_1k(fixture: &Path) -> TestOutcome {
    let id = "T5";
    let name = "hex 1 KiB";
    let baud = 9600;
    let start = Instant::now();
    let input = match std::fs::read_to_string(fixture) {
        Ok(s) => s,
        Err(e) => return TestOutcome::fail(id, name, baud, start, format!("read {}: {e}", fixture.display())),
    };
    let expected = match parse_hex_string(&input) {
        Ok(b) => b,
        Err(e) => return TestOutcome::fail(id, name, baud, start, format!("parse fixture: {e}")),
    };
    if expected.len() != 1024 {
        return TestOutcome::fail(
            id, name, baud, start,
            format!("fixture decoded to {} bytes, expected 1024", expected.len()),
        );
    }
    match hex_roundtrip(&input, &expected, baud) {
        Ok(()) => TestOutcome::ok(id, name, baud, start, "1024 bytes round-tripped".into()),
        Err(e) => TestOutcome::fail(id, name, baud, start, e),
    }
}

fn run_hex_case(id: &'static str, name: &'static str, input: &str, expected: &[u8]) -> TestOutcome {
    let baud = 9600;
    let start = Instant::now();
    match hex_roundtrip(input, expected, baud) {
        Ok(()) => TestOutcome::ok(id, name, baud, start, format!("{} bytes round-tripped", expected.len())),
        Err(e) => TestOutcome::fail(id, name, baud, start, e),
    }
}

fn hex_roundtrip(input: &str, expected: &[u8], baud: u32) -> Result<(), String> {
    let parsed = parse_hex_string(input).map_err(|e| format!("parse: {e}"))?;
    if parsed != expected {
        return Err(format!(
            "parser disagreed with fixture ({} parsed vs {} expected)",
            parsed.len(),
            expected.len(),
        ));
    }

    let mut port_a = open_port(LINK_A)?;
    let mut port_b = open_port(LINK_B)?;

    port_a.write_all(&parsed).map_err(|e| format!("write A: {e}"))?;
    port_a.flush().map_err(|e| format!("flush A: {e}"))?;

    let (received, err) = read_until(&mut port_b, parsed.len(), wire_timeout(baud, parsed.len()));
    if let Some(e) = err {
        return Err(e);
    }
    if received != expected {
        return Err(diff_summary(expected, &received));
    }
    Ok(())
}

// -- File tests (YMODEM) ----------------------------------------------

pub fn ymodem_4k(fixture: &Path) -> TestOutcome {
    file_test_ymodem("T6", "YMODEM 4 KiB", 115200, fixture, /*cancel*/ None)
}

pub fn ymodem_1m(fixture: &Path) -> TestOutcome {
    file_test_ymodem("T9", "YMODEM 1 MiB", 115200, fixture, None)
}

pub fn ymodem_slow_512(fixture: &Path) -> TestOutcome {
    file_test_ymodem("T11", "YMODEM 512 B over slow link", 9600, fixture, None)
}

pub fn cancel_midway(fixture: &Path) -> TestOutcome {
    let cancel = Arc::new(AtomicBool::new(false));
    let arm = Arc::clone(&cancel);
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        arm.store(true, Ordering::SeqCst);
    });
    file_test_ymodem("T10", "cancel mid-transfer", 115200, fixture, Some(cancel))
}

fn file_test_ymodem(
    id: &'static str,
    name: &'static str,
    baud: u32,
    fixture: &Path,
    cancel: Option<Arc<AtomicBool>>,
) -> TestOutcome {
    let start = Instant::now();
    let data = match std::fs::read(fixture) {
        Ok(d) => d,
        Err(e) => return TestOutcome::fail(id, name, baud, start, format!("read {}: {e}", fixture.display())),
    };
    let basename = fixture.file_name().unwrap().to_string_lossy().into_owned();
    let received_path = Path::new(RX_DIR).join(&basename);
    let _ = std::fs::remove_file(&received_path);

    let receiver = match spawn_rb() {
        Ok(r) => r,
        Err(e) => return TestOutcome::fail(id, name, baud, start, e),
    };

    let (read_file, write_file) = match open_port_pair(LINK_A) {
        Ok(pair) => pair,
        Err(e) => return finish_with_receiver(id, name, baud, start, Err(e), receiver),
    };
    let mut reader = PollReader::new(read_file.as_raw_fd());
    let mut writer = write_file;

    let want_cancel = cancel.is_some();
    let opts = Options { progress: None, cancel };
    let send_result = send_ymodem(&mut reader, &mut writer, &basename, &data, &opts);
    drop(writer);
    drop(read_file);

    let final_result = if want_cancel {
        match send_result {
            Err(transfer::TransferError::Cancelled) => Ok(()),
            Err(e) => Err(format!("expected Cancelled, got {e}")),
            Ok(()) => Err("expected Cancelled, transfer completed".into()),
        }
    } else {
        match send_result {
            Ok(()) => verify_received(&data, &received_path, false),
            Err(e) => Err(format!("send_ymodem: {e}")),
        }
    };

    finish_with_receiver(id, name, baud, start, final_result, receiver)
}

// -- File tests (XMODEM) ----------------------------------------------

pub fn xmodem_classic(fixture: &Path) -> TestOutcome {
    file_test_xmodem("T7c", "XMODEM classic (128/checksum)", XModemVariant::Classic, &[], fixture)
}

pub fn xmodem_crc(fixture: &Path) -> TestOutcome {
    file_test_xmodem("T7C", "XMODEM-CRC (128/CRC-16)", XModemVariant::Crc, &["-c"], fixture)
}

pub fn xmodem_1k(fixture: &Path) -> TestOutcome {
    // lrx doesn't have a "-k" flag — XMODEM-1K is selected purely by the
    // sender's STX (0x02) block header instead of SOH (0x01). The
    // receiver only needs to be in CRC mode; the block-size byte tells
    // it whether to expect 128 or 1024 data bytes per block.
    file_test_xmodem("T7k", "XMODEM-1K (1024/CRC-16)", XModemVariant::OneKilo, &["-c"], fixture)
}

pub fn xmodem_tiny(fixture: &Path) -> TestOutcome {
    file_test_xmodem("T8", "XMODEM single-block (SUB padding)", XModemVariant::Classic, &[], fixture)
}

fn file_test_xmodem(
    id: &'static str,
    name: &'static str,
    variant: XModemVariant,
    rx_flags: &[&str],
    fixture: &Path,
) -> TestOutcome {
    let baud = 115200;
    let start = Instant::now();
    let data = match std::fs::read(fixture) {
        Ok(d) => d,
        Err(e) => return TestOutcome::fail(id, name, baud, start, format!("read {}: {e}", fixture.display())),
    };
    let received_path = Path::new(RX_DIR).join("out.bin");
    let _ = std::fs::remove_file(&received_path);

    let receiver = match spawn_rx(rx_flags, &received_path) {
        Ok(r) => r,
        Err(e) => return TestOutcome::fail(id, name, baud, start, e),
    };

    let (read_file, write_file) = match open_port_pair(LINK_A) {
        Ok(pair) => pair,
        Err(e) => return finish_with_receiver(id, name, baud, start, Err(e), receiver),
    };
    let mut reader = PollReader::new(read_file.as_raw_fd());
    let mut writer = write_file;

    let opts = Options::default();
    let send_result = send_xmodem(&mut reader, &mut writer, &data, variant, &opts);
    drop(writer);
    drop(read_file);

    let final_result = match send_result {
        Ok(()) => verify_received(&data, &received_path, true),
        Err(e) => Err(format!("send_xmodem: {e}")),
    };

    finish_with_receiver(id, name, baud, start, final_result, receiver)
}

// -- Helpers ----------------------------------------------------------

pub fn ensure_rx_dir() -> Result<(), String> {
    std::fs::create_dir_all(RX_DIR).map_err(|e| format!("mkdir {RX_DIR}: {e}"))
}

fn open_port(path: &str) -> Result<File, String> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| format!("open {path}: {e}"))
}

/// Open the endpoint twice via `try_clone` so the reader thread and
/// the write side hold independent `File` handles sharing the same
/// underlying file description. Required because the protocol sender
/// needs `&mut R` and `&mut W` simultaneously.
fn open_port_pair(path: &str) -> Result<(File, File), String> {
    let read_side = open_port(path)?;
    let write_side = read_side.try_clone().map_err(|e| format!("clone {path} fd: {e}"))?;
    Ok((read_side, write_side))
}

/// Read `expected` bytes from `file` with a wall-clock deadline, using
/// `libc::poll` to wait for fd readiness. Returns the bytes actually
/// read plus an optional error message describing how the read ended
/// early (timeout, EOF, syscall error). Used by the hex tests where a
/// forwarder thread would leak across test boundaries and steal the
/// first byte of the next stream — see the comment in Cargo.toml.
fn read_until(file: &mut File, expected: usize, total_timeout: Duration) -> (Vec<u8>, Option<String>) {
    let fd = file.as_raw_fd();
    let deadline = Instant::now() + total_timeout;
    let mut out = Vec::with_capacity(expected);
    let mut buf = [0u8; 4096];
    while out.len() < expected {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            let got = out.len();
            return (out, Some(format!("read timed out: got {got}/{expected} bytes within {total_timeout:?}")));
        }
        let millis = remaining.as_millis().min(i32::MAX as u128) as libc::c_int;
        let mut pfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
        let rc = unsafe { libc::poll(&mut pfd, 1, millis) };
        if rc == 0 {
            let got = out.len();
            return (out, Some(format!("read timed out: got {got}/{expected} bytes within {total_timeout:?}")));
        }
        if rc < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return (out, Some(format!("poll: {e}")));
        }
        let want = (expected - out.len()).min(buf.len());
        match file.read(&mut buf[..want]) {
            Ok(0) => {
                let got = out.len();
                return (out, Some(format!("EOF after {got}/{expected} bytes")));
            }
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return (out, Some(format!("read: {e}"))),
        }
    }
    (out, None)
}

fn wire_timeout(baud: u32, n_bytes: usize) -> Duration {
    // 10 bits per byte plus a 3× slack and a 1s floor.
    let secs = (n_bytes as f64 * 10.0 / baud as f64) * 3.0 + 1.0;
    Duration::from_secs_f64(secs)
}

fn diff_summary(expected: &[u8], got: &[u8]) -> String {
    if expected.len() != got.len() {
        return format!("length mismatch: expected {}, got {}", expected.len(), got.len());
    }
    for (i, (e, g)) in expected.iter().zip(got.iter()).enumerate() {
        if e != g {
            return format!("byte {i} differs: expected {:#04x}, got {:#04x}", e, g);
        }
    }
    "byte content differs but length matches (?)".into()
}

fn spawn_rb() -> Result<Child, String> {
    // macOS Homebrew installs lrzsz binaries with an `l` prefix
    // (lrb / lrx / lsb / lsx) to avoid colliding with system tools.
    // Linux distros keep the canonical names. Try both.
    let cmd = pick_binary(&["rb", "lrb"])?;
    spawn_receiver(cmd, &["-v"])
}

fn spawn_rx(flags: &[&str], dest: &Path) -> Result<Child, String> {
    let cmd = pick_binary(&["rx", "lrx"])?;
    let mut args: Vec<String> = flags.iter().map(|s| (*s).to_string()).collect();
    args.push(dest.to_string_lossy().into_owned());
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    spawn_receiver(cmd, &args_ref)
}

fn pick_binary(candidates: &[&'static str]) -> Result<&'static str, String> {
    for c in candidates {
        // Walk $PATH ourselves so we don't have to depend on `which`.
        if let Some(path_env) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path_env) {
                if dir.join(c).is_file() {
                    return Ok(c);
                }
            }
        }
    }
    Err(format!("none of {candidates:?} on $PATH — install lrzsz"))
}

fn spawn_receiver(cmd: &str, args: &[&str]) -> Result<Child, String> {
    let port = OpenOptions::new()
        .read(true)
        .write(true)
        .open(LINK_B)
        .map_err(|e| format!("open {LINK_B}: {e}"))?;
    let port_dup = port.try_clone().map_err(|e| format!("clone {LINK_B} fd: {e}"))?;
    Command::new(cmd)
        .args(args)
        .current_dir(RX_DIR)
        .stdin(Stdio::from(port))
        .stdout(Stdio::from(port_dup))
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn {cmd}: {e}"))
}

fn finish_with_receiver(
    id: &'static str,
    name: &'static str,
    baud: u32,
    start: Instant,
    inner: Result<(), String>,
    mut receiver: Child,
) -> TestOutcome {
    let wait_result = wait_with_timeout(&mut receiver, Duration::from_secs(20));
    match (inner, wait_result) {
        (Ok(()), Ok(_)) => TestOutcome::ok(id, name, baud, start, "ok".into()),
        (Ok(()), Err(e)) => TestOutcome::fail(id, name, baud, start, format!("receiver: {e}")),
        // Cancel test will SIGKILL the receiver since rb can't recover
        // from mid-stream CAN cleanly — that's intentional, not a fail.
        (Err(e), _) => TestOutcome::fail(id, name, baud, start, e),
    }
}

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> Result<std::process::ExitStatus, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("did not exit within {timeout:?}"));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(format!("try_wait: {e}")),
        }
    }
}

fn verify_received(source: &[u8], received_path: &Path, padded: bool) -> Result<(), String> {
    let received = std::fs::read(received_path)
        .map_err(|e| format!("read {}: {e}", received_path.display()))?;
    if padded {
        if received.len() < source.len() {
            return Err(format!(
                "received {} bytes, source is {}",
                received.len(),
                source.len(),
            ));
        }
        if &received[..source.len()] != source {
            return Err(diff_summary(source, &received[..source.len()]));
        }
        // Verify the tail is all 0x1A (SUB) padding for the XMODEM cases.
        if let Some(pad) = received.get(source.len()..) {
            if pad.iter().any(|&b| b != transfer::SUB) {
                return Err("XMODEM tail had non-SUB padding bytes".into());
            }
        }
    } else if received != source {
        return Err(diff_summary(source, &received));
    }
    Ok(())
}
