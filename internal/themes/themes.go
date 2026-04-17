package themes

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
)

type Theme struct {
	ID     string `json:"id"`
	Name   string `json:"name"`
	Source string `json:"source"` // "builtin" or "user"

	Background          string `json:"background"`
	Foreground          string `json:"foreground"`
	Cursor              string `json:"cursor"`
	CursorAccent        string `json:"cursorAccent,omitempty"`
	Selection           string `json:"selection"`
	SelectionForeground string `json:"selectionForeground,omitempty"`

	Black         string `json:"black"`
	Red           string `json:"red"`
	Green         string `json:"green"`
	Yellow        string `json:"yellow"`
	Blue          string `json:"blue"`
	Magenta       string `json:"magenta"`
	Cyan          string `json:"cyan"`
	White         string `json:"white"`
	BrightBlack   string `json:"brightBlack"`
	BrightRed     string `json:"brightRed"`
	BrightGreen   string `json:"brightGreen"`
	BrightYellow  string `json:"brightYellow"`
	BrightBlue    string `json:"brightBlue"`
	BrightMagenta string `json:"brightMagenta"`
	BrightCyan    string `json:"brightCyan"`
	BrightWhite   string `json:"brightWhite"`
}

const DefaultThemeID = "seriesly"

type Store struct {
	dir  string
	mu   sync.RWMutex
	user []Theme
}

func NewStore(supportDir string) (*Store, error) {
	dir := filepath.Join(supportDir, "themes")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, fmt.Errorf("create themes dir: %w", err)
	}
	s := &Store{dir: dir}
	if err := s.loadUser(); err != nil {
		return nil, err
	}
	return s, nil
}

func (s *Store) List() []Theme {
	s.mu.RLock()
	defer s.mu.RUnlock()
	out := make([]Theme, 0, len(builtins)+len(s.user))
	out = append(out, builtins...)
	out = append(out, s.user...)
	return out
}

func (s *Store) Get(id string) (Theme, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	for _, t := range builtins {
		if t.ID == id {
			return t, true
		}
	}
	for _, t := range s.user {
		if t.ID == id {
			return t, true
		}
	}
	return Theme{}, false
}

// Resolve returns the theme with the given id, falling back to the default
// and finally the first builtin.
func (s *Store) Resolve(id string) Theme {
	if t, ok := s.Get(id); ok {
		return t
	}
	if t, ok := s.Get(DefaultThemeID); ok {
		return t
	}
	return builtins[0]
}

func (s *Store) Import(path string) (Theme, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return Theme{}, fmt.Errorf("read file: %w", err)
	}
	base := strings.TrimSuffix(filepath.Base(path), filepath.Ext(path))
	t, err := ParseItermColors(data, base)
	if err != nil {
		return Theme{}, err
	}
	t.Source = "user"

	s.mu.Lock()
	defer s.mu.Unlock()

	baseID := t.ID
	suffix := 2
	for s.hasIDLocked(t.ID) {
		t.ID = fmt.Sprintf("%s-%d", baseID, suffix)
		suffix++
	}

	if err := s.persistUser(t); err != nil {
		return Theme{}, err
	}
	s.user = append(s.user, t)
	return t, nil
}

func (s *Store) Delete(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	for i, t := range s.user {
		if t.ID == id {
			_ = os.Remove(filepath.Join(s.dir, t.ID+".json"))
			s.user = append(s.user[:i], s.user[i+1:]...)
			return nil
		}
	}
	return fmt.Errorf("user theme %s not found", id)
}

func (s *Store) loadUser() error {
	entries, err := os.ReadDir(s.dir)
	if err != nil {
		return err
	}
	for _, e := range entries {
		if e.IsDir() || !strings.HasSuffix(e.Name(), ".json") {
			continue
		}
		data, err := os.ReadFile(filepath.Join(s.dir, e.Name()))
		if err != nil {
			continue
		}
		var t Theme
		if err := json.Unmarshal(data, &t); err != nil {
			continue
		}
		t.Source = "user"
		s.user = append(s.user, t)
	}
	return nil
}

func (s *Store) persistUser(t Theme) error {
	data, err := json.MarshalIndent(t, "", "  ")
	if err != nil {
		return err
	}
	tmp := filepath.Join(s.dir, t.ID+".json.tmp")
	final := filepath.Join(s.dir, t.ID+".json")
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		return err
	}
	return os.Rename(tmp, final)
}

func (s *Store) hasIDLocked(id string) bool {
	for _, t := range builtins {
		if t.ID == id {
			return true
		}
	}
	for _, t := range s.user {
		if t.ID == id {
			return true
		}
	}
	return false
}

func slugify(s string) string {
	s = strings.ToLower(s)
	var b strings.Builder
	lastDash := true
	for _, r := range s {
		switch {
		case r >= 'a' && r <= 'z', r >= '0' && r <= '9':
			b.WriteRune(r)
			lastDash = false
		case r == ' ' || r == '-' || r == '_' || r == '.':
			if !lastDash {
				b.WriteRune('-')
				lastDash = true
			}
		}
	}
	out := strings.TrimSuffix(b.String(), "-")
	if out == "" {
		out = "theme"
	}
	return out
}

