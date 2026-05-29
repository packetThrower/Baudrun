//! Profile-editor form, split out of `app_view/mod.rs`. Covers the
//! full new/edit-profile pane:
//!
//!   * `build_editor` + `apply_editor_to_profile` —
//!     state-bridging between a `Profile` and the live form widgets.
//!   * `editor_fields_match` — dirty-check predicate used to
//!     toggle the Save button's brightness.
//!   * `EditorRender` + `impl EditorRender` — the cloned-out
//!     widget handles a single render pass needs (lets the
//!     render code hand `cx: &mut Context<AppView>` to form
//!     helpers without keeping `&self.editor` borrowed).
//!   * `form_pane` — the public entry into the form rendering
//!     (called from `Render for AppView` in mod.rs).
//!   * `form_header`, `form_body`, `form_tab_nav`, `section_card`,
//!     `section_card_with_desc`, `labeled`, `bool_field`,
//!     `bool_field_hinted`, `connection_card`, `terminal_card`,
//!     `highlighting_pane`, `advanced_pane`, `theme_card`,
//!     `control_lines_card`, `output_card`, `paste_safety_card` —
//!     internal helpers that compose the form's visual tree.
//!   * `detect_missing_drivers` + `driver_banner_row` — the
//!     unenrolled-USB-driver warning shown above the Serial Port
//!     field; lives here because it's only rendered by the form.
//!
//! `struct Editor` and `enum EditorTab` stay in mod.rs because the
//! parent module's methods on `AppView` (set_editor_tab,
//! save_editor, …) do direct field access — easier to keep the
//! type next to its callers than to widen 30+ fields to
//! `pub(super)`. The split here is "form rendering & state-
//! bridging logic" rather than "Editor type and friends".

use gpui::{
    div, prelude::*, px, rgba, Context, Entity, IntoElement, MouseButton, MouseUpEvent,
    ScrollHandle, SharedString, Window,
};
use gpui_component::{
    checkbox::Checkbox,
    input::{Input, InputState},
    scroll::ScrollableElement,
    select::{Select, SelectState},
    Disableable, Sizable,
};

use super::buttons::{pill_button, primary_button};
use super::opts::{
    backspace_opts, baud_opts, data_bits_opts, flow_control_opts, line_ending_opts,
    line_policy_opts, make_select, parity_opts, port_opts, read_select, stop_bits_opts, Opt,
};
use super::{AppView, Editor, EditorTab};
use crate::data::profiles::Profile;
use crate::data::themes;
use crate::skin_tokens::{self, SkinTokens};

/// Construct a fresh `Editor` whose every widget (text inputs +
/// selects + checkbox bool) is seeded from `profile`. Shared by
/// the new-profile path (`profile = Profile::defaults()`) and the
/// edit-profile path (`profile = store.get(id).unwrap()`) so the
/// initialisation logic for each field exists in exactly one place.
pub(super) fn build_editor(
    profile_id: Option<String>,
    profile: &Profile,
    themes_store: &themes::Store,
    detect_drivers: bool,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> Editor {
    let name = {
        let val = profile.name.clone();
        cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("My switch")
                .default_value(val)
        })
    };
    let port = make_select(
        port_opts(&profile.port_name),
        &profile.port_name,
        window,
        cx,
    );
    // Per-profile theme picker. Empty-id row "Use global default"
    // is the first option so the override is opt-in — saving an
    // editor with that selected reverts to inheriting from
    // settings.default_theme_id.
    let theme = {
        let mut opts = Vec::with_capacity(themes_store.list().len() + 1);
        opts.push(Opt::new("", "Use global default"));
        for t in themes_store.list() {
            let title = if t.source == "user" {
                format!("{} (custom)", t.name)
            } else {
                t.name
            };
            opts.push(Opt::new(&t.id, &title));
        }
        make_select(opts, &profile.theme_id, window, cx)
    };
    let paste_delay_val = profile.paste_char_delay_ms.unwrap_or(10).to_string();
    let paste_char_delay_ms = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("10")
            .default_value(paste_delay_val)
    });
    // Empty `dtr/rts` strings on a freshly-loaded profile fall back
    // to "default" for the select — the store accepts both, but
    // showing "default" in the dropdown is the same intent and
    // avoids a blank-looking field.
    fn policy_or_default(s: &str) -> &str {
        if s.is_empty() {
            "default"
        } else {
            s
        }
    }
    Editor {
        profile_id,
        tab: EditorTab::Connection,
        name,
        port,
        baud: make_select(baud_opts(), &profile.baud_rate.to_string(), window, cx),
        data_bits: make_select(data_bits_opts(), &profile.data_bits.to_string(), window, cx),
        parity: make_select(parity_opts(), &profile.parity, window, cx),
        stop_bits: make_select(stop_bits_opts(), &profile.stop_bits, window, cx),
        flow_control: make_select(flow_control_opts(), &profile.flow_control, window, cx),
        line_ending: make_select(line_ending_opts(), &profile.line_ending, window, cx),
        backspace_key: make_select(backspace_opts(), &profile.backspace_key, window, cx),
        local_echo: profile.local_echo,
        dtr_on_connect: make_select(
            line_policy_opts(),
            policy_or_default(&profile.dtr_on_connect),
            window,
            cx,
        ),
        rts_on_connect: make_select(
            line_policy_opts(),
            policy_or_default(&profile.rts_on_connect),
            window,
            cx,
        ),
        dtr_on_disconnect: make_select(
            line_policy_opts(),
            policy_or_default(&profile.dtr_on_disconnect),
            window,
            cx,
        ),
        rts_on_disconnect: make_select(
            line_policy_opts(),
            policy_or_default(&profile.rts_on_disconnect),
            window,
            cx,
        ),
        hex_view: profile.hex_view,
        timestamps: profile.timestamps,
        line_numbers: profile.line_numbers,
        log_enabled: profile.log_enabled,
        auto_reconnect: profile.auto_reconnect,
        theme,
        // `None` on the saved profile becomes "inherit global"; the
        // override flag stays false until the user explicitly opts
        // into a per-profile pack list.
        highlight: profile.highlight,
        override_highlight_packs: profile.enabled_highlight_presets.is_some(),
        enabled_highlight_packs: profile
            .enabled_highlight_presets
            .clone()
            .unwrap_or_default(),
        missing_drivers: if detect_drivers {
            detect_missing_drivers()
        } else {
            Vec::new()
        },
        paste_warn_multiline: profile.paste_warn_multiline,
        paste_slow: profile.paste_slow,
        paste_char_delay_ms,
        error: None,
        scroll_handle: ScrollHandle::new(),
        baseline: profile.clone(),
    }
}

