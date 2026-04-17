//go:build !darwin && !windows

package serial

// DetectMissingDrivers is a no-op on Linux for now. sysfs/udev enumeration
// would work — unimplemented until someone needs it.
func DetectMissingDrivers() ([]USBSerialCandidate, error) {
	return nil, nil
}
