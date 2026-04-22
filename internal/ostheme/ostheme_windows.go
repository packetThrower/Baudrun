//go:build windows

package ostheme

import (
	"fmt"
	"sync"

	"golang.org/x/sys/windows"
	"golang.org/x/sys/windows/registry"
)

// Personalization settings live under HKCU (per-user, no elevation
// required). AppsUseLightTheme is a DWORD: 0 = dark, 1 = light.
const (
	personalizeKeyPath = `Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`
	appsLightValueName = "AppsUseLightTheme"
)

// Current returns the OS-level appearance preference from the registry.
// Defaults to ThemeLight if the key is missing (pre-Windows 10 systems
// or registry corruption — light is the historical default).
func Current() Theme {
	k, err := registry.OpenKey(registry.CURRENT_USER, personalizeKeyPath, registry.QUERY_VALUE)
	if err != nil {
		return ThemeLight
	}
	defer k.Close()
	v, _, err := k.GetIntegerValue(appsLightValueName)
	if err != nil {
		return ThemeLight
	}
	if v == 0 {
		return ThemeDark
	}
	return ThemeLight
}

// Watch fires fn whenever the Personalize key is modified. Uses
// RegNotifyChangeKeyValue in async mode with a pair of events — one for
// the registry notification, one to unblock the wait when the caller
// calls stop.
func Watch(fn func(Theme)) (stop func(), err error) {
	k, err := registry.OpenKey(registry.CURRENT_USER, personalizeKeyPath,
		registry.QUERY_VALUE|registry.NOTIFY)
	if err != nil {
		return nil, fmt.Errorf("open Personalize key: %w", err)
	}

	notifyEvent, err := windows.CreateEvent(nil, 1, 0, nil)
	if err != nil {
		k.Close()
		return nil, fmt.Errorf("create notify event: %w", err)
	}

	stopEvent, err := windows.CreateEvent(nil, 1, 0, nil)
	if err != nil {
		windows.CloseHandle(notifyEvent)
		k.Close()
		return nil, fmt.Errorf("create stop event: %w", err)
	}

	var once sync.Once
	closed := make(chan struct{})

	go func() {
		defer k.Close()
		defer windows.CloseHandle(notifyEvent)
		defer windows.CloseHandle(stopEvent)

		handles := []windows.Handle{notifyEvent, stopEvent}

		for {
			if err := windows.RegNotifyChangeKeyValue(
				windows.Handle(k),
				false, // don't watch subtree
				windows.REG_NOTIFY_CHANGE_LAST_SET,
				notifyEvent,
				true, // async
			); err != nil {
				return
			}

			ret, _ := windows.WaitForMultipleObjects(handles, false, windows.INFINITE)
			// WAIT_OBJECT_0 + idx identifies which handle fired.
			if ret == windows.WAIT_OBJECT_0+1 {
				return
			}

			windows.ResetEvent(notifyEvent)

			select {
			case <-closed:
				return
			default:
			}

			fn(Current())
		}
	}()

	return func() {
		once.Do(func() {
			close(closed)
			windows.SetEvent(stopEvent)
		})
	}, nil
}
