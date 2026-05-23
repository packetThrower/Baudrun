//! Headless test harness for Baudrun's hex / XMODEM / YMODEM send paths.
//!
//! Drives the protocols against the `virtual-serial` bridge with
//! deterministic fixtures in `test/transfers/`, then byte-diffs the
//! received output against the source. One command, one verdict per
//! test (T1, T3, T5, T6, T7-c/C/k, T8, T9, T10, T11 from TESTING.md).
//!
//! Owns the bridge subprocess for the lifetime of the run, restarts
//! it between baud-rate groups, and tears down cleanly on Ctrl-C.
//!
//! Unix only — the bridge produces pty endpoints under `/tmp/`.
//! Windows dev machines should run the manual playbook in TESTING.md.

#![cfg(unix)]

// Lift Baudrun's protocol code verbatim. The path is relative to this
// source file (scripts/transfer-tests/src/main.rs); resolves to the
// repo's src/data/transfer.rs. Two compilers compile the same file:
// the main Baudrun crate ships its own copy as `crate::data::transfer`,
// this harness ships its own copy as `crate::transfer`. No runtime
// coupling, no symbol clashes, no drift — refactors to the source
// file are seen by both sites at the next `cargo build`.
//
// `dead_code` is suppressed on the module because the lifted file
// includes ChannelReader (used by main Baudrun but not here — we use
// `PollReader` instead, see port_reader.rs for why), plus a few helper
// constants the harness happens not to reference yet.
#[allow(dead_code)]
#[path = "../../../src/data/transfer.rs"]
mod transfer;

// Same #[path] trick for the hex parser. Lives in `src/data/hex.rs`
// for the same reason as transfer.rs — pure data, no UI deps. Lifted
// here so the harness uses the exact code Send-Hex ships, not a
// drift-prone copy.
#[path = "../../../src/data/hex.rs"]
mod hex;

mod bridge;
mod port_reader;
mod tests;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bridge::Bridge;
use tests::TestOutcome;

pub const LINK_A: &str = "/tmp/baudrun-a";
pub const LINK_B: &str = "/tmp/baudrun-b";
pub const RX_DIR: &str = "/tmp/baudrun-rx";

const FIXTURE_HEX_1K: &str = "test/transfers/hex-1k.txt";
const FIXTURE_PAYLOAD_4K: &str = "test/transfers/payload-4k.bin";
const FIXTURE_PAYLOAD_1M: &str = "test/transfers/payload-1m.bin";
const FIXTURE_PAYLOAD_512: &str = "test/transfers/payload-512.bin";
const FIXTURE_TINY: &str = "test/transfers/tiny.txt";

