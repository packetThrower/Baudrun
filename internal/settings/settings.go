package settings

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sync"
)

type Settings struct {
	DefaultThemeID         string `json:"defaultThemeId"`
	FontSize               int    `json:"fontSize,omitempty"`
	LogDir                 string `json:"logDir,omitempty"`
	DisableDriverDetection bool   `json:"disableDriverDetection,omitempty"`
	SkinID                 string `json:"skinId,omitempty"`
	// Appearance: "auto" (follow system), "light", or "dark".
	// Empty / missing is treated as "auto".
	Appearance string `json:"appearance,omitempty"`
	// CopyOnSelect copies the xterm selection to the clipboard
	// automatically when the user releases the mouse, PuTTY-style.
	CopyOnSelect bool `json:"copyOnSelect,omitempty"`
	// ScreenReaderMode enables xterm.js's screen-reader mode. When on,
	// xterm exposes incoming output to assistive tech through a live
	// DOM region. Small perf cost on heavy output; off by default.
	ScreenReaderMode bool `json:"screenReaderMode,omitempty"`
	// ScrollbackLines is how many rows xterm retains in its scrollback
	// buffer. Each line is ~400 bytes at a typical 200-col width, so
	// 10k ≈ 4 MB, 100k ≈ 40 MB. xterm doesn't resize the buffer in
	// place — changing this live tears down and rebuilds the <Terminal>
	// component, preserving plain text but dropping ANSI color
	// attributes on the existing scrollback. Default 10000.
	ScrollbackLines int `json:"scrollbackLines,omitempty"`
	// Shortcuts override the default keyboard-shortcut bindings for
	// session-level actions (Clear, Send Break, Suspend). Stored in
	// W3C KeyboardEvent modifier+key string form ("Meta+K",
	// "Control+Shift+B") so the format matches what the frontend puts
	// in aria-keyshortcuts and what DOM events naturally produce.
	// Unset / missing values fall back to a platform-appropriate
	// default picked by the frontend — nil here doesn't mean
	// "disabled," it means "use the default."
	Shortcuts map[string]string `json:"shortcuts,omitempty"`
}

type Store struct {
	path string
	mu   sync.RWMutex
	s    Settings
}

func NewStore(supportDir string) (*Store, error) {
	if err := os.MkdirAll(supportDir, 0o755); err != nil {
		return nil, fmt.Errorf("create support dir: %w", err)
	}
	st := &Store{path: filepath.Join(supportDir, "settings.json")}
	if err := st.load(); err != nil {
		return nil, err
	}
	return st, nil
}

func (st *Store) Get() Settings {
	st.mu.RLock()
	defer st.mu.RUnlock()
	return st.s
}

func (st *Store) Update(s Settings) (Settings, error) {
	st.mu.Lock()
	defer st.mu.Unlock()
	prev := st.s
	st.s = s
	if err := st.save(); err != nil {
		st.s = prev
		return Settings{}, err
	}
	return st.s, nil
}

func (st *Store) load() error {
	data, err := os.ReadFile(st.path)
	if errors.Is(err, os.ErrNotExist) {
		st.s = Settings{DefaultThemeID: "baudrun", FontSize: 13, SkinID: "baudrun", Appearance: "auto", ScrollbackLines: 10000}
		return nil
	}
	if err != nil {
		return fmt.Errorf("read settings: %w", err)
	}
	if len(data) == 0 {
		st.s = Settings{DefaultThemeID: "baudrun", FontSize: 13, SkinID: "baudrun", Appearance: "auto", ScrollbackLines: 10000}
		return nil
	}
	return json.Unmarshal(data, &st.s)
}

func (st *Store) save() error {
	data, err := json.MarshalIndent(st.s, "", "  ")
	if err != nil {
		return err
	}
	tmp := st.path + ".tmp"
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		return err
	}
	return os.Rename(tmp, st.path)
}
