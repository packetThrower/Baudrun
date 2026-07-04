//! Window-chrome helpers split out of `app_view/mod.rs`. Each
//! function returns an `IntoElement` (or `gpui::Div`) that the
//! root `Render for AppView` impl composes into the final tree.
//! Nothing here owns state — they all take the current
//! `SkinTokens`, an `&AppView`, or a `&mut Context<AppView>` and
//! produce a fresh subtree.
//!
//! Coverage:
//!   * Right-click profile context menu (overlay + per-row item)
//!   * Suspended-session banner + replacement pane
//!   * About dialog body
//!   * Friendly serial-open error formatter
//!   * Welcome / empty-state pane
//!   * Session header + status bar
//!   * Sidebar header + collapsed icon-strip variant
//!
//! All functions are `pub(super)` so mod.rs imports them by name.
//! Field/method access into the parent `AppView` (e.g.
//! `app.profile_context_menu`, `this.connect_profile_in_new_window`)
//! works without visibility widening — Rust's privacy rule lets
//! descendant modules see the parent's private items.

use std::time::Duration;

use gpui::{
    anchored, deferred, div, prelude::*, pulsating_between, px, rgba, Animation, AnimationExt,
    Context, IntoElement, MouseButton, MouseUpEvent, SharedString, Window,
};
use gpui_component::tooltip::Tooltip;

use super::buttons::{line_pill, pill_button, primary_button};
use super::{AppView, FooterEvent, LogSeverity, STATUS_DOT_PX};
use crate::data::profiles::Profile;
use crate::skin_tokens::{self, SkinTokens};

/// Build the deferred + anchored overlay that renders the profile-
/// row right-click context menu. Returns `None` when no menu is
/// open. Lives outside `Render::render` to keep the body readable.
pub(super) fn profile_context_menu_overlay(
    app: &AppView,
    cx: &mut Context<AppView>,
) -> Option<gpui::AnyElement> {
    let menu = app.profile_context_menu.as_ref()?;
    let s = *cx.global::<SkinTokens>();
    let profile_id = menu.profile_id.clone();
    let pos = menu.pos;
    // When the right-clicked profile is the one this window is
    // already connected to, "Connect in New Window" would either
    // race the existing session for the same port or quietly steal
    // it. Surface "Move to New Window" instead, reusing the same
    // detach/install_session machinery the toolbar Detach button
    // already drives.
    let is_connected_row = app.connected_profile_id.as_deref() == Some(profile_id.as_str());

    let item: gpui::Div = if is_connected_row {
        profile_menu_item(
            s,
            "Move Session to New Window",
            cx.listener(move |this, _: &MouseUpEvent, _window, cx| {
                this.profile_context_menu = None;
                this.move_session_to_new_window(None, cx);
            }),
        )
    } else {
        profile_menu_item(
            s,
            "Connect in New Window",
            cx.listener(move |this, _: &MouseUpEvent, _window, cx| {
                let id = profile_id.clone();
                this.profile_context_menu = None;
                this.connect_profile_in_new_window(id, None, cx);
            }),
        )
    };

    // Two-layer paint: opaque `bg_window` base, then the
    // translucent `bg_panel` skin overlay on top — same frosted
    // look the sidebar / main pane use, except this popup floats
    // in `deferred(anchored(...))` so there's no opaque chrome
    // beneath it. Without the base layer the menu sees right
    // through to whatever's anchored under (terminal grid, editor
    // pane, sidebar) and the text bleeds into the content
    // behind. Border + shadow stay on the outer wrapper so they
    // hug the popup outline. `--shadow-floating` from the active
    // skin overrides `shadow_md()` when present.
    let shadow_floating = cx.global::<skin_tokens::SkinShadows>().floating.clone();
    let panel = div()
        .min_w(px(220.0))
        .bg(rgba(s.bg_window))
        .border_1()
        .border_color(rgba(s.border_subtle))
        .rounded(px(s.radius_md))
        .map(|this| {
            if shadow_floating.is_empty() {
                this.shadow_md()
            } else {
                this.shadow(shadow_floating.clone())
            }
        })
        .child(
            div()
                .w_full()
                .bg(rgba(s.bg_panel))
                .rounded(px(s.radius_md))
                .py_1()
                .child(item),
        );
    Some(deferred(anchored().position(pos).child(panel)).into_any_element())
}

