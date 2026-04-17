package serial

import "strings"

// ChipsetInfo names a USB serial bridge family and points at the driver the
// user needs to install, if any.
type ChipsetInfo struct {
	Name      string
	DriverURL string
}

const silabsDriverURL = "https://www.silabs.com/developers/usb-to-uart-bridge-vcp-drivers"

// Chipset returns the chipset family name for a device with the given
// standard USB vendor ID, or empty string if unknown. Display-only helper
// used to annotate ports that are already accessible.
func Chipset(vid string) string {
	return chipsetsByVID[strings.ToLower(vid)]
}

// DriverURL returns the vendor's driver download page for the chipset at the
// given vendor ID, or empty string if no user-installable driver is needed
// (e.g., FTDI, CDC-ACM served by Apple's built-in stack).
func DriverURL(vid string) string {
	return driverURLs[strings.ToLower(vid)]
}

// IdentifyChipset tries three strategies in order: exact VID:PID rebrand
// lookup (for vendors that ship a CP210x/FTDI/etc. with their own VID),
// then standard VID lookup, then a manufacturer-string heuristic. Returns
// an empty ChipsetInfo if nothing matches.
func IdentifyChipset(vid, pid, manufacturer string) ChipsetInfo {
	vid = strings.ToLower(vid)
	pid = strings.ToLower(pid)

	if info, ok := knownRebrands[vid+":"+pid]; ok {
		return info
	}
	if name, ok := chipsetsByVID[vid]; ok {
		return ChipsetInfo{Name: name, DriverURL: driverURLs[vid]}
	}
	return chipsetFromManufacturer(manufacturer)
}

// NeedsDriver reports whether a detected chipset requires the user to install
// something. Only non-empty DriverURL means "nudge the user" — otherwise the
// device is either unknown or already covered by macOS's built-in drivers.
func (c ChipsetInfo) NeedsDriver() bool {
	return c.Name != "" && c.DriverURL != ""
}

var chipsetsByVID = map[string]string{
	"10c4": "CP210x (Silicon Labs)",
	"0403": "FTDI",
	"067b": "Prolific PL2303",
	"1a86": "WCH CH340/CH341",
	"04d8": "Microchip",
	"04b4": "Cypress",
	"0557": "ATEN",
	"0d28": "ARM mbed (CDC-ACM)",
}

var driverURLs = map[string]string{
	"10c4": silabsDriverURL,
	"067b": "https://www.prolific.com.tw",
	"1a86": "https://www.wch-ic.com/downloads/CH34XSER_MAC_ZIP.html",
	"04d8": "https://www.microchip.com/en-us/product/MCP2221A",
	"04b4": "https://www.infineon.com/cms/en/design-support/tools/sdk/",
}

// knownRebrands: (VID:PID) → chipset+driver for devices that use a common
// bridge chip reprogrammed with the vendor's own USB-IF VID. Add entries here
// as they come up.
var knownRebrands = map[string]ChipsetInfo{
	// Siemens RUGGEDCOM USB Serial console (RST2228 and similar).
	// Uses a CP210x under a Siemens VID; still needs the SiLabs VCP driver.
	"0908:01ff": {Name: "CP210x (Siemens RUGGEDCOM)", DriverURL: silabsDriverURL},
}

// Manufacturer-string heuristic. Used when VID/PID doesn't match anything we
// know — the chip's own descriptor string often outs the underlying silicon
// even when the enclosing device has a different VID.
var manufacturerMatches = []struct {
	match string
	info  ChipsetInfo
}{
	{"silicon lab", ChipsetInfo{Name: "CP210x (Silicon Labs)", DriverURL: silabsDriverURL}},
	{"silabs", ChipsetInfo{Name: "CP210x (Silicon Labs)", DriverURL: silabsDriverURL}},
	{"prolific", ChipsetInfo{Name: "Prolific PL2303", DriverURL: "https://www.prolific.com.tw"}},
	{"qinheng", ChipsetInfo{Name: "WCH CH340/CH341", DriverURL: "https://www.wch-ic.com/downloads/CH34XSER_MAC_ZIP.html"}},
	{"wch.cn", ChipsetInfo{Name: "WCH CH340/CH341", DriverURL: "https://www.wch-ic.com/downloads/CH34XSER_MAC_ZIP.html"}},
}

func chipsetFromManufacturer(s string) ChipsetInfo {
	if s == "" {
		return ChipsetInfo{}
	}
	lc := strings.ToLower(s)
	for _, m := range manufacturerMatches {
		if strings.Contains(lc, m.match) {
			return m.info
		}
	}
	return ChipsetInfo{}
}
