package serial

import (
	"errors"
	"fmt"
	"io"
	"sort"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"go.bug.st/serial"
	"go.bug.st/serial/enumerator"
)

// readTimeout bounds how long port.Read blocks before returning with n=0 so
// the read pump can observe Close() and exit promptly. Without this, closing
// from another goroutine can leave the read stuck in the kernel on macOS.
const readTimeout = 100 * time.Millisecond

type PortInfo struct {
	Name         string `json:"name"`
	IsUSB        bool   `json:"isUSB"`
	VID          string `json:"vid,omitempty"`
	PID          string `json:"pid,omitempty"`
	SerialNumber string `json:"serialNumber,omitempty"`
	Product      string `json:"product,omitempty"`
	Chipset      string `json:"chipset,omitempty"`
}

type Config struct {
	PortName    string
	BaudRate    int
	DataBits    int
	Parity      string
	StopBits    string
	FlowControl string

	// Control-line policies: "" or "default" means leave as-is (OS default).
	// "assert" forces the line high; "deassert" forces it low.
	DTROnConnect    string
	RTSOnConnect    string
	DTROnDisconnect string
	RTSOnDisconnect string
}

// ListPorts returns all available serial ports with USB metadata when available.
// The basic port list is authoritative; detailed USB info is merged in as a best effort
// so devices without USB ancestry (Bluetooth SPP, built-in serial, some adapters) still appear.
// On macOS, /dev/tty.* entries are filtered out — they're the blocking twin of the
// /dev/cu.* callout device and terminal apps should always use the cu.* path.
func ListPorts() ([]PortInfo, error) {
	names, err := serial.GetPortsList()
	if err != nil {
		return nil, fmt.Errorf("enumerate ports: %w", err)
	}

	meta := map[string]*enumerator.PortDetails{}
	if details, err := enumerator.GetDetailedPortsList(); err == nil {
		for _, d := range details {
			meta[d.Name] = d
		}
	}

	out := make([]PortInfo, 0, len(names))
	for _, n := range names {
		if strings.HasPrefix(n, "/dev/tty.") {
			continue
		}
		p := PortInfo{Name: n}
		if d := meta[n]; d != nil {
			p.IsUSB = d.IsUSB
			p.VID = d.VID
			p.PID = d.PID
			p.SerialNumber = d.SerialNumber
			p.Product = d.Product
			p.Chipset = Chipset(d.VID)
		}
		out = append(out, p)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Name < out[j].Name })
	return out, nil
}

type Session struct {
	port     serial.Port
	mu       sync.Mutex
	closed   atomic.Bool
	wg       sync.WaitGroup
	onRead   func([]byte)
	onExit   func(error)
	dtrState atomic.Bool
	rtsState atomic.Bool
	// Control-line policies to apply on Close.
	dtrOnClose string
	rtsOnClose string
	// Optional sink for received bytes (session logging). Owned by the
	// session; closed on Close.
	logWriter io.WriteCloser
}

// SetLogWriter attaches a writer that will receive a copy of every byte
// read from the port. Passing nil detaches and closes any existing writer.
// Intended to be called right after Open.
func (s *Session) SetLogWriter(w io.WriteCloser) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.logWriter != nil {
		_ = s.logWriter.Close()
	}
	s.logWriter = w
}

func Open(cfg Config, onRead func([]byte), onExit func(error)) (*Session, error) {
	mode, err := buildMode(cfg)
	if err != nil {
		return nil, err
	}
	port, err := serial.Open(cfg.PortName, mode)
	if err != nil {
		return nil, fmt.Errorf("open %s: %w", cfg.PortName, err)
	}
	if err := port.SetReadTimeout(readTimeout); err != nil {
		_ = port.Close()
		return nil, fmt.Errorf("set read timeout on %s: %w", cfg.PortName, err)
	}

	// macOS/Linux assert DTR and RTS by default after open.
	dtr := true
	rts := true
	if v, ok := applyLine(port.SetDTR, cfg.DTROnConnect); ok {
		dtr = v
	}
	if v, ok := applyLine(port.SetRTS, cfg.RTSOnConnect); ok {
		rts = v
	}

	s := &Session{
		port:       port,
		onRead:     onRead,
		onExit:     onExit,
		dtrOnClose: cfg.DTROnDisconnect,
		rtsOnClose: cfg.RTSOnDisconnect,
	}
	s.dtrState.Store(dtr)
	s.rtsState.Store(rts)
	s.wg.Add(1)
	go s.readPump()
	return s, nil
}

