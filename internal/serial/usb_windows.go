//go:build windows

package serial

import (
	"encoding/json"
	"fmt"
	"os/exec"
	"regexp"
	"strings"

	"Seriesly/internal/winconsole"

	"go.bug.st/serial/enumerator"
)

// DetectMissingDrivers returns USB devices that Windows has enumerated but
// failed to load a driver for (Status != "OK", typically Problem code 28 =
// CM_PROB_FAILED_INSTALL). Uses PowerShell's Get-PnpDevice; each device is
// serialized as its own JSON object on a separate line so we don't have to
// care whether PowerShell wraps one-result or many-result output as an
// array or a bare object.
func DetectMissingDrivers() ([]USBSerialCandidate, error) {
	drivered := map[string]bool{}
	if details, err := enumerator.GetDetailedPortsList(); err == nil {
		for _, d := range details {
			if d.IsUSB {
				drivered[strings.ToLower(d.VID)+":"+strings.ToLower(d.PID)] = true
			}
		}
	}

	script := `Get-PnpDevice -PresentOnly ` +
		`| Where-Object { $_.InstanceId -like 'USB\VID_*' -and $_.Status -ne 'OK' } ` +
		`| ForEach-Object { $_ | Select-Object InstanceId,FriendlyName,Manufacturer | ConvertTo-Json -Compress }`

	cmd := exec.Command("powershell.exe", "-NoProfile", "-NonInteractive", "-Command", script)
	winconsole.Hide(cmd)
	out, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("Get-PnpDevice: %w", err)
	}

	re := regexp.MustCompile(`(?i)USB\\VID_([0-9A-F]{4})&PID_([0-9A-F]{4})(?:\\(.*))?`)

	var missing []USBSerialCandidate
	seen := map[string]bool{}

	for _, line := range strings.Split(string(out), "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		var d struct {
			InstanceID   string `json:"InstanceId"`
			FriendlyName string `json:"FriendlyName"`
			Manufacturer string `json:"Manufacturer"`
		}
		if err := json.Unmarshal([]byte(line), &d); err != nil {
			continue
		}
		m := re.FindStringSubmatch(d.InstanceID)
		if m == nil {
			continue
		}
		vid := strings.ToLower(m[1])
		pid := strings.ToLower(m[2])
		serial := ""
		if len(m) >= 4 {
			serial = m[3]
		}

		info := IdentifyChipset(vid, pid, d.Manufacturer)
		if !info.NeedsDriver() {
			continue
		}
		key := vid + ":" + pid + ":" + serial
		if seen[key] {
			continue
		}
		seen[key] = true
		if drivered[vid+":"+pid] {
			continue
		}
		missing = append(missing, USBSerialCandidate{
			VID:          vid,
			PID:          pid,
			Chipset:      info.Name,
			Manufacturer: d.Manufacturer,
			Product:      d.FriendlyName,
			SerialNumber: serial,
			DriverURL:    info.DriverURL,
		})
	}

	// Also flag drivered ports whose product name is a driver-issue
	// placeholder (counterfeit Prolific detection).
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
