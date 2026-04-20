package main

import (
	"context"
	"encoding/base64"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"Seriesly/internal/appdata"
	"Seriesly/internal/profiles"
	sserial "Seriesly/internal/serial"
	"Seriesly/internal/settings"
	"Seriesly/internal/skins"
	"Seriesly/internal/themes"
	"Seriesly/internal/transfer"

	"github.com/wailsapp/wails/v2/pkg/runtime"
)

const (
	EventSerialData         = "serial:data"
	EventSerialDisconnect   = "serial:disconnect"
	EventSerialReconnecting = "serial:reconnecting"
	EventSerialReconnected  = "serial:reconnected"
	EventTransferProgress   = "transfer:progress"
	EventTransferComplete   = "transfer:complete"
	EventTransferError      = "transfer:error"

	reconnectInterval = 1 * time.Second
	reconnectTimeout  = 30 * time.Second
)

// TransferProgress is the payload emitted on EventTransferProgress.
type TransferProgress struct {
	Sent  int64 `json:"sent"`
	Total int64 `json:"total"`
}

type App struct {
	ctx      context.Context
	store    *profiles.Store
	themes   *themes.Store
	skins    *skins.Store
	settings *settings.Store
	sessMu   sync.Mutex
	session  *sserial.Session
	sessID   string
	// sessProfile snapshots the profile used for the current session
	// so auto-reconnect can reopen with the same config without hitting
	// the profile store again (user could have edited it mid-reconnect).
	sessProfile     profiles.Profile
	reconnectCancel context.CancelFunc
	// transferCancel is non-nil while a SendFile call is running.
	// Checked to block concurrent transfers and called by
	// CancelTransfer to abort mid-flight.
	transferCancel context.CancelFunc
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

	if sk, err := skins.NewStore(supportDir); err == nil {
		a.skins = sk
	} else {
		runtime.LogErrorf(ctx, "skin store init: %v", err)
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

// Skin API

func (a *App) ListSkins() []skins.Skin {
	if a.skins == nil {
		return []skins.Skin{}
	}
	return a.skins.List()
}

func (a *App) ImportSkin() (skins.Skin, error) {
	if a.skins == nil {
		return skins.Skin{}, errors.New("skins unavailable")
	}
	path, err := runtime.OpenFileDialog(a.ctx, runtime.OpenDialogOptions{
		Title: "Import skin",
		Filters: []runtime.FileFilter{
			{DisplayName: "Seriesly skin (*.json)", Pattern: "*.json"},
		},
		ShowHiddenFiles:            false,
		CanCreateDirectories:       false,
		TreatPackagesAsDirectories: true,
	})
	if err != nil {
		return skins.Skin{}, err
	}
	if path == "" {
		return skins.Skin{}, errors.New("cancelled")
	}
	return a.skins.Import(path)
}

func (a *App) DeleteSkin(id string) error {
	if a.skins == nil {
		return errors.New("skins unavailable")
	}
	return a.skins.Delete(id)
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

// PickLogDirectory opens a native folder-selection dialog and returns the
// chosen path, or empty string if the user cancelled.
func (a *App) PickLogDirectory() (string, error) {
	return runtime.OpenDirectoryDialog(a.ctx, runtime.OpenDialogOptions{
		Title: "Choose session log directory",
	})
}

// Config-directory relocation. The app reads profiles/themes/skins/
// settings from SupportDir at startup; moving that location takes
// effect on next launch. Existing files are not migrated — users
// copy them manually if they want to keep them.

// GetConfigDirectory returns the path the app is currently reading
// from.
func (a *App) GetConfigDirectory() (string, error) {
	return appdata.SupportDir()
}

// GetDefaultConfigDirectory returns the OS-idiomatic default,
// ignoring any override. Useful for the Settings UI's "reset to
// default" affordance.
func (a *App) GetDefaultConfigDirectory() (string, error) {
	return appdata.DefaultSupportDir()
}

// PickConfigDirectory opens a native folder picker for choosing a
// new config directory. Empty string means the user cancelled.
func (a *App) PickConfigDirectory() (string, error) {
	return runtime.OpenDirectoryDialog(a.ctx, runtime.OpenDialogOptions{
		Title: "Choose config directory",
	})
}

// SetConfigDirectory writes the override file. Passing "" clears
// the override so the next launch uses the default. Takes effect
// on next launch.
func (a *App) SetConfigDirectory(dir string) error {
	return appdata.WriteOverride(dir)
}

// DefaultLogDirectory returns the path session logs land in when no
// LogDir is configured — shown as a hint in the Settings UI.
func (a *App) DefaultLogDirectory() (string, error) {
	support, err := appdata.SupportDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(support, "logs"), nil
}

// Serial API

func (a *App) ListPorts() ([]sserial.PortInfo, error) {
	return sserial.ListPorts()
}

func (a *App) ListMissingDrivers() ([]sserial.USBSerialCandidate, error) {
	return sserial.DetectMissingDrivers()
}

func (a *App) Connect(profileID string) error {
	if a.store == nil {
		return errors.New("store unavailable")
	}
	p, ok := a.store.Get(profileID)
	if !ok {
		return fmt.Errorf("profile %s not found", profileID)
	}
	return a.openSession(p)
}

// openSession opens the port for a profile and wires the data/exit
// callbacks. Shared between the user-initiated Connect and the
// auto-reconnect retry loop so both paths produce identical sessions.
func (a *App) openSession(p profiles.Profile) error {
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
		a.onSessionExit,
	)
	if err != nil {
		return err
	}

	if p.LogEnabled {
		if logFile, err := a.openSessionLog(p); err == nil {
			sess.SetLogWriter(logFile)
		} else {
			runtime.LogErrorf(a.ctx, "open session log: %v", err)
		}
	}

	a.sessMu.Lock()
	a.session = sess
	a.sessID = p.ID
	a.sessProfile = p
	a.sessMu.Unlock()
	return nil
}

// onSessionExit fires from the read pump when the port returns an
// error — typically "device disconnected" when a USB-serial adapter
// re-enumerates or is unplugged. If the profile opts into
// auto-reconnect we kick off a retry loop instead of surfacing the
// disconnect; the xterm stays mounted on the frontend so scrollback
// survives the gap.
func (a *App) onSessionExit(exitErr error) {
	a.sessMu.Lock()
	profile := a.sessProfile
	sess := a.session
	a.session = nil
	a.sessMu.Unlock()

	// Release the orphaned session's fd + log writer. Close must be
	// async — we're running inside the session's read pump and Close
	// waits for that same pump to exit via WaitGroup, so a synchronous
	// call would deadlock.
	if sess != nil {
		go func() { _ = sess.Close() }()
	}

	if profile.AutoReconnect {
		a.startReconnect(profile)
		return
	}

	msg := ""
	if exitErr != nil {
		msg = exitErr.Error()
	}
	a.sessMu.Lock()
	a.sessID = ""
	a.sessProfile = profiles.Profile{}
	a.sessMu.Unlock()
	runtime.EventsEmit(a.ctx, EventSerialDisconnect, msg)
}

func (a *App) startReconnect(p profiles.Profile) {
	ctx, cancel := context.WithCancel(a.ctx)
	a.sessMu.Lock()
	if a.reconnectCancel != nil {
		a.reconnectCancel()
	}
	a.reconnectCancel = cancel
	a.sessMu.Unlock()

	runtime.EventsEmit(a.ctx, EventSerialReconnecting, p.PortName)
	go a.reconnectLoop(ctx, p)
}

func (a *App) reconnectLoop(ctx context.Context, p profiles.Profile) {
	defer func() {
		a.sessMu.Lock()
		a.reconnectCancel = nil
		a.sessMu.Unlock()
	}()

	deadline := time.Now().Add(reconnectTimeout)
	ticker := time.NewTicker(reconnectInterval)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			a.finishFailedReconnect("reconnect cancelled")
			return
		case <-ticker.C:
			if time.Now().After(deadline) {
				a.finishFailedReconnect("reconnect timeout")
				return
			}
			if err := a.openSession(p); err != nil {
				continue
			}
			runtime.EventsEmit(a.ctx, EventSerialReconnected, p.ID)
			return
		}
	}
}

