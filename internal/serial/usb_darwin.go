package serial

import (
	"bytes"
	"fmt"
	"os/exec"
	"regexp"
	"strconv"
	"strings"

	"go.bug.st/serial/enumerator"
)

// DetectMissingDrivers returns USB devices whose VID or manufacturer string
// matches a known serial chipset but which aren't currently accessible as
// serial ports. macOS-only; reads the IOKit registry via ioreg, which is
// more reliable than system_profiler on recent macOS versions.
func DetectMissingDrivers() ([]USBSerialCandidate, error) {
	drivered := map[string]bool{}
	if details, err := enumerator.GetDetailedPortsList(); err == nil {
		for _, d := range details {
			if d.IsUSB {
				drivered[strings.ToLower(d.VID)+":"+strings.ToLower(d.PID)] = true
			}
		}
	}

	// usbHandled is VID:PID pairs we can open via libusb without
	// the vendor driver — CP210x today, more as chipset subpackages
	// land in usbserial-go. Treat those the same as "drivered": the
	// user doesn't need to install anything, they'll pick the port
	// straight out of the picker.
	usbHandled := map[string]bool{}
	if direct, err := listDirectUSB(); err == nil {
		for _, p := range direct {
			usbHandled[strings.ToLower(p.VID)+":"+strings.ToLower(p.PID)] = true
		}
	}

	devices, err := readIoregUSB()
	if err != nil {
		return nil, err
	}

	var missing []USBSerialCandidate
	seen := map[string]bool{}
	for _, d := range devices {
		info := IdentifyChipset(d.VID, d.PID, d.Manufacturer)
		if !info.NeedsDriver() {
			continue
		}
		key := d.VID + ":" + d.PID + ":" + d.SerialNum
		if seen[key] {
			continue
		}
		seen[key] = true
		if drivered[d.VID+":"+d.PID] {
			continue
		}
		if usbHandled[d.VID+":"+d.PID] {
			continue
		}
		missing = append(missing, USBSerialCandidate{
			VID:          d.VID,
			PID:          d.PID,
			Chipset:      info.Name,
			Manufacturer: d.Manufacturer,
			Product:      d.Product,
			SerialNumber: d.SerialNum,
			DriverURL:    info.DriverURL,
		})
	}

	// Also flag drivered ports whose product name is a driver-issue
	// placeholder (counterfeit Prolific detection and similar).
	for _, c := range detectSuspectEnumeratedPorts() {
		key := c.VID + ":" + c.PID + ":" + c.SerialNumber
		if seen[key] {
			continue
		}
		seen[key] = true
		missing = append(missing, c)
	}
	return missing, nil
}

type ioregDevice struct {
	VID          string
	PID          string
	Manufacturer string
	Product      string
	SerialNum    string
}

var (
	// Matches the header line of a USB device entry:
	//   | | +-o RUGGEDCOM USB Serial console@00123000  <class IOUSBHostDevice, ...>
	ioregDeviceRE = regexp.MustCompile(`\+-o\s+(.+?)(?:@[0-9a-fA-F]+)?\s+<class\s+(\w+)`)
	// Matches a property line:
	//     "idVendor" = 2312
	//     "USB Product Name" = "RUGGEDCOM USB Serial console"
	ioregPropRE = regexp.MustCompile(`"([^"]+)"\s*=\s*(.+?)\s*$`)
)

func readIoregUSB() ([]ioregDevice, error) {
	cmd := exec.Command("/usr/sbin/ioreg", "-p", "IOUSB", "-l", "-w", "0")
	out, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("ioreg: %w", err)
	}
	return parseIoregUSB(out), nil
}

func parseIoregUSB(out []byte) []ioregDevice {
	var devices []ioregDevice
	var cur *ioregDevice
	flush := func() {
		if cur != nil && cur.VID != "" && cur.PID != "" {
			devices = append(devices, *cur)
		}
		cur = nil
	}
	for _, raw := range bytes.Split(out, []byte{'\n'}) {
		line := string(raw)
		if m := ioregDeviceRE.FindStringSubmatch(line); m != nil {
			flush()
			class := m[2]
			if class == "IOUSBHostDevice" || class == "IOUSBDevice" {
				cur = &ioregDevice{}
			}
			continue
		}
		if cur == nil {
			continue
		}
		m := ioregPropRE.FindStringSubmatch(line)
		if m == nil {
			continue
		}
		key := m[1]
		val := strings.Trim(strings.TrimSpace(m[2]), `"`)
		switch key {
		case "idVendor":
			cur.VID = numericToHex(val)
		case "idProduct":
			cur.PID = numericToHex(val)
		case "USB Vendor Name", "kUSBVendorString":
			if cur.Manufacturer == "" {
				cur.Manufacturer = val
			}
		case "USB Product Name", "kUSBProductString":
			if cur.Product == "" {
				cur.Product = val
			}
		case "USB Serial Number", "kUSBSerialNumberString":
			if cur.SerialNum == "" {
				cur.SerialNum = val
			}
		}
	}
	flush()
	return devices
}

// numericToHex accepts ioreg's numeric property format, which may be decimal
// ("2312") or hex ("0x908"), and returns a lowercase 4-digit hex string.
func numericToHex(s string) string {
	s = strings.TrimSpace(s)
	var n int64
	var err error
	if strings.HasPrefix(s, "0x") || strings.HasPrefix(s, "0X") {
		n, err = strconv.ParseInt(s[2:], 16, 32)
	} else {
		n, err = strconv.ParseInt(s, 10, 32)
	}
	if err != nil {
		return ""
	}
	return fmt.Sprintf("%04x", n)
}
