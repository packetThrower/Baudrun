//go:build unix

// Package main is a dev-only tool that creates two pty endpoints wired
// together with accurate inter-byte timing for a configurable baud rate.
// Point Baudrun at one endpoint, a test tool (xxd, lrzsz's rb / rx, etc.)
// at the other, and exercise code paths that care about real-world baud-
// rate pacing — paste safety, transfer progress, UART buffer behaviour —
// without a physical USB-serial adapter.
//
// Unix only (macOS + Linux). Windows dev machines should pair two virtual
// COM ports via com0com and talk to real hardware for the baud-rate-
// sensitive tests; the pty primitive this tool builds on has no Windows
// equivalent.
//
// See scripts/virtual-serial/README.md for usage examples.
package main

import (
	"flag"
	"fmt"
	"io"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/creack/pty"
	"golang.org/x/term"
)

func main() {
	baud := flag.Int("baud", 9600, "baud rate to simulate (bytes are paced as if each carries `bits` bits)")
	bits := flag.Int("bits", 10, "bits per byte including start + parity + stop (8N1 = 10, 8N2 = 11, 7E1 = 10)")
	linkA := flag.String("link-a", "", "optional stable symlink pointing at endpoint A (e.g. /tmp/baudrun-a)")
	linkB := flag.String("link-b", "", "optional stable symlink pointing at endpoint B (e.g. /tmp/baudrun-b)")
	flag.Parse()

	if *baud <= 0 || *bits <= 0 {
		fmt.Fprintln(os.Stderr, "baud and bits must be positive")
		os.Exit(1)
	}

	masterA, ttyA, err := pty.Open()
	if err != nil {
		fmt.Fprintln(os.Stderr, "open pty A:", err)
		os.Exit(1)
	}
	defer masterA.Close()
	defer ttyA.Close()

	masterB, ttyB, err := pty.Open()
	if err != nil {
		fmt.Fprintln(os.Stderr, "open pty B:", err)
		os.Exit(1)
	}
	defer masterB.Close()
	defer ttyB.Close()

	// Put both slaves in raw mode. Default pty line discipline is
	// canonical (ICANON, ECHO, OPOST on), which line-buffers bytes
	// moving from slave-writer to master-reader — a plain `cat` or
	// `xxd` reading the slave would only see bytes after each
	// newline. Any app that opens the port with go.bug.st/serial
	// (like Baudrun itself) resets termios on its own, but generic
	// Unix tools on the *other* endpoint benefit from raw-by-default.
	if _, err := term.MakeRaw(int(ttyA.Fd())); err != nil {
		fmt.Fprintln(os.Stderr, "raw-mode A:", err)
		os.Exit(1)
	}
	if _, err := term.MakeRaw(int(ttyB.Fd())); err != nil {
		fmt.Fprintln(os.Stderr, "raw-mode B:", err)
		os.Exit(1)
	}

	cleanup := func() {}
	if *linkA != "" {
		_ = os.Remove(*linkA)
		if err := os.Symlink(ttyA.Name(), *linkA); err != nil {
			fmt.Fprintln(os.Stderr, "symlink A:", err)
			os.Exit(1)
		}
		prev := cleanup
		cleanup = func() { prev(); _ = os.Remove(*linkA) }
	}
	if *linkB != "" {
		_ = os.Remove(*linkB)
		if err := os.Symlink(ttyB.Name(), *linkB); err != nil {
			fmt.Fprintln(os.Stderr, "symlink B:", err)
			cleanup()
			os.Exit(1)
		}
		prev := cleanup
		cleanup = func() { prev(); _ = os.Remove(*linkB) }
	}
	defer cleanup()

	byteDelay := time.Duration(float64(time.Second) * float64(*bits) / float64(*baud))

	printEndpoint(os.Stderr, "Endpoint A", ttyA.Name(), *linkA)
	printEndpoint(os.Stderr, "Endpoint B", ttyB.Name(), *linkB)
	fmt.Fprintf(os.Stderr, "Throttle:   %d baud, %d bits/byte → %s per byte\n",
		*baud, *bits, byteDelay.Round(time.Microsecond))
	fmt.Fprintln(os.Stderr, "Ctrl+C to quit.")

	sigs := make(chan os.Signal, 1)
	signal.Notify(sigs, syscall.SIGINT, syscall.SIGTERM)

	go throttle(masterA, masterB, byteDelay, "A→B")
	go throttle(masterB, masterA, byteDelay, "B→A")

	<-sigs
	fmt.Fprintln(os.Stderr, "\nShutting down.")
}

func printEndpoint(w io.Writer, label, tty, link string) {
	if link == "" {
		fmt.Fprintf(w, "%s: %s\n", label, tty)
		return
	}
	fmt.Fprintf(w, "%s: %s (→ %s)\n", label, tty, link)
}

// throttle copies bytes from src to dst one at a time, sleeping delay
// between each write. Reads are chunked for efficiency; only the writes
// are paced. A real UART would take exactly `delay` to clock each byte
// onto the wire, so this approximates that behaviour at the application
// layer.
func throttle(src io.Reader, dst io.Writer, delay time.Duration, tag string) {
	buf := make([]byte, 4096)
	for {
		n, err := src.Read(buf)
		if err != nil {
			if err != io.EOF {
				fmt.Fprintf(os.Stderr, "[%s] read: %v\n", tag, err)
			}
			return
		}
		for i := 0; i < n; i++ {
			time.Sleep(delay)
			if _, err := dst.Write(buf[i : i+1]); err != nil {
				fmt.Fprintf(os.Stderr, "[%s] write: %v\n", tag, err)
				return
			}
		}
	}
}
