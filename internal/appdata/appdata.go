package appdata

import (
	"os"
	"path/filepath"
)

// SupportDir returns the per-OS location for Seriesly's app data:
//   - macOS:   ~/Library/Application Support/Seriesly
//   - Windows: %APPDATA%\Seriesly
//   - Linux:   $XDG_CONFIG_HOME/Seriesly (or ~/.config/Seriesly)
func SupportDir() (string, error) {
	base, err := os.UserConfigDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(base, "Seriesly"), nil
}
