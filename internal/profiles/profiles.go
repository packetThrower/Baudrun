package profiles

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"

	"Baudrun/internal/appdata"

	"github.com/google/uuid"
)

type Profile struct {
	ID               string    `json:"id"`
	Name             string    `json:"name"`
	PortName         string    `json:"portName"`
	BaudRate         int       `json:"baudRate"`
	DataBits         int       `json:"dataBits"`
	Parity           string    `json:"parity"`
	StopBits         string    `json:"stopBits"`
	FlowControl      string    `json:"flowControl"`
	LineEnding       string    `json:"lineEnding"`
	LocalEcho        bool      `json:"localEcho"`
	Highlight        bool      `json:"highlight"`
	ThemeID          string    `json:"themeId"`
	DTROnConnect     string    `json:"dtrOnConnect"`    // "default" | "assert" | "deassert"
	RTSOnConnect     string    `json:"rtsOnConnect"`
	DTROnDisconnect  string    `json:"dtrOnDisconnect"`
	RTSOnDisconnect  string    `json:"rtsOnDisconnect"`
	HexView          bool      `json:"hexView"`
	Timestamps       bool      `json:"timestamps"`
	LogEnabled       bool      `json:"logEnabled"`
	// AutoReconnect keeps the session alive across adapter drops by polling
	// for the port name to reappear and reopening transparently. Common
	// with cheap USB-serial adapters that re-enumerate under load.
	AutoReconnect    bool      `json:"autoReconnect"`
	// BackspaceKey picks the byte the Backspace key sends — "del" (0x7f,
	// VT100/xterm default) or "bs" (0x08, what some older Cisco IOS /
	// Foundry builds expect). Empty is treated as "del".
	BackspaceKey     string    `json:"backspaceKey"`
	// PasteWarnMultiline prompts the user to confirm before sending a
	// paste that contains line breaks. Catches the "I pasted ten
	// commands into the wrong window" class of mistake.
	PasteWarnMultiline bool `json:"pasteWarnMultiline"`
	// PasteSlow sends pasted text one character at a time with a gap
	// between each. UARTs on microcontrollers and older network gear
	// silently corrupt fast pastes at 115200 without this.
	PasteSlow          bool `json:"pasteSlow"`
	// PasteCharDelayMs is the inter-character gap used by PasteSlow.
	// Zero (or missing) defaults to 10ms, which clears most real-world
	// UART buffer issues without being perceptibly slow.
	PasteCharDelayMs   int  `json:"pasteCharDelayMs,omitempty"`
	CreatedAt        time.Time `json:"createdAt"`
	UpdatedAt        time.Time `json:"updatedAt"`
}

func Defaults() Profile {
	return Profile{
		BaudRate:         9600,
		DataBits:         8,
		Parity:           "none",
		StopBits:         "1",
		FlowControl:      "none",
		LineEnding:       "cr",
		LocalEcho:        false,
		Highlight:        true,
		DTROnConnect:     "default",
		RTSOnConnect:     "default",
		DTROnDisconnect:  "default",
		RTSOnDisconnect:  "default",
		BackspaceKey:     "del",
		// Paste safety is on by default — this is a serial-console
		// app, and a surprise multi-line paste into a router CLI can
		// execute partial commands before the user sees what's
		// happening. Users can flip either off per-profile for known
		// fast/forgiving devices.
		PasteWarnMultiline: true,
		PasteSlow:          true,
		PasteCharDelayMs:   10,
	}
}

type Store struct {
	path     string
	mu       sync.RWMutex
	profiles []Profile
}

func NewStore() (*Store, error) {
	dir, err := appdata.SupportDir()
	if err != nil {
		return nil, err
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, fmt.Errorf("create support dir: %w", err)
	}
	s := &Store{path: filepath.Join(dir, "profiles.json")}
	if err := s.load(); err != nil {
		return nil, err
	}
	return s, nil
}

