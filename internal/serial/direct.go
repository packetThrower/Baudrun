package serial

import (
	"errors"
	"fmt"
	"io"
	"strings"
	"time"

	"github.com/packetThrower/usbserial-go/usbserial"
	// Blank-import the chipset subpackages we want usbserial-go to
	// know about. Only the ones listed here contribute code to the
	// binary; the usbserial package uses init()-time registration.
	_ "github.com/packetThrower/usbserial-go/cp210x"
)

// directPortPrefix marks a PortName as referring to a libusb-direct
// device rather than an OS-native /dev/* or COM path. Format:
//
//	usb:VID:PID            (device has no iSerial descriptor)
//	usb:VID:PID:Serial     (preferred — unique across duplicate models)
//
// VID and PID are lowercase 4-hex-digit strings. Serial is the
// iSerial descriptor with NUL / whitespace padding stripped.
const directPortPrefix = "usb:"

// isDirectUSBPortName reports whether a profile's stored PortName
// points at the libusb-direct path rather than the go.bug.st/serial
// path. Callers that open or enumerate ports dispatch on this.
func isDirectUSBPortName(name string) bool {
	return strings.HasPrefix(name, directPortPrefix)
}

// formatDirectUSBPortName builds the stable identifier we store in
// Profile.PortName for a libusb-direct device. Using VID+PID+Serial
// (rather than a bus/address tuple) keeps the identifier stable
// across re-plugs as long as the device has an iSerial — which all
// CP210x parts do by default.
func formatDirectUSBPortName(d usbserial.Device) string {
	if d.Serial != "" {
		return fmt.Sprintf("%s%04x:%04x:%s", directPortPrefix, d.VendorID, d.ProductID, d.Serial)
	}
	return fmt.Sprintf("%s%04x:%04x", directPortPrefix, d.VendorID, d.ProductID)
}

// parseDirectUSBPortName splits a "usb:VID:PID[:Serial]" name into
// its three fields. Serial is empty when the port name has only
// two fields after the prefix.
func parseDirectUSBPortName(name string) (vid, pid uint16, serial string, err error) {
	if !strings.HasPrefix(name, directPortPrefix) {
		return 0, 0, "", fmt.Errorf("not a direct-USB port name: %q", name)
	}
	tail := strings.TrimPrefix(name, directPortPrefix)
	parts := strings.SplitN(tail, ":", 3)
	if len(parts) < 2 {
		return 0, 0, "", fmt.Errorf("malformed direct-USB port name %q: want usb:VID:PID[:Serial]", name)
	}
	v, err := parseHex16(parts[0])
	if err != nil {
		return 0, 0, "", fmt.Errorf("vid in %q: %w", name, err)
	}
	p, err := parseHex16(parts[1])
	if err != nil {
		return 0, 0, "", fmt.Errorf("pid in %q: %w", name, err)
	}
	if len(parts) == 3 {
		serial = parts[2]
	}
	return v, p, serial, nil
}

func parseHex16(s string) (uint16, error) {
	var v uint64
	_, err := fmt.Sscanf(s, "%x", &v)
	if err != nil {
		return 0, fmt.Errorf("parse %q as hex: %w", s, err)
	}
	if v > 0xFFFF {
		return 0, fmt.Errorf("%q out of uint16 range", s)
	}
	return uint16(v), nil
}

// listDirectUSB enumerates every libusb-accessible adapter a
// registered chipset driver knows how to open. Unix-only in effect
// — on Windows, usbserial-go's enumerator returns COM-port paths
// that would just duplicate what go.bug.st/serial already reports,
// so those entries are filtered out by the directPortPrefix check
// below and never surface to the caller.
func listDirectUSB() ([]PortInfo, error) {
	devs, err := usbserial.List()
	if err != nil {
		return nil, err
	}
	out := make([]PortInfo, 0, len(devs))
	for _, d := range devs {
		// Skip Windows-style COM paths; the main enumeration picks
		// those up via the OS driver already.
		if !strings.HasPrefix(d.Path, "usb:") {
			continue
		}
		out = append(out, PortInfo{
			Name:         formatDirectUSBPortName(d),
			IsUSB:        true,
			VID:          fmt.Sprintf("%04x", d.VendorID),
			PID:          fmt.Sprintf("%04x", d.ProductID),
			SerialNumber: d.Serial,
			Product:      d.Product,
			Chipset:      string(d.Chipset),
			// Manufacturer lives on usbserial.Device but not on
			// PortInfo today. If we want it in the picker, add a
			// field later.
		})
	}
	return out, nil
}

// findDirectUSBDevice re-enumerates the bus and returns the
// usbserial.Device that matches the identifier parsed out of
// cfg.PortName. Separate from listDirectUSB because opening
// needs the Driver reference that List attaches to each entry,
// not just the display metadata.
func findDirectUSBDevice(portName string) (usbserial.Device, error) {
	vid, pid, serial, err := parseDirectUSBPortName(portName)
	if err != nil {
		return usbserial.Device{}, err
	}
	devs, err := usbserial.List()
	if err != nil {
		return usbserial.Device{}, fmt.Errorf("enumerate USB: %w", err)
	}
	for _, d := range devs {
		if d.VendorID != vid || d.ProductID != pid {
			continue
		}
		if serial != "" && d.Serial != serial {
			continue
		}
		return d, nil
	}
	return usbserial.Device{}, fmt.Errorf("USB device %04x:%04x (serial %q) not attached", vid, pid, serial)
}

