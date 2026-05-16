//! Manage the `virtual-serial` subprocess that bridges the two pty
//! endpoints. The harness owns the bridge's lifetime — spawning a
//! fresh instance per baud-rate group, killing it cleanly between
//! groups, and removing the symlinks it leaves in `/tmp/`.
//!
//! Why not assume an externally-running bridge? Because a clean test
//! run needs a known baud rate, and bumping it between T11 (9600) and
//! T9 (115200) requires a restart anyway. Owning the subprocess keeps
//! the harness self-contained: one command, no setup choreography.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::{LINK_A, LINK_B};

pub struct Bridge {
    bin: PathBuf,
    child: Option<Child>,
    pub baud: u32,
    pub verbose: bool,
}

impl Bridge {
    pub fn start(bin: PathBuf, baud: u32, verbose: bool) -> Result<Self, String> {
        let mut b = Self {
            bin,
            child: None,
            baud,
            verbose,
        };
        b.spawn()?;
        Ok(b)
    }

    /// Restart at a new baud rate. Existing pty handles held by the
    /// caller MUST be dropped first — the masters get EOF when this
    /// kills the child, and the new symlinks point at different
    /// /dev/ttys paths.
    pub fn restart(&mut self, baud: u32) -> Result<(), String> {
        self.stop();
        self.baud = baud;
        self.spawn()
    }

    fn spawn(&mut self) -> Result<(), String> {
        // Wipe any stale symlinks before launching — left over from a
        // SIGKILL'd previous run.
        let _ = std::fs::remove_file(LINK_A);
        let _ = std::fs::remove_file(LINK_B);

        let stdio = || {
            if self.verbose {
                Stdio::inherit()
            } else {
                Stdio::null()
            }
        };
        let child = Command::new(&self.bin)
            .args([
                "-baud",
                &self.baud.to_string(),
                "-link-a",
                LINK_A,
                "-link-b",
                LINK_B,
            ])
            .stdout(stdio())
            .stderr(stdio())
            .spawn()
            .map_err(|e| format!("spawn {}: {e}", self.bin.display()))?;
        self.child = Some(child);

        wait_for_link(Path::new(LINK_A), Duration::from_secs(5))?;
        wait_for_link(Path::new(LINK_B), Duration::from_secs(5))?;
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_file(LINK_A);
        let _ = std::fs::remove_file(LINK_B);
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop();
    }
}

fn wait_for_link(path: &Path, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        // `symlink_metadata` doesn't follow the link, so it returns Ok
        // as soon as the symlink itself exists — before the target
        // /dev/ttys path is necessarily readable. That's fine because
        // virtual-serial creates the symlink only after grantpt /
        // unlockpt have made the slave openable.
        if std::fs::symlink_metadata(path).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!("timed out waiting for {}", path.display()))
}