/// Read every widget in `editor` and write the values onto `profile`,
/// in place. Fields the form doesn't expose (theme, paste settings,
/// auto-reconnect, …) are left untouched, which is what makes the
/// edit-path safe to round-trip.
pub(super) fn apply_editor_to_profile(
    editor: &Editor,
    profile: &mut Profile,
    cx: &Context<AppView>,
) {
    profile.name = editor.name.read(cx).value().to_string();
    profile.port_name = read_select(&editor.port, cx);
    // Empty / non-numeric → 0, which `validate` rejects with
    // `InvalidBaud`; `Profile::data_bits` is i32 too. Let the store
    // be the single source of truth for what counts as valid rather
    // than duplicating its rules in the UI.
    profile.baud_rate = read_select(&editor.baud, cx).trim().parse().unwrap_or(0);
    profile.data_bits = read_select(&editor.data_bits, cx)
        .trim()
        .parse()
        .unwrap_or(0);
    profile.parity = read_select(&editor.parity, cx);
    profile.stop_bits = read_select(&editor.stop_bits, cx);
    profile.flow_control = read_select(&editor.flow_control, cx);
    profile.line_ending = read_select(&editor.line_ending, cx);
    profile.backspace_key = read_select(&editor.backspace_key, cx);
    profile.local_echo = editor.local_echo;
    profile.dtr_on_connect = read_select(&editor.dtr_on_connect, cx);
    profile.rts_on_connect = read_select(&editor.rts_on_connect, cx);
    profile.dtr_on_disconnect = read_select(&editor.dtr_on_disconnect, cx);
    profile.rts_on_disconnect = read_select(&editor.rts_on_disconnect, cx);
    profile.hex_view = editor.hex_view;
    profile.timestamps = editor.timestamps;
    profile.line_numbers = editor.line_numbers;
    profile.log_enabled = editor.log_enabled;
    profile.auto_reconnect = editor.auto_reconnect;
    // Empty id is the explicit "Use global default" pick — store
    // it as-is so `compute_palette` falls through to the global.
    profile.theme_id = read_select(&editor.theme, cx);
    profile.paste_warn_multiline = editor.paste_warn_multiline;
    profile.paste_slow = editor.paste_slow;
    // Empty / non-numeric → None (rolls back to the store's default
    // of 10ms via `Profile::defaults` on next load). Negative values
    // collapse to 0, which the store accepts.
    let delay_str = editor.paste_char_delay_ms.read(cx).value().to_string();
    profile.paste_char_delay_ms = delay_str.trim().parse::<i32>().ok().map(|v| v.max(0));
    profile.highlight = editor.highlight;
    // The override flag → `Option` shape: false collapses to None
    // (inherit global); true persists the current vec, even if it's
    // empty (an explicit "no packs at all for this profile" state).
    profile.enabled_highlight_presets = if editor.override_highlight_packs {
        Some(editor.enabled_highlight_packs.clone())
    } else {
        None
    };
}
/// Compare two profiles on the fields the editor actually exposes
/// — drives the Save button's "dirty" state. Skipping `id` /
/// `created_at` / `updated_at` because those aren't user-editable
/// (id is set by the store on create, timestamps update on save)
/// and would otherwise spuriously flag every edited profile as
/// "dirty" right after save.
fn editor_fields_match(a: &Profile, b: &Profile) -> bool {
    a.name == b.name
        && a.port_name == b.port_name
        && a.baud_rate == b.baud_rate
        && a.data_bits == b.data_bits
        && a.parity == b.parity
        && a.stop_bits == b.stop_bits
        && a.flow_control == b.flow_control
        && a.line_ending == b.line_ending
        && a.backspace_key == b.backspace_key
        && a.local_echo == b.local_echo
        && a.dtr_on_connect == b.dtr_on_connect
        && a.rts_on_connect == b.rts_on_connect
        && a.dtr_on_disconnect == b.dtr_on_disconnect
        && a.rts_on_disconnect == b.rts_on_disconnect
        && a.hex_view == b.hex_view
        && a.timestamps == b.timestamps
        && a.line_numbers == b.line_numbers
        && a.log_enabled == b.log_enabled
        && a.auto_reconnect == b.auto_reconnect
        && a.paste_warn_multiline == b.paste_warn_multiline
        && a.paste_slow == b.paste_slow
        && a.paste_char_delay_ms == b.paste_char_delay_ms
        && a.theme_id == b.theme_id
        && a.highlight == b.highlight
        && a.enabled_highlight_presets == b.enabled_highlight_presets
}

