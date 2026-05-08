//! Baudrun · alacritty + gpui prototype.
//!
//! Checkpoint #4: keyboard input. The window now mounts a
//! `TerminalView` entity that owns the Term + Processor + grid,
//! captures gpui keyboard events, encodes them as bytes, and feeds
//! them through the same parser path the boot-time sample uses.
//! Without a real serial port (checkpoint #5) the typed bytes loop
//! straight back into our own Term as local echo — verifies the
//! whole keyboard pipeline works, and gives us a terminal you can
//! type into with no device attached.

mod term_bridge;
mod terminal_grid;
mod terminal_view;

use alacritty_terminal::vte::ansi::Rgb;
use gpui::{
    px, App, AppContext, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions,
};

use terminal_view::TerminalView;

/// Default foreground / background for the prototype. Matches the
/// `baudrun` built-in theme. Used both to seed the Term's palette
/// (`NamedColor::Foreground` / `Background` slots) and as a
/// fallback inside the resolver for any palette slot that's still
/// `None`.
const DEFAULT_FG: Rgb = Rgb { r: 0xe4, g: 0xe4, b: 0xe7 };
const DEFAULT_BG: Rgb = Rgb { r: 0x0b, g: 0x0b, b: 0x0d };

/// Sample byte stream — what a Cisco IOS session might emit if
/// you ran `show running-config` on a session with `terminal
/// monitor` colorization enabled. Mixes:
///   * default-fg plain text
///   * SGR named colors (`\x1b[31m` red, `\x1b[36m` cyan, etc.)
///   * SGR bright colors (`\x1b[91m` bright red etc.)
///   * SGR reset (`\x1b[0m`) between runs
///   * Multiple lines + a final cursor-positioning prompt
///
/// Every escape here is one alacritty parses; we're not feeding
/// it anything exotic. If the rendered output matches what
/// `printf "$BYTES" | less -R` would show in a real terminal,
/// the bridge is working.
const SAMPLE_BYTES: &[u8] = b"\
\x1b[0m\
Router> \x1b[36mshow running-config\x1b[0m\r\n\
\r\n\
\x1b[90mBuilding configuration...\x1b[0m\r\n\
\r\n\
\x1b[90m!\x1b[0m\r\n\
\x1b[90mversion 15.4\x1b[0m\r\n\
\x1b[90mservice timestamps debug datetime msec\x1b[0m\r\n\
\x1b[90mservice password-encryption\x1b[0m\r\n\
\x1b[90m!\x1b[0m\r\n\
\r\n\
\x1b[34minterface GigabitEthernet0/1\x1b[0m\r\n\
  ip address \x1b[32m10.10.10.1\x1b[0m \x1b[32m255.255.255.0\x1b[0m\r\n\
  no ip redirects\r\n\
  duplex full\r\n\
  speed 1000\r\n\
\r\n\
\x1b[34minterface GigabitEthernet0/2\x1b[0m\r\n\
  \x1b[31mshutdown\x1b[0m\r\n\
  description \x1b[33mTO-CORE-SW1\x1b[0m\r\n\
\r\n\
\x1b[35mRouter#\x1b[0m ";

fn main() {
    env_logger::init();

    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, gpui::size(px(1100.0), px(720.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Baudrun (prototype) · checkpoint #4".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_window, cx| {
                    let rows = 24;
                    let cols = 80;
                    cx.new(|cx| {
                        let mut view =
                            TerminalView::new(rows, cols, DEFAULT_FG, DEFAULT_BG, cx);
                        // Boot-time sample so the window opens with
                        // colored content rather than a blank grid.
                        // After this, all bytes come from the keyboard.
                        view.feed_bytes(SAMPLE_BYTES, cx);
                        view
                    })
                },
            )
            .expect("open window");

        // Focus the terminal view at startup so keystrokes land
        // without the user having to click first.
        window
            .update(cx, |view, window, _cx| {
                view.focus_handle().clone().focus(window);
            })
            .expect("focus terminal view");

        cx.activate(true);
    });
}
