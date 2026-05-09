//! Standalone Settings window. Mirrors the Tauri layout — a separate
//! OS window so the user can change skins / themes / shortcuts and
//! watch them apply live in the main window without having to flip
//! back and forth past a modal sheet.
//!
//! Visual chrome (palette, card style, rail style, font sizes) is
//! kept in sync with the profile editor (`app_view.rs::form_pane`)
//! so the two windows feel like one app rather than two surfaces.

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, rgb, rgba, AppContext, Context, Entity, IntoElement, MouseButton,
    MouseUpEvent, Render, Subscription, Window,
};
use gpui_component::{
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState},
    Root, Sizable,
};

use crate::data::settings::{self, Settings};

/// Top-level Settings tabs. Order + labels mirror the Tauri
/// `Settings.svelte` left rail (Appearance, Themes, Shortcuts,
/// Highlighting, Advanced) so the shipping app's muscle memory
/// transfers directly.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Appearance,
    Themes,
    Shortcuts,
    Highlighting,
    Advanced,
}

impl SettingsTab {
    const ALL: [SettingsTab; 5] = [
        SettingsTab::Appearance,
        SettingsTab::Themes,
        SettingsTab::Shortcuts,
        SettingsTab::Highlighting,
        SettingsTab::Advanced,
    ];

    fn label(self) -> &'static str {
        match self {
            SettingsTab::Appearance => "Appearance",
            SettingsTab::Themes => "Themes",
            SettingsTab::Shortcuts => "Shortcuts",
            SettingsTab::Highlighting => "Highlighting",
            SettingsTab::Advanced => "Advanced",
        }
    }
}

/// Root view for the Settings window. Owns its own clone of the
/// settings store + a cached copy of the latest `Settings` value.
/// Edits flow `widget → field handler → commit() → store.update →
/// cache update → cx.notify`. The cache lets the next render read
/// the new state without going back through `store.get()` for
/// every widget.
pub struct SettingsView {
    settings_store: Rc<settings::Store>,
    settings: Settings,
    tab: SettingsTab,

    // -- Advanced tab field state --
    log_dir: Entity<InputState>,
    /// Subscription handle for the log-dir Blur listener. Held to
    /// keep the subscription alive for the SettingsView's lifetime;
    /// dropping it cancels the subscription.
    _log_dir_sub: Subscription,
}

impl SettingsView {
    pub fn new(
        settings_store: Rc<settings::Store>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let current = settings_store.get();

        let log_dir = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Default")
                .default_value(current.log_dir.as_str())
        });

        // Persist log_dir on Blur (focus loss). Live save per
        // keystroke would write the JSON file on every typed
        // character — wasteful, and noisy for `fs::write`.
        let log_dir_sub =
            cx.subscribe(&log_dir, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Blur) {
                    let value = input.read(cx).value().to_string();
                    if value != this.settings.log_dir {
                        let mut next = this.settings.clone();
                        next.log_dir = value;
                        this.commit(next, cx);
                    }
                }
            });

        Self {
            settings_store,
            settings: current,
            tab: SettingsTab::default(),
            log_dir,
            _log_dir_sub: log_dir_sub,
        }
    }

    fn set_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        if self.tab == tab {
            return;
        }
        self.tab = tab;
        cx.notify();
    }

    /// Push a new Settings value through the store. On success the
    /// cache + UI state advance; on failure (disk full, perm
    /// denied) the previous cache stays put and the error logs.
    /// Failure is silent in the UI today — a future slice can hang
    /// a toast off this same path.
    fn commit(&mut self, next: Settings, cx: &mut Context<Self>) {
        match self.settings_store.update(next) {
            Ok(saved) => {
                self.settings = saved;
                cx.notify();
            }
            Err(err) => {
                log::error!("save settings: {err}");
            }
        }
    }

    // -- Advanced tab field setters --

    fn set_detect_drivers(&mut self, enabled: bool, cx: &mut Context<Self>) {
        // Stored inverted (`disable_driver_detection`) so the
        // "default off" semantics serialize cleanly via
        // `skip_serializing_if = "is_false"`.
        let mut next = self.settings.clone();
        next.disable_driver_detection = !enabled;
        self.commit(next, cx);
    }

    fn set_copy_on_select(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let mut next = self.settings.clone();
        next.copy_on_select = enabled;
        self.commit(next, cx);
    }

    fn set_check_updates(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let mut next = self.settings.clone();
        // Inverted same as drivers — see comment above.
        next.disable_update_check = !enabled;
        self.commit(next, cx);
    }

    fn set_include_prerelease(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let mut next = self.settings.clone();
        next.include_prerelease_updates = enabled;
        self.commit(next, cx);
    }
}

// Palette mirrors `app_view.rs`'s Baudrun-skin constants verbatim
// so the Settings window's chrome reads as a continuation of the
// main window. Kept local rather than re-exported to avoid forcing
// app_view.rs to expose them publicly; the pair will be unified
// when the skin system lands in a later slice.
const SIDEBAR_FG: u32 = 0xd4d4d8;
const FORM_BG: u32 = 0x18181a;
const PANEL_BG: u32 = 0xFFFFFF0F;
const BORDER_SUBTLE: u32 = 0xFFFFFF14;
const FG_SECONDARY: u32 = 0xFFFFFFA6;
const FG_TERTIARY: u32 = 0xFFFFFF66;
const BTN_BG: u32 = 0xFFFFFF14;
const BTN_FG: u32 = 0xFFFFFFF2;
const TAB_ACTIVE_BG: u32 = 0x007AFF40;

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        div()
            .size_full()
            .relative()
            .bg(rgb(FORM_BG))
            .text_color(rgb(SIDEBAR_FG))
            .text_size(px(13.0))
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(window_header())
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .flex_row()
                            .child(rail(self.tab, cx))
                            .child(scrollable_pane(self.pane_content(cx))),
                    ),
            )
            .children(dialog_layer)
            .children(notification_layer)
    }
}