/// All Editor fields a render needs, cloned out so the call site
/// can hand `cx: &mut Context<AppView>` to the form helpers without
/// keeping `&self.editor` borrowed at the same time. Cloning is
/// cheap — `Entity<T>` is `Arc`-shaped — and it's only done once
/// per render.
pub(super) struct EditorRender {
    is_edit: bool,
    is_dirty: bool,
    tab: EditorTab,
    name: Entity<InputState>,
    port: Entity<SelectState<Vec<Opt>>>,
    baud: Entity<SelectState<Vec<Opt>>>,
    data_bits: Entity<SelectState<Vec<Opt>>>,
    parity: Entity<SelectState<Vec<Opt>>>,
    stop_bits: Entity<SelectState<Vec<Opt>>>,
    flow_control: Entity<SelectState<Vec<Opt>>>,
    line_ending: Entity<SelectState<Vec<Opt>>>,
    backspace_key: Entity<SelectState<Vec<Opt>>>,
    local_echo: bool,
    dtr_on_connect: Entity<SelectState<Vec<Opt>>>,
    rts_on_connect: Entity<SelectState<Vec<Opt>>>,
    dtr_on_disconnect: Entity<SelectState<Vec<Opt>>>,
    rts_on_disconnect: Entity<SelectState<Vec<Opt>>>,
    hex_view: bool,
    timestamps: bool,
    line_numbers: bool,
    log_enabled: bool,
    auto_reconnect: bool,
    paste_warn_multiline: bool,
    paste_slow: bool,
    paste_char_delay_ms: Entity<InputState>,
    theme: Entity<SelectState<Vec<Opt>>>,
    highlight: bool,
    override_highlight_packs: bool,
    enabled_highlight_packs: Vec<String>,
    missing_drivers: Vec<crate::data::serial::chipsets::USBSerialCandidate>,
    error: Option<String>,
    scroll_handle: ScrollHandle,
}

impl EditorRender {
    pub(super) fn from(e: &Editor, cx: &Context<AppView>) -> Self {
        // Derive a hypothetical "what would Save persist right now"
        // Profile by applying the live widget values onto a clone of
        // the saved baseline; any field difference flags the form
        // as dirty (drives the Save button's brightness).
        let mut current = e.baseline.clone();
        apply_editor_to_profile(e, &mut current, cx);
        let is_dirty = !editor_fields_match(&current, &e.baseline);
        Self {
            is_edit: e.profile_id.is_some(),
            is_dirty,
            tab: e.tab,
            name: e.name.clone(),
            port: e.port.clone(),
            baud: e.baud.clone(),
            data_bits: e.data_bits.clone(),
            parity: e.parity.clone(),
            stop_bits: e.stop_bits.clone(),
            flow_control: e.flow_control.clone(),
            line_ending: e.line_ending.clone(),
            backspace_key: e.backspace_key.clone(),
            local_echo: e.local_echo,
            dtr_on_connect: e.dtr_on_connect.clone(),
            rts_on_connect: e.rts_on_connect.clone(),
            dtr_on_disconnect: e.dtr_on_disconnect.clone(),
            rts_on_disconnect: e.rts_on_disconnect.clone(),
            hex_view: e.hex_view,
            timestamps: e.timestamps,
            line_numbers: e.line_numbers,
            log_enabled: e.log_enabled,
            auto_reconnect: e.auto_reconnect,
            paste_warn_multiline: e.paste_warn_multiline,
            paste_slow: e.paste_slow,
            paste_char_delay_ms: e.paste_char_delay_ms.clone(),
            theme: e.theme.clone(),
            highlight: e.highlight,
            override_highlight_packs: e.override_highlight_packs,
            enabled_highlight_packs: e.enabled_highlight_packs.clone(),
            missing_drivers: e.missing_drivers.clone(),
            error: e.error.clone(),
            scroll_handle: e.scroll_handle.clone(),
        }
    }
}

pub(super) fn form_pane(
    er: EditorRender,
    packs: Vec<crate::data::highlight::HighlightPack>,
    global_enabled: Vec<String>,
    connected_session: bool,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();
    div()
        .flex_1()
        // `min_w_0` lets the pane shrink below its intrinsic
        // content min-width so long card descriptions wrap to
        // fit instead of pushing the Connect button off-screen.
        // `min_h_0` is the same idea for height: in the parent
        // flex_row, cross-axis stretch tries to fit the pane to
        // the row's height, but without min_h_0 the form's
        // intrinsic content min-height keeps it tall, the
        // scrollable body never sees an overflow, and the
        // Scrollbar widget has nothing to render.
        .min_w_0()
        .min_h_0()
        // No bg_main here — the cards inside paint `bg_panel`
        // directly over `bg_window` (the opaque shell), matching
        // Settings window's two-layer composition. With bg_main
        // here the panels would stack alpha and the form pane
        // ended up brighter / less grey than the rest of the
        // chrome.
        .text_color(rgba(s.fg_primary))
        .text_size(px(13.0))
        .flex()
        .flex_col()
        .child(form_header(
            er.is_edit,
            er.is_dirty,
            er.name.clone(),
            connected_session,
            cx,
        ))
        .child(form_body(er, packs, global_enabled, cx))
}

