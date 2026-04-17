package main

import (
	"embed"

	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
	"github.com/wailsapp/wails/v2/pkg/options/mac"
)

//go:embed all:frontend/dist
var assets embed.FS

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
		BackgroundColour:  &options.RGBA{R: 30, G: 30, B: 34, A: 255},
		OnStartup:         app.startup,
		Bind:              []interface{}{app},
		Mac: &mac.Options{
			TitleBar:             mac.TitleBarHiddenInset(),
			Appearance:           mac.NSAppearanceNameDarkAqua,
			WindowIsTranslucent:  true,
			WebviewIsTransparent: true,
			About: &mac.AboutInfo{
				Title:   "Seriesly",
				Message: "A serial terminal for network devices.",
			},
		},
	})

	if err != nil {
		println("Error:", err.Error())
	}
}