/// One row inside the profile right-click menu. Plain hover-styled
/// div — keeping it hand-rolled rather than reaching for
/// gpui-component's PopupMenu (which routes through the Action
/// system) since we have a single click handler that already needs
/// the per-row profile id baked in.
pub(super) fn profile_menu_item<F>(s: SkinTokens, label: &'static str, on_click: F) -> gpui::Div
where
    F: Fn(&MouseUpEvent, &mut Window, &mut gpui::App) + 'static,
{
    let hover_bg = s.bg_hover;
    div()
        .px_3()
        .py(px(6.0))
        .text_size(px(13.0))
        .text_color(rgba(s.fg_primary))
        .cursor_pointer()
        .hover(move |st| st.bg(rgba(hover_bg)))
        .child(label)
        .on_mouse_up(MouseButton::Left, on_click)
}

/// Idle splash screen — shown when the app is launched with no
/// connected profile and the user hasn't opened the editor yet.
/// Mirrors the Tauri version's "no terminal until you pick a
/// profile" default. Wording adapts to whether any profiles
/// exist: with profiles, prompt to pick one; without, prompt to
/// click the `+` to create one.
/// Thin banner at the top of the editor when the connected profile
/// is being viewed while suspended. Click Resume to switch back to
/// the live terminal viewport.
pub(super) fn suspended_banner(s: SkinTokens, _cx: &mut Context<AppView>) -> impl IntoElement {
    // Banner is a passive reminder — no Resume button. The two
    // ways to resume are still wired:
    //   * click the connected profile row in the sidebar
    //     (handled by `select_profile` at the suspended branch
    //     around line 960).
    //   * close the editor by other means (Escape, ⋯ → Discard).
    // The footer pill the suspend action fires
    // ("Session kept alive in background") gives the initial
    // confirmation; this banner is the steady-state reminder
    // for as long as the editor is on screen.
    div()
        .w_full()
        .px_4()
        .py_2()
        .bg(rgba(s.bg_active))
        .border_b_1()
        .border_color(rgba(s.border_subtle))
        // Floating-card skins (`panel_radius_px > 0`) — macOS 26,
        // Tokyo Night, etc. — wrap the right pane in a rounded
        // container with `overflow_hidden`, but gpui's
        // `overflow_hidden` clips to the bounding box, not the
        // rounded shape. The banner's `bg_active` rectangle would
        // poke past the wrapper's rounded curve as visible sharp
        // top corners. Round our own top corners to match so the
        // banner's fill ends inside the panel's curve. Bottom stays
        // sharp because the form pane continues immediately below.
        .when(s.panel_radius_px > 0.0, |this| {
            let r = px(s.panel_radius_px);
            this.rounded_tl(r).rounded_tr(r)
        })
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgba(s.fg_primary))
                .child("Session suspended"),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgba(s.fg_secondary))
                .child("Port still open. Bytes keep flowing into scrollback."),
        )
}

/// Right-pane placeholder shown when the session is suspended and
/// no editor is open. Shows the connected profile's name + port and
/// a Resume button so the user has a one-click way back to the
/// live terminal.
pub(super) fn suspended_pane(
    s: SkinTokens,
    profile: Profile,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let port_line = if profile.port_name.is_empty() {
        "(no port)".to_string()
    } else {
        format!("{} @ {}", profile.port_name, profile.baud_rate)
    };
    // See the matching comment in `welcome_pane`: skip the bg
    // when the floating-card wrapper is painting it.
    let paint_bg = s.panel_radius_px == 0.0;
    div()
        .flex_1()
        .w_full()
        .h_full()
        .when(paint_bg, |this| this.bg(rgba(s.bg_main)))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_3()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgba(s.fg_tertiary))
                .child("SESSION SUSPENDED"),
        )
        .child(
            div()
                .text_size(px(20.0))
                .text_color(rgba(s.fg_primary))
                .child(profile.name.clone()),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgba(s.fg_secondary))
                .child(port_line),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgba(s.fg_tertiary))
                .child("Port stays open; bytes keep flowing into scrollback."),
        )
        .child(primary_button(s, "Resume").on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _: &MouseUpEvent, window, cx| {
                this.resume_session(window, cx);
            }),
        ))
}