/// Header bar: editable profile name as the visible title (no
/// input chrome — `appearance(false)` strips the border/bg so it
/// reads as a heading rather than a form field), uppercase mode
/// tag underneath, action buttons on the right.
fn form_header(
    is_edit: bool,
    is_dirty: bool,
    name: Entity<InputState>,
    connected_session: bool,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();
    let subtitle = if is_edit {
        "EDIT PROFILE"
    } else {
        "NEW PROFILE"
    };
    // Save button text-color is the only thing that changes for the
    // dirty state — pill bg stays the same so the button doesn't
    // visually "appear" mid-edit. Tertiary fg (40% white) when
    // clean reads as "no-op available," primary fg (95% white)
    // when dirty reads as "click me to persist your changes."
    let save_fg = if is_dirty {
        rgba(s.fg_primary)
    } else {
        rgba(s.fg_tertiary)
    };
    let delete_btn = is_edit.then(|| {
        pill_button(s, "Delete", true).on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _: &MouseUpEvent, window, cx| {
                this.delete_from_editor(window, cx);
            }),
        )
    });
    // Render the heading as a plain div instead of an Input —
    // gpui-component's Input fixes its own height to a small
    // `h_6` regardless of `Size::Size(_)`, which clipped 24px
    // text. Settings window's window_header uses the same plain-
    // div approach. Editing happens through a labeled "NAME"
    // field at the top of the Connection card now.
    let title_text = name.read(cx).value().to_string();
    let title_text = if title_text.is_empty() {
        "(unnamed)".to_string()
    } else {
        title_text
    };
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
                    div()
                        .text_size(px(24.0))
                        .text_color(rgba(s.fg_primary))
                        .child(title_text),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(rgba(s.fg_tertiary))
                        .child(subtitle),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .children(delete_btn)
                .child(
                    pill_button(s, "Save", false)
                        .text_color(save_fg)
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                                this.save_editor(cx);
                            }),
                        ),
                )
                .child(pill_button(s, "Cancel", false).on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                        this.cancel_editor(cx);
                    }),
                ))
                .when(connected_session, |row| {
                    // Suspended on the connected profile — swap the
                    // Connect button for the Disconnect + Resume
                    // pair Tauri shows in the same state. Connect on
                    // an already-connected profile would either
                    // race for its own port or re-open a session
                    // the user already has, neither of which is
                    // what they're after.
                    row.child(pill_button(s, "Disconnect", false).on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, window, cx| {
                            this.disconnect_current(window, cx);
                        }),
                    ))
                    .child(primary_button(s, "Resume").on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, window, cx| {
                            this.resume_session(window, cx);
                        }),
                    ))
                })
                .when(!connected_session, |row| {
                    row.child(primary_button(s, "Connect").on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, window, cx| {
                            this.save_and_connect(window, cx);
                        }),
                    ))
                }),
        )
}

/// Form body: a left rail of sub-tabs (Connection / Advanced) +
/// the active tab's content. Tab content is capped to a fixed
/// width so the cards keep form-shaped proportions on a wide
/// window. Mirrors the Tauri form's layout one-for-one.
fn form_body(
    er: EditorRender,
    packs: Vec<crate::data::highlight::HighlightPack>,
    global_enabled: Vec<String>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();
    let active = er.tab;
    let content: gpui::AnyElement = match er.tab {
        EditorTab::Connection => div()
            .flex()
            .flex_col()
            .gap_3()
            .child(connection_card(
                er.name.clone(),
                er.port,
                er.baud,
                er.data_bits,
                er.parity,
                er.stop_bits,
                er.flow_control,
                er.missing_drivers.clone(),
                cx,
            ))
            .child(terminal_card(
                er.line_ending,
                er.backspace_key,
                er.local_echo,
                cx,
            ))
            .child(theme_card(s, er.theme))
            .into_any_element(),
        EditorTab::Highlighting => highlighting_pane(
            er.highlight,
            er.override_highlight_packs,
            er.enabled_highlight_packs.clone(),
            packs,
            global_enabled,
            cx,
        )
        .into_any_element(),
        EditorTab::Advanced => advanced_pane(
            er.dtr_on_connect,
            er.rts_on_connect,
            er.dtr_on_disconnect,
            er.rts_on_disconnect,
            er.hex_view,
            er.timestamps,
            er.line_numbers,
            er.log_enabled,
            er.auto_reconnect,
            er.paste_warn_multiline,
            er.paste_slow,
            er.paste_char_delay_ms,
            cx,
        )
        .into_any_element(),
    };

    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_row()
        .child(form_tab_nav(active, cx))
        .child(
            // Bare `gpui::overflow_y_scroll` (no widget wrap) is
            // what actually scrolls — gpui-component's `Scrollable`
            // wrapper measures the scroll area incorrectly inside
            // our nested flex layout and ends up reporting "fits,
            // no scrollbar needed." So we wire the ScrollHandle
            // ourselves: scroll content tracks it via
            // `track_scroll`, and a sibling
            // `vertical_scrollbar(&handle)` paints the visible
            // bar. The parent div is `relative` so the scrollbar
            // (positioned absolutely internally) anchors to it.
            // Padding lives INSIDE the scrollable child, not on
            // the viewport — otherwise the bottom padding gets
            // eaten and the user can't scroll past the last card.
            div()
                .relative()
                .flex_1()
                .h_full()
                .min_w_0()
                .min_h_0()
                .child(
                    div()
                        .id("form-body-scroll")
                        .size_full()
                        .min_w_0()
                        .min_h_0()
                        .track_scroll(&er.scroll_handle)
                        .overflow_y_scroll()
                        .child(
                            div()
                                .w_full()
                                .min_w_0()
                                .px_6()
                                .py_4()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(content)
                                .children(er.error.map(|err| {
                                    div()
                                        .px_3()
                                        .py_2()
                                        .text_size(px(12.0))
                                        .text_color(rgba(s.sidebar_error))
                                        .child(err)
                                })),
                        ),
                )
                .vertical_scrollbar(&er.scroll_handle),
        )
}

