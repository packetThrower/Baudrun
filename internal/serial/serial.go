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
//
// Libusb-direct entries from usbserial-go are appended for devices
// that don't already have an OS-provided device node (CP210x with
// no SiLabs driver, vendor-rebranded VIDs the kernel's id_tables
// don't cover, etc.). Dedupe by VID:PID+Serial so a device that
// has both paths only shows up once, with the OS path preferred.
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
	// drivered tracks (vid:pid:serial) triples that already surface
	// through the OS enumerator. A libusb-direct entry with the
	// same triple would be a duplicate of the same physical device,
	// so we suppress it in favour of the native /dev/* path.
	drivered := map[string]bool{}
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
			if d.IsUSB {
				drivered[deviceKey(d.VID, d.PID, d.SerialNumber)] = true
			}
		}
		out = append(out, p)
	}

	// Merge libusb-direct entries for devices that aren't already
	// accessible via /dev/*. Enumeration failures here are
	// non-fatal — the user can still pick any OS port we already
	// surfaced above.
	if direct, err := listDirectUSB(); err == nil {
		for _, p := range direct {
			if drivered[deviceKey(p.VID, p.PID, p.SerialNumber)] {
				continue
			}
			out = append(out, p)
		}
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Name < out[j].Name })
	return out, nil
}

// deviceKey builds the triple we use to dedupe the same physical
// adapter across the OS-enumerator and libusb-direct paths. VID
// and PID are lower-cased so comparisons are case-insensitive.
func deviceKey(vid, pid, serial string) string {
	return strings.ToLower(vid) + ":" + strings.ToLower(pid) + ":" + serial
}

// portBackend is the minimal surface Session needs from whichever
// library is driving the underlying transport. Two implementations
// satisfy it today: go.bug.st/serial.Port for native /dev/* and
// COM ports, and a thin wrapper around usbserial.Port for direct-
// USB access to chipsets the OS has no driver for.
//
// serial.Port's own method signatures already match this interface
// exactly, so no adapter is needed on that side; the usbserial-go
// adapter lives in direct.go.
type portBackend interface {
	io.ReadWriteCloser
	SetDTR(bool) error
	SetRTS(bool) error
	Break(time.Duration) error
}

type Session struct {
	port     portBackend
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
	// transferRX, when set, diverts incoming bytes away from onRead.
	// Used by file-transfer protocols that need raw byte access and
	// don't want the normal event bus to also display the bytes.
	transferMu sync.Mutex
	transferRX func([]byte)
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

// Open opens the port identified by cfg.PortName and starts a read
// pump. The backend is chosen from the port name shape:
//
//   - "usb:VID:PID[:serial]" → libusb-direct via usbserial-go. Used
//     for chipsets the OS has no driver for (CP210x on macOS, and
//     vendor-rebranded VIDs the kernel's id_tables don't cover).
//   - anything else → go.bug.st/serial on the given device path or
//     COM name. This is the common path for every adapter the OS
//     already knows how to talk to.
func Open(cfg Config, onRead func([]byte), onExit func(error)) (*Session, error) {
	if isDirectUSBPortName(cfg.PortName) {
		return openDirectUSB(cfg, onRead, onExit)
	}
	return openBugST(cfg, onRead, onExit)
}

// openBugST is the go.bug.st/serial backed path — every native
// /dev/* or COM port flows through here.
func openBugST(cfg Config, onRead func([]byte), onExit func(error)) (*Session, error) {
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

	return newSession(port, cfg, dtr, rts, onRead, onExit), nil
}

// newSession finalises a Session around a ready portBackend. Kept
// separate so both openBugST and openDirectUSB share the same
// read-pump bring-up and on-close policy plumbing.
func newSession(p portBackend, cfg Config, dtr, rts bool, onRead func([]byte), onExit func(error)) *Session {
	s := &Session{
		port:       p,
		onRead:     onRead,
		onExit:     onExit,
		dtrOnClose: cfg.DTROnDisconnect,
		rtsOnClose: cfg.RTSOnDisconnect,
	}
	s.dtrState.Store(dtr)
	s.rtsState.Store(rts)
	s.wg.Add(1)
	go s.readPump()
	return s
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
			s.transferMu.Lock()
			transferHandler := s.transferRX
			s.transferMu.Unlock()
			if transferHandler != nil {
				transferHandler(chunk)
			} else if s.onRead != nil {
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

// Break holds the serial break condition (TX line low) for the given
// duration, then releases it. Used to drop into ROMMON on Cisco gear,
// the Juniper diagnostic prompt, or to break out of a boot loader.
// 300ms matches PuTTY's default and is long enough for every device
// I've seen without stalling the session noticeably.
func (s *Session) Break(d time.Duration) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed.Load() {
		return errors.New("session closed")
	}
	return s.port.Break(d)
}

// StartTransfer diverts incoming RX bytes to fn until EndTransfer is
// called. Normal onRead delivery (to the frontend event bus) is
// suspended during this period, so file-transfer protocols like
// XMODEM/YMODEM can drive the port without noisy display side effects.
func (s *Session) StartTransfer(fn func([]byte)) {
	s.transferMu.Lock()
	s.transferRX = fn
	s.transferMu.Unlock()
}

// EndTransfer restores normal onRead delivery.
func (s *Session) EndTransfer() {
	s.transferMu.Lock()
	s.transferRX = nil
	s.transferMu.Unlock()
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
