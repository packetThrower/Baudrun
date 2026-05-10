//! Standalone Settings window. Mirrors the Tauri layout — a separate
//! OS window so the user can change skins / themes / shortcuts and
//! watch them apply live in the main window without having to flip
//! back and forth past a modal sheet.
//!
//! Visual chrome (palette, card style, rail style, font sizes) is
//! kept in sync with the profile editor (`app_view.rs::form_pane`)
//! so the two windows feel like one app rather than two surfaces.

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    div, prelude::*, px, rgba, AppContext, Context, Entity, FocusHandle, IntoElement,
    KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseUpEvent, Render, SharedString,
    Subscription, Window,
};
use gpui_component::{
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState},
    kbd::Kbd,
    select::{Select, SelectEvent, SelectItem, SelectState},
    IndexPath, Root, Sizable,
};

use crate::data::highlight;
use crate::data::settings::Settings;
use crate::data::skins;
use crate::data::themes;
use crate::settings_bus::{SettingsBus, SettingsEvent};
use crate::skin_tokens::SkinTokens;

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

/// Local Select-item shape. Mirrors the same struct in
/// `app_view.rs` (kept duplicated rather than shared because the
/// SelectItem trait impl is the entire payload — moving it to a
/// shared module would just shuffle 30 lines around). Carries an id
/// + display title; `Value` is the id so selection events return a
/// matchable string.
#[derive(Clone)]
struct Opt {
    id: String,
    title: SharedString,
}

impl Opt {
    fn new(id: &str, title: &str) -> Self {
        Self {
            id: id.to_string(),
            title: SharedString::from(title.to_string()),
        }
    }
}

impl SelectItem for Opt {
    type Value = String;
    fn title(&self) -> SharedString {
        self.title.clone()
    }
    fn value(&self) -> &Self::Value {
        &self.id
    }
}

/// Root view for the Settings window. Owns its own clones of the
/// settings + skins stores + a cached copy of the latest `Settings`
/// value. Edits flow `widget → field handler → commit() →
/// store.update → cache update → cx.notify`. The cache lets the
/// next render read the new state without going back through
/// `store.get()` for every widget.
pub struct SettingsView {
    /// Shared settings entity. Replaces the direct Store handle:
    /// commit() now goes `bus.replace(next)` so the persistence
    /// + cross-window emit happen in one place. Read via
    /// `bus.read(cx).current()` or — more often — through the
    /// local `self.settings` cache which mirrors the bus.
    settings_bus: Entity<SettingsBus>,
    #[allow(dead_code)] // re-read on render via the entity cache; kept
                        // around for live-apply (skin import etc.)
    skins_store: Rc<skins::Store>,
    /// Highlight pack store. Re-read on every render of the
    /// Highlighting pane so the toggle list reflects packs the user
    /// imported in the current session (post-launch import refreshes
    /// without restarting Settings).
    highlight_store: Rc<highlight::Store>,
    /// SettingsBus subscription that triggers a re-render whenever
    /// AppView refreshes the `SkinTokens` global. Without this, a
    /// skin pick wouldn't repaint the Settings window itself —
    /// only the main window would update.
    _bus_sub: Subscription,
    #[allow(dead_code)] // enumerated through `theme_select` today;
                        // kept around for future "import theme"
                        // affordance + live-preview hooks.
    themes_store: Rc<themes::Store>,
    settings: Settings,
    tab: SettingsTab,

    // -- Appearance tab state --
    skin_select: Entity<SelectState<Vec<Opt>>>,
    _skin_sub: Subscription,
    appearance_select: Entity<SelectState<Vec<Opt>>>,
    _appearance_sub: Subscription,
    font_size: Entity<InputState>,
    _font_size_sub: Subscription,

    // -- Themes tab state --
    theme_select: Entity<SelectState<Vec<Opt>>>,
    _theme_sub: Subscription,