func (a *App) finishFailedReconnect(reason string) {
	a.sessMu.Lock()
	a.sessID = ""
	a.sessProfile = profiles.Profile{}
	a.sessMu.Unlock()
	runtime.EventsEmit(a.ctx, EventSerialDisconnect, reason)
}

func (a *App) openSessionLog(p profiles.Profile) (*os.File, error) {
	dir := ""
	if a.settings != nil {
		dir = a.settings.Get().LogDir
	}
	if dir == "" {
		support, err := appdata.SupportDir()
		if err != nil {
			return nil, err
		}
		dir = filepath.Join(support, "logs")
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, fmt.Errorf("create log dir: %w", err)
	}
	stamp := time.Now().Format("2006-01-02_150405")
	name := fmt.Sprintf("%s_%s.log", slugifyName(p.Name), stamp)
	return os.Create(filepath.Join(dir, name))
}

func slugifyName(s string) string {
	s = strings.ToLower(s)
	var b strings.Builder
	for _, r := range s {
		switch {
		case r >= 'a' && r <= 'z', r >= '0' && r <= '9':
			b.WriteRune(r)
		case r == ' ', r == '-', r == '_', r == '.':
			b.WriteByte('-')
		}
	}
	out := strings.Trim(b.String(), "-")
	if out == "" {
		return "session"
	}
	return out
}

