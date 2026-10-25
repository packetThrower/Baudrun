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
//! `app.connected_profile_id`, `this.connect_profile_in_new_window`)
//! works without visibility widening — Rust's privacy rule lets
//! descendant modules see the parent's private items.

use std::{borrow::Cow, time::Duration};

use gpui::{
    div, prelude::*, pulsating_between, px, rgba, Anchor, Animation, AnimationExt, ClickEvent,
    Context, Entity, IntoElement, SharedString, Window,
};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    empty::{Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    link::Link,
    menu::{DropdownMenu, PopupMenu, PopupMenuItem},
    status_bar::StatusBar,
    tag::Tag,
    tooltip::Tooltip,
    IconName, Sizable,
};

use super::buttons::{line_pill, pill_button, primary_button};
use super::{AppView, FooterEvent, LogSeverity, STATUS_DOT_PX};
use crate::data::profiles::Profile;
use crate::skin_tokens::SkinTokens;

/// Right-click menu for a sidebar profile row (GPUI Kit `PopupMenu`
/// via `.context_menu`). When this window is already connected to
/// the profile, "Connect in New Window" would race the session for
/// the same port, so offer "Move Session to New Window" instead —
/// the same detach machinery the overflow menu drives.
pub(super) fn profile_row_menu(
    menu: PopupMenu,
    app: &Entity<AppView>,
    profile_id: &str,
    cx: &gpui::App,
) -> PopupMenu {
    let connected = app.read(cx).connected_profile_id.as_deref() == Some(profile_id);
    let app = app.clone();
    if connected {
        menu.item(
            PopupMenuItem::new(t!("chrome.move_session_to_new_window")).on_click(
                move |_, _, cx| {
                    app.update(cx, |this, cx| this.move_session_to_new_window(None, cx))
                },
            ),
        )
    } else {
        let id = profile_id.to_string();
        menu.item(
            PopupMenuItem::new(t!("chrome.connect_in_new_window")).on_click(move |_, _, cx| {
                app.update(cx, |this, cx| {
                    this.connect_profile_in_new_window(id.clone(), None, cx)
                })
            }),
        )
    }
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
                .child(t!("chrome.session_suspended_title")),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgba(s.fg_secondary))
                .child(t!("chrome.suspended_banner_detail")),
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
        t!("chrome.no_port").to_string()
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
        .items_center()
        .justify_center()
        .child(
            Empty::new()
                .header(
                    EmptyHeader::new()
                        .media(
                            EmptyMedia::new().child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgba(s.fg_tertiary))
                                    .child(t!("chrome.session_suspended_heading")),
                            ),
                        )
                        .title(
                            EmptyTitle::new()
                                .text_size(px(20.0))
                                .text_color(rgba(s.fg_primary))
                                .child(profile.name.clone()),
                        )
                        .description(
                            EmptyDescription::new()
                                .text_color(rgba(s.fg_secondary))
                                .child(port_line)
                                .child(
                                    div()
                                        .text_color(rgba(s.fg_tertiary))
                                        .child(t!("chrome.suspended_pane_detail")),
                                ),
                        ),
                )
                .content(EmptyContent::new().child(
                    primary_button("suspended-resume", t!("chrome.resume_button")).on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.resume_session(window, cx);
                        }),
                    ),
                )),
        )
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
                .child(t!("chrome.about_version", version = version)),
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
                .child(t!("chrome.about_copyright")),
        )
        .child(
            Link::new("about-github-link")
                .href(GITHUB_URL)
                .text_sm()
                .child(t!("chrome.view_on_github")),
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
            return t!(
                "chrome.open_error_permission_linux",
                port = port,
                base = base
            )
            .to_string();
        }
    }
    t!("chrome.open_error", port = port, base = base).to_string()
}

