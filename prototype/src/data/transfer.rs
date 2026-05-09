//! XMODEM and YMODEM file-transfer protocols over a byte stream.
//! ZMODEM is deliberately not implemented — it's a much larger state
//! machine and most embedded bootloader targets don't speak it.
//!
//! Callers supply a [`TransferReader`] for inbound bytes (typically a
//! [`ChannelReader`] fed from [`crate::serial::Session::start_transfer`])
//! and any `std::io::Write` implementor for outbound.

use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use thiserror::Error;

// ASCII control codes.
pub const SOH: u8 = 0x01; // 128-byte block header
pub const STX: u8 = 0x02; // 1024-byte block header
pub const EOT: u8 = 0x04;
pub const ACK: u8 = 0x06;
pub const NAK: u8 = 0x15;
pub const CAN: u8 = 0x18;
pub const SUB: u8 = 0x1A; // filler byte
pub const CRC_REQ: u8 = 0x43; // ASCII 'C' — receiver requests CRC mode

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(60);
const ACK_TIMEOUT: Duration = Duration::from_secs(10);
const BLOCK_RETRIES: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XModemVariant {
    /// Original XMODEM: 128-byte blocks, 8-bit checksum. Receiver
    /// starts with NAK. Deprecated for new designs but still present
    /// in some older ROMs.
    Classic,
    /// 128-byte blocks, CRC-16. Receiver starts with 'C'.
    Crc,
    /// 1024-byte blocks, CRC-16 (sometimes called XMODEM-1K or YAM).
    /// Receiver starts with 'C'.
    OneKilo,
}

#[derive(Debug, Error)]
pub enum TransferError {
    #[error("transfer cancelled")]
    Cancelled,
    #[error("transfer timeout")]
    Timeout,
    #[error("block {block}: max retries")]
    MaxRetries { block: u16 },
    #[error("EOT not acknowledged")]
    EotNotAcked,
    #[error("transfer channel closed")]
    Closed,
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub type Result<T> = std::result::Result<T, TransferError>;

/// Byte-granularity reader with per-byte timeout. The transfer state
/// machines need to distinguish ACK/NAK/CAN, which `io::Read`'s
/// chunk API doesn't expose cleanly — hence the custom trait.
pub trait TransferReader {
    fn next_byte(&mut self, timeout: Duration) -> Result<u8>;
}

/// Progress callback: (sent_bytes, total_bytes).
pub type ProgressFn = Arc<dyn Fn(u64, u64) + Send + Sync>;

/// Optional knobs for a transfer. All fields are optional.
#[derive(Default, Clone)]
pub struct Options {
    pub progress: Option<ProgressFn>,
    pub cancel: Option<Arc<AtomicBool>>,
}

/// Channel-backed reader. The session's transfer RX callback sends
/// byte chunks; this reader buffers them and serves one byte at a
/// time to the protocol state machine.
pub struct ChannelReader {
    rx: Receiver<Vec<u8>>,
    pending: VecDeque<u8>,
}

impl ChannelReader {
    pub fn new(rx: Receiver<Vec<u8>>) -> Self {
        Self {
            rx,
            pending: VecDeque::new(),
        }
    }
}

impl TransferReader for ChannelReader {
    fn next_byte(&mut self, timeout: Duration) -> Result<u8> {
        if let Some(b) = self.pending.pop_front() {
            return Ok(b);
        }
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(TransferError::Timeout);
            }
            match self.rx.recv_timeout(remaining) {
                Ok(chunk) if chunk.is_empty() => continue,
                Ok(chunk) => {
                    self.pending.extend(chunk);
                    // SAFETY: we just pushed at least one byte.
                    return Ok(self.pending.pop_front().expect("non-empty after extend"));
                }
                Err(RecvTimeoutError::Timeout) => return Err(TransferError::Timeout),
                Err(RecvTimeoutError::Disconnected) => return Err(TransferError::Closed),
            }
        }
    }
}

