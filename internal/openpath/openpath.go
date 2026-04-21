// Package openpath opens a filesystem path in the OS's default file
// manager (Finder on macOS, Explorer on Windows, xdg-open's handler
// on Linux). Used by the Settings UI's "Open" buttons next to
// directory paths so users can jump to the config or log folder in
// one click.
package openpath

import (
	"fmt"
	"os/exec"
	"runtime"

	"Baudrun/internal/winconsole"
)

// Open launches the OS's default handler for path. path should be a
// directory or file that already exists; the command is fire-and-
// forget (we don't wait for the file manager to actually show up).
func Open(path string) error {
	if path == "" {
		return fmt.Errorf("empty path")
	}
	var cmd *exec.Cmd
	switch runtime.GOOS {
	case "darwin":
		cmd = exec.Command("open", path)
	case "windows":
		// explorer.exe directly — skips the cmd /c start dance that
		// would otherwise flash a console window. explorer ignores
		// its exit code conventions (returns 1 even on success) so
		// we can't rely on the return value either way.
		cmd = exec.Command("explorer.exe", path)
	case "linux", "freebsd", "openbsd", "netbsd":
		cmd = exec.Command("xdg-open", path)
	default:
		return fmt.Errorf("unsupported platform: %s", runtime.GOOS)
	}
	// Belt-and-suspenders: explorer.exe shouldn't flash a console
	// but HideWindow costs nothing and covers any stray code path.
	winconsole.Hide(cmd)
	return cmd.Start()
}
