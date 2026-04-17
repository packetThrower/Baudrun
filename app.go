package main

import (
	"context"
	"encoding/base64"
	"errors"
	"fmt"
	"sync"

	"Seriesly/internal/profiles"
	sserial "Seriesly/internal/serial"

	"github.com/wailsapp/wails/v2/pkg/runtime"
)

const (
	EventSerialData       = "serial:data"
	EventSerialDisconnect = "serial:disconnect"
)

type App struct {
	ctx     context.Context
	store   *profiles.Store
	sessMu  sync.Mutex
	session *sserial.Session
	sessID  string
}

func NewApp() *App {
	return &App{}
}

func (a *App) startup(ctx context.Context) {
	a.ctx = ctx
	store, err := profiles.NewStore()
	if err != nil {
		runtime.LogErrorf(ctx, "profile store init: %v", err)
		return
	}
	a.store = store
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
		PortName:    p.PortName,
		BaudRate:    p.BaudRate,
		DataBits:    p.DataBits,
		Parity:      p.Parity,
		StopBits:    p.StopBits,
		FlowControl: p.FlowControl,
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