/// Send `data` via XMODEM / XMODEM-CRC / XMODEM-1K. Blocks until
/// completion, cancellation, or retry exhaustion. Progress is
/// reported after each ACKed block.
pub fn send_xmodem<R, W>(
    r: &mut R,
    w: &mut W,
    data: &[u8],
    variant: XModemVariant,
    opts: &Options,
) -> Result<()>
where
    R: TransferReader,
    W: Write,
{
    let use_crc = variant != XModemVariant::Classic;
    let (block_size, header) = match variant {
        XModemVariant::OneKilo => (1024usize, STX),
        _ => (128usize, SOH),
    };

    wait_for_handshake(r, variant)?;

    let total = data.len() as u64;
    let mut block_num: u8 = 1;
    let mut offset = 0;
    while offset < data.len() {
        if cancelled(opts) {
            abort(w);
            return Err(TransferError::Cancelled);
        }
        let end = (offset + block_size).min(data.len());
        match send_block(r, w, header, block_num, &data[offset..end], block_size, use_crc) {
            Ok(()) => {}
            Err(err) => {
                abort(w);
                return Err(err);
            }
        }
        if let Some(cb) = &opts.progress {
            cb(end as u64, total);
        }
        offset = end;
        block_num = block_num.wrapping_add(1);
    }

    send_eot(r, w)
}

/// Send a single file via YMODEM (XMODEM-1K data blocks framed by a
/// block 0 header carrying filename + size, plus a terminating empty
/// block 0). Multi-file batch transfers are intentionally not
/// supported — one file per call keeps the UI and progress model
/// simple.
pub fn send_ymodem<R, W>(
    r: &mut R,
    w: &mut W,
    filename: &str,
    data: &[u8],
    opts: &Options,
) -> Result<()>
where
    R: TransferReader,
    W: Write,
{
    await_c(r)?;

    let header = build_ymodem_header(filename, data.len() as u64);
    send_block(r, w, STX, 0, &header, 1024, true).inspect_err(|_| abort(w))?;

    await_c(r)?;

    let total = data.len() as u64;
    const BLOCK_SIZE: usize = 1024;
    let mut block_num: u8 = 1;
    let mut offset = 0;
    while offset < data.len() {
        if cancelled(opts) {
            abort(w);
            return Err(TransferError::Cancelled);
        }
        let end = (offset + BLOCK_SIZE).min(data.len());
        if let Err(err) = send_block(r, w, STX, block_num, &data[offset..end], BLOCK_SIZE, true) {
            abort(w);
            return Err(err);
        }
        if let Some(cb) = &opts.progress {
            cb(end as u64, total);
        }
        offset = end;
        block_num = block_num.wrapping_add(1);
    }

    send_eot(r, w)?;

    // Terminating empty header block — tells receiver the batch (of
    // one file) is complete.
    await_c(r)?;
    let empty = [0u8; 128];
    send_block(r, w, SOH, 0, &empty, 128, true)
}

fn wait_for_handshake<R: TransferReader>(r: &mut R, variant: XModemVariant) -> Result<()> {
    let want = if variant == XModemVariant::Classic {
        NAK
    } else {
        CRC_REQ
    };
    let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
    while Instant::now() < deadline {
        match r.next_byte(Duration::from_secs(1)) {
            Ok(b) if b == want => return Ok(()),
            Ok(b) if b == CAN => {
                if drain_can(r) {
                    return Err(TransferError::Cancelled);
                }
            }
            Ok(_) => {}
            Err(TransferError::Timeout) => continue,
            Err(err) => return Err(err),
        }
    }
    Err(TransferError::Timeout)
}

fn await_c<R: TransferReader>(r: &mut R) -> Result<()> {
    let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
    while Instant::now() < deadline {
        match r.next_byte(Duration::from_secs(1)) {
            Ok(b) if b == CRC_REQ => return Ok(()),
            Ok(b) if b == CAN => {
                if drain_can(r) {
                    return Err(TransferError::Cancelled);
                }
            }
            Ok(_) => {}
            Err(TransferError::Timeout) => continue,
            Err(err) => return Err(err),
        }
    }
    Err(TransferError::Timeout)
}

