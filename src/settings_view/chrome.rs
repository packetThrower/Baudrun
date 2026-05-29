//! Settings window chrome + generic form controls split out of
//! `settings_view/mod.rs`.
//!
//! Two clusters of helpers:
//!
//!   1. Window-level chrome that frames the rail + pane layout:
//!      `window_header`, `rail`, `scrollable_pane`,
//!      `section_card_with_desc`, `bool_field`.
//!   2. Generic affordances reused across panes: `import_button`,
//!      `trash_button`, `make_select`, `dismiss_notification_after`,
//!      `encode_file_path`, `resolve_config_dir_display`.
//!
//! All entry points are `pub(super)` so mod.rs imports them by
//! name. Anything not called from mod.rs stays private.

use gpui::{
    div, prelude::*, px, rgba, AppContext, Context, DismissEvent, ElementId, Entity, IntoElement,
    MouseButton, MouseUpEvent, SharedString, Window,
};
use gpui_component::{
    checkbox::Checkbox,
    input::{Input, InputState},
    notification::Notification,
    select::SelectState,
    tooltip::Tooltip,
    IndexPath, Sizable,
};

use super::{Opt, SettingsTab, SettingsView};
use crate::data::appdata;
use crate::skin_tokens::{self, SkinTokens};

pub(super) fn window_header(
    s: SkinTokens,
    filter_input: &Entity<InputState>,
    filter_active: bool,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    div()
        .w_full()
        .px_6()
        .py_3()
        .border_b_1()
        .border_color(rgba(s.border_subtle))
        .flex()
        .flex_row()
        .items_center()
        .gap_4()
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    // h1 row: "Settings" + version badge inline,
                    // matching the Tauri Settings layout. The
                    // version pill is a small pilltext tooltipped
                    // with its provenance so a user inspecting it
                    // knows what the string represents.
                    div()
                        .flex()
                        .flex_row()
                        .items_baseline()
                        .gap_2()
                        .child(
                            // `--font-size-h1` from the active
                            // skin (default 24, macOS-26 ships
                            // 26 for the Liquid Glass look).
                            div()
                                .text_size(px(s.font_size_h1_px))
                                .text_color(rgba(s.fg_primary))
                                .child("Settings"),
                        )
                        .child(
                            div()
                                .id("settings-version-badge")
                                .px(px(6.0))
                                .py(px(1.0))
                                .rounded_md()
                                .border_1()
                                .border_color(rgba(s.border_subtle))
                                .bg(rgba(s.bg_input))
                                .text_size(px(11.0))
                                .text_color(rgba(s.fg_tertiary))
                                // `env!("CARGO_PKG_VERSION")`
                                // resolves at compile time. Our
                                // `release.yml` awk-patches the
                                // `version = "..."` line in
                                // `Cargo.toml` to the tag string
                                // before `cargo build` runs, so
                                // shipped binaries embed e.g.
                                // "0.9.7-alpha.1" here. Dev
                                // `cargo run` shows whatever the
                                // working-copy Cargo.toml says
                                // (currently "0.0.1").
                                .child(format!("v{}", env!("CARGO_PKG_VERSION")))
                                .tooltip(|window, cx| {
                                    Tooltip::new(SharedString::from(
                                        "Baudrun version (from Cargo.toml at build time)",
                                    ))
                                    .build(window, cx)
                                }),
                        ),
                )
                .child(
                    // `--font-size-label` + `--label-weight` +
                    // `--label-transform` from the active skin
                    // so authors can fully restyle small labels.
                    div()
                        .text_size(px(s.font_size_label_px))
                        .text_color(rgba(s.fg_tertiary))
                        .font_weight(gpui::FontWeight(s.label_weight as f32))
                        .child(s.label_transform.apply("GLOBAL DEFAULTS")),
                ),
        )
        .child(
            // `relative` so the absolutely-positioned clear glyph
            // anchors to the input's right edge. We hand-roll the
            // `×` instead of using `Input::cleanable(true)` — that
            // path renders gpui-component's `IconName::CircleX`
            // SVG which the prototype doesn't bundle (the icon
            // shows up blank, leaving just the hover circle).
            div()
                .relative()
                .w(px(220.0))
                .child(Input::new(filter_input).small().appearance(true))
                .when(filter_active, |row| {
                    row.child(
                        div()
                            .id("settings-filter-clear")
                            .absolute()
                            .top(px(0.0))
                            .right(px(6.0))
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(14.0))
                            .text_color(rgba(s.fg_tertiary))
                            .cursor_pointer()
                            .hover(|st| st.text_color(rgba(s.fg_primary)))
                            .tooltip(|window, cx| {
                                Tooltip::new(SharedString::from("Clear filter")).build(window, cx)
                            })
                            .child("\u{00D7}")
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _: &MouseUpEvent, window, cx| {
                                    // Clear the input AND the
                                    // derived `filter_text`
                                    // explicitly — relying on the
                                    // Change subscription to do
                                    // both isn't reliable (the
                                    // event sometimes doesn't reach
                                    // us when set_value runs from
                                    // inside another listener).
                                    this.filter_input.update(cx, |state, cx| {
                                        state.set_value("", window, cx);
                                    });
                                    if !this.filter_text.is_empty() {
                                        this.filter_text.clear();
                                        cx.notify();
                                    }
                                }),
                            ),
                    )
                }),
        )
}

