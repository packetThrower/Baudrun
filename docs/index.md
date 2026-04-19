# Seriesly documentation

Reference material for Seriesly's advanced features and
customization systems. For the product-level description,
screenshots, and install instructions see the
[top-level README](../README.md).

## Feature reference

**[Advanced features](ADVANCED.md)** — every feature beyond basic
connect-and-type, documented as reference. Send Break, hex send and
hex view, control-line policies (DTR/RTS), session logging, auto-
reconnect, paste safety, suspend/resume, syntax highlighting, theme
preview, light/dark appearance, and the USB-serial driver-detection
banner.

## Customization

**[Profiles](PROFILES.md)** — serial-connection settings. Full JSON
schema, valid values for every enum field, four worked examples
(network switch, USB-C console, Arduino, Modbus RTU debug), and
recipes for bulk-generating `profiles.json` from CSV inventory via
`jq` or Python.

**[Themes](THEMES.md)** — terminal-viewport color schemes. JSON
schema with the 16 ANSI slots plus cursor/selection/background
fields, how the highlighter maps to ANSI slots, `.itermcolors`
import workflow, pointers to the iterm2colorschemes /
mbadolato / Gogh ecosystems, and tips on picking a theme that reads
well on real network-gear output.

**[Skins](SKINS.md)** — app-chrome swaps. Complete CSS-variable
reference grouped by purpose (typography, surfaces, foreground,
borders, semantic colors, radii/elevation, layout, scrollbars,
overlay), light/dark handling caveats given the pinned-dark window
vibrancy on macOS, explicit list of what skins can and can't
reach, a minimal Ocean skin example, and dev tips for iterating
with DevTools.

## Project

**[TODO](../TODO.md)** — roadmap. Items tagged **[on request]** are
features that would be implemented if someone asks for them (macros,
file transfer over XMODEM/YMODEM/ZMODEM, keyboard-shortcut scheme
for Break / Clear / Suspend). Everything else is either done or
in the active backlog.