fn send_block<R, W>(
    r: &mut R,
    w: &mut W,
    header: u8,
    block_num: u8,
    chunk: &[u8],
    block_size: usize,
    use_crc: bool,
) -> Result<()>
where
    R: TransferReader,
    W: Write,
{
    let mut padded = vec![SUB; block_size];
    padded[..chunk.len()].copy_from_slice(chunk);

    let mut packet = Vec::with_capacity(block_size + 5);
    packet.push(header);
    packet.push(block_num);
    packet.push(!block_num);
    packet.extend_from_slice(&padded);
    if use_crc {
        let c = crc16_xmodem(&padded);
        packet.push((c >> 8) as u8);
        packet.push((c & 0xff) as u8);
    } else {
        let mut sum: u8 = 0;
        for b in &padded {
            sum = sum.wrapping_add(*b);
        }
        packet.push(sum);
    }

    for _ in 0..BLOCK_RETRIES {
        w.write_all(&packet)?;
        match r.next_byte(ACK_TIMEOUT) {
            Ok(ACK) => return Ok(()),
            Ok(NAK) => continue,
            Ok(CAN) => {
                if drain_can(r) {
                    return Err(TransferError::Cancelled);
                }
            }
            Ok(_) => {}
            Err(TransferError::Timeout) => continue,
            Err(err) => return Err(err),
        }
    }
    Err(TransferError::MaxRetries {
        block: block_num as u16,
    })
}

fn send_eot<R, W>(r: &mut R, w: &mut W) -> Result<()>
where
    R: TransferReader,
    W: Write,
{
    for _ in 0..BLOCK_RETRIES {
        w.write_all(&[EOT])?;
        match r.next_byte(ACK_TIMEOUT) {
            Ok(ACK) => return Ok(()),
            Ok(_) => {}
            Err(TransferError::Timeout) => continue,
            Err(err) => return Err(err),
        }
    }
    Err(TransferError::EotNotAcked)
}

fn build_ymodem_header(filename: &str, size: u64) -> Vec<u8> {
    let mut h = Vec::with_capacity(filename.len() + 16);
    h.extend_from_slice(filename.as_bytes());
    h.push(0);
    h.extend_from_slice(format!("{} 0 0 0", size).as_bytes());
    h.push(0);
    h
}

fn cancelled(opts: &Options) -> bool {
    opts.cancel
        .as_ref()
        .is_some_and(|flag| flag.load(Ordering::Acquire))
}

fn abort<W: Write>(w: &mut W) {
    let _ = w.write_all(&[CAN, CAN, CAN, CAN, CAN]);
}

/// A single CAN can be line noise; two in a row is the receiver
/// signalling abort. Returns true if the second CAN was observed.
fn drain_can<R: TransferReader>(r: &mut R) -> bool {
    matches!(r.next_byte(Duration::from_millis(500)), Ok(CAN))
}

