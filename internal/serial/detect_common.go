package serial

import (
	"strings"

	"go.bug.st/serial/enumerator"
)

// detectSuspectEnumeratedPorts walks the serial enumerator and returns any
// port whose product string looks like a driver-issue placeholder (see
// IsSuspectProduct). These ports are technically "drivered" — Windows gave
// them a COM number — but the driver is a stub that refuses to do I/O, so
// we want to flag them to the user anyway.
func detectSuspectEnumeratedPorts() []USBSerialCandidate {
	details, err := enumerator.GetDetailedPortsList()
	if err != nil {
		return nil
	}
	var out []USBSerialCandidate
	for _, d := range details {
		if !d.IsUSB {
			continue
		}
		if !IsSuspectProduct(d.Product) {
			continue
		}
		info := IdentifyChipset(d.VID, d.PID, "")
		if !info.NeedsDriver() {
			continue
		}
		c := USBSerialCandidate{
			VID:          strings.ToLower(d.VID),
			PID:          strings.ToLower(d.PID),
			Chipset:      info.Name,
			Product:      d.Product,
			SerialNumber: d.SerialNumber,
			DriverURL:    info.DriverURL,
		}
		// Prolific's current driver rejects pre-2016 chip revisions (PL2303HXA
		// et al.) with a scolding product string, even when the chip is
		// genuine. Common with reputable older cables like TRENDnet TU-S9.
		if strings.ToLower(d.VID) == "067b" {
			c.Chipset = "Prolific PL2303 (older chip revision)"
			c.Reason = "Chip is likely genuine but Prolific's current driver refuses older revisions. Install your cable vendor's driver (e.g. TRENDnet) or Prolific's legacy driver."
			c.DriverURL = "https://www.prolific.com.tw/US/ShowProduct.aspx?p_id=225&pcid=41"
		}
		out = append(out, c)
	}
	return out
}
