//! Window-opening entry point split out of `app_view/mod.rs`. Used
//! by `main.rs` at startup and by `AppView::open_new_window` /
//! `move_session_to_new_window` for spawning additional windows.

use std::rc::Rc;

use gpui::{
    px, AppContext, Bounds, Entity, TitlebarOptions, WindowBounds, WindowHandle, WindowOptions,
};
use gpui_component::{Root, TitleBar};

use super::session::{bounds_to_geometry, geometry_to_bounds, WindowInit};
use super::AppView;
use crate::data::{highlight, profiles, skins, themes};
use crate::settings_bus::SettingsBus;

/// Universal macOS traffic-light position used for every window
/// we open — both the main window opened by [`open_app_window`]
/// below and the Settings window opened by
/// `AppView::open_settings_window` in `mod.rs`, which imports
/// this const via `use window::TRAFFIC_LIGHT_POSITION_PX;`.
///
/// `(16, 16)` is a compromise between the two layouts:
///
///   - Floating-card skins (macOS-26): the lights sit visibly
///     inset into the sidebar card's top-left rather than hugging
///     the very window corner. (16, 16) clears the 18px sidebar
///     `panel_radius` curve.
///   - Flush-edged skins (every other built-in): the lights sit
///     within the 34px in-flex title-bar strip — at y=16 they're
///     roughly centered vertically (lights are ~12px tall, so
///     they span y=16-28 within the strip).
///
/// We don't resolve per-skin because gpui's macOS backend only
/// reads `WindowOptions.titlebar.traffic_light_position` at window
/// creation — there's no runtime setter to react to a live skin
/// swap. One fixed position keeps the lights from getting stuck in
/// the wrong spot when a user switches between flush and floating-
/// card skins without restarting.
pub(super) const TRAFFIC_LIGHT_POSITION_PX: f32 = 16.0;