/// Body content for the About dialog: app name, version, one-line
/// description, copyright, and a GitHub link. The Dialog wrapper
/// supplies the title bar and Close button; we just hand back the
/// flex column that sits below the title.
pub(super) fn about_dialog_body() -> impl IntoElement {
    const GITHUB_URL: &str = "https://github.com/packetThrower/Baudrun";
    let version = env!("CARGO_PKG_VERSION");
    div()
        .flex()
        .flex_col()
        .gap_3()
        .pt_2()
        .child(
            div()
                .text_xl()
                .font_weight(gpui::FontWeight::BOLD)
                .child("Baudrun"),
        )
        .child(
            div()
                .text_sm()
                .opacity(0.75)
                .child(format!("Version {version} (prototype)")),
        )
        .child(
            // Tagline pulls from the same source-of-truth as
            // Cargo.toml's `description` so a future product-copy
            // change doesn't have to remember to update two files.
            div()
                .text_sm()
                .child(env!("CARGO_PKG_DESCRIPTION").to_string()),
        )
        .child(
            div()
                .text_sm()
                .opacity(0.65)
                .child("© 2025–2026 packetThrower / Baudrun contributors"),
        )
        .child(
            div()
                .id("about-github-link")
                .text_sm()
                .text_color(gpui::rgba(0x3b82f6ffu32))
                .cursor_pointer()
                .hover(|s| s.text_color(gpui::rgba(0x60a5faffu32)))
                .on_click(|_evt, _window, cx| cx.open_url(GITHUB_URL))
                .child("View on GitHub"),
        )
}

/// Turn a `serialport::Error` from the open path into a string the
/// UI can render directly. On Linux a permission-denied error is
/// almost always a missing-udev-rule / dialout-group situation,
/// so we append a concrete fix-up hint there rather than just
/// showing the bare "Permission denied" text — the same enrich
/// the legacy `data::serial::session::enrich_open_error` did,
/// but at the right call site for the live serial-io path.
pub(super) fn friendly_open_error(port: &str, err: &serialport::Error) -> String {
    let base = err.to_string();
    #[cfg(target_os = "linux")]
    {
        let is_perm = matches!(
            err.kind(),
            serialport::ErrorKind::Io(std::io::ErrorKind::PermissionDenied)
        ) || base.to_ascii_lowercase().contains("permission denied");
        if is_perm {
            return format!(
                "open {port}: {base} — your user can't access this serial port. \
                 Fix: install Baudrun's udev rule (already done if you used the \
                 .deb / .rpm / .pkg.tar.zst installer; rerun the installer if it \
                 didn't take), then unplug + replug the USB adapter. \
                 As a one-off workaround: `sudo chmod 666 {port}` opens it for \
                 the current plug-in. The legacy dialout-group flow \
                 (`sudo usermod -aG dialout $USER` + log out + log in) also \
                 works."
            );
        }
    }
    format!("open {port}: {base}")
}

pub(super) fn welcome_pane(s: SkinTokens, has_profiles: bool) -> impl IntoElement {
    let prompt = if has_profiles {
        "Pick a profile from the sidebar to start a session."
    } else {
        "Click the + above the profile list to create one."
    };
    // Paint `bg_main` here only when the AppView right-pane
    // wrapper isn't already doing so. Floating-card skins
    // (panel_radius_px > 0) get the bg from the wrapper which
    // also handles the rounded clip; flush-edged skins still
    // need the welcome pane itself to lay down the overlay.
    let paint_bg = s.panel_radius_px == 0.0;
    div()
        .flex_1()
        .w_full()
        .h_full()
        .when(paint_bg, |this| this.bg(rgba(s.bg_main)))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_3()
        .child(
            // `--font-size-h1` from the active skin (default
            // 24, macOS-26 ships 26).
            div()
                .text_size(px(s.font_size_h1_px))
                .text_color(rgba(s.fg_primary))
                .child("Baudrun"),
        )
        .child(
            div()
                .text_size(px(13.0))
                .text_color(rgba(s.fg_secondary))
                .child(prompt),
        )
}