fn main() -> ExitCode {
    let args = match Args::parse_or_exit() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    if let Err(e) = check_fixtures() {
        eprintln!("error: {e}");
        eprintln!("hint:  run `./scripts/transfer-tests/regen-fixtures.sh` from the repo root.");
        return ExitCode::from(2);
    }
    if let Err(e) = tests::ensure_rx_dir() {
        eprintln!("error: {e}");
        return ExitCode::from(2);
    }

    let bridge_bin = match resolve_bridge_bin(args.bridge_bin.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!(
                "hint:  build it with `(cd scripts/virtual-serial && cargo build --release)`"
            );
            return ExitCode::from(2);
        }
    };

    // Trap SIGINT so the bridge subprocess gets killed cleanly on
    // user-initiated abort. Without this, Ctrl-C leaves a zombie
    // virtual-serial holding /tmp/baudrun-* symlinks open.
    let abort = Arc::new(AtomicBool::new(false));
    {
        let a = Arc::clone(&abort);
        let _ = ctrlc::set_handler(move || a.store(true, Ordering::SeqCst));
    }

    let plan = build_plan(&args);
    if plan.is_empty() {
        eprintln!("no tests selected — pass --tests=all or a subset");
        return ExitCode::from(2);
    }

    println!(
        "running {} test(s)",
        plan.iter().map(|g| g.cases.len()).sum::<usize>()
    );
    let mut outcomes: Vec<TestOutcome> = Vec::new();
    let mut bridge: Option<Bridge> = None;

    'outer: for group in &plan {
        match &mut bridge {
            None => match Bridge::start(bridge_bin.clone(), group.baud, args.verbose) {
                Ok(b) => bridge = Some(b),
                Err(e) => {
                    eprintln!("bridge start: {e}");
                    return ExitCode::from(2);
                }
            },
            Some(b) if b.baud != group.baud => {
                if let Err(e) = b.restart(group.baud) {
                    eprintln!("bridge restart: {e}");
                    return ExitCode::from(2);
                }
            }
            Some(_) => {}
        }
        // Pause briefly after a (re)start so the bridge's reader
        // threads are ready before we send the first byte.
        std::thread::sleep(Duration::from_millis(100));

        for case in &group.cases {
            if abort.load(Ordering::SeqCst) {
                eprintln!("\naborted by user");
                break 'outer;
            }
            print!("  {} {:<40} ", case.id, case.name);
            let _ = std::io::Write::flush(&mut std::io::stdout());
            let outcome = (case.run)();
            print_outcome(&outcome);
            outcomes.push(outcome);
        }
    }

    // Drop the bridge before printing the summary so the cleanup
    // chatter (if any) doesn't interleave with the verdict.
    drop(bridge);

    print_summary(&outcomes);

    if outcomes.iter().any(|o| !o.passed) {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

// -- CLI --------------------------------------------------------------

struct Args {
    quick: bool,
    verbose: bool,
    bridge_bin: Option<PathBuf>,
    selected: TestSelection,
}

#[derive(Default)]
struct TestSelection {
    hex: bool,
    ymodem: bool,
    xmodem: bool,
    cancel: bool,
}

impl TestSelection {
    fn all() -> Self {
        Self {
            hex: true,
            ymodem: true,
            xmodem: true,
            cancel: true,
        }
    }
    fn any(&self) -> bool {
        self.hex || self.ymodem || self.xmodem || self.cancel
    }
}

impl Args {
    fn parse_or_exit() -> Result<Self, String> {
        let mut args = Args {
            quick: false,
            verbose: false,
            bridge_bin: None,
            selected: TestSelection::default(),
        };
        let mut argv = std::env::args().skip(1);
        while let Some(arg) = argv.next() {
            match arg.as_str() {
                "--quick" => args.quick = true,
                "--verbose" | "-v" => args.verbose = true,
                "--bridge" => {
                    let v = argv.next().ok_or("missing value for --bridge")?;
                    args.bridge_bin = Some(PathBuf::from(v));
                }
                "--tests" => {
                    let v = argv.next().ok_or("missing value for --tests")?;
                    for piece in v.split(',') {
                        match piece.trim() {
                            "all" => args.selected = TestSelection::all(),
                            "hex" => args.selected.hex = true,
                            "ymodem" => args.selected.ymodem = true,
                            "xmodem" => args.selected.xmodem = true,
                            "cancel" => args.selected.cancel = true,
                            other => return Err(format!("unknown test group: {other}")),
                        }
                    }
                }
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                other => return Err(format!("unknown flag: {other}")),
            }
        }
        if !args.selected.any() {
            args.selected = TestSelection::all();
        }
        Ok(args)
    }
}

fn print_help() {
    eprintln!("transfer-tests — automated wire-level checks for Baudrun's send paths");
    eprintln!();
    eprintln!("usage: transfer-tests [flags]");
    eprintln!();
    eprintln!("flags:");
    eprintln!("  --tests LIST    comma-separated subset of: all,hex,ymodem,xmodem,cancel");
    eprintln!("                  (default: all)");
    eprintln!("  --quick         skip T9 (1 MiB transfer, ~91s at 115200 baud)");
    eprintln!("  --bridge PATH   path to virtual-serial binary (default: search relative");
    eprintln!("                  to this binary, then scripts/virtual-serial/target/release/)");
    eprintln!("  --verbose, -v   pass bridge subprocess stderr through to this terminal");
    eprintln!("  -h, --help      show this help");
}

// -- Plan -------------------------------------------------------------

struct TestCase {
    id: &'static str,
    name: &'static str,
    run: Box<dyn Fn() -> TestOutcome + Send + Sync>,
}

struct BaudGroup {
    baud: u32,
    cases: Vec<TestCase>,
}

fn build_plan(args: &Args) -> Vec<BaudGroup> {
    let s = &args.selected;
    let mut group_9600 = BaudGroup {
        baud: 9600,
        cases: Vec::new(),
    };
    let mut group_115200 = BaudGroup {
        baud: 115200,
        cases: Vec::new(),
    };

    let hex_1k_path = PathBuf::from(FIXTURE_HEX_1K);
    let payload_4k = PathBuf::from(FIXTURE_PAYLOAD_4K);
    let payload_1m = PathBuf::from(FIXTURE_PAYLOAD_1M);
    let payload_512 = PathBuf::from(FIXTURE_PAYLOAD_512);
    let tiny = PathBuf::from(FIXTURE_TINY);

    if s.hex {
        group_9600.cases.push(TestCase {
            id: "T1",
            name: "hex ASCII (Hello)",
            run: Box::new(tests::hex_ascii),
        });
        group_9600.cases.push(TestCase {
            id: "T3",
            name: "hex binary / non-printable",
            run: Box::new(tests::hex_binary),
        });
        let p = hex_1k_path.clone();
        group_9600.cases.push(TestCase {
            id: "T5",
            name: "hex 1 KiB",
            run: Box::new(move || tests::hex_1k(&p)),
        });
    }
    if s.ymodem {
        let p = payload_512.clone();
        group_9600.cases.push(TestCase {
            id: "T11",
            name: "YMODEM 512 B over slow link",
            run: Box::new(move || tests::ymodem_slow_512(&p)),
        });
        let p = payload_4k.clone();
        group_115200.cases.push(TestCase {
            id: "T6",
            name: "YMODEM 4 KiB",
            run: Box::new(move || tests::ymodem_4k(&p)),
        });
        if !args.quick {
            let p = payload_1m.clone();
            group_115200.cases.push(TestCase {
                id: "T9",
                name: "YMODEM 1 MiB (~91s)",
                run: Box::new(move || tests::ymodem_1m(&p)),
            });
        }
    }
    if s.xmodem {
        let p = payload_4k.clone();
        group_115200.cases.push(TestCase {
            id: "T7c",
            name: "XMODEM classic (128/checksum)",
            run: Box::new(move || tests::xmodem_classic(&p)),
        });
        let p = payload_4k.clone();
        group_115200.cases.push(TestCase {
            id: "T7C",
            name: "XMODEM-CRC (128/CRC-16)",
            run: Box::new(move || tests::xmodem_crc(&p)),
        });
        let p = payload_4k.clone();
        group_115200.cases.push(TestCase {
            id: "T7k",
            name: "XMODEM-1K (1024/CRC-16)",
            run: Box::new(move || tests::xmodem_1k(&p)),
        });
        let p = tiny.clone();
        group_115200.cases.push(TestCase {
            id: "T8",
            name: "XMODEM single-block (SUB padding)",
            run: Box::new(move || tests::xmodem_tiny(&p)),
        });
    }
    if s.cancel {
        let p = payload_1m.clone();
        group_115200.cases.push(TestCase {
            id: "T10",
            name: "cancel mid-transfer",
            run: Box::new(move || tests::cancel_midway(&p)),
        });
    }

    let mut plan = Vec::new();
    if !group_9600.cases.is_empty() {
        plan.push(group_9600);
    }
    if !group_115200.cases.is_empty() {
        plan.push(group_115200);
    }
    plan
}

// -- Output -----------------------------------------------------------

fn print_outcome(o: &TestOutcome) {
    let status = if o.passed { "ok  " } else { "FAIL" };
    println!(
        "{status}  ({:>6.2}s, {} baud) {}",
        o.duration.as_secs_f64(),
        o.baud,
        o.detail
    );
}

fn print_summary(outcomes: &[TestOutcome]) {
    let pass = outcomes.iter().filter(|o| o.passed).count();
    let fail = outcomes.iter().filter(|o| !o.passed).count();
    let total = outcomes.len();
    let wall: f64 = outcomes.iter().map(|o| o.duration.as_secs_f64()).sum();
    println!();
    if fail == 0 {
        println!("result: ok. {pass}/{total} passed ({wall:.1}s wall)");
    } else {
        println!("result: FAILED. {pass}/{total} passed, {fail} failed ({wall:.1}s wall)");
        for o in outcomes.iter().filter(|o| !o.passed) {
            println!("  {} {}: {}", o.id, o.name, o.detail);
        }
    }
}

// -- Bridge binary discovery -----------------------------------------

fn resolve_bridge_bin(explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(p) = explicit {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        return Err(format!("--bridge path does not exist: {}", p.display()));
    }
    // Search relative to argv[0], then relative to cwd.
    let candidates = [
        // sibling to this binary, under the virtual-serial workspace
        "../../virtual-serial/target/release/virtual-serial",
        // from repo root
        "scripts/virtual-serial/target/release/virtual-serial",
    ];
    let exe = std::env::current_exe().ok();
    for c in candidates {
        let rel = Path::new(c);
        // Try relative to the binary's parent.
        if let Some(exe) = &exe {
            if let Some(dir) = exe.parent() {
                let cand = dir.join(rel);
                if cand.exists() {
                    return Ok(cand);
                }
            }
        }
        // Try relative to cwd.
        if rel.exists() {
            return Ok(rel.to_path_buf());
        }
    }
    Err("could not find the virtual-serial binary".into())
}

// -- Setup checks -----------------------------------------------------

fn check_fixtures() -> Result<(), String> {
    let required = [
        FIXTURE_HEX_1K,
        FIXTURE_PAYLOAD_4K,
        FIXTURE_PAYLOAD_1M,
        FIXTURE_PAYLOAD_512,
        FIXTURE_TINY,
    ];
    for f in required {
        if !Path::new(f).exists() {
            return Err(format!("missing fixture: {f}"));
        }
    }
    Ok(())
}