    // -- Shortcuts tab state --
    /// Action id (`"connect"`, `"clear"`, …) of the row currently
    /// in capture mode, or `None` when no row is recording. Only
    /// one row can capture at a time so the typed key combo
    /// unambiguously belongs to one binding.
    capturing_shortcut: Option<&'static str>,
    /// Focus handle for the per-row capture div. Lives at the view
    /// level (one handle, swapped between rows via render-time
    /// conditional `.track_focus`) so we don't have to mint one
    /// per binding row.
    capture_focus: FocusHandle,

    // -- Advanced tab field state --
    log_dir: Entity<InputState>,
    /// Subscription handles held to keep the subscriptions alive
    /// for the SettingsView's lifetime; dropping them cancels the
    /// subscription. Underscore prefix marks "owned solely for
    /// drop-time effect, never read."
    _log_dir_sub: Subscription,
}

impl SettingsView {
    pub fn new(
        settings_bus: Entity<SettingsBus>,
        skins_store: Rc<skins::Store>,
        highlight_store: Rc<highlight::Store>,
        themes_store: Rc<themes::Store>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let current = settings_bus.read(cx).current().clone();
        // Re-render when a skin / theme / anything changes — the
        // chrome reads from `SkinTokens` (refreshed by AppView's
        // own subscription handler), so a notify here is enough to
        // pick up the new global on the next paint.
        let bus_sub = cx.subscribe(&settings_bus, |_, _, _: &SettingsEvent, cx| {
            cx.notify();
        });

        // -- Appearance tab widgets --

        let skin_opts: Vec<Opt> = skins_store
            .list()
            .into_iter()
            .map(|s| {
                // Append "(custom)" to user skins so they're
                // distinguishable from built-ins in the picker.
                let title = if s.source == "user" {
                    format!("{} (custom)", s.name)
                } else {
                    s.name
                };
                Opt::new(&s.id, &title)
            })
            .collect();
        let active_skin_id = if current.skin_id.is_empty() {
            skins::DEFAULT_SKIN_ID
        } else {
            current.skin_id.as_str()
        };
        let skin_select = make_select(skin_opts, active_skin_id, window, cx);
        let skin_sub = cx.subscribe(
            &skin_select,
            |this, _, event: &SelectEvent<Vec<Opt>>, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    if &this.settings.skin_id != id {
                        let mut next = this.settings.clone();
                        next.skin_id = id.clone();
                        this.commit(next, cx);
                    }
                }
            },
        );

        let appearance_opts = vec![
            Opt::new("auto", "Auto (follow system)"),
            Opt::new("light", "Light"),
            Opt::new("dark", "Dark"),
        ];
        let active_appearance = if current.appearance.is_empty() {
            "auto"
        } else {
            current.appearance.as_str()
        };
        let appearance_select =
            make_select(appearance_opts, active_appearance, window, cx);
        let appearance_sub = cx.subscribe(
            &appearance_select,
            |this, _, event: &SelectEvent<Vec<Opt>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    if &this.settings.appearance != value {
                        let mut next = this.settings.clone();
                        next.appearance = value.clone();
                        this.commit(next, cx);
                    }
                }
            },
        );

        let font_size_initial = if current.font_size > 0 {
            current.font_size.to_string()
        } else {
            String::new()
        };
        let font_size = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("13")
                .default_value(font_size_initial.as_str())
        });
        let font_size_sub =
            cx.subscribe(&font_size, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Blur) {
                    let raw = input.read(cx).value().to_string();
                    let parsed = raw.trim();
                    let next_value = if parsed.is_empty() {
                        // Empty resets to "use default" (stored as 0
                        // so `skip_serializing_if = "is_zero"` keeps
                        // settings.json clean).
                        0
                    } else {
                        match parsed.parse::<i32>() {
                            Ok(n) if n > 0 => n,
                            _ => {
                                log::warn!("font size: invalid value {raw:?}");
                                return;
                            }
                        }
                    };
                    if next_value != this.settings.font_size {
                        let mut next = this.settings.clone();
                        next.font_size = next_value;
                        this.commit(next, cx);
                    }
                }
            });

        // -- Themes tab widgets --

        let theme_opts: Vec<Opt> = themes_store
            .list()
            .into_iter()
            .map(|t| {
                let title = if t.source == "user" {
                    format!("{} (custom)", t.name)
                } else {
                    t.name
                };
                Opt::new(&t.id, &title)
            })
            .collect();
        let active_theme_id = if current.default_theme_id.is_empty() {
            themes::DEFAULT_THEME_ID
        } else {
            current.default_theme_id.as_str()
        };
        let theme_select = make_select(theme_opts, active_theme_id, window, cx);
        let theme_sub = cx.subscribe(
            &theme_select,
            |this, _, event: &SelectEvent<Vec<Opt>>, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    if &this.settings.default_theme_id != id {
                        let mut next = this.settings.clone();
                        next.default_theme_id = id.clone();
                        this.commit(next, cx);
                    }
                }
            },
        );

        // -- Advanced tab widgets --

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
            settings_bus,
            skins_store,
            highlight_store,
            themes_store,
            _bus_sub: bus_sub,
            settings: current,
            tab: SettingsTab::default(),
            skin_select,
            _skin_sub: skin_sub,
            appearance_select,
            _appearance_sub: appearance_sub,
            font_size,
            _font_size_sub: font_size_sub,
            theme_select,
            _theme_sub: theme_sub,
            capturing_shortcut: None,
            capture_focus: cx.focus_handle(),
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

    /// Push a new Settings value through the SettingsBus. The bus
    /// persists to disk and broadcasts an `Updated` event so the
    /// main window re-renders with the new values; on success we
    /// advance our local cache to mirror it. On failure (disk
    /// full, perm denied) the cache stays put and the error logs.
    /// Failure is silent in the UI today — a future slice can hang
    /// a toast off this same path.
    fn commit(&mut self, next: Settings, cx: &mut Context<Self>) {
        let result = self
            .settings_bus
            .update(cx, |bus, cx| bus.replace(next.clone(), cx));
        match result {
            Ok(()) => {
                self.settings = next;
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

    /// Effective enabled-pack list — falls back to `Settings::default`'s
    /// value when the user hasn't made an explicit pick yet (the
    /// `Option::None` state). Mirrors the Tauri reader, which treats
    /// `None` as "use defaults" and only honours `Some(_)` (including
    /// `Some(empty)` for the explicit "no highlighting" opt-out).
    fn enabled_packs(&self) -> Vec<String> {
        self.settings
            .enabled_highlight_presets
            .clone()
            .unwrap_or_else(|| {
                Settings::default()
                    .enabled_highlight_presets
                    .unwrap_or_default()
            })
    }

    fn toggle_highlight_pack(&mut self, id: String, enabled: bool, cx: &mut Context<Self>) {
        let mut packs = self.enabled_packs();
        let already = packs.iter().any(|p| p == &id);
        if enabled && !already {
            packs.push(id);
        } else if !enabled && already {
            packs.retain(|p| p != &id);
        } else {
            return;
        }
        let mut next = self.settings.clone();
        next.enabled_highlight_presets = Some(packs);
        self.commit(next, cx);
    }

    // -- Shortcuts tab handlers --

    fn start_capture(
        &mut self,
        action: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.capturing_shortcut = Some(action);
        // Pull focus into the capture div so the very next key
        // press is handled by `handle_capture_key`. Without this
        // the user would have to also click the div to focus it.
        self.capture_focus.focus(window, cx);
        cx.notify();
    }

    fn cancel_capture(&mut self, cx: &mut Context<Self>) {
        if self.capturing_shortcut.take().is_some() {
            cx.notify();
        }
    }

    fn handle_capture_key(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(action) = self.capturing_shortcut else {
            return;
        };
        let key = event.keystroke.key.as_str();
        // Bare modifier presses (Shift/Ctrl/Cmd/Alt with no other
        // key) report key as the modifier name itself in gpui —
        // ignore so the user can hold modifiers down before
        // pressing the actual key.
        if matches!(key, "shift" | "ctrl" | "control" | "cmd" | "alt" | "platform") {
            return;
        }
        // Bare Escape (no modifiers) cancels the capture; Esc
        // combined with modifiers IS a valid binding to set.
        if key == "escape" && !any_modifier(&event.keystroke.modifiers) {
            self.cancel_capture(cx);
            return;
        }
        let spec = format_spec(&event.keystroke);
        let mut overrides = self.settings.shortcuts.clone().unwrap_or_default();
        overrides.insert(action.to_string(), spec);
        let mut next = self.settings.clone();
        next.shortcuts = Some(overrides);
        self.commit(next, cx);
        self.capturing_shortcut = None;
        cx.notify();
    }

    fn reset_shortcut(&mut self, action: &'static str, cx: &mut Context<Self>) {
        let Some(mut overrides) = self.settings.shortcuts.clone() else {
            return;
        };
        if overrides.remove(action).is_none() {
            return;
        }
        let mut next = self.settings.clone();
        next.shortcuts = if overrides.is_empty() {
            None
        } else {
            Some(overrides)
        };
        self.commit(next, cx);
    }
}

// Chrome colours used to live as `const`s here, but Phase 4 slice 3
// moved them into the `SkinTokens` global so skin picks live-apply.
// SettingsView's render reads `cx.global::<SkinTokens>()` once and
// hands the value to the per-section helpers.

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *cx.global::<SkinTokens>();
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        div()
            .size_full()
            .relative()
            // Opaque shell base, then translucent main pane on
            // top — same layering AppView uses, so the Settings
            // window's frosted look matches the main one.
            .bg(rgba(s.bg_window))
            .text_color(rgba(s.fg_primary))
            .text_size(px(13.0))
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(window_header(s))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .flex_row()
                            .child(rail(s, self.tab, cx))
                            .child(scrollable_pane(self.pane_content(s, cx))),
                    ),
            )
            .children(dialog_layer)
            .children(notification_layer)
    }
}