/// Session header above the terminal viewport. Shows status dot +
/// profile name + connection meta on the left, and Clear /
/// Disconnect buttons on the right. Only rendered when a profile
/// is actually connected — loopback / no-device modes hide the
/// header so the prototype's no-profile path stays minimal.
pub(super) fn session_header(
    profile: Profile,
    reconnecting: bool,
    overflow_open: bool,
    dtr_asserted: bool,
    rts_asserted: bool,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let parity_letter = match profile.parity.as_str() {
        "odd" => "O",
        "even" => "E",
        "mark" => "M",
        "space" => "S",
        _ => "N",
    };
    // Mirror the Tauri header: full session line always, with
    // " · reconnecting…" appended during a retry window. The
    // appended phrase keeps the port/baud/8N1 info visible so the
    // user knows what the retry is targeting.
    let mut meta = format!(
        "{} · {} /{} {} {}",
        profile.port_name, profile.baud_rate, profile.data_bits, parity_letter, profile.stop_bits,
    );
    if reconnecting {
        meta.push_str(" · reconnecting…");
    }
    let s = *cx.global::<SkinTokens>();
    let dot_color = if reconnecting { s.warn } else { s.success };
    // Skip `bg_main` when the floating-card right-pane wrapper
    // already paints it. gpui's `overflow_hidden` only clips to
    // the bounding box, not the rounded shape, so this header's
    // sharp top-left corner pokes past the wrapper's rounded
    // curve as a visible "square inside the round" on macOS-26.
    // The `border_b_1` still draws the divider line between
    // header and terminal area in both modes.
    let paint_bg = s.panel_radius_px == 0.0;
    div()
        .w_full()
        .px_4()
        .py_2()
        .when(paint_bg, |this| this.bg(rgba(s.bg_main)))
        .border_b_1()
        .border_color(rgba(s.border_subtle))
        .text_size(px(13.0))
        .text_color(rgba(s.fg_primary))
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            div()
                .flex_1()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child({
                    let dot = div()
                        .w(px(STATUS_DOT_PX))
                        .h(px(STATUS_DOT_PX))
                        .rounded_full()
                        .bg(rgba(dot_color));
                    if reconnecting && !cx.global::<crate::ReduceMotion>().0 {
                        // Match the Tauri `.dot.reconnecting`
                        // pulse: 1s ease-in-out, opacity bounces
                        // between roughly 0.35 and 1.0. gpui's
                        // `pulsating_between` returns the easing
                        // curve; the per-frame closure applies the
                        // current alpha. Skipped under prefers-
                        // reduced-motion — the orange dot's colour
                        // alone is enough signal that we're in
                        // the reconnecting state.
                        dot.with_animation(
                            "session-header-reconnect-pulse",
                            Animation::new(Duration::from_secs(1))
                                .repeat()
                                .with_easing(pulsating_between(0.35, 1.0)),
                            |el, delta| el.opacity(delta),
                        )
                        .into_any_element()
                    } else {
                        dot.into_any_element()
                    }
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(rgba(s.fg_primary))
                                .child(profile.name),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgba(s.fg_secondary))
                                .child(meta),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(session_overflow_button(s, overflow_open, cx))
                .child(
                    div()
                        .id("session-dtr")
                        .child(line_pill(s, "DTR", dtr_asserted))
                        .tooltip(move |window, cx| {
                            Tooltip::new(SharedString::from(if dtr_asserted {
                                "DTR is asserted — click to deassert"
                            } else {
                                "DTR is deasserted — click to assert"
                            }))
                            .build(window, cx)
                        })
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                                this.toggle_dtr(cx);
                            }),
                        ),
                )
                .child(
                    div()
                        .id("session-rts")
                        .child(line_pill(s, "RTS", rts_asserted))
                        .tooltip(move |window, cx| {
                            Tooltip::new(SharedString::from(if rts_asserted {
                                "RTS is asserted — click to deassert"
                            } else {
                                "RTS is deasserted — click to assert"
                            }))
                            .build(window, cx)
                        })
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                                this.toggle_rts(cx);
                            }),
                        ),
                )
                .child(pill_button(s, "Clear", false).on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                        this.terminal.update(cx, |t, cx| t.clear_screen(cx));
                    }),
                ))
                .child(
                    div()
                        .id("session-suspend")
                        .child(pill_button(s, "Suspend", false))
                        .tooltip(|window, cx| {
                            Tooltip::new(SharedString::from(
                                "Keep session alive; return to profile",
                            ))
                            .build(window, cx)
                        })
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, window, cx| {
                                this.suspend_session(window, cx);
                            }),
                        ),
                )
                .child(primary_button(s, "Disconnect").on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, window, cx| {
                        this.disconnect_current(window, cx);
                    }),
                )),
        )
}