/// Left-rail sub-tab navigation. Each entry is a clickable row;
/// the active one paints `--bg-active` (translucent blue) so the
/// selected state reads instantly.
fn form_tab_nav(active: EditorTab, cx: &mut Context<AppView>) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();
    let item = move |label: &'static str, tab: EditorTab| {
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
        // Foundry, where the hover tint reads as "barely there".
        let hover_bg = s.bg_hover;
        div()
            // Stable id so gpui notifies on hover-state transitions —
            // without it the `.hover()` style only paints when some
            // unrelated event happens to dirty AppView. Same fix as
            // `profile_row`. Labels here are unique within the rail.
            .id(label)
            .w_full()
            .px_3()
            .py(px(6.0))
            .rounded_md()
            .bg(bg)
            .text_color(fg)
            .cursor_pointer()
            // Hover bg only when NOT active — see the matching
            // comment in `profile_row`.
            .when(!is_active, |this| {
                this.hover(move |st| st.bg(rgba(hover_bg)))
            })
            .child(label)
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseUpEvent, _window, cx| {
                    this.set_editor_tab(tab, cx);
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
        .child(item("Connection", EditorTab::Connection))
        .child(item("Highlighting", EditorTab::Highlighting))
        .child(item("Advanced", EditorTab::Advanced))
}

/// One section of the form — a translucent panel with a heading,
/// optional description, and a body. Section title size is
/// `--font-size-section` (15px); description is the muted
/// `--fg-secondary`. Panel uses `--radius-lg` (10px) and
/// `--bg-panel` / `--border-subtle`.
fn section_card(s: SkinTokens, title: &'static str, body: impl IntoElement) -> gpui::Div {
    section_card_with_desc(s, title, None, body)
}

fn section_card_with_desc(
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
                // gpui's text default is `whitespace: nowrap` — a
                // long description like the Control Lines blurb
                // would otherwise render as a single line and run
                // off the right edge of the window.
                .whitespace_normal()
                .child(desc),
        );
    }
    div()
        .w_full()
        .bg(rgba(s.bg_panel))
        // `--panel-border` from the skin. `Solid(w, c)` paints a
        // border; `None` paints nothing. Default (when the skin
        // doesn't declare the var) is `Solid(1, border_subtle)`
        // so legacy / partially-authored skins keep their outline.
        .map(|this| match s.panel_border {
            skin_tokens::PanelBorder::None => this,
            skin_tokens::PanelBorder::Solid(w, colour) => {
                this.border(px(w)).border_color(rgba(colour))
            }
        })
        .rounded(px(s.radius_lg))
        // macOS 26 / Tahoe-style raised card. Matches the same
        // shadow_sm used by `settings_view::section_card_with_desc`
        // so the profile editor and the Settings window read at
        // the same elevation. `--panel-shadow` from the skin is
        // parsed into `SkinShadows.panel` but not yet threaded
        // through each card site — most skins ship
        // `--panel-shadow: none` anyway, and macOS-26's inset
        // entries can't render in gpui, so the visible delta
        // would be tiny vs the signature surface needed. Future
        // pass can replace `shadow_sm()` here once the wiring
        // is worth the cost.
        .shadow_sm()
        .px_4()
        .py_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(header)
        .child(body)
}

/// Per-field label + widget pair. Label uses Baudrun's
/// `--font-size-label` (11px), `--label-transform: uppercase`
/// (passed in already shouted by the caller), and `--fg-secondary`.
/// Label is `whitespace_nowrap` because gpui defaults to wrap, and
/// short fixed strings like "SLOW-PASTE DELAY (MS)" wrapping mid-
/// label inside a narrow container looks broken.
fn labeled(s: SkinTokens, label: &'static str, widget: impl IntoElement) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgba(s.fg_secondary))
                .whitespace_nowrap()
                .child(label),
        )
        .child(widget)
}

/// Detect unenrolled USB-serial adapters on platforms that need
/// vendor drivers. macOS / Windows have a real implementation in
/// `data::serial::detect`; Linux relies on kernel-side driver
/// loading (`pl2303.ko`, `ftdi_sio.ko`, `cp210x.ko`, …) and has no
/// equivalent missing-driver scenario, so it returns empty.
fn detect_missing_drivers() -> Vec<crate::data::serial::chipsets::USBSerialCandidate> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        crate::data::serial::detect::detect_suspect_enumerated_ports()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Vec::new()
    }
}

