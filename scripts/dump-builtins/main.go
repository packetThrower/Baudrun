// One-off helper that serializes the Go built-in themes and skins to
// JSON files under src-tauri/resources/, so the Tauri v2 Rust backend
// can embed them with include_str! without needing to hand-translate
// 1600+ lines of Go struct literals. Run from the repo root:
//
//	go run ./scripts/dump-builtins
//
// Safe to rerun — output files are overwritten. Kept around during the
// Wails → Tauri v2 migration; delete after the Go backend is removed.
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"Baudrun/internal/skins"
	"Baudrun/internal/themes"
)

const resourcesDir = "src-tauri/resources"

func main() {
	tmp, err := os.MkdirTemp("", "dump-builtins-*")
	if err != nil {
		die("mkdir temp: %v", err)
	}
	defer os.RemoveAll(tmp)

	if err := os.MkdirAll(resourcesDir, 0o755); err != nil {
		die("mkdir %s: %v", resourcesDir, err)
	}

	// Themes — List() returns builtins first, then user. A temp dir
	// with no user imports means every returned entry is a builtin.
	themeStore, err := themes.NewStore(tmp)
	if err != nil {
		die("themes.NewStore: %v", err)
	}
	if err := writeJSON(
		filepath.Join(resourcesDir, "builtin_themes.json"),
		filterBuiltin(themeStore.List(), func(t themes.Theme) string { return t.Source }),
	); err != nil {
		die("write themes: %v", err)
	}

	// Skins — same pattern.
	skinStore, err := skins.NewStore(tmp)
	if err != nil {
		die("skins.NewStore: %v", err)
	}
	if err := writeJSON(
		filepath.Join(resourcesDir, "builtin_skins.json"),
		filterBuiltin(skinStore.List(), func(s skins.Skin) string { return s.Source }),
	); err != nil {
		die("write skins: %v", err)
	}

	fmt.Println("dumped builtins to", resourcesDir)
}

// filterBuiltin returns only entries whose Source field is "builtin",
// so user-imported content picked up from a non-empty support dir
// never leaks into the embedded resources.
func filterBuiltin[T any](items []T, source func(T) string) []T {
	out := make([]T, 0, len(items))
	for _, it := range items {
		if source(it) == "builtin" {
			out = append(out, it)
		}
	}
	return out
}

func writeJSON(path string, v any) error {
	data, err := json.MarshalIndent(v, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, data, 0o644)
}

func die(format string, args ...any) {
	fmt.Fprintf(os.Stderr, format+"\n", args...)
	os.Exit(1)
}