/// Bottom-of-window status bar — single-line muted text, full
/// window width (sits under both the sidebar and the right pane).
/// Mirrors the Tauri version's footer status: shows the live
/// connection target when connected, the profile being edited
/// when the editor is open, and a neutral "Not connected" when
/// idle. Future slices will hang scan indicators, update toasts,
/// and the undo-delete countdown off this same row.
pub(super) fn status_bar(
    s: SkinTokens,
    connected: Option<&Profile>,
    reconnecting: bool,
    editing_profile_name: Option<&str>,
    scrollback: Option<(usize, usize)>,
    event: Option<&FooterEvent>,
) -> impl IntoElement {
    // Transient event log takes priority over the default
    // connection-state text. When set, the bar shows the event
    // tinted by severity; when None it falls back to the steady-
    // state text below.
    let (text, text_color) = match event {
        Some(ev) => {
            let colour = match ev.severity {
                LogSeverity::Info => rgba(s.fg_secondary),
                LogSeverity::Warn => rgba(s.warn),
                LogSeverity::Error => rgba(s.danger),
            };
            (ev.text.to_string(), colour)
        }
        None => {
            let text = match (connected, editing_profile_name) {
                (Some(p), _) if reconnecting => {
                    format!("Reconnecting to {} @ {}…", p.port_name, p.baud_rate)
                }
                (Some(p), _) => format!("Connected to {} @ {}", p.port_name, p.baud_rate),
                (None, Some(name)) if !name.is_empty() => format!("Editing {name}"),
                (None, Some(_)) => "Editing new profile".to_string(),
                (None, None) => "Not connected".to_string(),
            };
            (text, rgba(s.fg_secondary))
        }
    };
    // Right-side indicators — only when a profile is actually
    // connected (so the bar doesn't show stale chips after
    // disconnect and doesn't reveal flags that haven't taken
    // effect yet on an idle window).
    //
    // Each active flag (`hex_view` / `timestamps` /
    // `line_numbers`) renders as a small uppercase chip. The
    // user gets a quick at-a-glance read of which formatters
    // the live byte stream is going through without having to
    // open the profile editor.
    let (indicators, scrollback_text): (Vec<&'static str>, Option<String>) = match connected {
        Some(p) => {
            let mut tags: Vec<&'static str> = Vec::new();
            if p.hex_view {
                tags.push("HEX");
            }
            if p.timestamps {
                tags.push("TIME");
            }
            if p.line_numbers {
                tags.push("LINE#");
            }
            if p.log_enabled {
                tags.push("TO FILE");
            }
            (
                tags,
                scrollback.map(|(filled, max)| format!("{filled}/{max}")),
            )
        }
        None => (Vec::new(), None),
    };
    let bg_input = s.bg_input;
    let fg_secondary = s.fg_secondary;
    let chip = move |label: &'static str| {
        // Flat pill — no border + no vertical padding so the
        // status bar's overall height stays the same as the
        // text-only baseline. A taller bar steals a row from
        // the terminal viewport and the last line gets clipped.
        div()
            .px(px(6.0))
            .rounded_md()
            .bg(rgba(bg_input))
            .text_color(rgba(fg_secondary))
            .text_size(px(10.0))
            .child(label)
    };
    div()
        .w_full()
        .px_4()
        .py_1()
        .bg(rgba(s.bg_sidebar))
        .border_t_1()
        .border_color(rgba(s.border_subtle))
        .text_size(px(11.0))
        .text_color(rgba(s.fg_secondary))
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(div().flex_1().text_color(text_color).child(text))
        .children(indicators.into_iter().map(chip))
        .children(scrollback_text.map(|t| {
            div()
                .id("status-scrollback")
                .child(t)
                .tooltip(|window, cx| {
                    Tooltip::new(SharedString::from("Scrollback lines: filled / max"))
                        .build(window, cx)
                })
        }))
}