/// One driver-not-loaded banner row. Matches the Tauri layout:
/// yellow `!` icon on the left, chipset + reason / product / serial
/// in the middle, and an "Install driver…" pill on the right when
/// the chipset entry carries a vendor URL. Clicking the pill opens
/// the URL in the user's default browser via `cx.open_url`.
fn driver_banner_row(
    s: SkinTokens,
    candidate: crate::data::serial::chipsets::USBSerialCandidate,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    // Secondary line: prefer real product strings, but skip the
    // "please install…" / counterfeit placeholders the suspect-
    // product detector keys off — they're noise once we've already
    // resolved a chipset name above.
    let lower = candidate.product.to_lowercase();
    let product_is_placeholder = lower.contains("please install")
        || lower.contains("please download")
        || lower.contains("support windows")
        || lower.contains("counterfeit")
        || lower.contains("not support");
    let mut meta = if !candidate.product.is_empty() && !product_is_placeholder {
        candidate.product.clone()
    } else if !candidate.manufacturer.is_empty() {
        candidate.manufacturer.clone()
    } else {
        "USB device".to_string()
    };
    if !candidate.serial_number.is_empty() {
        meta.push_str(" \u{00B7} serial ");
        meta.push_str(&candidate.serial_number);
    }
    let title = format!("{} detected \u{2014} driver not loaded", candidate.chipset);

    let mut text_col = div().flex_1().min_w_0().flex().flex_col().gap_1().child(
        div()
            .text_size(px(13.0))
            .text_color(rgba(s.fg_primary))
            .child(title),
    );
    if !candidate.reason.is_empty() {
        text_col = text_col.child(
            div()
                .text_size(px(11.0))
                .text_color(rgba(s.fg_secondary))
                .whitespace_normal()
                .child(candidate.reason.clone()),
        );
    }
    text_col = text_col.child(
        div()
            .text_size(px(11.0))
            .text_color(rgba(s.fg_tertiary))
            .whitespace_normal()
            .child(meta),
    );

    let mut row = div()
        .w_full()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(rgba(0xE3A93A55u32))
        .bg(rgba(0xE3A93A18u32))
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            div()
                .w(px(22.0))
                .h(px(22.0))
                .rounded_full()
                .bg(rgba(0xE3A93AFFu32))
                .text_color(rgba(0x1A1A1AFFu32))
                .text_size(px(14.0))
                .flex()
                .items_center()
                .justify_center()
                .child("!"),
        )
        .child(text_col);
    if !candidate.driver_url.is_empty() {
        let url = candidate.driver_url.clone();
        row = row.child(pill_button(s, "Install driver\u{2026}", false).on_mouse_up(
            MouseButton::Left,
            cx.listener(move |_, _: &MouseUpEvent, _, cx| {
                cx.open_url(&url);
            }),
        ));
    }
    row
}

#[allow(clippy::too_many_arguments)]
fn connection_card(
    name: Entity<InputState>,
    port: Entity<SelectState<Vec<Opt>>>,
    baud: Entity<SelectState<Vec<Opt>>>,
    data_bits: Entity<SelectState<Vec<Opt>>>,
    parity: Entity<SelectState<Vec<Opt>>>,
    stop_bits: Entity<SelectState<Vec<Opt>>>,
    flow_control: Entity<SelectState<Vec<Opt>>>,
    missing_drivers: Vec<crate::data::serial::chipsets::USBSerialCandidate>,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    let s = *cx.global::<SkinTokens>();
    // bg_hover (matching the main sidebar) tints more reliably on warm
    // dark skins like Foundry where bg_input_hover stays close to the
    // panel bg.
    let hover_bg = s.bg_hover;
    // Serial port row: select on the left (flex_1), rescan icon
    // on the right. Click rescans the OS port list and reapplies
    // the current selection.
    let port_row = div()
        .flex()
        .flex_row()
        .items_end()
        .gap_2()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(labeled(s, "SERIAL PORT", Select::new(&port))),
        )
        .child(
            div()
                // Stable id so gpui notifies on hover transitions —
                // same fix as `profile_row`.
                .id("editor-rescan-ports")
                .px_2()
                .py_1()
                .bg(rgba(s.bg_input))
                .text_color(rgba(s.fg_primary))
                .rounded_md()
                .cursor_pointer()
                .hover(move |st| st.bg(rgba(hover_bg)))
                .child("↻")
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, window, cx| {
                        this.rescan_ports(window, cx);
                    }),
                ),
        );
    // Build a "driver not loaded" banner per detected candidate so
    // an unenrolled CP210x / FTDI / PL2303 / CH340 shows up RIGHT
    // ABOVE the Serial Port picker — same shape the Tauri version
    // uses. Hidden when `Settings::disable_driver_detection` is on
    // (build_editor passes an empty Vec).
    let driver_banners: Vec<gpui::Div> = missing_drivers
        .into_iter()
        .map(|d| driver_banner_row(s, d, cx))
        .collect();
    let body = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(labeled(s, "NAME", Input::new(&name).appearance(true)))
        .when(!driver_banners.is_empty(), |this| {
            this.child(div().flex().flex_col().gap_2().children(driver_banners))
        })
        .child(port_row)
        // Two-column rows of selects.
        .child(
            div()
                .flex()
                .flex_row()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .child(labeled(s, "BAUD RATE", Select::new(&baud))),
                )
                .child(
                    div()
                        .flex_1()
                        .child(labeled(s, "DATA BITS", Select::new(&data_bits))),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .child(labeled(s, "PARITY", Select::new(&parity))),
                )
                .child(
                    div()
                        .flex_1()
                        .child(labeled(s, "STOP BITS", Select::new(&stop_bits))),
                ),
        )
        .child(labeled(s, "FLOW CONTROL", Select::new(&flow_control)));
    section_card(s, "Connection", body)
}

fn terminal_card(
    line_ending: Entity<SelectState<Vec<Opt>>>,
    backspace_key: Entity<SelectState<Vec<Opt>>>,
    local_echo: bool,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    let s = *cx.global::<SkinTokens>();
    let body = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_row()
                .gap_3()
                .child(div().flex_1().child(labeled(
                    s,
                    "SEND LINE ENDING",
                    Select::new(&line_ending),
                )))
                .child(div().flex_1().child(labeled(
                    s,
                    "BACKSPACE SENDS",
                    Select::new(&backspace_key),
                ))),
        )
        .child(bool_field(
            "local-echo",
            "Local echo",
            local_echo,
            cx,
            |ed, v| ed.local_echo = v,
        ));
    section_card(s, "Terminal", body)
}

/// Generic checkbox row that writes back to the open editor when
/// toggled. The closure parameter (`F`) is how we get a typed
/// `&mut Editor` lookup at the right field — passing the field
/// name as a string would force runtime dispatch for no benefit.
fn bool_field<F>(
    id: &'static str,
    label: &'static str,
    checked: bool,
    cx: &mut Context<AppView>,
    set: F,
) -> gpui::Div
where
    F: Fn(&mut Editor, bool) + 'static,
{
    bool_field_hinted(id, label, None, checked, cx, set)
}

