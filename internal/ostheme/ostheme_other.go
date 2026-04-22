//go:build !darwin && !windows && !linux

package ostheme

// Current returns ThemeLight on unsupported platforms. Baudrun ships
// binaries only for darwin/windows/linux today; this stub exists so
// go build ./... succeeds on other GOOS values (e.g., when tooling
// enumerates platforms).
func Current() Theme { return ThemeLight }

// Watch is a no-op on unsupported platforms.
func Watch(fn func(Theme)) (stop func(), err error) {
	return func() {}, nil
}
