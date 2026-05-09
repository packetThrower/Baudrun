//! Standalone Settings window. Mirrors the Tauri layout — a separate
//! OS window so the user can change skins / themes / shortcuts and
//! watch them apply live in the main window without having to flip
//! back and forth past a modal sheet.
//!
//! Phase 3 slice 1 (scaffold): the window opens, the left rail
//! switches between five tabs, and each tab renders a "coming soon"
//! placeholder. Subsequent slices fill the panes.

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, rgb, Context, IntoElement, MouseButton, MouseUpEvent, Render, Window,
};
use gpui_component::Root;

use crate::data::settings;

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
/// settings store so changes round-trip to disk without going
/// through `AppView`. Future slices that need the main window to
/// react live (skin/theme swap) will plug in via a shared
/// notification mechanism (TBD — a `gpui::Subscription` against an
/// observable settings entity is the most likely fit).
pub struct SettingsView {
    #[allow(dead_code)] // wired in subsequent Phase-3 slices
    settings_store: Rc<settings::Store>,
    tab: SettingsTab,
}

impl SettingsView {
    pub fn new(settings_store: Rc<settings::Store>, _cx: &mut Context<Self>) -> Self {
        Self {
            settings_store,
            tab: SettingsTab::default(),
        }
    }

    fn set_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        if self.tab == tab {
            return;
        }
        self.tab = tab;
        cx.notify();
    }
}

// Palette mirrors the values used in `app_view.rs` so the Settings
// window agrees visually with the main window. Kept local rather
// than re-exported to avoid coupling the two modules' chrome
// constants — the pair will be unified when the skin system lands
// in a later slice.
const SIDEBAR_BG: u32 = 0x262627;
const SIDEBAR_BORDER: u32 = 0x2a2a30;
const SIDEBAR_FG: u32 = 0xd4d4d8;
const SIDEBAR_SELECTED: u32 = 0x2d3548;
const FORM_BG: u32 = 0x18181a;

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.tab;

        let rail = div()
            .w(px(180.0))
            .h_full()
            .bg(rgb(SIDEBAR_BG))
            .border_r_1()
            .border_color(rgb(SIDEBAR_BORDER))
            .px_2()
            .py_3()
            .flex()
            .flex_col()
            .gap_1()
            .text_color(rgb(SIDEBAR_FG))
            .text_size(px(13.0))
            .children(SettingsTab::ALL.iter().map(|&tab| {
                let is_active = tab == active;
                div()
                    .w_full()
                    .px_3()
                    .py_2()
                    .rounded_sm()
                    .bg(if is_active {
                        rgb(SIDEBAR_SELECTED)
                    } else {
                        rgb(SIDEBAR_BG)
                    })
                    .hover(|s| s.bg(rgb(SIDEBAR_SELECTED)))
                    .cursor_pointer()
                    .child(tab.label())
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseUpEvent, _, cx| {
                            this.set_tab(tab, cx);
                        }),
                    )
            }));

        let pane = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .p_4()
            .bg(rgb(FORM_BG))
            .text_color(rgb(SIDEBAR_FG))
            .text_size(px(13.0))
            .child(format!("{} (coming soon)", active.label()));

        // Render the gpui-component dialog/notification overlay
        // layers so future slices' confirmations (e.g. delete-skin)
        // and toasts paint on top of the Settings content. Mirrors
        // the AppView render path.
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_row()
                    .child(rail)
                    .child(pane),
            )
            .children(dialog_layer)
            .children(notification_layer)
    }
}