func (a *App) Disconnect() error {
	a.sessMu.Lock()
	sess := a.session
	cancel := a.reconnectCancel
	a.session = nil
	a.sessID = ""
	a.sessProfile = profiles.Profile{}
	a.reconnectCancel = nil
	a.sessMu.Unlock()
	// Cancel a pending reconnect before closing — the reconnect loop
	// might otherwise briefly reopen the port between our Close and
	// the user's expectation that the port is free.
	if cancel != nil {
		cancel()
	}
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

// SendBreak holds the TX line low for ~300ms — the signal Cisco gear
// reads as "drop into ROMMON", Juniper as "enter diagnostic mode", and
// many boot loaders as "interrupt autoboot".
func (a *App) SendBreak() error {
	a.sessMu.Lock()
	sess := a.session
	a.sessMu.Unlock()
	if sess == nil {
		return errors.New("not connected")
	}
	return sess.Break(300 * time.Millisecond)
}

// File transfer API.
//
// The flow is driven from the frontend: a file picker, a protocol
// picker, then SendFile blocks until the transfer completes or
// fails. Progress is surfaced via transfer:progress events;
// completion or error via transfer:complete / transfer:error.

// PickSendFile opens a native file dialog and returns the selected
// path, or empty string on cancel.
func (a *App) PickSendFile() (string, error) {
	return runtime.OpenFileDialog(a.ctx, runtime.OpenDialogOptions{
		Title: "Choose a file to send",
	})
}

// SendFile drives an XMODEM or YMODEM transfer over the active
// session. Valid protocols: "xmodem", "xmodem-crc", "xmodem-1k",
// "ymodem". Returns when the transfer completes, fails, or the
// caller cancels via CancelTransfer.
func (a *App) SendFile(protocol, path string) error {
	a.sessMu.Lock()
	sess := a.session
	if a.transferCancel != nil {
		a.sessMu.Unlock()
		return errors.New("transfer already in progress")
	}
	if sess == nil {
		a.sessMu.Unlock()
		return errors.New("not connected")
	}
	ctx, cancel := context.WithCancel(a.ctx)
	a.transferCancel = cancel
	a.sessMu.Unlock()

	defer func() {
		a.sessMu.Lock()
		a.transferCancel = nil
		a.sessMu.Unlock()
	}()

	data, err := os.ReadFile(path)
	if err != nil {
		runtime.EventsEmit(a.ctx, EventTransferError, fmt.Sprintf("read file: %v", err))
		return fmt.Errorf("read file: %w", err)
	}

	rx := make(chan byte, 8192)
	sess.StartTransfer(func(chunk []byte) {
		for _, b := range chunk {
			select {
			case rx <- b:
			default:
				// Drop on overflow. In practice the transfer
				// state machine reads continuously, so the
				// channel shouldn't fill.
			}
		}
	})
	defer sess.EndTransfer()

	reader := &byteChanReader{ch: rx}
	writer := sessionWriter{sess: sess}
	opts := transfer.Options{
		Cancel: ctx,
		Progress: func(sent, total int64) {
			runtime.EventsEmit(a.ctx, EventTransferProgress, TransferProgress{Sent: sent, Total: total})
		},
	}

	switch protocol {
	case "xmodem":
		err = transfer.SendXModem(reader, writer, data, transfer.XModem, opts)
	case "xmodem-crc":
		err = transfer.SendXModem(reader, writer, data, transfer.XModemCRC, opts)
	case "xmodem-1k":
		err = transfer.SendXModem(reader, writer, data, transfer.XModem1K, opts)
	case "ymodem":
		err = transfer.SendYModem(reader, writer, filepath.Base(path), data, opts)
	default:
		err = fmt.Errorf("unknown protocol: %s", protocol)
	}

	if err != nil {
		runtime.EventsEmit(a.ctx, EventTransferError, err.Error())
		return err
	}
	runtime.EventsEmit(a.ctx, EventTransferComplete, filepath.Base(path))
	return nil
}

// CancelTransfer aborts an in-flight transfer. No-op when no
// transfer is running.
func (a *App) CancelTransfer() {
	a.sessMu.Lock()
	cancel := a.transferCancel
	a.sessMu.Unlock()
	if cancel != nil {
		cancel()
	}
}

// byteChanReader adapts a byte channel to the transfer.Reader
// interface. Channel reads honor a per-call timeout so protocol
// handshakes (which wait for a specific byte) can bound their wait.
type byteChanReader struct {
	ch <-chan byte
}

func (r *byteChanReader) NextByte(timeout time.Duration) (byte, error) {
	select {
	case b := <-r.ch:
		return b, nil
	case <-time.After(timeout):
		return 0, transfer.ErrTimeout
	}
}

// sessionWriter adapts *sserial.Session to transfer.Writer.
// Session.Write returns (int, error) with an `error` type; the
// transfer package only needs the standard io.Writer shape.
type sessionWriter struct {
	sess *sserial.Session
}

func (w sessionWriter) Write(p []byte) (int, error) {
	return w.sess.Write(p)
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
