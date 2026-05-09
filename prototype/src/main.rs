//! Baudrun · alacritty + gpui prototype.
//!
//! Checkpoint #5: real serial input. `cargo run -- <port>` opens a
//! serial port at 9600 8N1, spawns a blocking read thread that ships
//! bytes into a flume channel, and drains that channel from a gpui
//! foreground task into `TerminalView::feed_bytes`. A second thread
//! pumps typed bytes the other direction. With no `<port>` arg the
//! prototype runs in checkpoint-#4 loopback mode so it stays usable
//! without hardware on the dev machine.

mod app_view;
mod data;
mod serial_io;
mod settings_view;
mod term_bridge;
mod terminal_grid;
mod terminal_view;

use std::rc::Rc;

use alacritty_terminal::vte::ansi::Rgb;
use gpui::{
    px, rgba, App, AppContext, Bounds, Hsla, TitlebarOptions, WindowBounds, WindowOptions,
};
use gpui_component::{scroll::ScrollbarShow, Root, Theme, ThemeMode};

use app_view::AppView;
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
        // gpui-component widgets (Input, Form, Dialog, …) need a
        // global theme + tooltip/notification manager installed
        // before any of them render. `init` is the canonical setup
        // call — without it the first `Input::new` panics looking
        // for the Theme global. The widgets we mounted before
        // Phase 2.4 (plain divs only) didn't need this; the moment
        // an Input appears, this is mandatory.
        gpui_component::init(cx);
        // `init` defaults the theme to Light, which makes our
        // chrome (built on dark bg constants from the Baudrun
        // skin) ship with black text + a black cursor. Flip to
        // Dark so widget-rendered text (Checkbox labels, Input
        // cursor, Select chevron) picks up the white-on-dark
        // palette that matches our hand-styled bits.
        Theme::change(ThemeMode::Dark, None, cx);

        // Tighten the gpui-component widget colors. The `input`
        // field doubles as both the widget border colour and (via
        // `0.7 * input` in `Theme::input_background`) the widget
        // bg in dark mode. The Baudrun skin's `--bg-input` is 8%
        // white, but at that opacity the widget bg blends almost
        // perfectly into the section card bg — visually the field
        // disappears. We push to ~25% which gives a ~17% bg and a
        // clearly visible field, trading spec-purity for usability
        // (per direct user feedback: "the inputs lost their colour
        // and are dark again, can't tell where they are").
        let theme = Theme::global_mut(cx);
        let input_border: Hsla = rgba(0xFFFFFF40).into();
        theme.input = input_border;
        // Force scrollbars to always paint. The default
        // (`Hover` on macOS unless system "always show scrollbars"
        // is set) makes the form look like there's no overflow at
        // rest, which loses the affordance — users don't realise
        // there's more content below the fold.
        theme.scrollbar_show = ScrollbarShow::Always;

        // Build the profile + settings stores once at startup. Both
        // read from the user's real config dir (same paths the
        // existing main app uses), so any profiles or settings
        // created in the shipping build appear in the prototype
        // without manual setup. Either store falling back to an
        // empty/default state still lets the UI render — no crashes.
        let support = data::appdata::support_dir();
        let profile_store = match &support {
            Ok(dir) => match data::profiles::Store::new(dir) {
                Ok(store) => Rc::new(store),
                Err(err) => fallback_profile_store(format!("profile store init failed: {err}")),
            },
            Err(err) => {
                fallback_profile_store(format!("support dir unavailable: {err}"))
            }
        };
        let settings_store = match &support {
            Ok(dir) => match data::settings::Store::new(dir) {
                Ok(store) => Rc::new(store),
                Err(err) => fallback_settings_store(format!(
                    "settings store init failed: {err}"
                )),
            },
            Err(err) => fallback_settings_store(format!("support dir unavailable: {err}")),
        };
        let skins_store = match &support {
            Ok(dir) => match data::skins::Store::new(dir) {
                Ok(store) => Rc::new(store),
                Err(err) => fallback_skins_store(format!("skins store init failed: {err}")),
            },
            Err(err) => fallback_skins_store(format!("support dir unavailable: {err}")),
        };
        let highlight_store = match &support {
            Ok(dir) => match data::highlight::Store::new(dir) {
                Ok(store) => Rc::new(store),
                Err(err) => fallback_highlight_store(format!(
                    "highlight store init failed: {err}"
                )),
            },
            Err(err) => fallback_highlight_store(format!("support dir unavailable: {err}")),
        };
        let themes_store = match &support {
            Ok(dir) => match data::themes::Store::new(dir) {
                Ok(store) => Rc::new(store),
                Err(err) => fallback_themes_store(format!("themes store init failed: {err}")),
            },
            Err(err) => fallback_themes_store(format!("support dir unavailable: {err}")),
        };

        let bounds = Bounds::centered(None, gpui::size(px(1100.0), px(720.0)), cx);

        // Build the TerminalView entity first; AppView will own a
        // handle to it and render it inside the right pane.
        let terminal = cx.new(|cx| TerminalView::new(24, 80, DEFAULT_FG, DEFAULT_BG, cx));

        let profile_store_for_window = profile_store.clone();
        let settings_store_for_window = settings_store.clone();
        let skins_store_for_window = skins_store.clone();
        let highlight_store_for_window = highlight_store.clone();
        let themes_store_for_window = themes_store.clone();
        let terminal_for_window = terminal.clone();
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Baudrun (prototype) · phase 3".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    // The window root must be a `Root` so the
                    // tooltip/notification/modal layer paints on top
                    // of (and dispatches events through) the rest of
                    // the UI. Our actual app view lives as Root's
                    // child.
                    let app_view = cx.new(|cx| {
                        AppView::new(
                            terminal_for_window,
                            profile_store_for_window,
                            settings_store_for_window,
                            skins_store_for_window,
                            highlight_store_for_window,
                            themes_store_for_window,
                            cx,
                        )
                    });
                    cx.new(|cx| Root::new(app_view, window, cx))
                },
            )
            .expect("open window");

        // Re-bind for the rest of the function (serial / focus
        // wiring still operates on the TerminalView directly).
        let view = terminal;

        match port_path.as_deref() {
            // CLI fallback path predates the profile system, so it
            // can't carry per-profile DTR/RTS policies — pass the
            // default (leave-as-is) policies on every line.
            Some(path) => match serial_io::open(path, DEFAULT_BAUD, Default::default()) {
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

        // Focus the TerminalView at startup so keystrokes land in
        // the grid without the user having to click first. The
        // window root is now AppView, but we still want focus on
        // the inner viewport — pull its focus_handle directly from
        // the Entity<TerminalView> we stashed before opening the
        // window.
        let viewport_focus = view.read(cx).focus_handle().clone();
        window
            .update(cx, |_, window, cx| viewport_focus.focus(window, cx))
            .expect("focus terminal view");

        cx.activate(true);
    });
}