impl SettingsView {
    fn pane_content(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        match self.tab {
            SettingsTab::Advanced => self.advanced_pane(cx).into_any_element(),
            other => placeholder_pane(other.label()).into_any_element(),
        }
    }

    fn advanced_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let s = &self.settings;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(section_card_with_desc(
                "Session Log Directory",
                Some(
                    "Where profiles with \"Record session to file\" enabled \
                     write their logs. Leave blank to use the default.",
                ),
                Input::new(&self.log_dir).small().appearance(true),
            ))
            .child(section_card_with_desc(
                "USB Driver Detection",
                Some(
                    "Show a banner in the profile form when a USB-serial \
                     adapter is plugged in without its vendor driver \
                     installed.",
                ),
                bool_field(
                    "settings-detect-drivers",
                    "Detect un-drivered USB adapters",
                    !s.disable_driver_detection,
                    cx,
                    SettingsView::set_detect_drivers,
                ),
            ))
            .child(section_card_with_desc(
                "Copy on Select",
                Some(
                    "PuTTY-style — copy the terminal selection to the \
                     clipboard automatically when the mouse is released. \
                     Avoids having to press \u{2318}/Ctrl+C for every snippet.",
                ),
                bool_field(
                    "settings-copy-on-select",
                    "Copy terminal selection to clipboard automatically",
                    s.copy_on_select,
                    cx,
                    SettingsView::set_copy_on_select,
                ),
            ))
            .child(section_card_with_desc(
                "Updates",
                Some("Check GitHub on app launch for a newer Baudrun release."),
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(bool_field(
                        "settings-check-updates",
                        "Check for updates on launch",
                        !s.disable_update_check,
                        cx,
                        SettingsView::set_check_updates,
                    ))
                    .child(bool_field(
                        "settings-prerelease-updates",
                        "Include pre-releases (alpha / beta / rc)",
                        s.include_prerelease_updates,
                        cx,
                        SettingsView::set_include_prerelease,
                    )),
            ))
    }
}

/// Window-top header bar. Mirrors `form_header` — title at the
/// Baudrun `--font-size-h1` (24px), uppercase tag underneath, full
/// width with a subtle bottom border.
fn window_header() -> impl IntoElement {
    div()
        .w_full()
        .px_6()
        .py_3()
        .border_b_1()
        .border_color(rgba(BORDER_SUBTLE))
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(24.0))
                .text_color(rgb(SIDEBAR_FG))
                .child("Settings"),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(rgba(FG_TERTIARY))
                .child("GLOBAL DEFAULTS"),
        )
}

/// Left-rail tab nav. Same shape as `form_tab_nav` — translucent
/// blue active background, muted text for inactive entries, hover
/// fades to the button-bg token.
fn rail(active: SettingsTab, cx: &mut Context<SettingsView>) -> impl IntoElement {
    let item = |label: &'static str, tab: SettingsTab| {
        let is_active = tab == active;
        let bg = if is_active {
            rgba(TAB_ACTIVE_BG)
        } else {
            rgba(0x00000000)
        };
        let fg = if is_active {
            rgba(BTN_FG)
        } else {
            rgba(FG_SECONDARY)
        };
        div()
            .w_full()
            .px_3()
            .py(px(6.0))
            .rounded_md()
            .bg(bg)
            .text_color(fg)
            .cursor_pointer()
            .hover(|s| s.bg(rgba(BTN_BG)))
            .child(label)
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
        .border_color(rgba(BORDER_SUBTLE))
        .flex()
        .flex_col()
        .gap_1()
        .text_size(px(13.0))
        .children(SettingsTab::ALL.iter().map(|&t| item(t.label(), t)))
}

/// Scrollable pane wrapper. Same min_w_0/min_h_0 dance as
/// `form_body` — without those the cards' intrinsic min-widths
/// push the pane past the window edge and the scroll never fires.
fn scrollable_pane(content: impl IntoElement) -> impl IntoElement {
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
fn section_card_with_desc(
    title: &'static str,
    description: Option<&'static str>,
    body: impl IntoElement,
) -> gpui::Div {
    let mut header = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(15.0))
                .text_color(rgb(SIDEBAR_FG))
                .child(title),
        );
    if let Some(desc) = description {
        header = header.child(
            div()
                .text_size(px(12.0))
                .text_color(rgba(FG_SECONDARY))
                // gpui defaults text to nowrap; long descriptions
                // would otherwise run off the right edge.
                .whitespace_normal()
                .child(desc),
        );
    }
    div()
        .w_full()
        .bg(rgba(PANEL_BG))
        .border_1()
        .border_color(rgba(BORDER_SUBTLE))
        .rounded(px(10.0))
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
fn bool_field<F>(
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

fn placeholder_pane(label: &'static str) -> impl IntoElement {
    div()
        .text_size(px(13.0))
        .text_color(rgba(FG_SECONDARY))
        .child(format!("{label} (coming soon)"))
}
