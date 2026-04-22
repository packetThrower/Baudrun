// Package ostheme exposes the OS-level dark/light appearance preference
// and a subscription for changes to it.
//
// The "OS-level" qualifier matters for Baudrun: this reads the user's
// *system* preference, not the app's own effective appearance. The app
// pins its NSAppearance to dark on macOS so translucent skins (Liquid
// Glass, default Baudrun) render on dark vibrancy material — which means
// the WKWebView's `prefers-color-scheme` media query always reports
// dark, regardless of the OS. This package bypasses the webview and
// queries the native OS APIs directly so the "Auto" appearance
// preference can actually track system dark/light.
package ostheme

// Theme is the user's OS-level appearance preference.
type Theme int

const (
	ThemeLight Theme = iota
	ThemeDark
)

// String returns "light" or "dark". Matches the string value emitted on
// the "system:theme" Wails event.
func (t Theme) String() string {
	if t == ThemeDark {
		return "dark"
	}
	return "light"
}
