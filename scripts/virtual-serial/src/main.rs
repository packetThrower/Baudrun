//! Baud-throttled virtual serial pair for Baudrun dev testing.
//!
//! Creates two pty endpoints wired together with accurate inter-byte
//! timing for a configurable baud rate. Point Baudrun at one endpoint,
//! a test tool (xxd, lrzsz's rb / rx, etc.) at the other, and exercise
//! code paths that care about real-world baud-rate pacing — paste
//! safety, transfer progress, UART buffer behaviour — without a
//! physical USB-serial adapter.
//!
//! Unix only (macOS + Linux). Windows dev machines should pair two
//! virtual COM ports via com0com and talk to real hardware for the
//! baud-rate-sensitive tests; the pty primitive this tool builds on
//! has no Windows equivalent.
//!
//! See `README.md` for usage examples.

#![cfg(unix)]

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{symlink, OpenOptionsExt};
use std::os::unix::io::{FromRawFd, IntoRawFd};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use nix::fcntl::OFlag;
use nix::pty::{grantpt, posix_openpt, ptsname, unlockpt};
use nix::sys::termios::{cfmakeraw, tcgetattr, tcsetattr, SetArg};

fn main() {
    let args = Args::parse_or_exit();

    let (master_a, slave_a, slave_a_path) = open_pty_pair()
        .unwrap_or_else(|e| die(&format!("open pty A: {e}")));
    let (master_b, slave_b, slave_b_path) = open_pty_pair()
        .unwrap_or_else(|e| die(&format!("open pty B: {e}")));

    // Symlinks (best-effort cleanup on exit).
    let mut cleanup_links: Vec<String> = Vec::new();
    if let Some(link) = &args.link_a {
        let _ = std::fs::remove_file(link);
        if let Err(e) = symlink(&slave_a_path, link) {
            die(&format!("symlink A: {e}"));
        }
        cleanup_links.push(link.clone());
    }
    if let Some(link) = &args.link_b {
        let _ = std::fs::remove_file(link);
        if let Err(e) = symlink(&slave_b_path, link) {
            for l in &cleanup_links {
                let _ = std::fs::remove_file(l);
            }
            die(&format!("symlink B: {e}"));
        }
        cleanup_links.push(link.clone());
    }

    let byte_delay = Duration::from_secs_f64(args.bits as f64 / args.baud as f64);

    print_endpoint("Endpoint A", &slave_a_path, args.link_a.as_deref());
    print_endpoint("Endpoint B", &slave_b_path, args.link_b.as_deref());
    eprintln!(
        "Throttle:   {} baud, {} bits/byte → {} per byte",
        args.baud,
        args.bits,
        format_duration(byte_delay),
    );
    eprintln!("Ctrl+C to quit.");

    // Each direction needs read access to one master and write access
    // to the other. `try_clone` dup's the underlying fd so two threads
    // can hold independent handles without `&mut` contention.
    let master_a_reader = master_a
        .try_clone()
        .unwrap_or_else(|e| die(&format!("clone master A: {e}")));
    let master_b_writer = master_b
        .try_clone()
        .unwrap_or_else(|e| die(&format!("clone master B: {e}")));

    let _ta = thread::spawn(move || {
        throttle(master_a_reader, master_b_writer, byte_delay, "A→B");
    });
    let _tb = thread::spawn(move || {
        throttle(master_b, master_a, byte_delay, "B→A");
    });

    // Block until SIGINT / SIGTERM. `ctrlc` installs a single
    // signal-trampoline thread; the channel send wakes us up here.
    let (tx, rx) = mpsc::channel::<()>();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })
    .unwrap_or_else(|e| die(&format!("install signal handler: {e}")));

    let _ = rx.recv();
    eprintln!();
    eprintln!("Shutting down.");

    // Keep the slaves alive until shutdown — closing them earlier
    // would let a momentary disconnect (Baudrun reconnects between
    // sessions, say) tear the pty pair down.
    drop(slave_a);
    drop(slave_b);

    for link in cleanup_links {
        let _ = std::fs::remove_file(link);
    }
}

