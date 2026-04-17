package appdata

import (
	"os"
	"path/filepath"
)

// SupportDir returns ~/Library/Application Support/Seriesly.
func SupportDir() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, "Library", "Application Support", "Seriesly"), nil
}
