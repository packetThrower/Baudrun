package transfer

import (
	"fmt"
	"time"
)

// SendYModem transmits a single file via YMODEM (XMODEM-1K data
// blocks framed by a header block 0 carrying filename + size, and
// a terminating empty block 0). Multi-file batch transfers aren't
// supported — one file per call keeps the UI and progress model
// simple.
func SendYModem(r Reader, w Writer, filename string, data []byte, opts Options) error {
	// Initial handshake: receiver sends 'C' asking for block 0.
	if err := awaitC(r); err != nil {
		return fmt.Errorf("initial handshake: %w", err)
	}

	header := buildHeader(filename, int64(len(data)))
	if err := sendBlock(r, w, STX, 0, header, 1024, true); err != nil {
		abort(w)
		return fmt.Errorf("header block: %w", err)
	}

	// Second 'C' from the receiver opens the data stream.
	if err := awaitC(r); err != nil {
		return fmt.Errorf("data handshake: %w", err)
	}

	total := int64(len(data))
	const blockSize = 1024
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
		if err := sendBlock(r, w, STX, blockNum, data[i:end], blockSize, true); err != nil {
			abort(w)
			return fmt.Errorf("block %d: %w", blockNum, err)
		}
		if opts.Progress != nil {
			opts.Progress(int64(end), total)
		}
		blockNum++
	}

	if err := sendEOT(r, w); err != nil {
		return err
	}

	// Terminating empty header: receiver sends one more 'C', we
	// respond with an all-zeros 128-byte block 0, which tells it
	// the batch (of one file) is done.
	if err := awaitC(r); err != nil {
		return fmt.Errorf("end handshake: %w", err)
	}
	empty := make([]byte, 128)
	if err := sendBlock(r, w, SOH, 0, empty, 128, true); err != nil {
		return fmt.Errorf("end block: %w", err)
	}
	return nil
}

// buildHeader packs the YMODEM block 0 payload: filename, null,
// size + mtime + mode + serial number as space-separated decimals,
// null. mtime/mode/serial are zeroed — most receivers don't care.
func buildHeader(filename string, size int64) []byte {
	h := append([]byte(filename), 0)
	h = append(h, []byte(fmt.Sprintf("%d 0 0 0", size))...)
	h = append(h, 0)
	return h
}

func awaitC(r Reader) error {
	deadline := time.Now().Add(handshakeTimeout)
	for time.Now().Before(deadline) {
		b, err := r.NextByte(time.Second)
		if err != nil {
			continue
		}
		switch b {
		case crcReq:
			return nil
		case CAN:
			if drainCAN(r) {
				return ErrCancelled
			}
		}
	}
	return ErrTimeout
}