/// Stand up an empty profile store under a tmpdir as a last-resort
/// fallback so the UI can still render a (blank) sidebar even when
/// the user's real config dir is unreachable. Logs why we fell
/// back so the user can fix the underlying problem.
fn fallback_profile_store(reason: String) -> Rc<data::profiles::Store> {
    eprintln!("{reason}; using empty in-tmpdir profile store");
    let tmp = std::env::temp_dir().join("baudrun-prototype-empty");
    Rc::new(
        data::profiles::Store::new(&tmp)
            .expect("temp profile store should always init"),
    )
}

/// Settings-store equivalent of `fallback_profile_store`: lets the
/// app open with the built-in `Settings::default()` when we can't
/// touch the real config dir (read-only home, missing perms…).
/// Edits made via the UI in this state still write to the tmpdir
/// path and are lost between launches — that's the trade for not
/// crashing.
fn fallback_settings_store(reason: String) -> Rc<data::settings::Store> {
    eprintln!("{reason}; using default in-tmpdir settings store");
    let tmp = std::env::temp_dir().join("baudrun-prototype-empty");
    Rc::new(
        data::settings::Store::new(&tmp)
            .expect("temp settings store should always init"),
    )
}

/// Skins-store fallback. Built-in skins are still available — they
/// embed at compile time — so the user-skin import path is the only
/// thing this fallback loses. Same trade as the other fallbacks:
/// the UI keeps working, edits round-trip to the tmpdir until the
/// real config dir comes back.
fn fallback_skins_store(reason: String) -> Rc<data::skins::Store> {
    eprintln!("{reason}; using empty in-tmpdir skins store");
    let tmp = std::env::temp_dir().join("baudrun-prototype-empty");
    Rc::new(
        data::skins::Store::new(&tmp)
            .expect("temp skins store should always init"),
    )
}

/// Highlight-pack-store fallback. Bundled packs (built-in vendor
/// rule sets) embed at compile time so the picker still has rows
/// to render; only the user pack + custom imports are lost.
fn fallback_highlight_store(reason: String) -> Rc<data::highlight::Store> {
    eprintln!("{reason}; using empty in-tmpdir highlight store");
    let tmp = std::env::temp_dir().join("baudrun-prototype-empty");
    Rc::new(
        data::highlight::Store::new(&tmp)
            .expect("temp highlight store should always init"),
    )
}

/// Themes-store fallback. Built-in themes embed at compile time so
/// the picker is still populated; only user-imported `.itermcolors`
/// / JSON themes are lost.
fn fallback_themes_store(reason: String) -> Rc<data::themes::Store> {
    eprintln!("{reason}; using empty in-tmpdir themes store");
    let tmp = std::env::temp_dir().join("baudrun-prototype-empty");
    Rc::new(
        data::themes::Store::new(&tmp)
            .expect("temp themes store should always init"),
    )
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