pub(super) fn welcome_pane(s: SkinTokens, has_profiles: bool) -> impl IntoElement {
    let prompt = if has_profiles {
        t!("welcome.pick_profile")
    } else {
        t!("welcome.create_first")
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
        .items_center()
        .justify_center()
        .child(
            Empty::new()
                .header(
                    EmptyHeader::new()
                        // `--font-size-h1` from the active skin
                        // (default 24, macOS-26 ships 26).
                        .title(
                            EmptyTitle::new()
                                .text_size(px(s.font_size_h1_px))
                                .text_color(rgba(s.fg_primary))
                                .child(t!("welcome.heading")),
                        )
                        .description(
                            EmptyDescription::new()
                                .text_color(rgba(s.fg_secondary))
                                .child(prompt),
                        ),
                )
                .content(EmptyContent::new().child(
                    primary_button("welcome-new-profile", t!("welcome.new_profile")).on_click(
                        |_, window, cx| window.dispatch_action(Box::new(crate::NewProfile), cx),
                    ),
                )),
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
        meta.push_str(&t!("chrome.reconnecting_suffix"));
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
                .child(session_overflow_button(cx))
                .child(
                    line_pill(s, "session-dtr", "DTR", dtr_asserted)
                        .tooltip(if dtr_asserted {
                            t!("chrome.dtr_asserted_tooltip")
                        } else {
                            t!("chrome.dtr_deasserted_tooltip")
                        })
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.toggle_dtr(cx);
                        })),
                )
                .child(
                    line_pill(s, "session-rts", "RTS", rts_asserted)
                        .tooltip(if rts_asserted {
                            t!("chrome.rts_asserted_tooltip")
                        } else {
                            t!("chrome.rts_deasserted_tooltip")
                        })
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.toggle_rts(cx);
                        })),
                )
                .child(
                    pill_button(s, "session-clear", t!("chrome.clear_button"), false).on_click(
                        cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.terminal.update(cx, |t, cx| t.clear_screen(cx));
                        }),
                    ),
                )
                .child(
                    pill_button(s, "session-suspend", t!("chrome.suspend_button"), false)
                        .tooltip(t!("chrome.suspend_tooltip"))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.suspend_session(window, cx);
                        })),
                )
                .child(
                    primary_button("session-disconnect", t!("chrome.disconnect_button")).on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.disconnect_current(window, cx);
                        }),
                    ),
                ),
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
                (Some(p), _) if reconnecting => t!(
                    "chrome.reconnecting",
                    port = p.port_name,
                    baud = p.baud_rate
                )
                .to_string(),
                (Some(p), _) => {
                    t!("chrome.connected", port = p.port_name, baud = p.baud_rate).to_string()
                }
                (None, Some(name)) if !name.is_empty() => {
                    t!("chrome.editing", name = name).to_string()
                }
                (None, Some(_)) => t!("chrome.editing_new").to_string(),
                (None, None) => t!("chrome.not_connected").to_string(),
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
    let (indicators, scrollback_text): (Vec<Cow<'static, str>>, Option<String>) = match connected {
        Some(p) => {
            let mut tags: Vec<Cow<'static, str>> = Vec::new();
            if p.hex_view {
                tags.push(t!("chrome.chip_hex"));
            }
            if p.timestamps {
                tags.push(t!("chrome.chip_time"));
            }
            if p.line_numbers {
                tags.push(t!("chrome.chip_line"));
            }
            if p.log_enabled {
                tags.push(t!("chrome.chip_to_file"));
            }
            (
                tags,
                scrollback.map(|(filled, max)| format!("{filled}/{max}")),
            )
        }
        None => (Vec::new(), None),
    };
    // Flat chips — no vertical padding so the bar stays at the
    // text-only height. A taller bar steals a row from the terminal
    // viewport and the last line gets clipped.
    let chip = |label: Cow<'static, str>| {
        Tag::custom(
            rgba(s.bg_input).into(),
            rgba(s.fg_secondary).into(),
            gpui::transparent_black(),
        )
        .xsmall()
        .py_0()
        .child(label)
    };
    let mut bar = StatusBar::new()
        .px_4()
        .bg(rgba(s.bg_sidebar))
        .border_color(rgba(s.border_subtle))
        .text_size(px(11.0))
        .text_color(rgba(s.fg_secondary))
        .child(div().flex_1().text_color(text_color).child(text));
    for label in indicators {
        bar = bar.right(chip(label));
    }
    if let Some(t) = scrollback_text {
        bar = bar.right(
            div()
                .id("status-scrollback")
                .child(t)
                .tooltip(|window, cx| {
                    Tooltip::new(SharedString::from(t!("chrome.scrollback_tooltip")))
                        .build(window, cx)
                }),
        );
    }
    bar
}

