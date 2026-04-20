//go:build windows

// Package winconsole suppresses the console window that Windows
// would otherwise flash when a GUI-subsystem process shells out to
// a console command (powershell, cmd, etc.). A Wails app is a GUI
// app but exec.Command subprocesses inherit the "new console"
// default, so every Get-PnpDevice call or folder-open produces a
// split-second cmd window flicker. Apply Hide() to an exec.Cmd
// before Run/Start/Output to suppress that.
package winconsole

import (
	"os/exec"
	"syscall"
)

// CREATE_NO_WINDOW — documented in the Win32 API as 0x08000000.
// Not re-exported from the stdlib syscall package for this field,
// so we spell it out. Sets the child process's console-alloc
// behavior to "no console at all."
const createNoWindow = 0x08000000

// Hide suppresses the console window when cmd runs. No-op on
// non-Windows builds (see winconsole_other.go).
func Hide(cmd *exec.Cmd) {
	if cmd.SysProcAttr == nil {
		cmd.SysProcAttr = &syscall.SysProcAttr{}
	}
	cmd.SysProcAttr.HideWindow = true
	cmd.SysProcAttr.CreationFlags |= createNoWindow
}
