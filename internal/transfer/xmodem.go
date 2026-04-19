package transfer

import (
	"fmt"
	"time"
)

const (
	handshakeTimeout = 60 * time.Second
	ackTimeout       = 10 * time.Second
	blockRetries     = 10
)

// SendXModem transmits data via XMODEM / XMODEM-CRC / XMODEM-1K.
// Blocks until completion, cancellation, or retry exhaustion.
func SendXModem(r Reader, w Writer, data []byte, variant XModemVariant, opts Options) error {
	useCRC := variant != XModem
	blockSize := 128
	header := SOH
	if variant == XModem1K {
		blockSize = 1024
		header = STX
	}

	if err := waitForHandshake(r, variant); err != nil {
		return fmt.Errorf("handshake: %w", err)
	}

	total := int64(len(data))
	blockNum := byte(1)
	for i := 0; i < len(data); i += blockSize {
		if err := checkCancel(opts.Cancel); err != nil {
			abort(w)
			return err
		}
		end := i + blockSize
		if end > len(data) {
			end = len(data)
		}
		if err := sendBlock(r, w, header, blockNum, data[i:end], blockSize, useCRC); err != nil {
			abort(w)
			return fmt.Errorf("block %d: %w", blockNum, err)
		}
		if opts.Progress != nil {
			opts.Progress(int64(end), total)
		}
		blockNum++
	}

	return sendEOT(r, w)
}

func waitForHandshake(r Reader, variant XModemVariant) error {
	want := NAK
	if variant != XModem {
		want = crcReq
	}
	deadline := time.Now().Add(handshakeTimeout)
	for time.Now().Before(deadline) {
		b, err := r.ReadByte(time.Second)
		if err != nil {
			continue
		}
		switch b {
		case want:
			return nil
		case CAN:
			if drainCAN(r) {
				return ErrCancelled
			}
		}
	}
	return ErrTimeout
}

// sendBlock emits one block and waits for ACK. On NAK the block is
// re-sent (up to blockRetries times). CAN CAN from the receiver
// aborts.
func sendBlock(r Reader, w Writer, header, blockNum byte, chunk []byte, blockSize int, useCRC bool) error {
	padded := make([]byte, blockSize)
	copy(padded, chunk)
	for i := len(chunk); i < blockSize; i++ {
		padded[i] = SUB
	}

	packet := make([]byte, 0, blockSize+5)
	packet = append(packet, header, blockNum, ^blockNum)
	packet = append(packet, padded...)
	if useCRC {
		c := crc16xmodem(padded)
		packet = append(packet, byte(c>>8), byte(c&0xff))
	} else {
		var sum byte
		for _, b := range padded {
			sum += b
		}
		packet = append(packet, sum)
	}

	for retry := 0; retry < blockRetries; retry++ {
		if _, err := w.Write(packet); err != nil {
			return err
		}
		b, err := r.ReadByte(ackTimeout)
		if err != nil {
			continue
		}
		switch b {
		case ACK:
			return nil
		case NAK:
			continue
		case CAN:
			if drainCAN(r) {
				return ErrCancelled
			}
		}
	}
	return fmt.Errorf("max retries")
}

// sendEOT closes out a transfer. Some receivers NAK the first EOT
// and ACK a retry — that's expected per the XMODEM spec.
func sendEOT(r Reader, w Writer) error {
	for retry := 0; retry < blockRetries; retry++ {
		if _, err := w.Write([]byte{EOT}); err != nil {
			return err
		}
		b, err := r.ReadByte(ackTimeout)
		if err != nil {
			continue
		}
		if b == ACK {
			return nil
		}
	}
	return fmt.Errorf("EOT not acknowledged")
}