/// Left-rail tab nav. Same shape as `form_tab_nav` — translucent
/// blue active background, muted text for inactive entries, hover
/// fades to the button-bg token. Tabs whose sections all miss the
/// current filter render at 0.35 opacity so the user can see at a
/// glance where their hits are; they stay clickable so a typo in
/// the filter doesn't lock the user out of the rest of the panel.
///
/// `update_pending` is `true` when the boot-time update check
/// found a newer release the user hasn't dismissed yet — drives
/// the amber dot painted next to the "Updates" tab label.
pub(super) fn rail(
    s: SkinTokens,
    active: SettingsTab,
    has_matches: &[(SettingsTab, bool)],
    update_pending: bool,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let lookup_match = |tab: SettingsTab| -> bool {
        has_matches
            .iter()
            .find(|(t, _)| *t == tab)
            .map(|(_, m)| *m)
            .unwrap_or(true)
    };
    let item = move |label: &'static str, tab: SettingsTab, dimmed: bool| {
        let is_active = tab == active;
        let bg = if is_active {
            rgba(s.bg_active)
        } else {
            rgba(0x00000000)
        };
        let fg = if is_active {
            rgba(s.fg_primary)
        } else {
            rgba(s.fg_secondary)
        };
        // Match the main app's sidebar hover (profile_row uses bg_hover).
        // bg_input is too close to the panel bg on warm dark skins like
        // Foundry — the hover tint reads as "barely there"; bg_hover is
        // tuned to be visible against the panel.
        let hover_bg = s.bg_hover;
        let opacity = if dimmed { 0.35 } else { 1.0 };
        let show_dot = update_pending && tab == SettingsTab::Updates;
        div()
            // Stable id so gpui's mouse-move handler actually notifies
            // on hover-state transitions — without it the `.hover()`
            // style only paints when some unrelated event happens to
            // dirty SettingsView. Same fix as `profile_row`. The label
            // is unique within the rail so it works as a stable key.
            .id(label)
            .w_full()
            .px_3()
            .py(px(6.0))
            .rounded_md()
            .bg(bg)
            .text_color(fg)
            .opacity(opacity)
            .cursor_pointer()
            // Hover bg only when NOT the active tab — see the
            // matching comment in `profile_row` in `app_view.rs`.
            // Without this gate the grey hover overlay paints
            // over the blue `bg_active` while the user's cursor
            // sits on the row they just clicked, and the tab
            // looks active only after the mouse leaves.
            .when(!is_active, |this| {
                this.hover(move |st| st.bg(rgba(hover_bg)))
            })
            .child(
                // Flex-row so the label + the optional amber dot
                // sit on the same baseline. The label gets
                // `flex_1` to push the dot to the right edge of
                // the rail item.
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().child(label))
                    .when(show_dot, |this| {
                        // Same amber dot as `app_view::sidebar_header`'s
                        // gear-icon indicator — same colour token
                        // (`s.warn`) + same 8px diameter, so the
                        // two surfaces feel like one signal.
                        this.child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(rgba(s.warn)))
                    }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseUpEvent, _, cx| {
                    this.set_tab(tab, cx);
                }),
            )
    };

    div()
        .w(px(160.0))
        .h_full()
        .px_3()
        .py_4()
        .border_r_1()
        .border_color(rgba(s.border_subtle))
        .flex()
        .flex_col()
        .gap_1()
        .text_size(px(13.0))
        .children(
            SettingsTab::ALL
                .iter()
                .map(|&t| item(t.label(), t, !lookup_match(t))),
        )
}