/// Sidebar header row: muted "PROFILES" label on the left, "+"
/// affordance on the right that opens the new-profile form. The
/// `update_pending` is `true` when the boot-time update check
/// (`crate::updater`) found a newer release the user hasn't
/// dismissed yet. The gear icon gets a small amber dot in the
/// top-right corner — mirrors the dot painted on the Settings
/// rail's "Updates" row so both surfaces feel like one signal.
pub(super) fn sidebar_header(update_pending: bool, cx: &mut Context<AppView>) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();
    // Ghost icon buttons; each needs a stable id for the tooltip
    // layer and its own tooltip text.
    let icon_btn = |id: &'static str, icon: IconName, tip: Cow<'static, str>| {
        Button::new(id).ghost().small().icon(icon).tooltip(tip)
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
                    icon_btn(
                        "nav-collapse-sidebar",
                        IconName::PanelLeftClose,
                        t!("chrome.collapse_sidebar_tooltip"),
                    )
                    .on_click(cx.listener(
                        |this, _: &ClickEvent, _window, cx| {
                            this.toggle_sidebar(cx);
                        },
                    )),
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
                        .child(s.label_transform.apply(&t!("chrome.profiles_label"))),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .child(
                    icon_btn(
                        "nav-add-profile",
                        IconName::Plus,
                        t!("chrome.new_profile_tooltip"),
                    )
                    .on_click(cx.listener(
                        |this, _: &ClickEvent, window, cx| {
                            this.open_editor(window, cx);
                        },
                    )),
                )
                // Two joined squares — the new-window icon.
                .child(
                    icon_btn(
                        "nav-new-window",
                        IconName::Copy,
                        t!("chrome.new_window_tooltip"),
                    )
                    .on_click(cx.listener(
                        |this, _: &ClickEvent, window, cx| {
                            this.open_new_window(window, cx);
                        },
                    )),
                )
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
                            icon_btn(
                                "nav-settings",
                                IconName::Settings,
                                t!("chrome.settings_tooltip"),
                            )
                            .on_click(cx.listener(
                                |this, _: &ClickEvent, window, cx| {
                                    this.open_settings(window, cx);
                                },
                            )),
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
    // Same ghost icon buttons as `sidebar_header`, stretched to the
    // strip's width so the hover rect reads as a full button target.
    let icon_btn = |id: &'static str, icon: IconName, tip: Cow<'static, str>| {
        Button::new(id).ghost().w_full().icon(icon).tooltip(tip)
    };
    div()
        .w_full()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .child(
            // Expand — mirror of the collapse button in
            // `sidebar_header`.
            icon_btn(
                "nav-expand-sidebar",
                IconName::PanelLeftOpen,
                t!("chrome.expand_sidebar_tooltip"),
            )
            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                this.toggle_sidebar(cx);
            })),
        )
        .child(
            icon_btn(
                "nav-add-profile-collapsed",
                IconName::Plus,
                t!("chrome.new_profile_tooltip"),
            )
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.open_editor(window, cx);
            })),
        )
        .child(
            icon_btn(
                "nav-new-window-collapsed",
                IconName::Copy,
                t!("chrome.new_window_tooltip"),
            )
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.open_new_window(window, cx);
            })),
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
                    icon_btn(
                        "nav-settings-collapsed",
                        IconName::Settings,
                        t!("chrome.settings_tooltip"),
                    )
                    .on_click(cx.listener(
                        |this, _: &ClickEvent, window, cx| {
                            this.open_settings(window, cx);
                        },
                    )),
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
/// `⋯` button + drop-down menu in the session header (GPUI Kit
/// `DropdownMenu`, which owns its own open state and dismissal).
pub(super) fn session_overflow_button(cx: &mut Context<AppView>) -> impl IntoElement {
    let app = cx.entity();
    Button::new("session-overflow-btn")
        .secondary()
        .icon(IconName::Ellipsis)
        .tooltip(t!("chrome.more_actions"))
        .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
            let item =
                |label: Cow<'static, str>,
                 run: fn(&mut AppView, &mut Window, &mut Context<AppView>)| {
                    let app = app.clone();
                    PopupMenuItem::new(label).on_click(move |_, window, cx| {
                        app.update(cx, |this, cx| run(this, window, cx))
                    })
                };
            menu.item(item(t!("chrome.send_break"), AppView::send_break_now))
                .item(item(t!("chrome.send_hex"), AppView::open_send_hex))
                .item(item(t!("chrome.send_file"), AppView::start_send_file))
                .item(item(t!("chrome.move_to_new_window"), |this, _, cx| {
                    this.move_session_to_new_window(None, cx)
                }))
        })
}