/// Sidebar header row: muted "PROFILES" label on the left, "+"
/// affordance on the right that opens the new-profile form. The
/// "+" is a div-with-click rather than a real button widget — same
/// reasoning as the rest of the sidebar (less surface area than
/// adopting `gpui_component::button` for one element).
///
/// `update_pending` is `true` when the boot-time update check
/// (`crate::updater`) found a newer release the user hasn't
/// dismissed yet. The gear icon gets a small amber dot in the
/// top-right corner — mirrors the dot painted on the Settings
/// rail's "Updates" row so both surfaces feel like one signal.
pub(super) fn sidebar_header(update_pending: bool, cx: &mut Context<AppView>) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();
    let hover_bg = s.bg_hover;
    // Shared chrome for the inline icon-buttons. Each button needs
    // its own stable id so the tooltip layer can disambiguate hover
    // targets, and a label string for the tooltip itself.
    let icon_btn = move |id: &'static str, tip: &'static str| {
        let tip_text = SharedString::from(tip);
        div()
            .id(SharedString::from(id))
            .px_2()
            .rounded_sm()
            .text_color(rgba(s.fg_primary))
            .hover(move |st| st.bg(rgba(hover_bg)))
            .cursor_pointer()
            .tooltip(move |window, cx| Tooltip::new(tip_text.clone()).build(window, cx))
    };
    div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .py_1()
        .child(
            // Left group: collapse-chevron + PROFILES label.
            // Click the chevron (or hit Cmd+B / Ctrl+B) to fold
            // the sidebar into the 48px icon strip.
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .child(
                    // `‹` (U+2039) is the leftward single-angle
                    // quotation mark — same chrome-glyph aesthetic
                    // as the other inline buttons (+ / ⧉ / ⚙), so
                    // the four read as one cluster.
                    icon_btn("nav-collapse-sidebar", "Collapse sidebar")
                        .text_size(px(15.0))
                        .child("\u{2039}")
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                                this.toggle_sidebar(cx);
                            }),
                        ),
                )
                .child(
                    // PROFILES header. Font size + weight + text
                    // transform come from the skin so authors can
                    // tune the label aesthetic without code changes:
                    //   - macOS-26 sets `--label-transform: none` for
                    //     sentence-case ("Profiles");
                    //   - High Contrast / Cyberpunk bump
                    //     `--label-weight` to 600 for chunkier UI.
                    div()
                        .text_size(px(s.font_size_label_px))
                        .text_color(rgba(s.fg_tertiary))
                        .font_weight(gpui::FontWeight(s.label_weight as f32))
                        .child(s.label_transform.apply("PROFILES")),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .child(
                    icon_btn("nav-add-profile", "New profile")
                        .text_size(px(16.0))
                        .child("+")
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, window, cx| {
                                this.open_editor(window, cx);
                            }),
                        ),
                )
                // Unicode "two joined squares" (`⧉`) — the new-window
                // glyph. Same icon-button chrome as `+` and `⚙` so
                // the trio reads as one cluster. macOS users who
                // expect Cmd+N can still use it once Phase 8 wires
                // the application menu.
                .child(
                    icon_btn("nav-new-window", "New window")
                        .text_size(px(15.0))
                        .child("\u{29C9}")
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, window, cx| {
                                this.open_new_window(window, cx);
                            }),
                        ),
                )
                // Unicode gear (`⚙`). Avoids pulling in an icon
                // crate for a single chrome glyph; we can swap to
                // gpui-component's `Icon` later if more accents
                // arrive. Sized one px smaller than the `+` so the
                // two glyphs visually balance — `+` is a thin stroke,
                // the gear is a denser shape.
                //
                // When the boot-time update check has a newer
                // release pending (`update_pending`) we paint a
                // small amber dot in the gear's top-right corner.
                // Wrapping in a `.relative()` div anchors the
                // `.absolute()` dot to the button rather than to
                // the sidebar — the dot rides the gear glyph and
                // disappears with it if the row ever re-flows.
                // Same colour (`s.warn`) + same 8px diameter as
                // the Settings rail's Updates-row dot so both
                // indicators feel like one signal.
                .child(
                    div()
                        .relative()
                        .child(
                            icon_btn("nav-settings", "Settings")
                                .text_size(px(15.0))
                                .child("\u{2699}")
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(|this, _: &MouseUpEvent, window, cx| {
                                        this.open_settings(window, cx);
                                    }),
                                ),
                        )
                        .when(update_pending, |this| {
                            this.child(
                                div()
                                    .absolute()
                                    .top(px(-1.0))
                                    .right(px(-1.0))
                                    .w(px(8.0))
                                    .h(px(8.0))
                                    .rounded_full()
                                    .bg(rgba(s.warn)),
                            )
                        }),
                ),
        )
}

