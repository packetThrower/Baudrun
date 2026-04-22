# Screenshots

A walk-through of Baudrun's interface, features, and built-in skins.
Captured on macOS; Windows and Linux render very similarly (window-
chrome decoration aside). All image pairs below swap automatically
when you toggle this site's theme in the top navigation bar.

## Profiles

The profile list and editor — per-device connection settings (port,
baud, framing, flow control, line ending, send-on-connect sequence,
and more) stored as plain JSON. Clicking a profile opens the serial
port and drops you straight into the terminal.

![Profile list (light)](assets/screenshots/macos-profiles-light.png#only-light)
![Profile list (dark)](assets/screenshots/macos-profiles-dark.png#only-dark)

## Settings

The settings pane covers default theme, skin selection, appearance
mode, font size, session-log directory, config-directory relocation,
and global toggles (screen-reader mode, copy-on-select, USB driver
detection).

![Settings (light)](assets/screenshots/macos-settings-light.png#only-light)
![Settings (dark)](assets/screenshots/macos-settings-dark.png#only-dark)

## Features

### Hex view

Per-profile hex view formats incoming bytes as a 16-byte-per-line
hex + ASCII dump instead of a text stream. Useful for raw protocols
(Modbus RTU, custom binary framing) where you need to see every
byte — including non-printable control characters.

![Hex view (light)](assets/screenshots/macos-hex-view-light.png#only-light)
![Hex view (dark)](assets/screenshots/macos-hex-view-dark.png#only-dark)

### Line timestamps

Optional per-line timestamp prefix. Handy for correlating device
logs with external events, or for long-running captures where you
want to know when each message arrived.

![Line timestamps (light)](assets/screenshots/macos-timestamp-light.png#only-light)
![Line timestamps (dark)](assets/screenshots/macos-timestamp-dark.png#only-dark)

### Advanced features

The profile editor's Advanced section collects control-line
policies (DTR/RTS on connect/disconnect), auto-reconnect, paste
safety (multi-line confirm + slow paste), session logging, and
backspace key mapping. See [Advanced features](ADVANCED.md) for the
full reference on every flag.

![Advanced features (light)](assets/screenshots/macos-advanced-features-light.png#only-light)
![Advanced features (dark)](assets/screenshots/macos-advanced-features-dark.png#only-dark)

## Built-in skins

Baudrun ships with 14 skins spanning modern OSes, retro OSes,
aesthetic styles, and accessibility. Skins swap the app chrome
(colors, window styling, font choices) independently of the terminal
theme — mix freely. See [Skins](SKINS.md) for the full reference and
the authoring guide for custom skins.

### Baudrun (default)

![Baudrun (light)](assets/screenshots/macos-light-baudrun.png#only-light)
![Baudrun (dark)](assets/screenshots/macos-dark-baudrun.png#only-dark)

### macOS 26 (Liquid Glass)

![Liquid Glass (light)](assets/screenshots/macos-light-liquid-glass.png#only-light)
![Liquid Glass (dark)](assets/screenshots/macos-dark-liquid-glass.png#only-dark)

### Windows 11 (Fluent)

![Windows 11 (light)](assets/screenshots/macos-light-windows11.png#only-light)
![Windows 11 (dark)](assets/screenshots/macos-dark-windows11.png#only-dark)

### GNOME (Adwaita)

![GNOME (light)](assets/screenshots/macos-light-gnome.png#only-light)
![GNOME (dark)](assets/screenshots/macos-dark-gnome.png#only-dark)

### High Contrast

![High Contrast (light)](assets/screenshots/macos-light-high-contrast.png#only-light)
![High Contrast (dark)](assets/screenshots/macos-dark-high-contrast.png#only-dark)

### CRT (Green Phosphor)

A dark-only skin evoking a classic phosphor terminal — green-on-
black, DotGothic16 type, subtle scanline treatment.

![CRT](assets/screenshots/macos-dark-crt.png)

### Cyberpunk (Synthwave)

A dark-only skin with saturated magenta / cyan accents.

![Cyberpunk](assets/screenshots/macos-dark-cyberpunk.png)
