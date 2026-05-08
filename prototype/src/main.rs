//! Baudrun · alacritty + gpui prototype.
//!
//! Checkpoint #3: feed bytes through `alacritty_terminal::Term`
//! (driven by `vte::ansi::Processor`) instead of writing the
//! grid by hand. The hardcoded sample now lives as a byte string
//! with embedded ANSI escapes — the same shape a real device
//! would emit. After feeding, `mirror_to_grid` walks the Term's
//! grid and copies cells into the render-side `TerminalGrid`.
//!
//! Validates the integration end-to-end short of a real serial
//! port: VT parsing works, color escapes resolve through our
//! palette, and the rendering pipeline still produces the right
//! output. Real PTY / serial input lands in checkpoint #4.

mod term_bridge;
mod terminal_grid;

use alacritty_terminal::vte::ansi::Rgb;
use gpui::{
    px, App, AppContext, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions,
};

use term_bridge::{make_term, mirror_to_grid};
use terminal_grid::TerminalGrid;

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
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Baudrun (prototype) · checkpoint #3".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_window, cx| {
                cx.new(|_| {
                    let rows = 24;
                    let cols = 80;
                    let mut grid = TerminalGrid::new(rows, cols, DEFAULT_FG, DEFAULT_BG);

                    // Build a Term + Processor pre-loaded with our
                    // palette, feed it the sample bytes, then mirror
                    // the resulting grid into our render-side cells.
                    let (mut term, mut processor) = make_term(rows, cols);
                    processor.advance(&mut term, SAMPLE_BYTES);
                    mirror_to_grid(&term, &mut grid, DEFAULT_FG, DEFAULT_BG);

                    grid
                })
            },
        )
        .expect("open window");
        cx.activate(true);
    });
}