/// Collapsed-mode sidebar contents: the chrome buttons stacked
/// vertically in a 48px strip, with an expand chevron at the top.
/// Mirrors what `sidebar_header` would render in expanded mode
/// (same hover-bg, same tooltip wiring, same actions) but with a
/// vertical layout that fits the narrow strip. The profile list
/// is intentionally absent in this mode — profile names wouldn't
/// fit at 48px wide, and the user expanded the sidebar
/// specifically to skip the list anyway.
pub(super) fn sidebar_icon_strip(
    update_pending: bool,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();
    let hover_bg = s.bg_hover;
    // Same `icon_btn` recipe as `sidebar_header`, but stretched to
    // fill the strip's width so the hover-bg rect reads as a
    // proper button target rather than a small glyph-only chip.
    let icon_btn = move |id: &'static str, tip: &'static str| {
        let tip_text = SharedString::from(tip);
        div()
            .id(SharedString::from(id))
            .w_full()
            .py(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .text_color(rgba(s.fg_primary))
            .hover(move |st| st.bg(rgba(hover_bg)))
            .cursor_pointer()
            .tooltip(move |window, cx| Tooltip::new(tip_text.clone()).build(window, cx))
    };
    div()
        .w_full()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .child(
            // Expand chevron — `›` (U+203A), mirror of the
            // collapse glyph in `sidebar_header`.
            icon_btn("nav-expand-sidebar", "Expand sidebar")
                .text_size(px(16.0))
                .child("\u{203A}")
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                        this.toggle_sidebar(cx);
                    }),
                ),
        )
        .child(
            icon_btn("nav-add-profile-collapsed", "New profile")
                .text_size(px(16.0))
                .child("+")
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, window, cx| {
                        this.open_editor(window, cx);
                    }),
                ),
        )
        .child(
            icon_btn("nav-new-window-collapsed", "New window")
                .text_size(px(15.0))
                .child("\u{29C9}")
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, window, cx| {
                        this.open_new_window(window, cx);
                    }),
                ),
        )
        .child(
            // Settings with the same amber update-pending dot as
            // expanded mode — `.relative()` wrapper anchors the
            // `.absolute()` dot to the gear glyph rather than the
            // strip itself, so the dot rides the icon if the
            // layout ever reflows.
            div()
                .relative()
                .w_full()
                .child(
                    icon_btn("nav-settings-collapsed", "Settings")
                        .text_size(px(15.0))
                        .child("\u{2699}")
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, window, cx| {
                                this.open_settings(window, cx);
                            }),
                        ),
                )
                .when(update_pending, |this| {
                    this.child(
                        div()
                            .absolute()
                            .top(px(2.0))
                            .right(px(10.0))
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(rgba(s.warn)),
                    )
                }),
        )
}

