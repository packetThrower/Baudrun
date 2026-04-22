//go:build linux

package ostheme

import (
	"fmt"
	"sync"

	"github.com/godbus/dbus/v5"
)

// XDG Desktop Portal's Settings interface. Works in and out of Flatpak,
// and is the sanctioned cross-DE way to read GNOME / KDE / Cosmic /
// other implementations of the system appearance preference.
//
// Ref: https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Settings.html
const (
	portalDest     = "org.freedesktop.portal.Desktop"
	portalPath     = "/org/freedesktop/portal/desktop"
	settingsIface  = "org.freedesktop.portal.Settings"
	appearanceNs   = "org.freedesktop.appearance"
	colorSchemeKey = "color-scheme"
)

// Current returns the OS-level appearance preference via the desktop
// portal. Falls back to ThemeLight on any failure (portal unavailable,
// DBus not running) — silent fallback is better than surfacing a
// can't-read-settings error to the user, since the app still renders
// correctly in the pinned dark mode either way.
func Current() Theme {
	conn, err := dbus.SessionBus()
	if err != nil {
		return ThemeLight
	}
	obj := conn.Object(portalDest, portalPath)
	var val dbus.Variant
	if err := obj.Call(settingsIface+".Read", 0, appearanceNs, colorSchemeKey).Store(&val); err != nil {
		return ThemeLight
	}
	return colorSchemeVariantToTheme(val)
}

// colorSchemeVariantToTheme unpacks the portal's color-scheme value.
// The value is a uint32 enum — 0: no preference, 1: prefer-dark,
// 2: prefer-light. Older portal implementations return a VARIANT
// wrapping a VARIANT wrapping the uint32, so unwrap one level
// defensively.
func colorSchemeVariantToTheme(v dbus.Variant) Theme {
	inner := v.Value()
	if nested, ok := inner.(dbus.Variant); ok {
		inner = nested.Value()
	}
	switch n := inner.(type) {
	case uint32:
		if n == 1 {
			return ThemeDark
		}
	case int32:
		if n == 1 {
			return ThemeDark
		}
	}
	return ThemeLight
}

// Watch listens for SettingChanged signals on the portal. Filters on the
// appearance/color-scheme pair and re-reads the current value via
// Current() rather than unpacking the signal payload — keeps decoding
// logic in one place.
func Watch(fn func(Theme)) (stop func(), err error) {
	conn, err := dbus.SessionBus()
	if err != nil {
		return nil, fmt.Errorf("connect to session bus: %w", err)
	}

	matchOpts := []dbus.MatchOption{
		dbus.WithMatchObjectPath(portalPath),
		dbus.WithMatchInterface(settingsIface),
		dbus.WithMatchMember("SettingChanged"),
	}
	if err := conn.AddMatchSignal(matchOpts...); err != nil {
		return nil, fmt.Errorf("add match signal: %w", err)
	}

	ch := make(chan *dbus.Signal, 10)
	conn.Signal(ch)

	stopCh := make(chan struct{})
	var once sync.Once

	go func() {
		for {
			select {
			case sig, ok := <-ch:
				if !ok {
					return
				}
				if len(sig.Body) < 2 {
					continue
				}
				ns, nsOk := sig.Body[0].(string)
				key, keyOk := sig.Body[1].(string)
				if !nsOk || !keyOk {
					continue
				}
				if ns != appearanceNs || key != colorSchemeKey {
					continue
				}
				fn(Current())
			case <-stopCh:
				conn.RemoveSignal(ch)
				_ = conn.RemoveMatchSignal(matchOpts...)
				return
			}
		}
	}()

	return func() {
		once.Do(func() { close(stopCh) })
	}, nil
}
