package main

import (
	"embed"

	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
	"github.com/wailsapp/wails/v2/pkg/options/linux"
	"github.com/wailsapp/wails/v2/pkg/options/mac"
)

//go:embed all:frontend/dist
var assets embed.FS

//go:embed build/appicon.png
var appIcon []byte

func main() {
	app := NewApp()

	err := wails.Run(&options.App{
		Title:     "Seriesly",
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
		Mac: &mac.Options{
			TitleBar: mac.TitleBarHiddenInset(),
			// Pin the window to the dark system appearance so the
			// NSVisualEffectView behind translucent skins (Liquid Glass,
			// Seriesly) renders on a dark frosted material. Wails v2.12's
			// runtime theme setters are empty stubs on macOS, so this has
			// to be decided at startup. Until live-switch is wired up,
			// the app presents as dark-only regardless of the CSS
			// Appearance preference.
			Appearance:           mac.NSAppearanceNameDarkAqua,
			WindowIsTranslucent:  true,
			WebviewIsTransparent: true,
			About: &mac.AboutInfo{
				Title:   "Seriesly",
				Message: "A serial terminal for network devices.",
			},
		},
		Linux: &linux.Options{
			Icon:             appIcon,
			ProgramName:      "Seriesly",
			WebviewGpuPolicy: linux.WebviewGpuPolicyOnDemand,
		},
	})

	if err != nil {
		println("Error:", err.Error())
	}
}
