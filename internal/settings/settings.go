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
	DefaultThemeID string `json:"defaultThemeId"`
	FontSize       int    `json:"fontSize,omitempty"`
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
		st.s = Settings{DefaultThemeID: "seriesly", FontSize: 13}
		return nil
	}
	if err != nil {
		return fmt.Errorf("read settings: %w", err)
	}
	if len(data) == 0 {
		st.s = Settings{DefaultThemeID: "seriesly", FontSize: 13}
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
