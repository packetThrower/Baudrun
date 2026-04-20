package appdata

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
)

// DefaultSupportDir returns the OS-idiomatic config directory for
// Seriesly, without considering any user override:
//   - macOS:   ~/Library/Application Support/Seriesly
//   - Windows: %APPDATA%\Seriesly
//   - Linux:   $XDG_CONFIG_HOME/Seriesly (or ~/.config/Seriesly)
func DefaultSupportDir() (string, error) {
	base, err := os.UserConfigDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(base, "Seriesly"), nil
}

// OverrideFile is the bootstrap redirect file — a single-line text
// file containing the absolute path of the user's chosen config
// directory. Lives inside the platform default so it remains
// findable even after the real config has moved. A user cleaning
// up can delete this file (or its containing dir) and the app will
// fall back to the default on next launch.
func OverrideFile() (string, error) {
	d, err := DefaultSupportDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(d, "config_dir_override"), nil
}

// SupportDir returns the active config directory. Reads the
// override file if present; otherwise falls back to DefaultSupportDir.
func SupportDir() (string, error) {
	override, err := readOverride()
	if err == nil && override != "" {
		return override, nil
	}
	return DefaultSupportDir()
}

// WriteOverride points future launches at dir. Pass "" to clear the
// override and revert to the default on next start. The target
// directory is created if it doesn't exist. Absolute paths only.
func WriteOverride(dir string) error {
	f, err := OverrideFile()
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(f), 0o755); err != nil {
		return err
	}
	if dir == "" {
		if rmErr := os.Remove(f); rmErr != nil && !os.IsNotExist(rmErr) {
			return rmErr
		}
		return nil
	}
	if !filepath.IsAbs(dir) {
		return errors.New("config directory path must be absolute")
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}
	return os.WriteFile(f, []byte(dir+"\n"), 0o644)
}

func readOverride() (string, error) {
	f, err := OverrideFile()
	if err != nil {
		return "", err
	}
	data, err := os.ReadFile(f)
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(data)), nil
}
