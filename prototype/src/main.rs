//! Baudrun · alacritty + gpui prototype.
//!
//! Checkpoint #5: real serial input. `cargo run -- <port>` opens a
//! serial port at 9600 8N1, spawns a blocking read thread that ships
//! bytes into a flume channel, and drains that channel from a gpui
//! foreground task into `TerminalView::feed_bytes`. A second thread
//! pumps typed bytes the other direction. With no `<port>` arg the
//! prototype runs in checkpoint-#4 loopback mode so it stays usable
//! without hardware on the dev machine.

mod serial_io;
mod term_bridge;
mod terminal_grid;
mod terminal_view;

use alacritty_terminal::vte::ansi::Rgb;
use gpui::{px, App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions};

use terminal_view::TerminalView;

/// Default foreground / background for the prototype. Matches the
/// `baudrun` built-in theme. Used both to seed the Term's palette
/// (`NamedColor::Foreground` / `Background` slots) and as a
/// fallback inside the resolver for any palette slot that's still
/// `None`.
const DEFAULT_FG: Rgb = Rgb { r: 0xe4, g: 0xe4, b: 0xe7 };
const DEFAULT_BG: Rgb = Rgb { r: 0x0b, g: 0x0b, b: 0x0d };

/// Default baud rate. 9600 8N1 is the universal serial-console speed
/// for the network gear Baudrun targets — Cisco, Juniper, Aruba,
/// Mikrotik all default to it. A real settings panel will eventually
/// parameterize this; for the spike a constant is fine.
const DEFAULT_BAUD: u32 = 9600;

/// Sample byte stream — what a Cisco IOS session might emit if
/// you ran `show running-config` on a session with `terminal
/// monitor` colorization enabled. Mixes:
///   * default-fg plain text
///   * SGR named colors (`\x1b[31m` red, `\x1b[36m` cyan, etc.)
///   * SGR bright colors (`\x1b[91m` bright red etc.)
///   * SGR reset (`\x1b[0m`) between runs
///   * Multiple lines + a final cursor-positioning prompt
///
/// Only fed at boot when running in loopback mode; with a real
/// device attached, the device provides its own output.
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
\x1b[1mbold\x1b[0m \x1b[3mitalic\x1b[0m \x1b[4munderline\x1b[0m \
\x1b[9mstrike\x1b[0m \x1b[2mdim\x1b[0m \x1b[7minverse\x1b[0m\r\n\
\r\n\
\x1b[35mRouter#\x1b[0m ";

fn main() {
    env_logger::init();

    // Args: `cargo run -- <port_path>`. Anything after the binary
    // name; we don't accept flags yet because there's nothing to
    // configure besides the path.
    let port_path = std::env::args().nth(1);

    gpui_platform::application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, gpui::size(px(1100.0), px(720.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Baudrun (prototype) · checkpoint #5".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_window, cx| {
                    let rows = 24;
                    let cols = 80;
                    cx.new(|cx| TerminalView::new(rows, cols, DEFAULT_FG, DEFAULT_BG, cx))
                },
            )
            .expect("open window");

        let view = window.entity(cx).expect("read terminal view entity");

        match port_path.as_deref() {
            Some(path) => match serial_io::open(path, DEFAULT_BAUD) {
                Ok(channels) => {
                    log::info!("opened serial port {path} at {DEFAULT_BAUD} 8N1");
                    // Hand the write half to the view so its key
                    // handler can push typed bytes onto the wire.
                    view.update(cx, |v, _| v.set_serial_tx(channels.write_tx));

                    // Foreground async task: drain the read channel
                    // and pipe each chunk through `feed_bytes`.
                    // Re-renders happen via `cx.notify()` inside
                    // `feed_bytes` itself.
                    let weak = view.downgrade();
                    let read_rx = channels.read_rx;
                    cx.spawn(async move |cx| {
                        while let Ok(bytes) = read_rx.recv_async().await {
                            if weak
                                .update(cx, |v, cx| v.feed_bytes(&bytes, cx))
                                .is_err()
                            {
                                break;
                            }
                        }
                    })
                    .detach();
                }
                Err(e) => {
                    eprintln!(
                        "failed to open serial port {path}: {e}\n\
                         falling back to loopback mode."
                    );
                    seed_loopback(&view, cx);
                }
            },
            None => {
                eprintln!(
                    "no serial port specified — running in loopback mode.\n\
                     usage: cargo run -- <port>      \
                     (macOS: /dev/tty.usbserial-XXX, Windows: COM3, Linux: /dev/ttyUSB0)"
                );
                seed_loopback(&view, cx);
            }
        }

        // Focus the terminal view at startup so keystrokes land
        // without the user having to click first.
        window
            .update(cx, |view, window, cx| {
                view.focus_handle().clone().focus(window, cx);
            })
            .expect("focus terminal view");

        cx.activate(true);
    });
}

/// In loopback mode (no device), feed the boot-time sample so the
/// window opens with colored content rather than a blank grid. Real
/// serial sessions skip this — the device's own output drives the
/// screen instead. Repeated `SEED_REPEATS` times so the prototype
/// has enough content to push earlier lines into alacritty's
/// scrollback — otherwise the wheel-scroll path can't be exercised
/// without a chatty device on the wire.
const SEED_REPEATS: usize = 3;
fn seed_loopback(view: &gpui::Entity<TerminalView>, cx: &mut App) {
    view.update(cx, |v, cx| {
        for i in 0..SEED_REPEATS {
            // SAMPLE_BYTES doesn't end with a newline, so without
            // a separator each repeat would land on the trailing
            // `Router# ` of the previous one. CRLF between them
            // makes the loopback test view actually look like
            // three discrete sessions.
            if i > 0 {
                v.feed_bytes(b"\r\n", cx);
            }
            v.feed_bytes(SAMPLE_BYTES, cx);
        }
    });
}