/// Like `bool_field`, but also renders a small muted hint string
/// to the right of the checkbox. Mirrors the Tauri form's "Hex
/// view ┄ show incoming bytes as hex dump" pattern.
fn bool_field_hinted<F>(
    id: &'static str,
    label: &'static str,
    hint: Option<&'static str>,
    checked: bool,
    cx: &mut Context<AppView>,
    set: F,
) -> gpui::Div
where
    F: Fn(&mut Editor, bool) + 'static,
{
    let s = *cx.global::<SkinTokens>();
    let cb = Checkbox::new(id)
        .checked(checked)
        .label(label)
        .on_click(cx.listener(move |this, checked: &bool, _window, cx| {
            if let Some(ed) = this.editor.as_mut() {
                set(ed, *checked);
            }
            cx.notify();
        }));
    let mut row = div().flex().flex_row().items_center().gap_3().child(cb);
    if let Some(h) = hint {
        row = row.child(
            div()
                .text_size(px(12.0))
                .text_color(rgba(s.fg_secondary))
                .child(h),
        );
    }
    row
}

/// Profile-level Highlighting tab. Two cards: the master toggle that
/// enables/disables highlighting for this profile (mirrors
/// `Profile::highlight`), and the per-pack list with an "Override
/// global" switch on top. When the override is off, the per-pack
/// rows are still shown but read-only, so the user can see what
/// they're inheriting from Settings without flipping the switch.
fn highlighting_pane(
    highlight: bool,
    override_packs: bool,
    enabled_packs: Vec<String>,
    packs: Vec<crate::data::highlight::HighlightPack>,
    global_enabled: Vec<String>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();

    // When override is off, the rows mirror the global pick; when
    // on, they reflect the per-profile working list. Disabled state
    // is the same in either case — toggling needs override + master.
    let effective: &Vec<String> = if override_packs {
        &enabled_packs
    } else {
        &global_enabled
    };

    let rows: Vec<gpui::Div> = packs
        .into_iter()
        .map(|p| {
            let id_for_setter = p.id.clone();
            let cb_id = SharedString::from(format!("profile-highlight-{}", p.id));
            let is_on = effective.iter().any(|e| e == &p.id);
            let label = if p.source == "user" || p.source == "import" {
                format!("{} (custom)", p.name)
            } else {
                p.name.clone()
            };
            let desc = p
                .description
                .clone()
                .filter(|d| !d.is_empty())
                .unwrap_or_else(|| "\u{2014}".to_string());
            let cb = Checkbox::new(cb_id)
                .checked(is_on)
                .disabled(!override_packs || !highlight)
                .label(label)
                .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                    this.toggle_editor_highlight_pack(id_for_setter.clone(), *checked, cx);
                }));
            div().flex().flex_col().gap_1().child(cb).child(
                div()
                    .pl(px(24.0))
                    .text_size(px(12.0))
                    .text_color(rgba(s.fg_secondary))
                    .whitespace_normal()
                    .child(desc),
            )
        })
        .collect();

    let master_card = section_card_with_desc(
        s,
        "Highlighting",
        Some(
            "Master switch for this profile. When off, incoming \
             output is rendered without any rule-based colouring \
             regardless of which packs are enabled below.",
        ),
        bool_field(
            "profile-highlight-master",
            "Highlight terminal output for this profile",
            highlight,
            cx,
            |ed, on| ed.highlight = on,
        ),
    );

    // Override toggle goes directly to AppView so it can seed the
    // per-profile list from the global on first-on (otherwise the
    // user flips the switch and silently loses their inherited
    // selection).
    let override_row = div().flex().flex_row().items_center().gap_3().child(
        Checkbox::new("profile-highlight-override")
            .checked(override_packs)
            .disabled(!highlight)
            .label("Override global pack selection")
            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                this.set_editor_override_highlight(*checked, cx);
            })),
    );

    let packs_card = section_card_with_desc(
        s,
        "Highlight Packs",
        Some(
            "Inherit the global selection from Settings, or override \
             it for this profile. With override off, the rows show \
             what the global is currently broadcasting (read-only).",
        ),
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(override_row)
            .child(div().flex().flex_col().gap_2().children(rows)),
    );

    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(master_card)
        .child(packs_card)
}

#[allow(clippy::too_many_arguments)]
fn advanced_pane(
    dtr_on_connect: Entity<SelectState<Vec<Opt>>>,
    rts_on_connect: Entity<SelectState<Vec<Opt>>>,
    dtr_on_disconnect: Entity<SelectState<Vec<Opt>>>,
    rts_on_disconnect: Entity<SelectState<Vec<Opt>>>,
    hex_view: bool,
    timestamps: bool,
    line_numbers: bool,
    log_enabled: bool,
    auto_reconnect: bool,
    paste_warn_multiline: bool,
    paste_slow: bool,
    paste_char_delay_ms: Entity<InputState>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let s = *cx.global::<SkinTokens>();
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            // Top-of-tab heading + description, mirroring the Tauri
            // form. Lives outside the cards so it groups them
            // visually under one umbrella concern.
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(18.0))
                        .text_color(rgba(s.fg_primary))
                        .child("Advanced"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgba(s.fg_secondary))
                        .whitespace_normal()
                        .child("Control lines, hex view, timestamps, session logging."),
                ),
        )
        .child(control_lines_card(
            s,
            dtr_on_connect,
            rts_on_connect,
            dtr_on_disconnect,
            rts_on_disconnect,
        ))
        .child(output_card(
            hex_view,
            timestamps,
            line_numbers,
            log_enabled,
            auto_reconnect,
            cx,
        ))
        .child(paste_safety_card(
            paste_warn_multiline,
            paste_slow,
            paste_char_delay_ms,
            cx,
        ))
}