// openDirectUSB opens a libusb-direct port, configures it per cfg,
// and returns a Session. Mirrors openBugST's contract so Open()
// can dispatch between the two without the caller caring which is
// in use.
func openDirectUSB(cfg Config, onRead func([]byte), onExit func(error)) (*Session, error) {
	d, err := findDirectUSBDevice(cfg.PortName)
	if err != nil {
		return nil, err
	}

	up, err := usbserial.Open(d)
	if err != nil {
		return nil, fmt.Errorf("open %s: %w", cfg.PortName, err)
	}

	if err := up.SetBaudRate(cfg.BaudRate); err != nil {
		_ = up.Close()
		return nil, fmt.Errorf("set baud on %s: %w", cfg.PortName, err)
	}
	framing, err := buildUSBFraming(cfg)
	if err != nil {
		_ = up.Close()
		return nil, err
	}
	if err := up.SetFraming(framing); err != nil {
		_ = up.Close()
		return nil, fmt.Errorf("set framing on %s: %w", cfg.PortName, err)
	}
	flow, err := buildUSBFlow(cfg)
	if err != nil {
		_ = up.Close()
		return nil, err
	}
	if err := up.SetFlowControl(flow); err != nil {
		_ = up.Close()
		return nil, fmt.Errorf("set flow control on %s: %w", cfg.PortName, err)
	}

	// Default to DTR/RTS asserted to match openBugST, then apply
	// the profile's explicit policies.
	dtr, rts := true, true
	if v, ok := applyLine(up.SetDTR, cfg.DTROnConnect); ok {
		dtr = v
	}
	if v, ok := applyLine(up.SetRTS, cfg.RTSOnConnect); ok {
		rts = v
	}

	backend := &usbDirectBackend{p: up}
	return newSession(backend, cfg, dtr, rts, onRead, onExit), nil
}

// buildUSBFraming maps Baudrun's profile-level strings to the
// typed Framing struct usbserial-go expects.
func buildUSBFraming(cfg Config) (usbserial.Framing, error) {
	if cfg.DataBits < 5 || cfg.DataBits > 8 {
		return usbserial.Framing{}, fmt.Errorf("data bits must be 5..8, got %d", cfg.DataBits)
	}
	f := usbserial.Framing{DataBits: cfg.DataBits}
	switch cfg.StopBits {
	case "", "1":
		f.StopBits = 1
	case "1.5":
		f.StopBits = 15
	case "2":
		f.StopBits = 2
	default:
		return usbserial.Framing{}, fmt.Errorf("invalid stop bits: %q", cfg.StopBits)
	}
	switch cfg.Parity {
	case "", "none":
		f.Parity = usbserial.ParityNone
	case "odd":
		f.Parity = usbserial.ParityOdd
	case "even":
		f.Parity = usbserial.ParityEven
	case "mark":
		f.Parity = usbserial.ParityMark
	case "space":
		f.Parity = usbserial.ParitySpace
	default:
		return usbserial.Framing{}, fmt.Errorf("invalid parity: %q", cfg.Parity)
	}
	return f, nil
}

// buildUSBFlow maps Baudrun's profile-level flow-control string to
// usbserial-go's FlowControl enum.
func buildUSBFlow(cfg Config) (usbserial.FlowControl, error) {
	switch cfg.FlowControl {
	case "", "none":
		return usbserial.FlowNone, nil
	case "hardware", "rtscts":
		return usbserial.FlowRTSCTS, nil
	case "software", "xonxoff":
		return usbserial.FlowXONXOFF, nil
	default:
		return 0, fmt.Errorf("invalid flow control: %q", cfg.FlowControl)
	}
}

// usbDirectBackend adapts usbserial.Port (SendBreak) to Session's
// portBackend interface (Break). Every other method passes through
// unchanged.
type usbDirectBackend struct {
	p usbserial.Port
}

var _ portBackend = (*usbDirectBackend)(nil)
var _ io.ReadWriteCloser = (*usbDirectBackend)(nil)

func (u *usbDirectBackend) Read(b []byte) (int, error) {
	if u == nil || u.p == nil {
		return 0, errors.New("usb backend: not open")
	}
	return u.p.Read(b)
}

func (u *usbDirectBackend) Write(b []byte) (int, error) {
	if u == nil || u.p == nil {
		return 0, errors.New("usb backend: not open")
	}
	return u.p.Write(b)
}

func (u *usbDirectBackend) Close() error {
	if u == nil || u.p == nil {
		return nil
	}
	return u.p.Close()
}

func (u *usbDirectBackend) SetDTR(v bool) error { return u.p.SetDTR(v) }
func (u *usbDirectBackend) SetRTS(v bool) error { return u.p.SetRTS(v) }
func (u *usbDirectBackend) Break(d time.Duration) error {
	return u.p.SendBreak(d)
}
