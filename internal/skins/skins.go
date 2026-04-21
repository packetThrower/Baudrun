// Package skins manages app-chrome skins: named sets of CSS custom-property
// values that the frontend applies to document.documentElement. Distinct
// from terminal themes (internal/themes) which change only the terminal's
// color scheme.
package skins

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
)

// Skin describes a named skin. Vars maps CSS custom-property names
// (including the leading "--") to their values, e.g.
//
//	"--bg-sidebar": "rgba(255, 255, 255, 0.1)"
//
// Vars are always applied. DarkVars overlay on top when the app is in
// dark appearance; LightVars when in light. The applier reads the
// current appearance (from Settings.Appearance, respecting system
// preference when set to "auto") and picks the right overlay.
//
// SupportsLight=false means the skin is dark-only (e.g., CRT, Matrix).
// In that case the applier pins the dark overlay regardless of the
// user's global appearance preference.
type Skin struct {
	ID            string            `json:"id"`
	Name          string            `json:"name"`
	Source        string            `json:"source"` // "builtin" | "user"
	Description   string            `json:"description,omitempty"`
	Vars          map[string]string `json:"vars"`
	DarkVars      map[string]string `json:"darkVars,omitempty"`
	LightVars     map[string]string `json:"lightVars,omitempty"`
	SupportsLight bool              `json:"supportsLight"`
}

const DefaultSkinID = "baudrun"

type Store struct {
	dir  string
	mu   sync.RWMutex
	user []Skin
}

func NewStore(supportDir string) (*Store, error) {
	dir := filepath.Join(supportDir, "skins")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, fmt.Errorf("create skins dir: %w", err)
	}
	s := &Store{dir: dir}
	if err := s.loadUser(); err != nil {
		return nil, err
	}
	return s, nil
}

func (s *Store) List() []Skin {
	s.mu.RLock()
	defer s.mu.RUnlock()
	out := make([]Skin, 0, len(builtins)+len(s.user))
	out = append(out, builtins...)
	out = append(out, s.user...)
	return out
}

func (s *Store) Get(id string) (Skin, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	for _, sk := range builtins {
		if sk.ID == id {
			return sk, true
		}
	}
	for _, sk := range s.user {
		if sk.ID == id {
			return sk, true
		}
	}
	return Skin{}, false
}

// Resolve returns the skin with the given id, falling back to the default.
func (s *Store) Resolve(id string) Skin {
	if sk, ok := s.Get(id); ok {
		return sk
	}
	if sk, ok := s.Get(DefaultSkinID); ok {
		return sk
	}
	return builtins[0]
}

func (s *Store) Import(path string) (Skin, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return Skin{}, fmt.Errorf("read file: %w", err)
	}
	var sk Skin
	if err := json.Unmarshal(data, &sk); err != nil {
		return Skin{}, fmt.Errorf("parse skin json: %w", err)
	}
	if err := validate(sk); err != nil {
		return Skin{}, err
	}
	sk.Source = "user"

	s.mu.Lock()
	defer s.mu.Unlock()

	base := sk.ID
	if base == "" {
		base = slugify(sk.Name)
		sk.ID = base
	}
	suffix := 2
	for s.hasIDLocked(sk.ID) {
		sk.ID = fmt.Sprintf("%s-%d", base, suffix)
		suffix++
	}

	if err := s.persistUser(sk); err != nil {
		return Skin{}, err
	}
	s.user = append(s.user, sk)
	return sk, nil
}

func (s *Store) Delete(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	for i, sk := range s.user {
		if sk.ID == id {
			_ = os.Remove(filepath.Join(s.dir, sk.ID+".json"))
			s.user = append(s.user[:i], s.user[i+1:]...)
			return nil
		}
	}
	return fmt.Errorf("user skin %s not found", id)
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
		var sk Skin
		if err := json.Unmarshal(data, &sk); err != nil {
			continue
		}
		sk.Source = "user"
		s.user = append(s.user, sk)
	}
	return nil
}

func (s *Store) persistUser(sk Skin) error {
	data, err := json.MarshalIndent(sk, "", "  ")
	if err != nil {
		return err
	}
	tmp := filepath.Join(s.dir, sk.ID+".json.tmp")
	final := filepath.Join(s.dir, sk.ID+".json")
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		return err
	}
	return os.Rename(tmp, final)
}

func (s *Store) hasIDLocked(id string) bool {
	for _, sk := range builtins {
		if sk.ID == id {
			return true
		}
	}
	for _, sk := range s.user {
		if sk.ID == id {
			return true
		}
	}
	return false
}

func validate(sk Skin) error {
	if sk.Name == "" {
		return errors.New("skin name required")
	}
	if len(sk.Vars) == 0 && len(sk.DarkVars) == 0 && len(sk.LightVars) == 0 {
		return errors.New("skin has no variables")
	}
	for _, m := range []map[string]string{sk.Vars, sk.DarkVars, sk.LightVars} {
		for k := range m {
			if !strings.HasPrefix(k, "--") {
				return fmt.Errorf("skin var %q must start with --", k)
			}
		}
	}
	return nil
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
		out = "skin"
	}
	return out
}