impl SettingsView {
    fn pane_content(&self, s: SkinTokens, cx: &mut Context<Self>) -> gpui::AnyElement {
        match self.tab {
            SettingsTab::Appearance => self.appearance_pane(s).into_any_element(),
            SettingsTab::Themes => self.themes_pane(s).into_any_element(),
            SettingsTab::Shortcuts => self.shortcuts_pane(s, cx).into_any_element(),
            SettingsTab::Highlighting => self.highlighting_pane(s, cx).into_any_element(),
            SettingsTab::Advanced => self.advanced_pane(s, cx).into_any_element(),
        }
    }

    fn shortcuts_pane(&self, s: SkinTokens, cx: &mut Context<Self>) -> impl IntoElement {
        let overrides = self.settings.shortcuts.clone().unwrap_or_default();
        let capturing = self.capturing_shortcut;
        let rows = SHORTCUT_ACTIONS.iter().map(|&action| {
            let label = shortcut_label(action);
            let is_capturing = capturing == Some(action);
            let is_overridden = overrides.contains_key(action);
            let effective = effective_shortcut(action, &overrides);

            // The binding cell flips appearance when capturing —
            // accent border + "Press a key…" text — so the user
            // has unambiguous feedback that input is being eaten.
            let cell: gpui::AnyElement = if is_capturing {
                div()
                    .min_w(px(150.0))
                    .px_3()
                    .py(px(6.0))
                    .rounded_md()
                    .border_1()
                    .border_color(rgba(s.accent))
                    .bg(rgba(s.bg_active))
                    .text_color(rgba(s.fg_primary))
                    .text_size(px(12.0))
                    .track_focus(&self.capture_focus)
                    .on_key_down(cx.listener(Self::handle_capture_key))
                    .child("Press a key… (Esc to cancel)")
                    .into_any_element()
            } else {
                let display: gpui::AnyElement = match parse_spec(&effective) {
                    Some(stroke) => Kbd::new(stroke).appearance(true).into_any_element(),
                    None => div()
                        .text_size(px(12.0))
                        .text_color(rgba(s.fg_tertiary))
                        .child("\u{2014}")
                        .into_any_element(),
                };
                let accent = s.accent;
                div()
                    .min_w(px(150.0))
                    .px_3()
                    .py(px(6.0))
                    .rounded_md()
                    .border_1()
                    .border_color(rgba(s.border_subtle))
                    .bg(rgba(s.bg_input))
                    .text_color(rgba(s.fg_primary))
                    .text_size(px(12.0))
                    .cursor_pointer()
                    .hover(move |st| st.border_color(rgba(accent)))
                    .child(display)
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseUpEvent, window, cx| {
                            this.start_capture(action, window, cx);
                        }),
                    )
                    .into_any_element()
            };

            // Reset button only renders when there's an actual
            // override to clear — otherwise the row is already at
            // its default and the button would be a no-op.
            let reset: gpui::AnyElement = if is_overridden {
                let hover_bg = s.bg_input;
                let hover_fg = s.fg_primary;
                div()
                    .px_2()
                    .py(px(4.0))
                    .rounded_sm()
                    .text_size(px(13.0))
                    .text_color(rgba(s.fg_secondary))
                    .cursor_pointer()
                    .hover(move |st| st.bg(rgba(hover_bg)).text_color(rgba(hover_fg)))
                    // U+21BA — "anticlockwise open circle arrow",
                    // the reset glyph Tauri uses next to each row.
                    .child("\u{21BA}")
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseUpEvent, _, cx| {
                            this.reset_shortcut(action, cx);
                        }),
                    )
                    .into_any_element()
            } else {
                div().w(px(20.0)).into_any_element()
            };

            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(div().flex_1().text_color(rgba(s.fg_primary)).child(label))
                .child(cell)
                .child(reset)
        });

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(section_card_with_desc(
                s,
                "Keyboard Shortcuts",
                Some(
                    "Click a binding to record a new key combo; Escape \
                     cancels. Use \u{21BA} to reset that row to the \
                     platform default. macOS uses Cmd-based combos \
                     (Cmd is never a terminal control character so plain \
                     \u{2318}K is safe); Linux/Windows use Ctrl+Shift so \
                     plain Ctrl+letter still passes through to the device.",
                ),
                div().flex().flex_col().gap_2().children(rows),
            ))
    }

    fn themes_pane(&self, s: SkinTokens) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(section_card_with_desc(
                s,
                "Default Theme",
                Some(
                    "Terminal palette new sessions start with — affects \
                     the 16 ANSI colours, default foreground/background, \
                     selection, and cursor. Per-profile overrides go on \
                     the profile form. Built-in themes ship with the \
                     app; user themes load from $SUPPORT_DIR/themes/.",
                ),
                Select::new(&self.theme_select),
            ))
    }

    fn highlighting_pane(&self, s: SkinTokens, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = self.enabled_packs();
        let packs = self.highlight_store.list();
        let rows = packs.into_iter().map(move |p| {
            let id_for_label = p.id.clone();
            let id_for_setter = p.id.clone();
            let is_on = enabled.iter().any(|e| e == &p.id);
            // Append "(custom)" to user/imported packs so they're
            // distinguishable from the built-in vendor presets.
            let label = if p.source == "user" || p.source == "import" {
                format!("{} (custom)", p.name)
            } else {
                p.name.clone()
            };
            // Fall back to the pack's own description; some bundled
            // packs ship without one — show a quiet "—" rather than
            // a blank row that looks like the data is missing.
            let desc = p
                .description
                .clone()
                .filter(|d| !d.is_empty())
                .unwrap_or_else(|| "\u{2014}".to_string());

            let cb_id = SharedString::from(format!("settings-highlight-{}", id_for_label));
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    Checkbox::new(cb_id)
                        .checked(is_on)
                        .label(label)
                        .on_click(cx.listener(
                            move |this, checked: &bool, _, cx| {
                                this.toggle_highlight_pack(
                                    id_for_setter.clone(),
                                    *checked,
                                    cx,
                                );
                            },
                        )),
                )
                .child(
                    div()
                        .pl(px(24.0))
                        .text_size(px(12.0))
                        .text_color(rgba(s.fg_secondary))
                        .whitespace_normal()
                        .child(desc),
                )
        });

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(section_card_with_desc(
                s,
                "Highlight Packs",
                Some(
                    "Choose which built-in or imported rule packs colorize \
                     terminal output. Stack as many as you want — matches \
                     from each pack are merged in order. With every box \
                     unchecked highlighting is off, even if a profile has \
                     it enabled.",
                ),
                div().flex().flex_col().gap_3().children(rows),
            ))
    }

    fn appearance_pane(&self, s: SkinTokens) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(section_card_with_desc(
                s,
                "App Skin",
                Some(
                    "How the app chrome (sidebar, panes, buttons) looks. \
                     Doesn't affect the terminal palette — that's the \
                     Themes tab. Imported user skins live next to the \
                     built-ins after a launch.",
                ),
                Select::new(&self.skin_select),
            ))
            .child(section_card_with_desc(
                s,
                "Appearance",
                Some(
                    "Light or dark variant of the active skin. \"Auto\" \
                     follows the system setting. Skins flagged dark-only \
                     ignore this and pin dark.",
                ),
                Select::new(&self.appearance_select),
            ))
            .child(section_card_with_desc(
                s,
                "Terminal Font Size",
                Some(
                    "Pixel size of the terminal pane's monospace font. \
                     Leave blank to use the default (13). Changes the \
                     terminal grid only — chrome text size is fixed.",
                ),
                Input::new(&self.font_size).small().appearance(true),
            ))
    }

    fn advanced_pane(&self, s: SkinTokens, cx: &mut Context<Self>) -> impl IntoElement {
        let cur = &self.settings;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(section_card_with_desc(
                s,
                "Session Log Directory",
                Some(
                    "Where profiles with \"Record session to file\" enabled \
                     write their logs. Leave blank to use the default.",
                ),
                Input::new(&self.log_dir).small().appearance(true),
            ))
            .child(section_card_with_desc(
                s,
                "USB Driver Detection",
                Some(
                    "Show a banner in the profile form when a USB-serial \
                     adapter is plugged in without its vendor driver \
                     installed.",
                ),
                bool_field(
                    "settings-detect-drivers",
                    "Detect un-drivered USB adapters",
                    !cur.disable_driver_detection,
                    cx,
                    SettingsView::set_detect_drivers,
                ),
            ))
            .child(section_card_with_desc(
                s,
                "Copy on Select",
                Some(
                    "PuTTY-style — copy the terminal selection to the \
                     clipboard automatically when the mouse is released. \
                     Avoids having to press \u{2318}/Ctrl+C for every snippet.",
                ),
                bool_field(
                    "settings-copy-on-select",
                    "Copy terminal selection to clipboard automatically",
                    cur.copy_on_select,
                    cx,
                    SettingsView::set_copy_on_select,
                ),
            ))
            .child(section_card_with_desc(
                s,
                "Updates",
                Some("Check GitHub on app launch for a newer Baudrun release."),
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(bool_field(
                        "settings-check-updates",
                        "Check for updates on launch",
                        !cur.disable_update_check,
                        cx,
                        SettingsView::set_check_updates,
                    ))
                    .child(bool_field(
                        "settings-prerelease-updates",
                        "Include pre-releases (alpha / beta / rc)",
                        cur.include_prerelease_updates,
                        cx,
                        SettingsView::set_include_prerelease,
                    )),
            ))
    }
}