func (s *Store) load() error {
	data, err := os.ReadFile(s.path)
	if errors.Is(err, os.ErrNotExist) {
		s.profiles = []Profile{}
		return nil
	}
	if err != nil {
		return fmt.Errorf("read profiles: %w", err)
	}
	if len(data) == 0 {
		s.profiles = []Profile{}
		return nil
	}
	return json.Unmarshal(data, &s.profiles)
}

func (s *Store) save() error {
	data, err := json.MarshalIndent(s.profiles, "", "  ")
	if err != nil {
		return err
	}
	tmp := s.path + ".tmp"
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		return err
	}
	return os.Rename(tmp, s.path)
}

func (s *Store) List() []Profile {
	s.mu.RLock()
	defer s.mu.RUnlock()
	out := make([]Profile, len(s.profiles))
	copy(out, s.profiles)
	return out
}

func (s *Store) Get(id string) (Profile, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	for _, p := range s.profiles {
		if p.ID == id {
			return p, true
		}
	}
	return Profile{}, false
}

func (s *Store) Create(p Profile) (Profile, error) {
	if err := validate(p); err != nil {
		return Profile{}, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	now := time.Now()
	p.ID = uuid.NewString()
	p.CreatedAt = now
	p.UpdatedAt = now
	s.profiles = append(s.profiles, p)
	if err := s.save(); err != nil {
		s.profiles = s.profiles[:len(s.profiles)-1]
		return Profile{}, err
	}
	return p, nil
}

func (s *Store) Update(p Profile) (Profile, error) {
	if p.ID == "" {
		return Profile{}, errors.New("profile id required")
	}
	if err := validate(p); err != nil {
		return Profile{}, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	for i, existing := range s.profiles {
		if existing.ID == p.ID {
			p.CreatedAt = existing.CreatedAt
			p.UpdatedAt = time.Now()
			prev := s.profiles[i]
			s.profiles[i] = p
			if err := s.save(); err != nil {
				s.profiles[i] = prev
				return Profile{}, err
			}
			return p, nil
		}
	}
	return Profile{}, fmt.Errorf("profile %s not found", p.ID)
}

func (s *Store) Delete(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	for i, p := range s.profiles {
		if p.ID == id {
			prev := s.profiles
			s.profiles = append(s.profiles[:i], s.profiles[i+1:]...)
			if err := s.save(); err != nil {
				s.profiles = prev
				return err
			}
			return nil
		}
	}
	return fmt.Errorf("profile %s not found", id)
}

func validate(p Profile) error {
	if p.Name == "" {
		return errors.New("name required")
	}
	if p.PortName == "" {
		return errors.New("port required")
	}
	if p.BaudRate <= 0 {
		return errors.New("baud rate must be positive")
	}
	if p.DataBits < 5 || p.DataBits > 8 {
		return errors.New("data bits must be 5-8")
	}
	switch p.Parity {
	case "none", "odd", "even", "mark", "space":
	default:
		return fmt.Errorf("invalid parity: %s", p.Parity)
	}
	switch p.StopBits {
	case "1", "1.5", "2":
	default:
		return fmt.Errorf("invalid stop bits: %s", p.StopBits)
	}
	switch p.FlowControl {
	case "none", "rtscts", "xonxoff":
	default:
		return fmt.Errorf("invalid flow control: %s", p.FlowControl)
	}
	switch p.LineEnding {
	case "cr", "lf", "crlf":
	default:
		return fmt.Errorf("invalid line ending: %s", p.LineEnding)
	}
	for _, f := range []struct {
		name, value string
	}{
		{"dtrOnConnect", p.DTROnConnect},
		{"rtsOnConnect", p.RTSOnConnect},
		{"dtrOnDisconnect", p.DTROnDisconnect},
		{"rtsOnDisconnect", p.RTSOnDisconnect},
	} {
		switch f.value {
		case "", "default", "assert", "deassert":
		default:
			return fmt.Errorf("invalid %s: %s", f.name, f.value)
		}
	}
	switch p.BackspaceKey {
	case "", "del", "bs":
	default:
		return fmt.Errorf("invalid backspaceKey: %s", p.BackspaceKey)
	}
	return nil
}