// Chrome colours used to live as `const`s here, but Phase 4 slice 3
// moved them into the `SkinTokens` global so skin picks live-apply.
// Render code reads `cx.global::<SkinTokens>()`; helpers without a
// Context take the `SkinTokens` value as a parameter (Copy, 64
// bytes, cheap to pass).
/// `⋯` button + drop-down menu rendered inline in the session
/// header. The container is `relative` so the menu (positioned
/// `absolute` below) anchors to it; `deferred` puts the panel above
/// other toolbar siblings in paint order.
pub(super) fn session_overflow_button(
    s: SkinTokens,
    open: bool,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    // `--shadow-floating` from the active skin for the popup
    // panel below. Empty (skin sets `"none"` or doesn't declare)
    // falls through to the hardcoded `shadow_md()` so flush-
    // edged skins keep their existing soft drop shadow.
    let shadow_floating = cx.global::<skin_tokens::SkinShadows>().floating.clone();
    let panel: Option<gpui::AnyElement> = if open {
        Some(
            deferred(
                // Two-layer paint: opaque `bg_window` base + the
                // translucent skin overlay on top. See the matching
                // comment in `profile_context_menu_overlay` — same
                // see-through issue when a popup floats over the
                // terminal viewport instead of sitting inside the
                // layered chrome.
                div()
                    .absolute()
                    .top_full()
                    .right_0()
                    .mt_1()
                    .min_w(px(180.0))
                    .bg(rgba(s.bg_window))
                    .border_1()
                    .border_color(rgba(s.border_subtle))
                    .rounded(px(s.radius_md))
                    .map(|this| {
                        if shadow_floating.is_empty() {
                            this.shadow_md()
                        } else {
                            this.shadow(shadow_floating.clone())
                        }
                    })
                    .child(
                        div()
                            .w_full()
                            .bg(rgba(s.bg_panel))
                            .rounded(px(s.radius_md))
                            .py_1()
                            .child(profile_menu_item(
                                s,
                                "Send Break",
                                cx.listener(|this, _: &MouseUpEvent, window, cx| {
                                    this.send_break_now(window, cx);
                                }),
                            ))
                            .child(profile_menu_item(
                                s,
                                "Send Hex\u{2026}",
                                cx.listener(|this, _: &MouseUpEvent, window, cx| {
                                    this.open_send_hex(window, cx);
                                }),
                            ))
                            .child(profile_menu_item(
                                s,
                                "Send File\u{2026}",
                                cx.listener(|this, _: &MouseUpEvent, window, cx| {
                                    this.start_send_file(window, cx);
                                }),
                            ))
                            .child(profile_menu_item(
                                s,
                                "Move to New Window",
                                cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                                    this.move_session_to_new_window(None, cx);
                                }),
                            )),
                    ),
            )
            .into_any_element(),
        )
    } else {
        None
    };
    div()
        .relative()
        .child(
            div()
                .id("session-overflow-btn")
                .px_3()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(rgba(s.border_subtle))
                .bg(rgba(s.bg_input))
                .text_color(rgba(s.fg_primary))
                .text_size(px(13.0))
                .cursor_pointer()
                .hover(move |st| st.bg(rgba(s.bg_hover)))
                .tooltip(|window, cx| {
                    Tooltip::new(SharedString::from("More actions")).build(window, cx)
                })
                .child("\u{22EF}")
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, _, cx| {
                        // Without `stop_propagation`, the AppView
                        // root's mouse-up listener (which dismisses
                        // any open popup) fires immediately after
                        // and closes the menu we just opened.
                        cx.stop_propagation();
                        this.toggle_session_overflow(cx);
                    }),
                ),
        )
        .children(panel)
}