/// Window-top header bar. Mirrors `form_header` — title at the
/// Baudrun `--font-size-h1` (24px), uppercase tag underneath, full
/// width with a subtle bottom border.
fn window_header(s: SkinTokens) -> impl IntoElement {
    div()
        .w_full()
        .px_6()
        .py_3()
        .border_b_1()
        .border_color(rgba(s.border_subtle))
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(24.0))
                .text_color(rgba(s.fg_primary))
                .child("Settings"),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(rgba(s.fg_tertiary))
                .child("GLOBAL DEFAULTS"),
        )
}

/// Left-rail tab nav. Same shape as `form_tab_nav` — translucent
/// blue active background, muted text for inactive entries, hover
/// fades to the button-bg token.
fn rail(s: SkinTokens, active: SettingsTab, cx: &mut Context<SettingsView>) -> impl IntoElement {
    let item = move |label: &'static str, tab: SettingsTab| {
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
        let hover_bg = s.bg_input;
        div()
            .w_full()
            .px_3()
            .py(px(6.0))
            .rounded_md()
            .bg(bg)
            .text_color(fg)
            .cursor_pointer()
            .hover(move |st| st.bg(rgba(hover_bg)))
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
        .border_color(rgba(s.border_subtle))
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
    s: SkinTokens,
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
        .border_1()
        .border_color(rgba(s.border_subtle))
        .rounded(px(s.radius_lg))
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

/// Build a `SelectState<Vec<Opt>>` pre-selected to whichever option
/// in `opts` has `id == selected`. Same shape as `app_view::make_select`
/// but bound to `Context<SettingsView>` (gpui's `Context<T>` is
/// invariant in `T` so the helper has to live where it's called).
fn make_select(
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

// -- Shortcut tables (mirror Tauri's `src/lib/shortcuts.ts`) ----------

/// Display order in Settings → Shortcuts. Same grouping as Tauri:
/// session control first, then transfer / window management, then
/// view actions.
const SHORTCUT_ACTIONS: &[&str] = &[
    "connect",
    "disconnect",
    "suspend",
    "resume",
    "clear",
    "break",
    "send-file",
    "new-profile",
    "open-window",
    "font-increase",
    "font-decrease",
    "font-reset",
];

fn shortcut_label(action: &'static str) -> &'static str {
    match action {
        "connect" => "Connect",
        "disconnect" => "Disconnect",
        "suspend" => "Suspend session",
        "resume" => "Resume session",
        "clear" => "Clear terminal",
        "break" => "Send Break",
        "send-file" => "Send file (X/YMODEM)",
        "new-profile" => "New profile",
        "open-window" => "Open profile in new window",
        "font-increase" => "Increase font size",
        "font-decrease" => "Decrease font size",
        "font-reset" => "Reset font size",
        _ => action,
    }
}

#[cfg(target_os = "macos")]
fn default_for_action(action: &str) -> &'static str {
    match action {
        "connect" => "Meta+Enter",
        "disconnect" => "Meta+Shift+D",
        "suspend" => "Meta+Shift+S",
        "resume" => "Meta+Shift+R",
        "clear" => "Meta+K",
        "break" => "Meta+Shift+B",
        "send-file" => "Meta+Shift+T",
        "new-profile" => "Meta+N",
        "open-window" => "Meta+Shift+Enter",
        "font-increase" => "Meta+=",
        "font-decrease" => "Meta+-",
        "font-reset" => "Meta+0",
        _ => "",
    }
}

#[cfg(not(target_os = "macos"))]
fn default_for_action(action: &str) -> &'static str {
    match action {
        "connect" => "Control+Enter",
        "disconnect" => "Control+Shift+D",
        "suspend" => "Control+Shift+S",
        "resume" => "Control+Shift+R",
        "clear" => "Control+Shift+K",
        "break" => "Control+Shift+B",
        "send-file" => "Control+Shift+T",
        "new-profile" => "Control+N",
        "open-window" => "Control+Shift+Enter",
        "font-increase" => "Control+=",
        "font-decrease" => "Control+-",
        "font-reset" => "Control+0",
        _ => "",
    }
}

/// Effective spec — user override if present + non-empty, else
/// platform default. Matches Tauri's `effectiveShortcut`: empty
/// string in the override map is treated as "unset" so the reset
/// affordance can clear without having to delete the key.
fn effective_shortcut(action: &str, overrides: &HashMap<String, String>) -> String {
    if let Some(s) = overrides.get(action) {
        if !s.trim().is_empty() {
            return s.clone();
        }
    }
    default_for_action(action).to_string()
}

// -- Spec ↔ Keystroke conversion --------------------------------------

fn any_modifier(m: &Modifiers) -> bool {
    m.control || m.alt || m.shift || m.platform || m.function
}

/// Format a captured `Keystroke` into the W3C aria-keyshortcuts
/// shape Tauri persists (`Meta+Shift+K`). Only the modifiers we
/// surface in the UI are emitted — `function` is captured but
/// dropped from the spec since the persisted format has no slot
/// for it (Fn-key bindings on macOS aren't useful as terminal
/// shortcuts anyway).
fn format_spec(keystroke: &Keystroke) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(5);
    let m = &keystroke.modifiers;
    // Order matches Tauri's parser tolerance: any order parses,
    // but we standardise on Control → Meta → Shift → Alt for
    // round-trip stability.
    if m.control {
        parts.push("Control");
    }
    if m.platform {
        parts.push("Meta");
    }
    if m.shift {
        parts.push("Shift");
    }
    if m.alt {
        parts.push("Alt");
    }
    let key_str = canonical_key_for_storage(keystroke.key.as_str());
    let mut out = parts.join("+");
    if !out.is_empty() {
        out.push('+');
    }
    out.push_str(&key_str);
    out
}

