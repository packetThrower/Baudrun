// Package transfer implements XMODEM and YMODEM file-transfer protocols
// over a byte stream — typically a serial port. ZMODEM is deliberately
// not implemented here; it's a much larger state machine and most
// embedded bootloader targets don't speak it.
package transfer

import (
	"context"
	"errors"
	"time"
)

// Protocol constants (ASCII control codes).
const (
	SOH    byte = 0x01 // 128-byte block header
	STX    byte = 0x02 // 1024-byte block header
	EOT    byte = 0x04 // end of transmission
	ACK    byte = 0x06
	NAK    byte = 0x15
	CAN    byte = 0x18
	SUB    byte = 0x1A // filler byte (padding)
	crcReq byte = 0x43 // ASCII 'C' — receiver requests CRC mode
)

// ErrCancelled is returned when the caller's context fires during a
// transfer. Before returning, the sender emits CAN CAN CAN CAN CAN
// so the receiver knows to abort.
var ErrCancelled = errors.New("transfer cancelled")

// ErrTimeout is returned when a protocol-level wait (handshake,
// block ACK, EOT ACK) exceeds its budget.
var ErrTimeout = errors.New("transfer timeout")

// Reader delivers incoming bytes one at a time with a timeout. The
// transfer protocol state machines need byte-granularity reads to
// distinguish ACK/NAK/CAN, which the usual io.Reader chunk API
// doesn't give cleanly. (Method is NextByte rather than ReadByte
// to avoid shadowing io.ByteReader's parameterless signature.)
type Reader interface {
	NextByte(timeout time.Duration) (byte, error)
}

// Writer sends bytes out — typically to the serial port.
type Writer interface {
	Write(p []byte) (int, error)
}

// Options configures a transfer. All fields are optional.
type Options struct {
	// Progress is called with cumulative bytes sent and total bytes
	// after each block ACK.
	Progress func(sent, total int64)
	// Cancel lets the caller abort a transfer. Checked between
	// blocks; on cancel we send CAN CAN CAN CAN CAN and return
	// ErrCancelled.
	Cancel context.Context
}

// XModemVariant picks which flavour of XMODEM to speak. The
// receiver's handshake byte — NAK vs 'C' — partly forces our hand,
// but we still need to know whether to pack 128-byte or 1024-byte
// blocks and which block header code to use.
type XModemVariant int

const (
	// XModem is the original: 128-byte blocks, 8-bit checksum,
	// receiver starts with NAK. Deprecated for new designs but
	// still present in some older ROMs.
	XModem XModemVariant = iota
	// XModemCRC is 128-byte blocks with CRC-16. Receiver starts
	// with 'C'.
	XModemCRC
	// XModem1K is 1024-byte blocks with CRC-16 (sometimes called
	// XMODEM-1K or YAM). Receiver starts with 'C'.
	XModem1K
)

// crc16xmodem: CCITT polynomial 0x1021, seed 0x0000.
func crc16xmodem(data []byte) uint16 {
	var crc uint16
	for _, b := range data {
		crc ^= uint16(b) << 8
		for i := 0; i < 8; i++ {
			if crc&0x8000 != 0 {
				crc = (crc << 1) ^ 0x1021
			} else {
				crc <<= 1
			}
		}
	}
	return crc
}

func checkCancel(ctx context.Context) error {
	if ctx == nil {
		return nil
	}
	select {
	case <-ctx.Done():
		return ErrCancelled
	default:
		return nil
	}
}

func abort(w Writer) {
	_, _ = w.Write([]byte{CAN, CAN, CAN, CAN, CAN})
}

// drainCAN is used after seeing a single CAN — a lone CAN can be
// line noise, but CAN CAN is the receiver signalling abort.
func drainCAN(r Reader) bool {
	b, err := r.NextByte(500 * time.Millisecond)
	return err == nil && b == CAN
}
