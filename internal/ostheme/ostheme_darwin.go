//go:build darwin

package ostheme

/*
#cgo LDFLAGS: -framework Foundation

// Declarations only — implementations live in ostheme_darwin.m so the
// Objective-C class isn't duplicated across CGo's translation units.
int osthemeCurrentIsDark(void);
void osthemeStartWatch(void);
void osthemeStopWatch(void);
*/
import "C"

import "sync"

// osthemeGoPoke is called from Objective-C when the OS theme changes.
// It's deliberately minimal — no Go-to-C calls, no locks held during
// the C return — to keep the Obj-C ↔ Go boundary as thin as possible.
//
// The actual work (reading the current state via CGo, invoking the
// user's callback, emitting Wails events) runs on a Go-owned watcher
// goroutine that drains pokeCh. That keeps the user callback out of
// the NSDistributedNotificationCenter dispatch context — which on
// Apple Silicon can trip PAC / thread-affinity issues when re-entering
// CGo, since the Wails runtime has its own main-thread locking
// conventions.
//
//export osthemeGoPoke
func osthemeGoPoke() {
	mu.Lock()
	ch := pokeCh
	mu.Unlock()
	if ch == nil {
		return
	}
	// Non-blocking send. If the watcher goroutine hasn't caught up, a
	// pending poke is already queued — coalesce.
	select {
	case ch <- struct{}{}:
	default:
	}
}

var (
	mu     sync.Mutex
	cb     func(Theme)
	pokeCh chan struct{}
	doneCh chan struct{}
)

// Current returns the current OS appearance.
func Current() Theme {
	if C.osthemeCurrentIsDark() != 0 {
		return ThemeDark
	}
	return ThemeLight
}

// Watch calls fn on each subsequent OS-level appearance change. The stop
// function unregisters the observer and tears down the watcher
// goroutine. Only one watcher is supported at a time; calling Watch a
// second time replaces the previous watcher.
func Watch(fn func(Theme)) (stop func(), err error) {
	mu.Lock()
	// Tear down any previous watcher cleanly.
	if doneCh != nil {
		C.osthemeStopWatch()
		close(doneCh)
		doneCh = nil
		pokeCh = nil
	}
	cb = fn
	pokeCh = make(chan struct{}, 1)
	doneCh = make(chan struct{})
	localPoke := pokeCh
	localDone := doneCh
	mu.Unlock()

	go func() {
		for {
			select {
			case <-localPoke:
				mu.Lock()
				f := cb
				mu.Unlock()
				if f != nil {
					f(Current())
				}
			case <-localDone:
				return
			}
		}
	}()

	C.osthemeStartWatch()

	return func() {
		mu.Lock()
		if doneCh == nil {
			mu.Unlock()
			return
		}
		C.osthemeStopWatch()
		close(doneCh)
		doneCh = nil
		pokeCh = nil
		cb = nil
		mu.Unlock()
	}, nil
}