/// Map gpui's lowercase key names to the W3C key value names Tauri
/// stores. Letter / digit / punctuation keys round-trip as their
/// raw form (uppercase letters); arrow / page / function names
/// are normalized to the W3C names so the shipping app can read
/// them back.
fn canonical_key_for_storage(key: &str) -> String {
    match key {
        "up" => "ArrowUp".into(),
        "down" => "ArrowDown".into(),
        "left" => "ArrowLeft".into(),
        "right" => "ArrowRight".into(),
        "pageup" => "PageUp".into(),
        "pagedown" => "PageDown".into(),
        "enter" => "Enter".into(),
        "escape" => "Escape".into(),
        "tab" => "Tab".into(),
        "space" => " ".into(),
        "backspace" => "Backspace".into(),
        "delete" => "Delete".into(),
        "home" => "Home".into(),
        "end" => "End".into(),
        // Single ASCII letter → uppercase. Digits / punctuation
        // / multi-char already-named keys (F1, etc.) pass through
        // unchanged.
        other if other.len() == 1 && other.chars().next().unwrap().is_ascii_alphabetic() => {
            other.to_ascii_uppercase()
        }
        other => other.to_string(),
    }
}

/// Reverse of `format_spec` — parse a stored Tauri spec back into
/// a gpui `Keystroke` so the `Kbd` widget can render it. Returns
/// `None` for malformed specs (no key, only modifiers, …) so the
/// caller can fall back to a placeholder.
fn parse_spec(spec: &str) -> Option<Keystroke> {
    if spec.is_empty() {
        return None;
    }
    let mut modifiers = Modifiers::default();
    let mut key: Option<String> = None;
    for raw in spec.split('+') {
        let tok = raw.trim();
        if tok.is_empty() {
            continue;
        }
        match tok.to_ascii_lowercase().as_str() {
            "control" | "ctrl" => modifiers.control = true,
            "meta" | "cmd" | "command" | "super" | "win" => modifiers.platform = true,
            "shift" => modifiers.shift = true,
            "alt" | "option" => modifiers.alt = true,
            // Last non-modifier token wins, like Tauri's parser.
            _ => key = Some(canonical_key_for_display(tok)),
        }
    }
    let key = key?;
    Some(Keystroke {
        modifiers,
        key,
        key_char: None,
    })
}

/// W3C → gpui key name. Inverse of `canonical_key_for_storage`.
/// Unknown keys lowercase by default (matches gpui's convention
/// for letters / punctuation).
fn canonical_key_for_display(key: &str) -> String {
    match key {
        "ArrowUp" => "up".into(),
        "ArrowDown" => "down".into(),
        "ArrowLeft" => "left".into(),
        "ArrowRight" => "right".into(),
        "PageUp" => "pageup".into(),
        "PageDown" => "pagedown".into(),
        "Enter" => "enter".into(),
        "Escape" => "escape".into(),
        "Tab" => "tab".into(),
        " " | "Space" => "space".into(),
        "Backspace" => "backspace".into(),
        "Delete" => "delete".into(),
        "Home" => "home".into(),
        "End" => "end".into(),
        other if other.len() == 1 && other.chars().next().unwrap().is_ascii_alphabetic() => {
            other.to_ascii_lowercase()
        }
        other => other.to_string(),
    }
}
