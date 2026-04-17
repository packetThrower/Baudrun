package serial

import (
	"errors"
	"fmt"
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
}

type Config struct {
	PortName    string
	BaudRate    int
	DataBits    int
	Parity      string
	StopBits    string
	FlowControl string
}

// ListPorts returns all available serial ports with USB metadata when available.
// The basic port list is authoritative; detailed USB info is merged in as a best effort
// so devices without USB ancestry (Bluetooth SPP, built-in serial, some adapters) still appear.
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
		p := PortInfo{Name: n}
		if d := meta[n]; d != nil {
			p.IsUSB = d.IsUSB
			p.VID = d.VID
			p.PID = d.PID
			p.SerialNumber = d.SerialNumber
			p.Product = d.Product
		}
		out = append(out, p)
	}
	return out, nil
}

type Session struct {
	port   serial.Port
	mu     sync.Mutex
	closed atomic.Bool
	wg     sync.WaitGroup
	onRead func([]byte)
	onExit func(error)
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
	s := &Session{port: port, onRead: onRead, onExit: onExit}
	s.wg.Add(1)
	go s.readPump()
	return s, nil
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
		if n > 0 && s.onRead != nil {
			chunk := make([]byte, n)
			copy(chunk, buf[:n])
			s.onRead(chunk)
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
	return s.port.SetRTS(v)
}

func (s *Session) SetDTR(v bool) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed.Load() {
		return errors.New("session closed")
	}
	return s.port.SetDTR(v)
}

func (s *Session) Close() error {
	if !s.closed.CompareAndSwap(false, true) {
		return nil
	}
	err := s.port.Close()
	// Wait for the read pump to exit so the OS-level FD is definitely
	// released before we return. With readTimeout this is bounded.
	s.wg.Wait()
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