/// Scrollable pane wrapper. Same min_w_0/min_h_0 dance as
/// `form_body` — without those the cards' intrinsic min-widths
/// push the pane past the window edge and the scroll never fires.
pub(super) fn scrollable_pane(content: impl IntoElement) -> impl IntoElement {
    div()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .id("settings-pane-scroll")
        .overflow_y_scroll()
        .child(div().w_full().min_w_0().px_6().py_4().child(content))
}

/// Translucent panel — title at 15px, optional muted description
/// beneath, body below. Same `--bg-panel` / `--border-subtle` /
/// `--radius-lg` tokens as the profile editor's `section_card`.
pub(super) fn section_card_with_desc(
    s: SkinTokens,
    title: &'static str,
    description: Option<&'static str>,
    body: impl IntoElement,
) -> gpui::Div {
    let mut header = div().flex().flex_col().gap_1().child(
        div()
            .text_size(px(s.font_size_section_px))
            .text_color(rgba(s.fg_primary))
            .child(title),
    );
    if let Some(desc) = description {
        header = header.child(
            div()
                .text_size(px(12.0))
                .text_color(rgba(s.fg_secondary))
                // gpui defaults text to nowrap; long descriptions
                // would otherwise run off the right edge.
                .whitespace_normal()
                .child(desc),
        );
    }
    div()
        .w_full()
        .bg(rgba(s.bg_panel))
        // `--panel-border` from the skin (see `app_view.rs`).
        .map(|this| match s.panel_border {
            skin_tokens::PanelBorder::None => this,
            skin_tokens::PanelBorder::Solid(w, colour) => {
                this.border(px(w)).border_color(rgba(colour))
            }
        })
        .rounded(px(s.radius_lg))
        // macOS 26 / Tahoe-style raised card. The Tailwind-grade
        // shadow_sm pair (1px+3px blur over 1px+2px) gives the same
        // soft depth Apple uses for sheet content without painting a
        // visible halo on dark backgrounds.
        .shadow_sm()
        .px_4()
        .py_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(header)
        .child(body)
}

/// Checkbox row. Mirrors `bool_field_hinted` from the profile editor:
/// the checkbox is the whole interactive surface, the label sits
/// inside the widget so click-on-text toggles too.
pub(super) fn bool_field<F>(
    id: &'static str,
    label: &'static str,
    checked: bool,
    cx: &mut Context<SettingsView>,
    setter: F,
) -> gpui::Div
where
    F: Fn(&mut SettingsView, bool, &mut Context<SettingsView>) + 'static,
{
    div().child(
        Checkbox::new(id)
            .checked(checked)
            .label(label)
            .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                setter(this, *checked, cx);
            })),
    )
}

/// Schedule a Notification to dismiss itself ~1.5 s after the user
/// clicks its action button. Without this the toast stays up for
/// the full autohide window (5 s) even after the user already
/// acted on it — feels like the click had no effect. The brief
/// linger gives time to read the follow-up "Restored …" toast
/// before the original "Removed …" one fades.
pub(super) fn dismiss_notification_after(notif: Entity<Notification>, cx: &mut gpui::App) {
    // Spawn from the Notification's own Context, not via
    // `App::spawn`. The latter route gets dropped when the user
    // switches tabs while the timer is pending — the SettingsView
    // re-renders, the click-handler closure is rebuilt, and the
    // detached App-spawn task evidently doesn't survive the
    // turnover. Entity-scoped spawns stay alive as long as the
    // notification entity does.
    notif.update(cx, |_, ctx| {
        ctx.spawn(async move |weak, cx_async| {
            cx_async
                .background_executor()
                .timer(std::time::Duration::from_millis(1500))
                .await;
            let _ = weak.update(cx_async, |_, ctx| {
                ctx.emit(DismissEvent);
            });
        })
        .detach();
    });
}

