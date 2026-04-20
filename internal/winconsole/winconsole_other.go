//go:build !windows

package winconsole

import "os/exec"

// Hide is a no-op on non-Windows platforms — only the Win32 API
// has the "new console window by default" behaviour to suppress.
func Hide(cmd *exec.Cmd) {}