/// Open one pty pair, raw-mode the slave, and return both halves
/// plus the slave's filesystem path (`/dev/ttys007` etc.). The slave
/// is held open by the bridge for the lifetime of main — if every
/// process holding the slave closes it, the master gets EOF on read
/// and the pair collapses.
///
/// `posix_openpt` returns the master only; the slave path is
/// discovered via `ptsname_r` and then opened with `O_NOCTTY` so this
/// process doesn't accidentally take it as its controlling terminal.
fn open_pty_pair() -> Result<(File, File, String), String> {
    let master_pty = posix_openpt(OFlag::O_RDWR | OFlag::O_NOCTTY)
        .map_err(|e| format!("posix_openpt: {e}"))?;
    grantpt(&master_pty).map_err(|e| format!("grantpt: {e}"))?;
    unlockpt(&master_pty).map_err(|e| format!("unlockpt: {e}"))?;
    // SAFETY: `ptsname` uses a static buffer and is not thread-safe.
    // We call it exactly twice (once per pty pair) at startup before
    // either throttle thread is spawned, so the racy buffer is only
    // ever read by the main thread. `ptsname_r` would be safer but
    // is Linux-only; macOS exposes only the legacy `ptsname`.
    let slave_path =
        unsafe { ptsname(&master_pty) }.map_err(|e| format!("ptsname: {e}"))?;

    let slave = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY)
        .open(&slave_path)
        .map_err(|e| format!("open slave {slave_path}: {e}"))?;

    // Default pty line discipline is canonical (ICANON, ECHO, OPOST
    // on), which line-buffers bytes moving from slave-writer to
    // master-reader — a plain `cat` or `xxd` reading the slave would
    // only see bytes after each newline. Any app that opens the port
    // with `serialport-rs` (like Baudrun itself) resets termios on
    // its own, but generic Unix tools on the *other* endpoint
    // benefit from raw-by-default.
    let mut termios = tcgetattr(&slave).map_err(|e| format!("tcgetattr: {e}"))?;
    cfmakeraw(&mut termios);
    tcsetattr(&slave, SetArg::TCSANOW, &termios)
        .map_err(|e| format!("tcsetattr: {e}"))?;

    // Convert the master into a plain `File` for ergonomic std::io.
    // PtyMaster wraps an `OwnedFd` internally; `into_raw_fd` peels
    // it off without running PtyMaster's destructor, and
    // `File::from_raw_fd` takes ownership for the eventual close.
    let master_fd = master_pty.into_raw_fd();
    // SAFETY: we just took exclusive ownership of master_fd from
    // PtyMaster::into_raw_fd, which guarantees no other handle is
    // live for that fd.
    let master_file = unsafe { File::from_raw_fd(master_fd) };

    Ok((master_file, slave, slave_path))
}

/// Copy bytes from `src` to `dst` one at a time, sleeping `delay`
/// between each write. Reads are chunked for efficiency; only the
/// writes are paced. A real UART would take exactly `delay` to clock
/// each byte onto the wire, so this approximates that behaviour at
/// the application layer.
fn throttle(mut src: File, mut dst: File, delay: Duration, tag: &str) {
    let mut buf = [0u8; 4096];
    loop {
        let n = match src.read(&mut buf) {
            Ok(0) => return, // EOF — the peer dropped the master
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                eprintln!("[{tag}] read: {e}");
                return;
            }
        };
        for byte in &buf[..n] {
            thread::sleep(delay);
            if let Err(e) = dst.write_all(std::slice::from_ref(byte)) {
                eprintln!("[{tag}] write: {e}");
                return;
            }
        }
    }
}

fn print_endpoint(label: &str, tty: &str, link: Option<&str>) {
    match link {
        None => eprintln!("{label}: {tty}"),
        Some(l) => eprintln!("{label}: {tty} (→ {l})"),
    }
}

/// Format a Duration as the closest of `Nµs`, `N.NNNms`, or `N.NNNs`,
/// matching the Go `time.Duration.Round(time.Microsecond)` style the
/// README and TESTING.md transcripts assume.
fn format_duration(d: Duration) -> String {
    let us = d.as_micros();
    if us < 1_000 {
        format!("{us}µs")
    } else if us < 1_000_000 {
        format!("{:.3}ms", d.as_secs_f64() * 1000.0)
    } else {
        format!("{:.3}s", d.as_secs_f64())
    }
}

struct Args {
    baud: u32,
    bits: u32,
    link_a: Option<String>,
    link_b: Option<String>,
}

impl Args {
    fn parse_or_exit() -> Self {
        let mut args = Args {
            baud: 9600,
            bits: 10,
            link_a: None,
            link_b: None,
        };
        let mut argv = std::env::args().skip(1);
        while let Some(arg) = argv.next() {
            match arg.as_str() {
                "-baud" | "--baud" => {
                    args.baud = next_value(&mut argv, "-baud")
                        .parse()
                        .unwrap_or_else(|_| die("baud must be a positive integer"));
                }
                "-bits" | "--bits" => {
                    args.bits = next_value(&mut argv, "-bits")
                        .parse()
                        .unwrap_or_else(|_| die("bits must be a positive integer"));
                }
                "-link-a" | "--link-a" => {
                    args.link_a = Some(next_value(&mut argv, "-link-a"));
                }
                "-link-b" | "--link-b" => {
                    args.link_b = Some(next_value(&mut argv, "-link-b"));
                }
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                other => {
                    eprintln!("unknown flag: {other}");
                    print_help();
                    std::process::exit(1);
                }
            }
        }
        if args.baud == 0 || args.bits == 0 {
            die("baud and bits must be positive");
        }
        args
    }
}

fn next_value(argv: &mut impl Iterator<Item = String>, flag: &str) -> String {
    argv.next()
        .unwrap_or_else(|| die(&format!("missing value for {flag}")))
}

fn die(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(1);
}

fn print_help() {
    eprintln!("virtual-serial — baud-throttled virtual serial pair for Baudrun dev testing");
    eprintln!();
    eprintln!("usage: virtual-serial [flags]");
    eprintln!();
    eprintln!("flags:");
    eprintln!("  -baud N         baud rate to simulate (default 9600)");
    eprintln!("  -bits N         bits per byte including start/parity/stop (default 10 = 8N1)");
    eprintln!("  -link-a PATH    optional stable symlink pointing at endpoint A");
    eprintln!("  -link-b PATH    optional stable symlink pointing at endpoint B");
    eprintln!("  -h, --help      show this help");
}
