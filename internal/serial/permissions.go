package serial

import (
	"errors"
	"fmt"
	"os"
	"runtime"
	"strings"
)

// enrichOpenError turns a raw port-open failure into one with
// actionable setup guidance on Linux when the cause looks like
// "user isn't allowed to open serial devices." Returns the error
// unchanged on other OSes or for non-permission failures.
//
// Background: on Linux, /dev/tty* serial nodes are owned by
// `root:dialout` (Debian/Ubuntu) or `root:uucp` (some others), and
// libusb device nodes under /dev/bus/usb/ default to root-only
// access. Our .deb/.rpm/pacman packages ship a udev rule
// (packaging/linux/60-baudrun-serial.rules) that adds uaccess ACLs
// for the current console user so Baudrun can open without any
// group-add dance. But that doesn't help in three situations:
//
//   - AppImage installs, which don't drop system files
//   - The rule was installed but udev hasn't seen the device yet
//     (no re-plug since install)
//   - A non-packaged manual build
//
// Those all surface to the user as `permission denied`, which
// without context looks like a Baudrun bug. Rewriting the error
// to include the actual fix is cheaper than a docs-chase.
func enrichOpenError(portName string, err error) error {
	if runtime.GOOS != "linux" {
		return fmt.Errorf("open %s: %w", portName, err)
	}
	if !looksLikePermissionDenied(err) {
		return fmt.Errorf("open %s: %w", portName, err)
	}
	return fmt.Errorf(
		"open %s: permission denied. Fix: run `sudo usermod -aG dialout $USER`, then log out and back in. "+
			"(Baudrun's .deb/.rpm/pacman packages also ship a udev rule that avoids this entirely — "+
			"installed via AppImage or from source, you're on the manual path.)",
		portName, err)
}

// looksLikePermissionDenied peeks at an arbitrary error to decide
// whether it's likely an EACCES from a device-node open. Checks the
// stdlib's canonical permission sentinel first, then a string
// match as a backup — go.bug.st/serial wraps errno in its own
// PortError type which may not unwrap cleanly to os.ErrPermission
// on every path, and the libusb/gousb side throws its own wrapped
// errors too. Both layers surface "permission denied" in Error().
func looksLikePermissionDenied(err error) bool {
	if errors.Is(err, os.ErrPermission) {
		return true
	}
	return strings.Contains(strings.ToLower(err.Error()), "permission denied")
}