/// CRC-16 CCITT, polynomial 0x1021, seed 0x0000.
pub fn crc16_xmodem(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &b in data {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    struct MockReader {
        queue: VecDeque<u8>,
    }

    impl MockReader {
        fn new(bytes: &[u8]) -> Self {
            Self {
                queue: bytes.iter().copied().collect(),
            }
        }
    }

    impl TransferReader for MockReader {
        fn next_byte(&mut self, _timeout: Duration) -> Result<u8> {
            self.queue
                .pop_front()
                .ok_or(TransferError::Timeout)
        }
    }

    #[test]
    fn crc16_matches_known_vectors() {
        // Classic test vector: ASCII "123456789" → 0x31C3.
        assert_eq!(crc16_xmodem(b"123456789"), 0x31C3);
        // All zeros → 0.
        assert_eq!(crc16_xmodem(&[0u8; 128]), 0x0000);
    }

    #[test]
    fn ymodem_header_layout() {
        let h = build_ymodem_header("foo.bin", 2048);
        // "foo.bin\0" + "2048 0 0 0" + "\0"
        let mut expected: Vec<u8> = b"foo.bin\0".to_vec();
        expected.extend_from_slice(b"2048 0 0 0");
        expected.push(0);
        assert_eq!(h, expected);
    }

    #[test]
    fn xmodem_classic_sends_all_blocks() {
        // Receiver sends NAK (init), then ACK for each block + final
        // EOT ACK. 200 bytes of data = two 128-byte blocks.
        let responses = vec![
            NAK, // initial handshake
            ACK, // block 1
            ACK, // block 2
            ACK, // EOT
        ];
        let mut reader = MockReader::new(&responses);
        let mut writer: Vec<u8> = Vec::new();
        let data: Vec<u8> = (0..200).map(|i| i as u8).collect();

        send_xmodem(
            &mut reader,
            &mut writer,
            &data,
            XModemVariant::Classic,
            &Options::default(),
        )
        .expect("xmodem classic");

        // 2 blocks × (1 header + 1 num + 1 ~num + 128 data + 1 checksum)
        // + 1 EOT byte = 262 + 1 = 263.
        assert_eq!(writer.len(), 2 * (1 + 1 + 1 + 128 + 1) + 1);
        assert_eq!(writer[0], SOH);
        assert_eq!(writer[1], 1); // block 1
        assert_eq!(writer[2], !1u8); // complement
        assert_eq!(writer[writer.len() - 1], EOT);
    }

    #[test]
    fn xmodem_nak_triggers_retry() {
        // Receiver: 'C' (CRC handshake), NAK (fail first block), ACK
        // (retry succeeds), ACK (EOT).
        let responses = vec![CRC_REQ, NAK, ACK, ACK];
        let mut reader = MockReader::new(&responses);
        let mut writer: Vec<u8> = Vec::new();
        let data = vec![0xAAu8; 50];

        send_xmodem(
            &mut reader,
            &mut writer,
            &data,
            XModemVariant::Crc,
            &Options::default(),
        )
        .expect("xmodem crc");

        // One block sent twice (132 bytes each) + EOT = 265.
        let block_len = 1 + 1 + 1 + 128 + 2;
        assert_eq!(writer.len(), 2 * block_len + 1);
    }

    #[test]
    fn cancel_aborts_transfer() {
        let responses = vec![CRC_REQ, ACK, ACK, ACK];
        let mut reader = MockReader::new(&responses);
        let mut writer: Vec<u8> = Vec::new();
        let data = vec![0u8; 2048];

        let flag = Arc::new(AtomicBool::new(true));
        let opts = Options {
            progress: None,
            cancel: Some(flag),
        };

        let err = send_xmodem(&mut reader, &mut writer, &data, XModemVariant::Crc, &opts)
            .expect_err("should cancel");
        assert!(matches!(err, TransferError::Cancelled));
        // Abort sequence CAN×5 at the tail.
        assert_eq!(writer[writer.len() - 5..], [CAN, CAN, CAN, CAN, CAN]);
    }

    #[test]
    fn channel_reader_serves_bytes_one_at_a_time() {
        let (tx, rx) = mpsc::channel();
        tx.send(b"abc".to_vec()).unwrap();
        drop(tx);

        let mut reader = ChannelReader::new(rx);
        assert_eq!(reader.next_byte(Duration::from_millis(50)).unwrap(), b'a');
        assert_eq!(reader.next_byte(Duration::from_millis(50)).unwrap(), b'b');
        assert_eq!(reader.next_byte(Duration::from_millis(50)).unwrap(), b'c');
        // Sender dropped + queue drained → Closed.
        assert!(matches!(
            reader.next_byte(Duration::from_millis(50)),
            Err(TransferError::Closed)
        ));
    }
}