/// Per-profile theme override card. The `theme` Select's first
/// option is "Use global default" with an empty id; saving with
/// that selected leaves `Profile::theme_id` empty, which
/// `compute_palette` treats as fall-through to
/// `Settings::default_theme_id`. The same Select otherwise lists
/// every theme `themes_store` knows about.
fn theme_card(s: SkinTokens, theme: Entity<SelectState<Vec<Opt>>>) -> gpui::Div {
    section_card_with_desc(
        s,
        "Terminal Theme",
        Some(
            "Override the global default theme just for this profile. \
             Useful for keeping different palettes on different \
             devices (e.g. red-tinged for production routers, calm \
             green for the lab switch). Leave on \"Use global \
             default\" to inherit from Settings \u{2192} Themes.",
        ),
        Select::new(&theme),
    )
}

fn control_lines_card(
    s: SkinTokens,
    dtr_on_connect: Entity<SelectState<Vec<Opt>>>,
    rts_on_connect: Entity<SelectState<Vec<Opt>>>,
    dtr_on_disconnect: Entity<SelectState<Vec<Opt>>>,
    rts_on_disconnect: Entity<SelectState<Vec<Opt>>>,
) -> gpui::Div {
    let row = |left_label, left, right_label, right| {
        div()
            .flex()
            .flex_row()
            .gap_3()
            .child(div().flex_1().child(labeled(s, left_label, left)))
            .child(div().flex_1().child(labeled(s, right_label, right)))
    };
    let body = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(row(
            "DTR ON CONNECT",
            Select::new(&dtr_on_connect),
            "RTS ON CONNECT",
            Select::new(&rts_on_connect),
        ))
        .child(row(
            "DTR ON DISCONNECT",
            Select::new(&dtr_on_disconnect),
            "RTS ON DISCONNECT",
            Select::new(&rts_on_disconnect),
        ));
    section_card_with_desc(
        s,
        "Control Lines",
        Some(
            "Only needed for specific adapters or devices (RS-485 direction, \
             Arduino DTR-reset, firmwares that key off DTR for session lifecycle).",
        ),
        body,
    )
}

fn output_card(
    hex_view: bool,
    timestamps: bool,
    line_numbers: bool,
    log_enabled: bool,
    auto_reconnect: bool,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    let s = *cx.global::<SkinTokens>();
    // Single column. The 2-col grid for Hex view + Line timestamps
    // produced an awkward orphan-feel where the right-hand "Line
    // timestamps" hint wrapped while the others were full-width;
    // stacking reads cleaner and matches the rest of the card's
    // vertical rhythm.
    let body = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(bool_field_hinted(
            "timestamps",
            "Line timestamps",
            Some("prefix each line with wall-clock time"),
            timestamps,
            cx,
            |ed, v| ed.timestamps = v,
        ))
        .child(bool_field_hinted(
            "line-numbers",
            "Line numbers",
            Some("prefix each line with a session-local counter (resets on reconnect)"),
            line_numbers,
            cx,
            |ed, v| ed.line_numbers = v,
        ))
        .child(bool_field_hinted(
            "hex-view",
            "Hex view",
            Some("show incoming bytes as hex dump"),
            hex_view,
            cx,
            |ed, v| ed.hex_view = v,
        ))
        .child(bool_field_hinted(
            "log-enabled",
            "Record session to file",
            Some("raw bytes; destination set in Settings → Advanced"),
            log_enabled,
            cx,
            |ed, v| ed.log_enabled = v,
        ))
        .child(bool_field_hinted(
            "auto-reconnect",
            "Auto-reconnect on drop",
            Some("poll for the port to reappear (up to 30s) and reopen transparently"),
            auto_reconnect,
            cx,
            |ed, v| ed.auto_reconnect = v,
        ));
    section_card(s, "Output", body)
}

fn paste_safety_card(
    paste_warn_multiline: bool,
    paste_slow: bool,
    paste_char_delay_ms: Entity<InputState>,
    cx: &mut Context<AppView>,
) -> gpui::Div {
    let s = *cx.global::<SkinTokens>();
    // Slow-paste delay input gets its own row, indented under the
    // checkbox. Sharing a flex_row with the "Slow paste" hint made
    // the input visibly shrink as the window grew because the hint
    // text claimed more horizontal space at the input's expense.
    // 120px is enough for the largest sane delay (3 digits) plus
    // the small chevron padding the Input draws internally.
    let body = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(bool_field_hinted(
            "paste-warn",
            "Confirm multi-line pastes",
            Some("prompt before sending pasted text that contains line breaks"),
            paste_warn_multiline,
            cx,
            |ed, v| ed.paste_warn_multiline = v,
        ))
        .child(bool_field_hinted(
            "paste-slow",
            "Slow paste",
            Some("send one char at a time with a delay"),
            paste_slow,
            cx,
            |ed, v| ed.paste_slow = v,
        ))
        .child(div().pl_6().w(px(160.0)).child(labeled(
            s,
            "SLOW-PASTE DELAY (MS)",
            Input::new(&paste_char_delay_ms).small().appearance(true),
        )));
    section_card_with_desc(
        s,
        "Paste safety",
        Some(
            "Catch the \"I pasted into the wrong window\" mistake, and pace pastes so \
             UARTs on slower devices don't drop bytes.",
        ),
        body,
    )
}