/// Open a new top-level Baudrun window with a fresh `AppView`. The
/// stores + `SettingsBus` are shared (cloned `Rc`/`Entity`) so each
/// window's settings live-update in lockstep with the others, but
/// the `TerminalView`, sidebar, profile editor, and transfer state
/// are per-window — connecting in one window doesn't touch the
/// terminal in another. Used both at startup (one window) and from
/// `AppView::open_new_window` / `move_session_to_new_window`.
// 8 args: five of them are the shared store handles every window
// clones. A params struct would just relocate the noise to six
// call sites; revisit if the list grows again.
#[allow(clippy::too_many_arguments)]
pub fn open_app_window(
    cx: &mut gpui::App,
    init: WindowInit,
    origin: Option<gpui::Point<gpui::Pixels>>,
    profile_store: Rc<profiles::Store>,
    settings_bus: Entity<SettingsBus>,
    skins_store: Rc<skins::Store>,
    highlight_store: Rc<highlight::Store>,
    themes_store: Rc<themes::Store>,
) -> gpui::Result<WindowHandle<Root>> {
    let current_settings = settings_bus.read(cx).current().clone();
    let restore_state = !current_settings.disable_window_state_restore;
    let mut bounds = restore_state
        .then(|| {
            current_settings
                .main_window
                .as_ref()
                .and_then(geometry_to_bounds)
        })
        .flatten()
        .unwrap_or_else(|| Bounds::centered(None, gpui::size(px(1100.0), px(720.0)), cx));
    // Tear-off drops pass the release point (global screen coords)
    // so the new window lands under the cursor instead of at the
    // remembered position. Only the origin is overridden — the size
    // still honours the restored geometry. Offset mirrors zorite's
    // tear-off: the cursor ends up over the new window's title
    // area rather than its top-left corner. No clamping — negative
    // coords are legal on multi-monitor layouts (screens left of /
    // above the primary), and the OS constrains genuinely
    // off-screen windows itself.
    if let Some(p) = origin {
        bounds.origin = gpui::point(p.x - px(160.0), p.y - px(12.0));
    }
    let settings_bus_for_close = settings_bus.clone();
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            // `appears_transparent` + `traffic_light_position`
            // come from `TitleBar::title_bar_options()`; we
            // preserve `title` so the OS taskbar / dock /
            // window-list still labels the window. The custom
            // `TitleBar` widget (added at the top of the AppView
            // render) draws the visible chrome — on macOS the
            // native title bar paints transparently so the
            // traffic lights float over the widget; on
            // Windows / Linux the native chrome is hidden and
            // the widget draws its own min/max/close controls.
            // Required for GNOME-Wayland, where the compositor
            // refuses xdg-decoration and a server-side title bar
            // is never rendered.
            titlebar: Some(TitlebarOptions {
                title: Some("Baudrun".into()),
                // Fixed (16, 16) for every skin — see
                // `TRAFFIC_LIGHT_POSITION_PX`. macOS doesn't let
                // gpui re-position the lights at runtime, so we
                // can't track skin swaps live; one universal
                // position avoids stuck-in-the-wrong-place lights
                // when the user changes skins without restarting.
                traffic_light_position: Some(gpui::point(
                    px(TRAFFIC_LIGHT_POSITION_PX),
                    px(TRAFFIC_LIGHT_POSITION_PX),
                )),
                ..TitleBar::title_bar_options()
            }),
            // app_id matches `StartupWMClass=Baudrun` in
            // packaging/linux/baudrun.desktop so GNOME / KDE can
            // associate the live window with the installed
            // .desktop entry and show the Baudrun dock / taskbar
            // icon. No-op on macOS (CFBundleIdentifier handles
            // this) and on Windows (PE resource icon).
            app_id: Some("Baudrun".into()),
            ..Default::default()
        },
        move |window, cx| {
            // Resize hit-test margin for the WM/compositor. See the
            // matching comment in `AppView::open_settings_window`
            // — fixes the unreachably-thin diagonal resize zone on
            // Linux Wayland client-side decorations. No-op on
            // macOS / Windows.
            window.set_client_inset(px(10.0));
            // Snapshot bounds when the OS asks the window to close.
            // Reads the live `disable_window_state_restore` flag so a
            // user who turned the feature off after open doesn't get
            // their pin overwritten on quit.
            window.on_window_should_close(cx, move |window, cx| {
                let geom = bounds_to_geometry(window.bounds());
                settings_bus_for_close.update(cx, |bus, cx| {
                    let mut next = bus.current().clone();
                    if !next.disable_window_state_restore {
                        next.main_window = Some(geom);
                        if let Err(err) = bus.replace(next, cx) {
                            log::error!("save window state: {err}");
                        }
                    }
                });
                true
            });
            // Snapshot the OS appearance for the picker's "Auto" pick;
            // the appearance observer below picks up live changes.
            let system_dark = matches!(
                window.appearance(),
                gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
            );
            let (terminal, session, auto_connect_id) = match init {
                WindowInit::Fresh(t) => (t, None, None),
                WindowInit::WithSession(bundle) => {
                    // Unbox here so downstream `install_session`
                    // doesn't need to know about the Box; the
                    // variant's box exists purely to keep
                    // `WindowInit`'s stack footprint small.
                    let bundle = *bundle;
                    (bundle.terminal.clone(), Some(bundle), None)
                }
                WindowInit::FreshAutoConnect {
                    terminal,
                    profile_id,
                } => (terminal, None, Some(profile_id)),
            };
            let app_view = cx.new(|cx| {
                AppView::new(
                    terminal,
                    profile_store,
                    settings_bus,
                    skins_store,
                    highlight_store,
                    themes_store,
                    system_dark,
                    cx,
                )
            });
            app_view.update(cx, |this, view_cx| {
                this.attach_appearance_observer(window, view_cx);
                if let Some(bundle) = session {
                    this.install_session(bundle, view_cx);
                }
                if let Some(id) = auto_connect_id {
                    if let Some(profile) = this.profile_store.get(&id) {
                        this.connect_to(profile, view_cx);
                    } else {
                        log::warn!("auto-connect: profile {id:?} not found in store");
                    }
                }
            });
            cx.new(|cx| Root::new(app_view, window, cx))
        },
    )
}
