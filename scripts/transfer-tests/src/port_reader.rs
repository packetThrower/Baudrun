//! `TransferReader` impl backed directly by a raw fd + `libc::poll`.
//!
//! No forwarder thread. Why: the production Baudrun's `ChannelReader`
//! works because the read pump is owned by the long-lived serial
//! session — its thread exists for the lifetime of the connection.
//! The harness runs many short-lived tests against the same pty, and
//! if each test spawned a forwarder thread that thread would leak on
//! function exit (still blocked in `read(2)` with no way to wake it
//! without either signals or self-pipe machinery). The next test's
//! fresh forwarder would then race the zombie for the first byte off
//! the shared slave fd, and the zombie wins about 50% of the time —
//! a deterministic off-by-one byte loss we observed during bring-up.
//!
//! `poll` + `read` from the calling thread keeps the read lifetime
//! tied to the test function's scope: when the function returns, the
//! `File` drops, the fd closes, and there's nothing left to race.

use std::os::fd::RawFd;
use std::time::Duration;

use crate::transfer::{self, TransferReader};

pub struct PollReader {
    fd: RawFd,
}

impl PollReader {
    pub fn new(fd: RawFd) -> Self {
        Self { fd }
    }
}

impl TransferReader for PollReader {
    fn next_byte(&mut self, timeout: Duration) -> transfer::Result<u8> {
        let millis = timeout.as_millis().min(i32::MAX as u128) as libc::c_int;
        loop {
            let mut pfd = libc::pollfd {
                fd: self.fd,
                events: libc::POLLIN,
                revents: 0,
            };
            let rc = unsafe { libc::poll(&mut pfd, 1, millis) };
            if rc == 0 {
                return Err(transfer::TransferError::Timeout);
            }
            if rc < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(transfer::TransferError::Io(e));
            }
            // POLLHUP / POLLERR may set with no readable bytes.
            if pfd.revents & libc::POLLIN == 0 {
                if pfd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
                    return Err(transfer::TransferError::Closed);
                }
                continue;
            }
            let mut buf = [0u8; 1];
            let n = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, 1) };
            if n == 1 {
                return Ok(buf[0]);
            }
            if n == 0 {
                return Err(transfer::TransferError::Closed);
            }
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(transfer::TransferError::Io(e));
        }
    }
}