/// Percent-encode a filesystem path for use in a `file://` URL.
/// Keeps the RFC 3986 unreserved set plus `/` (path separator)
/// verbatim and `%xx`-encodes everything else. Handles the common
/// case of macOS paths containing spaces
/// (`~/Library/Application Support/...`) — without this, the URL
/// constructor rejects the unencoded space and the open is a
/// silent no-op.
pub(super) fn encode_file_path(p: &std::path::Path) -> String {
    let s = p.to_string_lossy();
    let mut out = String::with_capacity(s.len());
    for &byte in s.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Resolve the current Baudrun support directory for display in
/// Settings → Advanced → Config Directory. Falls back to a sentinel
/// string when the resolver errors out so the UI still has
/// something to show.
pub(super) fn resolve_config_dir_display() -> SharedString {
    match appdata::support_dir() {
        Ok(path) => SharedString::from(path.display().to_string()),
        Err(err) => SharedString::from(format!("(unavailable: {err})")),
    }
}

/// Pill-style "Import\u{2026}" button used at the bottom of each
/// asset section. Matches the visual weight of the surrounding
/// chrome — slightly raised on the panel's translucent bg, accent
/// border on hover for affordance. The on-click handler lives in
/// `SettingsView::start_*_import`.
///
/// `id` is required (not derived from `label`) because the Advanced
/// tab carries two pairs of "Choose…" / "Reset" buttons (log dir
/// and config dir) — auto-deriving from label would collide. The id
/// also satisfies gpui's `.hover()` requirement: without a stable
/// id, gpui's mouse-move handler skips the `cx.notify` that would
/// otherwise repaint the window on hover-state transitions, so the
/// accent-border swap only paints when an unrelated event happens
/// to dirty SettingsView.
pub(super) fn import_button<F>(
    s: SkinTokens,
    id: impl Into<ElementId>,
    label: &'static str,
    cx: &mut Context<SettingsView>,
    on_click: F,
) -> gpui::Div
where
    F: Fn(&mut SettingsView, &mut Window, &mut Context<SettingsView>) + 'static,
{
    let accent = s.accent;
    div().child(
        div()
            .id(id)
            .px_3()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(rgba(s.border_subtle))
            .bg(rgba(s.bg_input))
            .text_color(rgba(s.fg_primary))
            .text_size(px(12.0))
            .cursor_pointer()
            .hover(move |st| st.border_color(rgba(accent)))
            .child(label)
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseUpEvent, window, cx| {
                    on_click(this, window, cx);
                }),
            ),
    )
}

/// Square icon-button rendering a trash glyph. Used next to import
/// buttons (skin / theme tabs) and per-row in the highlight tab to
/// remove a user-imported entry. Hover swaps to the danger token so
/// the destructive intent is obvious before the click.
pub(super) fn trash_button<F>(
    s: SkinTokens,
    id: impl Into<ElementId>,
    tip: &'static str,
    cx: &mut Context<SettingsView>,
    on_click: F,
) -> gpui::Div
where
    F: Fn(&mut SettingsView, &mut Window, &mut Context<SettingsView>) + 'static,
{
    let danger = s.danger;
    let tip_text = SharedString::from(tip);
    div().child(
        div()
            .id(id)
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(rgba(s.border_subtle))
            .bg(rgba(s.bg_input))
            .text_color(rgba(s.fg_secondary))
            .text_size(px(12.0))
            .cursor_pointer()
            .hover(move |st| st.border_color(rgba(danger)).text_color(rgba(danger)))
            .tooltip(move |window, cx| Tooltip::new(tip_text.clone()).build(window, cx))
            .child("\u{1F5D1}")
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseUpEvent, window, cx| {
                    on_click(this, window, cx);
                }),
            ),
    )
}

/// Build a `SelectState<Vec<Opt>>` pre-selected to whichever option
/// in `opts` has `id == selected`. Same shape as `app_view::make_select`
/// but bound to `Context<SettingsView>` (gpui's `Context<T>` is
/// invariant in `T` so the helper has to live where it's called).
pub(super) fn make_select(
    opts: Vec<Opt>,
    selected: &str,
    window: &mut Window,
    cx: &mut Context<SettingsView>,
) -> Entity<SelectState<Vec<Opt>>> {
    let idx = opts
        .iter()
        .position(|o| o.id == selected)
        .map(IndexPath::new);
    cx.new(|cx| SelectState::new(opts, idx, window, cx))
}