// applyLine honors a policy ("assert"/"deassert"/"default"/"") by calling the
// setter if the policy is explicit. Returns the resulting logical state and
// whether the setter was invoked.
func applyLine(setter func(bool) error, policy string) (bool, bool) {
	switch policy {
	case "assert":
		_ = setter(true)
		return true, true
	case "deassert":
		_ = setter(false)
		return false, true
	default:
		return true, false
	}
}

func (s *Session) readPump() {
	defer s.wg.Done()
	buf := make([]byte, 4096)
	for {
		if s.closed.Load() {
			return
		}
		n, err := s.port.Read(buf)
		if s.closed.Load() {
			return
		}
		if err != nil {
			if s.onExit != nil {
				s.onExit(err)
			}
			return
		}
		if n > 0 {
			chunk := make([]byte, n)
			copy(chunk, buf[:n])
			if s.onRead != nil {
				s.onRead(chunk)
			}
			if w := s.logWriter; w != nil {
				_, _ = w.Write(chunk)
			}
		}
	}
}

func (s *Session) Write(data []byte) (int, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed.Load() {
		return 0, errors.New("session closed")
	}
	return s.port.Write(data)
}

func (s *Session) SetRTS(v bool) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed.Load() {
		return errors.New("session closed")
	}
	if err := s.port.SetRTS(v); err != nil {
		return err
	}
	s.rtsState.Store(v)
	return nil
}

func (s *Session) SetDTR(v bool) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed.Load() {
		return errors.New("session closed")
	}
	if err := s.port.SetDTR(v); err != nil {
		return err
	}
	s.dtrState.Store(v)
	return nil
}

// ControlLines reports the last known state of the DTR and RTS lines.
func (s *Session) ControlLines() (dtr, rts bool) {
	return s.dtrState.Load(), s.rtsState.Load()
}

func (s *Session) Close() error {
	if !s.closed.CompareAndSwap(false, true) {
		return nil
	}
	// Apply on-disconnect control-line policy before releasing the port.
	applyLine(s.port.SetDTR, s.dtrOnClose)
	applyLine(s.port.SetRTS, s.rtsOnClose)
	err := s.port.Close()
	// Wait for the read pump to exit so the OS-level FD is definitely
	// released before we return. With readTimeout this is bounded.
	s.wg.Wait()
	if s.logWriter != nil {
		_ = s.logWriter.Close()
		s.logWriter = nil
	}
	return err
}

func buildMode(cfg Config) (*serial.Mode, error) {
	if cfg.BaudRate <= 0 {
		return nil, errors.New("baud rate must be positive")
	}
	if cfg.DataBits < 5 || cfg.DataBits > 8 {
		return nil, errors.New("data bits must be 5-8")
	}
	m := &serial.Mode{
		BaudRate: cfg.BaudRate,
		DataBits: cfg.DataBits,
	}
	switch cfg.Parity {
	case "", "none":
		m.Parity = serial.NoParity
	case "odd":
		m.Parity = serial.OddParity
	case "even":
		m.Parity = serial.EvenParity
	case "mark":
		m.Parity = serial.MarkParity
	case "space":
		m.Parity = serial.SpaceParity
	default:
		return nil, fmt.Errorf("invalid parity: %s", cfg.Parity)
	}
	switch cfg.StopBits {
	case "", "1":
		m.StopBits = serial.OneStopBit
	case "1.5":
		m.StopBits = serial.OnePointFiveStopBits
	case "2":
		m.StopBits = serial.TwoStopBits
	default:
		return nil, fmt.Errorf("invalid stop bits: %s", cfg.StopBits)
	}
	return m, nil
}
