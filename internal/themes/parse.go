package themes

import (
	"bytes"
	"fmt"

	"howett.net/plist"
)

type itermColor struct {
	Red   float64 `plist:"Red Component"`
	Green float64 `plist:"Green Component"`
	Blue  float64 `plist:"Blue Component"`
	Alpha float64 `plist:"Alpha Component"`
}

func (c itermColor) Hex() string {
	clamp := func(f float64) int {
		v := int(f*255 + 0.5)
		if v < 0 {
			return 0
		}
		if v > 255 {
			return 255
		}
		return v
	}
	return fmt.Sprintf("#%02x%02x%02x", clamp(c.Red), clamp(c.Green), clamp(c.Blue))
}

func (c itermColor) IsSet() bool {
	return c.Alpha > 0 || c.Red > 0 || c.Green > 0 || c.Blue > 0
}

type itermTheme struct {
	Ansi0  itermColor `plist:"Ansi 0 Color"`
	Ansi1  itermColor `plist:"Ansi 1 Color"`
	Ansi2  itermColor `plist:"Ansi 2 Color"`
	Ansi3  itermColor `plist:"Ansi 3 Color"`
	Ansi4  itermColor `plist:"Ansi 4 Color"`
	Ansi5  itermColor `plist:"Ansi 5 Color"`
	Ansi6  itermColor `plist:"Ansi 6 Color"`
	Ansi7  itermColor `plist:"Ansi 7 Color"`
	Ansi8  itermColor `plist:"Ansi 8 Color"`
	Ansi9  itermColor `plist:"Ansi 9 Color"`
	Ansi10 itermColor `plist:"Ansi 10 Color"`
	Ansi11 itermColor `plist:"Ansi 11 Color"`
	Ansi12 itermColor `plist:"Ansi 12 Color"`
	Ansi13 itermColor `plist:"Ansi 13 Color"`
	Ansi14 itermColor `plist:"Ansi 14 Color"`
	Ansi15 itermColor `plist:"Ansi 15 Color"`

	Background   itermColor `plist:"Background Color"`
	Foreground   itermColor `plist:"Foreground Color"`
	Cursor       itermColor `plist:"Cursor Color"`
	CursorText   itermColor `plist:"Cursor Text Color"`
	Selection    itermColor `plist:"Selection Color"`
	SelectedText itermColor `plist:"Selected Text Color"`
}

// ParseItermColors parses an iTerm2 .itermcolors XML plist into a Theme.
// The name is used as the display name and slug source for the ID.
func ParseItermColors(data []byte, name string) (Theme, error) {
	var it itermTheme
	decoder := plist.NewDecoder(bytes.NewReader(data))
	if err := decoder.Decode(&it); err != nil {
		return Theme{}, fmt.Errorf("decode plist: %w", err)
	}

	t := Theme{
		ID:     slugify(name),
		Name:   name,
		Source: "user",

		Background: it.Background.Hex(),
		Foreground: it.Foreground.Hex(),
		Cursor:     it.Cursor.Hex(),
		Selection:  it.Selection.Hex(),

		Black:         it.Ansi0.Hex(),
		Red:           it.Ansi1.Hex(),
		Green:         it.Ansi2.Hex(),
		Yellow:        it.Ansi3.Hex(),
		Blue:          it.Ansi4.Hex(),
		Magenta:       it.Ansi5.Hex(),
		Cyan:          it.Ansi6.Hex(),
		White:         it.Ansi7.Hex(),
		BrightBlack:   it.Ansi8.Hex(),
		BrightRed:     it.Ansi9.Hex(),
		BrightGreen:   it.Ansi10.Hex(),
		BrightYellow:  it.Ansi11.Hex(),
		BrightBlue:    it.Ansi12.Hex(),
		BrightMagenta: it.Ansi13.Hex(),
		BrightCyan:    it.Ansi14.Hex(),
		BrightWhite:   it.Ansi15.Hex(),
	}

	if it.CursorText.IsSet() {
		t.CursorAccent = it.CursorText.Hex()
	}
	if it.SelectedText.IsSet() {
		t.SelectionForeground = it.SelectedText.Hex()
	}

	return t, nil
}
