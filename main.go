package main

import (
	"embed"

	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
	"github.com/wailsapp/wails/v2/pkg/options/linux"
	"github.com/wailsapp/wails/v2/pkg/options/mac"
	"github.com/wailsapp/wails/v2/pkg/runtime"
)

//go:embed all:frontend/dist
var assets embed.FS

//go:embed build/appicon.png
var appIcon []byte

func main() {
	app := NewApp()

	err := wails.Run(&options.App{
		Title:     "Baudrun",
		Width:     1100,
		Height:    720,
		MinWidth:  780,
		MinHeight: 480,
		AssetServer: &assetserver.Options{
			Assets: assets,
		},
		BackgroundColour: &options.RGBA{R: 30, G: 30, B: 34, A: 255},
		OnStartup:        app.startup,
		Bind:             []interface{}{app},
		// The embedded asset server is strictly local and serves our
		// own frontend. Webview "fraudulent website" heuristics were
		// designed for real-world web browsing and occasionally false-
		// positive on this pattern; turn it off explicitly so no one
		// wonders why they're seeing a warning on their own app.
		EnableFraudulentWebsiteDetection: false,
		// Hardware access doesn't coordinate between processes — two
		// Baudrun instances trying to open the same /dev/cu.* or COM
		// would fight over the port. Single-instance lock makes the
		// second launch surface the existing window instead.
		SingleInstanceLock: &options.SingleInstanceLock{
			UniqueId: "baudrun",
			OnSecondInstanceLaunch: func(data options.SecondInstanceData) {
				runtime.WindowUnminimise(app.ctx)
				runtime.WindowShow(app.ctx)
			},
		},
		Mac: &mac.Options{
			TitleBar: mac.TitleBarHiddenInset(),
			// Intentionally no Appearance / WindowIsTranslucent /
			// WebviewIsTransparent here. Translucent vibrancy would look
			// nice for the Baudrun and Liquid Glass skins, but Wails v2.12
			// can't change NSAppearance at runtime — so enabling
			// translucency forces pinning the window to dark to keep
			// NSVisualEffectView's vibrancy on a dark material, which in
			// turn locks WKWebView's prefers-color-scheme to dark and
			// breaks the app's own light/dark tracking of the OS setting.
			// Wails v3 is expected to land runtime NSAppearance switching;
			// revisit translucency once that's available. Shelved
			// alternative: branch `shelved/ostheme-watcher`.
			About: &mac.AboutInfo{
				Title:   "Baudrun",
				Message: "A serial terminal for network devices.",
			},
		},
		Linux: &linux.Options{
			Icon:             appIcon,
			ProgramName:      "Baudrun",
			WebviewGpuPolicy: linux.WebviewGpuPolicyOnDemand,
		},
	})

	if err != nil {
		println("Error:", err.Error())
	}
}
