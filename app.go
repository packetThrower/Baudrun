package main

import (
	"context"
	"encoding/base64"
	"errors"
	"fmt"
	"sync"

	"Seriesly/internal/appdata"
	"Seriesly/internal/profiles"
	sserial "Seriesly/internal/serial"
	"Seriesly/internal/settings"
	"Seriesly/internal/themes"

	"github.com/wailsapp/wails/v2/pkg/runtime"
)

const (
	EventSerialData       = "serial:data"
	EventSerialDisconnect = "serial:disconnect"
)

type App struct {
	ctx      context.Context
	store    *profiles.Store
	themes   *themes.Store
	settings *settings.Store
	sessMu   sync.Mutex
	session  *sserial.Session
	sessID   string
}

func NewApp() *App {
	return &App{}
}

func (a *App) startup(ctx context.Context) {
	a.ctx = ctx

	supportDir, err := appdata.SupportDir()
	if err != nil {
		runtime.LogErrorf(ctx, "resolve support dir: %v", err)
		return
	}

	if store, err := profiles.NewStore(); err == nil {
		a.store = store
	} else {
		runtime.LogErrorf(ctx, "profile store init: %v", err)
	}

	if ts, err := themes.NewStore(supportDir); err == nil {
		a.themes = ts
	} else {
		runtime.LogErrorf(ctx, "theme store init: %v", err)
	}

	if st, err := settings.NewStore(supportDir); err == nil {
		a.settings = st
	} else {
		runtime.LogErrorf(ctx, "settings store init: %v", err)
	}
}

// Profile API

func (a *App) ListProfiles() []profiles.Profile {
	if a.store == nil {
		return []profiles.Profile{}
	}
	return a.store.List()
}

func (a *App) CreateProfile(p profiles.Profile) (profiles.Profile, error) {
	if a.store == nil {
		return profiles.Profile{}, errors.New("store unavailable")
	}
	return a.store.Create(p)
}

func (a *App) UpdateProfile(p profiles.Profile) (profiles.Profile, error) {
	if a.store == nil {
		return profiles.Profile{}, errors.New("store unavailable")
	}
	return a.store.Update(p)
}

func (a *App) DeleteProfile(id string) error {
	if a.store == nil {
		return errors.New("store unavailable")
	}
	return a.store.Delete(id)
}

func (a *App) DefaultProfile() profiles.Profile {
	return profiles.Defaults()
}

// Theme API

func (a *App) ListThemes() []themes.Theme {
	if a.themes == nil {
		return []themes.Theme{}
	}
	return a.themes.List()
}

func (a *App) ImportTheme() (themes.Theme, error) {
	if a.themes == nil {
		return themes.Theme{}, errors.New("themes unavailable")
	}
	path, err := runtime.OpenFileDialog(a.ctx, runtime.OpenDialogOptions{
		Title: "Import iTerm2 color scheme",
		Filters: []runtime.FileFilter{
			{DisplayName: "iTerm2 Color Schemes (*.itermcolors)", Pattern: "*.itermcolors"},
		},
		ShowHiddenFiles:            false,
		CanCreateDirectories:       false,
		TreatPackagesAsDirectories: true,
	})
	if err != nil {
		return themes.Theme{}, err
	}
	if path == "" {
		return themes.Theme{}, errors.New("cancelled")
	}
	return a.themes.Import(path)
}

func (a *App) DeleteTheme(id string) error {
	if a.themes == nil {
		return errors.New("themes unavailable")
	}
	return a.themes.Delete(id)
}

// Settings API

func (a *App) GetSettings() settings.Settings {
	if a.settings == nil {
		return settings.Settings{DefaultThemeID: themes.DefaultThemeID, FontSize: 13}
	}
	return a.settings.Get()
}

func (a *App) UpdateSettings(s settings.Settings) (settings.Settings, error) {
	if a.settings == nil {
		return settings.Settings{}, errors.New("settings unavailable")
	}
	return a.settings.Update(s)
}

// Serial API

func (a *App) ListPorts() ([]sserial.PortInfo, error) {
	return sserial.ListPorts()
}

func (a *App) Connect(profileID string) error {
	if a.store == nil {
		return errors.New("store unavailable")
	}
	p, ok := a.store.Get(profileID)
	if !ok {
		return fmt.Errorf("profile %s not found", profileID)
	}

	a.sessMu.Lock()
	if a.session != nil {
		a.sessMu.Unlock()
		return errors.New("already connected — disconnect first")
	}
	a.sessMu.Unlock()

	cfg := sserial.Config{
		PortName:        p.PortName,
		BaudRate:        p.BaudRate,
		DataBits:        p.DataBits,
		Parity:          p.Parity,
		StopBits:        p.StopBits,
		FlowControl:     p.FlowControl,
		DTROnConnect:    p.DTROnConnect,
		RTSOnConnect:    p.RTSOnConnect,
		DTROnDisconnect: p.DTROnDisconnect,
		RTSOnDisconnect: p.RTSOnDisconnect,
	}

	sess, err := sserial.Open(cfg,
		func(chunk []byte) {
			runtime.EventsEmit(a.ctx, EventSerialData, base64.StdEncoding.EncodeToString(chunk))
		},
		func(err error) {
			msg := ""
			if err != nil {
				msg = err.Error()
			}
			a.sessMu.Lock()
			a.session = nil
			a.sessID = ""
			a.sessMu.Unlock()
			runtime.EventsEmit(a.ctx, EventSerialDisconnect, msg)
		},
	)
	if err != nil {
		return err
	}

	a.sessMu.Lock()
	a.session = sess
	a.sessID = profileID
	a.sessMu.Unlock()
	return nil
}

func (a *App) Disconnect() error {
	a.sessMu.Lock()
	sess := a.session
	a.session = nil
	a.sessID = ""
	a.sessMu.Unlock()
	if sess == nil {
		return nil
	}
	return sess.Close()
}

func (a *App) Send(data string) error {
	a.sessMu.Lock()
	sess := a.session
	a.sessMu.Unlock()
	if sess == nil {
		return errors.New("not connected")
	}
	bytes, err := base64.StdEncoding.DecodeString(data)
	if err != nil {
		return fmt.Errorf("decode send payload: %w", err)
	}
	_, err = sess.Write(bytes)
	return err
}

func (a *App) SetRTS(v bool) error {
	a.sessMu.Lock()
	sess := a.session
	a.sessMu.Unlock()
	if sess == nil {
		return errors.New("not connected")
	}
	return sess.SetRTS(v)
}

func (a *App) SetDTR(v bool) error {
	a.sessMu.Lock()
	sess := a.session
	a.sessMu.Unlock()
	if sess == nil {
		return errors.New("not connected")
	}
	return sess.SetDTR(v)
}

func (a *App) ActiveProfileID() string {
	a.sessMu.Lock()
	defer a.sessMu.Unlock()
	return a.sessID
}

type ControlLines struct {
	DTR bool `json:"dtr"`
	RTS bool `json:"rts"`
}

func (a *App) GetControlLines() (ControlLines, error) {
	a.sessMu.Lock()
	sess := a.session
	a.sessMu.Unlock()
	if sess == nil {
		return ControlLines{}, errors.New("not connected")
	}
	dtr, rts := sess.ControlLines()
	return ControlLines{DTR: dtr, RTS: rts}, nil
}
