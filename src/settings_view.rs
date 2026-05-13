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
    div, prelude::*, px, rgba, AppContext, Context, DismissEvent, ElementId, Entity,
    FocusHandle, IntoElement, KeyDownEvent, Keystroke, Modifiers, MouseButton,
    MouseUpEvent, PathPromptOptions, Render, SharedString, Subscription, Window,
};
use gpui_component::{
    button::Button,
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState},
    kbd::Kbd,
    notification::Notification,
    select::{Select, SelectEvent, SelectItem, SelectState},
    tooltip::Tooltip,
    IndexPath, Root, Sizable, TitleBar, WindowExt,
};

use crate::data::appdata;
use crate::data::highlight;
use crate::data::settings::Settings;
use crate::data::skins;
use crate::data::themes;
use crate::settings_bus::{SettingsBus, SettingsEvent};
use crate::skin_tokens::{self, SkinTokens};

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
    Accessibility,
    Updates,
    Advanced,
}

impl SettingsTab {
    const ALL: [SettingsTab; 7] = [
        SettingsTab::Appearance,
        SettingsTab::Themes,
        SettingsTab::Shortcuts,
        SettingsTab::Highlighting,
        SettingsTab::Accessibility,
        SettingsTab::Updates,
        SettingsTab::Advanced,
    ];

    fn label(self) -> &'static str {
        match self {
            SettingsTab::Appearance => "Appearance",
            SettingsTab::Themes => "Themes",
            SettingsTab::Shortcuts => "Shortcuts",
            SettingsTab::Highlighting => "Highlighting",
            SettingsTab::Accessibility => "Accessibility",
            SettingsTab::Updates => "Updates",
            SettingsTab::Advanced => "Advanced",
        }
    }
}

/// Local Select-item shape. Mirrors the same struct in
/// `app_view.rs` (kept duplicated rather than shared because the
/// SelectItem trait impl is the entire payload — moving it to a
/// shared module would just shuffle 30 lines around). Carries
/// an id plus a display title; `Value` is the id so selection
/// events return a matchable string.
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
    /// commit() now goes `bus.replace(next)` so persistence and
    /// the cross-window emit happen in one place. Read via
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
    scrollback: Entity<InputState>,
    _scrollback_sub: Subscription,

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

    // -- Window-header filter state --
    filter_input: Entity<InputState>,
    _filter_sub: Subscription,
    /// Lowercased mirror of `filter_input`'s text. Empty means "no
    /// filter active"; non-empty drives the per-section fade + the
    /// dim-tabs-without-matches behaviour in the left rail.
    filter_text: String,
    /// Cached resolved support-dir path. Read once at construction
    /// and refreshed after Choose / Reset so the read-only display
    /// reflects what the next launch will use without re-touching
    /// the filesystem on every render.
    config_dir_display: SharedString,
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
                // Commit on Blur (focus loss) AND on Enter so the
                // user can lock in a value without tabbing away.
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
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

        let scrollback_initial = if current.scrollback_lines > 0 {
            current.scrollback_lines.to_string()
        } else {
            String::new()
        };
        let scrollback = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("10000")
                .default_value(scrollback_initial.as_str())
        });
        let scrollback_sub =
            cx.subscribe(&scrollback, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    let raw = input.read(cx).value().to_string();
                    let parsed = raw.trim();
                    let next_value = if parsed.is_empty() {
                        0
                    } else {
                        match parsed.parse::<i32>() {
                            // Sanity cap: 1M lines is already
                            // multi-gigabyte territory and anything
                            // higher is a typo. Negative gets
                            // rejected via the `n > 0` arm.
                            Ok(n) if (1..=1_000_000).contains(&n) => n,
                            _ => {
                                log::warn!("scrollback: invalid value {raw:?}");
                                return;
                            }
                        }
                    };
                    if next_value != this.settings.scrollback_lines {
                        let mut next = this.settings.clone();
                        next.scrollback_lines = next_value;
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
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    let value = input.read(cx).value().to_string();
                    if value != this.settings.log_dir {
                        let mut next = this.settings.clone();
                        next.log_dir = value;
                        this.commit(next, cx);
                    }
                }
            });

        // Filter input — lives in the window header, drives the
        // per-section fade and the tab-rail dimming. Subscribed on
        // `Change` (every keystroke) so the dim animates live; no
        // need to wait for Blur the way the saved-setting inputs do.
        let filter_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Filter settings\u{2026}")
        });
        let filter_sub =
            cx.subscribe(&filter_input, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = input.read(cx).value().to_string();
                    let next = value.trim().to_lowercase();
                    if next != this.filter_text {
                        this.filter_text = next;
                        cx.notify();
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
            scrollback,
            _scrollback_sub: scrollback_sub,
            _font_size_sub: font_size_sub,
            theme_select,
            _theme_sub: theme_sub,
            capturing_shortcut: None,
            capture_focus: cx.focus_handle(),
            log_dir,
            _log_dir_sub: log_dir_sub,
            config_dir_display: resolve_config_dir_display(),
            filter_input,
            _filter_sub: filter_sub,
            filter_text: String::new(),
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

    /// Opacity multiplier for a section card given the current
    /// filter text. Empty filter → 1.0 (full visibility). Non-empty
    /// → 1.0 when the section's title or keyword list contains the
    /// filter substring (case-insensitive), 0.30 otherwise. The
    /// dimmed section stays interactive; the user can still tweak
    /// it via keyboard / mouse without clearing the filter first.
    fn filter_opacity(&self, title: &str) -> f32 {
        if self.section_matches_filter(title) { 1.0 } else { 0.18 }
    }

    /// Wrap a section card with the filter-aware opacity. Every
    /// pane builder calls this in place of the bare
    /// `section_card_with_desc` so the filter dim applies
    /// consistently. Non-matching sections still paint full-size
    /// and stay clickable — they just fade.
    fn filtered_card(
        &self,
        s: SkinTokens,
        title: &'static str,
        description: Option<&'static str>,
        body: impl IntoElement,
    ) -> gpui::Div {
        section_card_with_desc(s, title, description, body)
            .opacity(self.filter_opacity(title))
    }

    fn section_matches_filter(&self, title: &str) -> bool {
        if self.filter_text.is_empty() {
            return true;
        }
        if title.to_lowercase().contains(&self.filter_text) {
            return true;
        }
        SECTION_KEYWORDS
            .iter()
            .find(|(t, _)| *t == title)
            .map(|(_, kws)| kws.to_lowercase().contains(&self.filter_text))
            .unwrap_or(false)
    }

    /// True when the given tab has at least one matching section
    /// under the current filter. Drives the tab-rail dim — tabs
    /// with no hits read as "nothing here to find."
    fn tab_has_filter_matches(&self, tab: SettingsTab) -> bool {
        if self.filter_text.is_empty() {
            return true;
        }
        TAB_SECTIONS
            .iter()
            .find(|(t, _)| *t == tab)
            .map(|(_, titles)| {
                titles.iter().any(|title| self.section_matches_filter(title))
            })
            .unwrap_or(true)
    }

    fn set_include_prerelease(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let mut next = self.settings.clone();
        next.include_prerelease_updates = enabled;
        self.commit(next, cx);
    }

    /// Persist a dismissal of the currently-advertised update so
    /// the amber-indicator chain stops painting until a NEWER
    /// release shows up. Stored on `Settings.dismissed_update_version`
    /// so the next launch's check can compare against it before
    /// surfacing anything.
    fn dismiss_update(&mut self, version: String, cx: &mut Context<Self>) {
        let mut next = self.settings.clone();
        next.dismissed_update_version = Some(version);
        self.commit(next, cx);
    }

    /// Reveal button on the Config Directory card. Opens the
    /// current support dir IN PLACE (showing its contents) in the
    /// platform file manager. `cx.reveal_path` on macOS opens the
    /// PARENT directory and selects the target, which leaves the
    /// user staring at `~/Library/Application Support/` with
    /// Baudrun highlighted — not what we want here. `open_url`
    /// with a `file://` URL hands off to the OS handler which
    /// opens the directory's contents instead.
    fn reveal_config_dir(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        match appdata::support_dir() {
            Ok(path) => {
                let encoded = encode_file_path(&path);
                cx.open_url(&format!("file://{encoded}"));
            }
            Err(err) => {
                log::error!("reveal config dir: {err}");
                cx.spawn(async move |this, cx| {
                    let _ = this.update_in(cx, |_, window, view_cx| {
                        window.push_notification(
                            Notification::error(SharedString::from(format!(
                                "Couldn't open config directory: {err}"
                            ))),
                            view_cx,
                        );
                    });
                })
                .detach();
            }
        }
    }

    /// Choose… button on the Config Directory card. Opens the OS
    /// folder picker; on result writes the override file so the
    /// next launch resolves to the new directory, then refreshes
    /// the read-only display. Surfaces a "restart to use" toast —
    /// re-binding every live Store at runtime is more surgery than
    /// this slice covers.
    fn choose_config_dir(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose Baudrun config directory".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            let _ = this.update_in(cx, |this, window, view_cx| {
                if let Err(err) = appdata::write_override(Some(&path)) {
                    log::error!("write config dir override: {err}");
                    window.push_notification(
                        Notification::error(SharedString::from(format!(
                            "Couldn't set config directory: {err}"
                        ))),
                        view_cx,
                    );
                    return;
                }
                this.config_dir_display = resolve_config_dir_display();
                view_cx.notify();
                window.push_notification(
                    Notification::success(SharedString::from(
                        "Config directory set. Restart Baudrun to use it.",
                    )),
                    view_cx,
                );
            });
        })
        .detach();
    }

    /// Reset the Config Directory override back to the platform
    /// default. Same restart caveat as Choose…
    fn reset_config_dir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(err) = appdata::write_override(None) {
            log::error!("clear config dir override: {err}");
            window.push_notification(
                Notification::error(SharedString::from(format!(
                    "Couldn't reset config directory: {err}"
                ))),
                cx,
            );
            return;
        }
        self.config_dir_display = resolve_config_dir_display();
        cx.notify();
        window.push_notification(
            Notification::success(SharedString::from(
                "Config directory reset. Restart Baudrun to use the default.",
            )),
            cx,
        );
    }

    /// Choose… button on Session Log Directory. Opens the OS folder
    /// picker; on result we mirror the path back into the Input
    /// (visible feedback) AND commit it to the persisted Settings.
    fn choose_log_dir(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose session log directory".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            let path_str = path.display().to_string();
            let _ = this.update_in(cx, |this, window, view_cx| {
                this.log_dir
                    .update(view_cx, |state, view_cx| {
                        state.set_value(path_str.clone(), window, view_cx);
                    });
                if this.settings.log_dir != path_str {
                    let mut next = this.settings.clone();
                    next.log_dir = path_str;
                    this.commit(next, view_cx);
                }
            });
        })
        .detach();
    }

    /// Reset button on Session Log Directory. Empties the field and
    /// commits an empty value — the Phase-1 logger reads that as
    /// "use the default location" (the support dir's `logs/` subdir).
    fn reset_log_dir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.log_dir.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        if !self.settings.log_dir.is_empty() {
            let mut next = self.settings.clone();
            next.log_dir = String::new();
            self.commit(next, cx);
        }
    }

    /// Reset the Settings window's geometry — clears the saved
    /// `settings_window` from settings.json AND resizes the live
    /// Settings window (the one this button lives in) back to its
    /// default centered size. Live resize so the user sees the
    /// effect immediately rather than having to close + reopen.
    fn reset_settings_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings.settings_window.is_some() {
            let mut next = self.settings.clone();
            next.settings_window = None;
            self.commit(next, cx);
        }
        // Default size matches `AppView::open_settings`. Resize is
        // size-only, not position — gpui doesn't expose
        // `set_window_bounds` directly on Window, and macOS's
        // resize-without-recenter is the more familiar UX for a
        // settings sheet ("shrink it back to default size without
        // moving it") anyway.
        window.resize(gpui::size(px(560.0), px(520.0)));
    }

    /// Reset the main window's geometry — clears saved
    /// `main_window` from settings.json AND resizes every open
    /// main (AppView-rooted) window to the default centered size.
    /// Iterates `cx.windows()` instead of taking a single handle
    /// so that, when the user has multiple terminal windows open,
    /// the click affects them all in one shot.
    fn reset_main_window(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.main_window.is_some() {
            let mut next = self.settings.clone();
            next.main_window = None;
            self.commit(next, cx);
        }
        // 1100×720 mirrors `app_view::open_app_window`'s centered
        // fallback. Each window's own `on_window_should_close`
        // re-saves on close, so the cleared state only stays clear
        // until the next close — that's the same trade-off as the
        // Settings-window button above.
        for handle in cx.windows() {
            let _ = handle.update(cx, |_root, window, cx| {
                let Some(Some(root)) = window.root::<gpui_component::Root>() else {
                    return;
                };
                // Resize only AppView-rooted windows; the Settings
                // window also has a `Root` root, but its inner view
                // is SettingsView, not AppView, and we don't want
                // the main-window button to also resize the
                // Settings window the user is clicking from.
                if root
                    .read(cx)
                    .view()
                    .clone()
                    .downcast::<crate::app_view::AppView>()
                    .is_err()
                {
                    return;
                }
                window.resize(gpui::size(px(1100.0), px(720.0)));
            });
        }
    }

    fn set_restore_window_state(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let mut next = self.settings.clone();
        // Stored inverted — same shape as drivers / update check —
        // so the default "on" state serialises as the absence of
        // the field.
        next.disable_window_state_restore = !enabled;
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

    // -- Import handlers ---------------------------------------------
    //
    // Three near-identical flows. Each spawns the platform file
    // picker, awaits the user's choice, calls the relevant store's
    // import method, and then re-enters the window context to
    // rebuild the affected Select state(s) in place — picking up
    // the new entry without forcing a Settings-window reopen.
    //
    // The window-handle dance is needed because file dialogs are
    // async and Context<SettingsView> doesn't carry a Window
    // reference — we capture the handle in the click listener (where
    // `&mut Window` is in scope) and replay it through the AsyncApp
    // when the receiver fires.

    fn start_skin_import(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import skin JSON".into()),
        });
        let store = self.skins_store.clone();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            match store.import(&path) {
                Ok(skin) => {
                    log::info!("imported skin: {}", skin.name);
                    let name = skin.name.clone();
                    let _ = this.update_in(cx, |this, window, view_cx| {
                        this.rebuild_skin_select(window, view_cx);
                        window.push_notification(
                            Notification::success(SharedString::from(format!(
                                "Imported skin \u{201C}{name}\u{201D}"
                            ))),
                            view_cx,
                        );
                    });
                }
                Err(err) => {
                    log::error!("import skin: {err}");
                    let msg = format!("Couldn't import skin: {err}");
                    let _ = this.update_in(cx, |_, window, view_cx| {
                        window.push_notification(
                            Notification::error(SharedString::from(msg)),
                            view_cx,
                        );
                    });
                }
            }
        })
        .detach();
    }

    fn start_theme_import(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import theme (.itermcolors or JSON)".into()),
        });
        let store = self.themes_store.clone();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            match store.import(&path) {
                Ok(theme) => {
                    log::info!("imported theme: {}", theme.name);
                    let name = theme.name.clone();
                    let _ = this.update_in(cx, |this, window, view_cx| {
                        this.rebuild_theme_select(window, view_cx);
                        window.push_notification(
                            Notification::success(SharedString::from(format!(
                                "Imported theme \u{201C}{name}\u{201D}"
                            ))),
                            view_cx,
                        );
                    });
                }
                Err(err) => {
                    log::error!("import theme: {err}");
                    let msg = format!("Couldn't import theme: {err}");
                    let _ = this.update_in(cx, |_, window, view_cx| {
                        window.push_notification(
                            Notification::error(SharedString::from(msg)),
                            view_cx,
                        );
                    });
                }
            }
        })
        .detach();
    }

    /// Per-row delete on the Installed Skins list. Refuses non-
    /// custom skins. If the skin being deleted is the currently-
    /// selected one, resets `skin_id` to the built-in default so
    /// the live UI doesn't keep pointing at a dead id.
    fn delete_named_skin(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(skin) = self.skins_store.get(&id) else { return };
        if skin.source != "user" {
            return;
        }
        let snapshot = skin.clone();
        let name = skin.name.clone();
        if let Err(err) = self.skins_store.delete(&id) {
            log::error!("delete skin {id}: {err}");
            window.push_notification(
                Notification::error(SharedString::from(format!(
                    "Couldn't delete skin: {err}"
                ))),
                cx,
            );
            return;
        }
        let was_active = self.settings.skin_id == id;
        if was_active {
            let mut next = self.settings.clone();
            next.skin_id = skins::DEFAULT_SKIN_ID.to_string();
            self.commit(next, cx);
        }
        self.rebuild_skin_select(window, cx);

        // Undo toast — captures the snapshot and the entity so the
        // user can restore the skin within the notification's
        // autohide window (~5s).
        let app = cx.entity();
        let prev_active_id = if was_active {
            Some(snapshot.id.clone())
        } else {
            None
        };
        window.push_notification(
            Notification::success(SharedString::from(format!(
                "Removed skin \u{201C}{name}\u{201D}"
            )))
            .action(move |_, _, ctx| {
                let app = app.clone();
                let snapshot = snapshot.clone();
                let prev_active_id = prev_active_id.clone();
                let notif = ctx.entity();
                Button::new("undo-skin-delete")
                    .label("Undo")
                    .on_click(move |_, window, cx| {
                        let app = app.clone();
                        let snapshot = snapshot.clone();
                        let prev_active_id = prev_active_id.clone();
                        let notif = notif.clone();
                        app.update(cx, |this, view_cx| {
                            this.restore_skin(snapshot, prev_active_id, window, view_cx);
                        });
                        dismiss_notification_after(notif, cx);
                    })
            })
            // `.action(...)` hardcodes `autohide = false` so the
            // user has time to read "Undo"; the trade-off is the
            // toast sits forever until they hover-and-X out of it
            // (the close button is `.invisible()` + group-hover
            // reveal — easy to miss on a non-touchscreen). Override
            // back to true so the toast self-dismisses after gpui-
            // component's default 5s timeout. The Undo click handler
            // still fires fine if the user catches the toast before
            // it fades.
            .autohide(true),
            cx,
        );
    }

    fn restore_skin(
        &mut self,
        skin: skins::Skin,
        prev_active_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = skin.name.clone();
        let id = skin.id.clone();
        if let Err(err) = self.skins_store.restore(skin) {
            log::error!("restore skin {id}: {err}");
            window.push_notification(
                Notification::error(SharedString::from(format!(
                    "Couldn't restore skin: {err}"
                ))),
                cx,
            );
            return;
        }
        if let Some(prev) = prev_active_id {
            let mut next = self.settings.clone();
            next.skin_id = prev;
            self.commit(next, cx);
        }
        self.rebuild_skin_select(window, cx);
        window.push_notification(
            Notification::success(SharedString::from(format!(
                "Restored skin \u{201C}{name}\u{201D}"
            ))),
            cx,
        );
    }

    /// Per-row delete on the Themes tab. Refuses non-custom themes.
    /// If the theme being deleted is the currently-selected default,
    /// also resets `default_theme_id` to the built-in baudrun palette
    /// so the live terminal doesn't end up pointing at a dead id.
    fn delete_named_theme(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(theme) = self.themes_store.get(&id) else { return };
        if theme.source != "user" {
            return;
        }
        let snapshot = theme.clone();
        let name = theme.name.clone();
        if let Err(err) = self.themes_store.delete(&id) {
            log::error!("delete theme {id}: {err}");
            window.push_notification(
                Notification::error(SharedString::from(format!(
                    "Couldn't delete theme: {err}"
                ))),
                cx,
            );
            return;
        }
        let was_active = self.settings.default_theme_id == id;
        if was_active {
            let mut next = self.settings.clone();
            next.default_theme_id = themes::DEFAULT_THEME_ID.to_string();
            self.commit(next, cx);
        }
        self.rebuild_theme_select(window, cx);
        let app = cx.entity();
        let prev_active_id = if was_active {
            Some(snapshot.id.clone())
        } else {
            None
        };
        window.push_notification(
            Notification::success(SharedString::from(format!(
                "Removed theme \u{201C}{name}\u{201D}"
            )))
            .action(move |_, _, ctx| {
                let app = app.clone();
                let snapshot = snapshot.clone();
                let prev_active_id = prev_active_id.clone();
                let notif = ctx.entity();
                Button::new("undo-theme-delete")
                    .label("Undo")
                    .on_click(move |_, window, cx| {
                        let app = app.clone();
                        let snapshot = snapshot.clone();
                        let prev_active_id = prev_active_id.clone();
                        let notif = notif.clone();
                        app.update(cx, |this, view_cx| {
                            this.restore_theme(snapshot, prev_active_id, window, view_cx);
                        });
                        dismiss_notification_after(notif, cx);
                    })
            })
            // See the matching comment on the skin-delete toast.
            .autohide(true),
            cx,
        );
    }

    fn restore_theme(
        &mut self,
        theme: themes::Theme,
        prev_active_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = theme.name.clone();
        let id = theme.id.clone();
        if let Err(err) = self.themes_store.restore(theme) {
            log::error!("restore theme {id}: {err}");
            window.push_notification(
                Notification::error(SharedString::from(format!(
                    "Couldn't restore theme: {err}"
                ))),
                cx,
            );
            return;
        }
        if let Some(prev) = prev_active_id {
            let mut next = self.settings.clone();
            next.default_theme_id = prev;
            self.commit(next, cx);
        }
        self.rebuild_theme_select(window, cx);
        window.push_notification(
            Notification::success(SharedString::from(format!(
                "Restored theme \u{201C}{name}\u{201D}"
            ))),
            cx,
        );
    }

    /// Open a modal showing the theme's bg/fg painted with a sample
    /// of styled terminal output — lets the user evaluate a palette
    /// without committing it as the default.
    fn open_theme_preview(
        &mut self,
        theme_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(theme) = self.themes_store.get(&theme_id) else { return };
        let title = SharedString::from(theme.name.clone());
        let subtitle = SharedString::from(if theme.source == "user" {
            "Imported theme \u{00B7} sample network-gear output"
        } else {
            "Built-in theme \u{00B7} sample network-gear output"
        });
        window.open_dialog(cx, move |dlg, _, _| {
            // `close_button(true)` adds the `×` glyph in the title
            // bar (close affordance for users who don't think to
            // press Esc); `keyboard(true)` is the AlertDialog
            // default but on the plain Dialog path it's off by
            // default — set explicitly so Esc dismisses too.
            // `overlay_closable(true)` also lets a click outside
            // the dialog dismiss it, same as native macOS
            // settings-style sheets.
            dlg.title(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_size(px(16.0)).child(title.clone()))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(0x808080AAu32))
                            .child(subtitle.clone()),
                    ),
            )
            .close_button(true)
            .keyboard(true)
            .overlay_closable(true)
            .w(px(720.0))
            .child(theme_preview_block(&theme))
        });
    }

    /// Per-row delete on the Highlighting tab. Only invoked from rows
    /// rendered for `source == "user"` packs (the trash icon is hidden
    /// on bundled rows). Also strips the id from the enabled list so
    /// the live engine doesn't keep a phantom reference around.
    fn delete_highlight_pack(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pack) = self.highlight_store.list().into_iter().find(|p| p.id == id)
        else {
            return;
        };
        if pack.source != "user" && pack.source != "import" {
            return;
        }
        let snapshot = pack.clone();
        let name = pack.name.clone();
        if let Err(err) = self.highlight_store.delete_user_pack(&id) {
            log::error!("delete highlight pack {id}: {err}");
            window.push_notification(
                Notification::error(SharedString::from(format!(
                    "Couldn't delete pack: {err}"
                ))),
                cx,
            );
            return;
        }
        let was_enabled = self.enabled_packs().iter().any(|p| p == &id);
        if was_enabled {
            let mut packs = self.enabled_packs();
            packs.retain(|p| p != &id);
            let mut next = self.settings.clone();
            next.enabled_highlight_presets = Some(packs);
            self.commit(next, cx);
        } else {
            cx.notify();
        }
        let app = cx.entity();
        window.push_notification(
            Notification::success(SharedString::from(format!(
                "Removed pack \u{201C}{name}\u{201D}"
            )))
            .action(move |_, _, ctx| {
                let app = app.clone();
                let snapshot = snapshot.clone();
                let notif = ctx.entity();
                Button::new("undo-pack-delete")
                    .label("Undo")
                    .on_click(move |_, window, cx| {
                        let app = app.clone();
                        let snapshot = snapshot.clone();
                        let notif = notif.clone();
                        app.update(cx, |this, view_cx| {
                            this.restore_highlight_pack(
                                snapshot,
                                was_enabled,
                                window,
                                view_cx,
                            );
                        });
                        dismiss_notification_after(notif, cx);
                    })
            })
            // See the matching comment on the skin-delete toast.
            .autohide(true),
            cx,
        );
    }

    fn restore_highlight_pack(
        &mut self,
        pack: highlight::HighlightPack,
        re_enable: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = pack.name.clone();
        let id = pack.id.clone();
        if let Err(err) = self.highlight_store.restore_user_pack(&pack) {
            log::error!("restore highlight pack {id}: {err}");
            window.push_notification(
                Notification::error(SharedString::from(format!(
                    "Couldn't restore pack: {err}"
                ))),
                cx,
            );
            return;
        }
        if re_enable {
            let mut packs = self.enabled_packs();
            if !packs.iter().any(|p| p == &id) {
                packs.push(id);
                let mut next = self.settings.clone();
                next.enabled_highlight_presets = Some(packs);
                self.commit(next, cx);
            }
        } else {
            cx.notify();
        }
        window.push_notification(
            Notification::success(SharedString::from(format!(
                "Restored pack \u{201C}{name}\u{201D}"
            ))),
            cx,
        );
    }

    fn start_highlight_import(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import highlight pack JSON".into()),
        });
        let store = self.highlight_store.clone();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            match store.import_user_pack(&path) {
                Ok(pack) => {
                    log::info!("imported highlight pack: {}", pack.name);
                    // Highlighting pane reads the store directly on
                    // each render, so a notify is enough to refresh
                    // the toggle list — no SelectState to rebuild.
                    let name = pack.name.clone();
                    let _ = this.update_in(cx, |_, window, view_cx| {
                        view_cx.notify();
                        window.push_notification(
                            Notification::success(SharedString::from(format!(
                                "Imported pack \u{201C}{name}\u{201D}"
                            ))),
                            view_cx,
                        );
                    });
                }
                Err(err) => {
                    log::error!("import highlight: {err}");
                    let msg = format!("Couldn't import pack: {err}");
                    let _ = this.update_in(cx, |_, window, view_cx| {
                        window.push_notification(
                            Notification::error(SharedString::from(msg)),
                            view_cx,
                        );
                    });
                }
            }
        })
        .detach();
    }

    /// Replace `skin_select` with a freshly-built SelectState that
    /// includes the just-imported skin in its option list. The
    /// active selection (the `current.skin_id` the original was
    /// seeded with) is preserved.
    fn rebuild_skin_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.settings.clone();
        let opts = build_skin_opts(&self.skins_store);
        let active = if current.skin_id.is_empty() {
            skins::DEFAULT_SKIN_ID
        } else {
            current.skin_id.as_str()
        };
        let new_state = make_select(opts, active, window, cx);
        let new_sub = cx.subscribe(
            &new_state,
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
        self.skin_select = new_state;
        self._skin_sub = new_sub;
        cx.notify();
    }

    /// Same as `rebuild_skin_select` for the Themes tab.
    fn rebuild_theme_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.settings.clone();
        let opts = build_theme_opts(&self.themes_store);
        let active = if current.default_theme_id.is_empty() {
            themes::DEFAULT_THEME_ID
        } else {
            current.default_theme_id.as_str()
        };
        let new_state = make_select(opts, active, window, cx);
        let new_sub = cx.subscribe(
            &new_state,
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
        self.theme_select = new_state;
        self._theme_sub = new_sub;
        cx.notify();
    }
}

/// Pull skin options out of the store + tag user-imported entries
/// with "(custom)". Used at SettingsView::new and on rebuild after
/// import.
fn build_skin_opts(store: &skins::Store) -> Vec<Opt> {
    store
        .list()
        .into_iter()
        .map(|s| {
            let title = if s.source == "user" {
                format!("{} (custom)", s.name)
            } else {
                s.name
            };
            Opt::new(&s.id, &title)
        })
        .collect()
}

fn build_theme_opts(store: &themes::Store) -> Vec<Opt> {
    store
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
        .collect()
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
            // `window_background()` picks up the skin's
            // `--shell-bg` linear gradient when present.
            .bg(s.window_background())
            .text_color(rgba(s.fg_primary))
            .text_size(px(13.0))
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    // Title bar — flush-edged skins get gpui-
                    // component's default with its theme bg +
                    // bottom border, height from
                    // `--titlebar-height`. Floating-card skins
                    // override to transparent + no border so
                    // the shell colour reads continuously from
                    // the top edge. See the matching branch in
                    // `app_view.rs`.
                    .child(if s.panel_radius_px > 0.0 {
                        TitleBar::new()
                            .bg(gpui::transparent_black())
                            .border_color(gpui::transparent_black())
                    } else {
                        TitleBar::new().h(px(s.titlebar_height_px))
                    })
                    .child(window_header(
                        s,
                        &self.filter_input,
                        !self.filter_text.is_empty(),
                        cx,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .flex_row()
                            .child({
                                let matches: Vec<(SettingsTab, bool)> = SettingsTab::ALL
                                    .iter()
                                    .map(|&t| (t, self.tab_has_filter_matches(t)))
                                    .collect();
                                // Amber-dot trigger for the
                                // Updates rail item.
                                // `update_pending` mirrors what
                                // `updates_pane` uses to gate
                                // its status-card amber dot:
                                // there's an available release
                                // AND the user hasn't dismissed
                                // it for this exact version.
                                let update_pending = cx
                                    .global::<crate::updater::UpdateState>()
                                    .available
                                    .as_ref()
                                    .map(|a| {
                                        self.settings
                                            .dismissed_update_version
                                            .as_deref()
                                            != Some(a.version.as_str())
                                    })
                                    .unwrap_or(false);
                                rail(s, self.tab, &matches, update_pending, cx)
                            })
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
            SettingsTab::Appearance => self.appearance_pane(s, cx).into_any_element(),
            SettingsTab::Themes => self.themes_pane(s, cx).into_any_element(),
            SettingsTab::Shortcuts => self.shortcuts_pane(s, cx).into_any_element(),
            SettingsTab::Highlighting => self.highlighting_pane(s, cx).into_any_element(),
            SettingsTab::Accessibility => {
                self.accessibility_pane(s, cx).into_any_element()
            }
            SettingsTab::Updates => self.updates_pane(s, cx).into_any_element(),
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
            .child(self.filtered_card(
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

    fn themes_pane(&self, s: SkinTokens, cx: &mut Context<Self>) -> impl IntoElement {
        // -- Card 1: Default Theme picker --
        let default_card = self.filtered_card(
            s,
            "Default Theme",
            Some("Used by any profile that doesn't set its own theme."),
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_size(px(11.0)).text_color(rgba(s.fg_secondary)).child("Theme"))
                .child(Select::new(&self.theme_select)),
        );

        // -- Card 2: Installed Themes list --
        let themes_list = self.themes_store.list();
        let rows: Vec<gpui::Div> = themes_list
            .into_iter()
            .map(|theme| self.theme_row(&theme, s, cx))
            .collect();

        let installed_card = div()
            .w_full()
            .bg(rgba(s.bg_panel))
            .border_1()
            .border_color(rgba(s.border_subtle))
            .rounded(px(s.radius_lg))
            // Match the section_card_with_desc shadow so this
            // hand-rolled card sits at the same depth as the rest.
            .shadow_sm()
            .px_4()
            .py_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                // Title row with Import button on the right — matches
                // the Tauri layout where "Installed Themes" and the
                // import action share one line.
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(15.0))
                            .text_color(rgba(s.fg_primary))
                            .child("Installed Themes"),
                    )
                    .child(import_button(
                        s,
                        "Import .itermcolors\u{2026}",
                        cx,
                        |this, window, cx| this.start_theme_import(window, cx),
                    )),
            )
            .child(div().flex().flex_col().gap_2().children(rows));
        let installed_card = installed_card.opacity(self.filter_opacity("Installed Themes"));

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(default_card)
            .child(installed_card)
    }

    /// One row inside the Installed Themes card. Mirrors Tauri's layout:
    /// swatch strip on the left, name + source-tag in the middle, and
    /// (for user imports) a trash button followed by a Preview button
    /// on the right. The Preview button opens the modal regardless of
    /// whether the theme is currently set as default.
    fn theme_row(
        &self,
        theme: &themes::Theme,
        s: SkinTokens,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let theme_id = theme.id.clone();
        let theme_id_for_delete = theme.id.clone();
        let is_custom = theme.source == "user";
        let tag = if is_custom { "Custom" } else { "Built-in" };

        let mut right = div().flex().flex_row().items_center().gap_2();
        if is_custom {
            let trash_id = ElementId::Name(SharedString::from(format!(
                "trash-theme-{}",
                theme.id
            )));
            right = right.child(trash_button(
                s,
                trash_id,
                "Delete imported theme",
                cx,
                move |this, window, cx| {
                    this.delete_named_theme(theme_id_for_delete.clone(), window, cx);
                },
            ));
        }
        right = right.child(import_button(
            s,
            // The shape of import_button (pill, accent hover) is also
            // what we want for Preview — reuse rather than duplicate
            // a near-identical helper.
            "Preview",
            cx,
            move |this, window, cx| this.open_theme_preview(theme_id.clone(), window, cx),
        ));

        div()
            .w_full()
            .bg(rgba(s.bg_input))
            .border_1()
            .border_color(rgba(s.border_subtle))
            .rounded(px(s.radius_md))
            .px_3()
            .py_2()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .child(theme_swatches(theme))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgba(s.fg_primary))
                            .child(SharedString::from(theme.name.clone())),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgba(s.fg_secondary))
                            .child(tag),
                    ),
            )
            .child(right)
    }

    fn highlighting_pane(&self, s: SkinTokens, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = self.enabled_packs();
        let packs = self.highlight_store.list();
        // Collect eagerly so cx is no longer captured by the
        // lazy iterator by the time we hand it to import_button
        // below — a `move` closure on the iterator would pin the
        // mutable borrow of cx across the import_button call and
        // the borrow checker rejects it.
        let rows: Vec<gpui::Div> = packs.into_iter().map(|p| {
            let id_for_label = p.id.clone();
            let id_for_setter = p.id.clone();
            let id_for_delete = p.id.clone();
            let is_on = enabled.iter().any(|e| e == &p.id);
            let is_custom = p.source == "user" || p.source == "import";
            // Append "(custom)" to user/imported packs so they're
            // distinguishable from the built-in vendor presets.
            let label = if is_custom {
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
            let mut top = div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    div().flex_1().child(
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
                    ),
                );
            if is_custom {
                let trash_id = ElementId::Name(SharedString::from(format!(
                    "trash-pack-{}",
                    id_for_delete
                )));
                top = top.child(trash_button(
                    s,
                    trash_id,
                    "Delete imported pack",
                    cx,
                    move |this, window, cx| {
                        this.delete_highlight_pack(id_for_delete.clone(), window, cx);
                    },
                ));
            }

            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(top)
                .child(
                    div()
                        .pl(px(24.0))
                        .text_size(px(12.0))
                        .text_color(rgba(s.fg_secondary))
                        .whitespace_normal()
                        .child(desc),
                )
        }).collect();

        let body = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(import_button(
                s,
                "Import pack\u{2026}",
                cx,
                |this, window, cx| this.start_highlight_import(window, cx),
            ))
            .child(div().flex().flex_col().gap_3().children(rows));
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.filtered_card(
                s,
                "Highlight Packs",
                Some(
                    "Choose which built-in or imported rule packs colorize \
                     terminal output. Stack as many as you want — matches \
                     from each pack are merged in order. With every box \
                     unchecked highlighting is off, even if a profile has \
                     it enabled.",
                ),
                body,
            ))
    }

    fn appearance_pane(&self, s: SkinTokens, cx: &mut Context<Self>) -> impl IntoElement {
        // -- App Skin: just the picker now. Import + per-skin
        // delete moved into the Installed Skins card below to
        // match the Tauri layout.
        let app_skin_card = self.filtered_card(
            s,
            "App Skin",
            Some(
                "How the app chrome (sidebar, panes, buttons) looks. \
                 Doesn't affect the terminal palette — that's the \
                 Themes tab.",
            ),
            Select::new(&self.skin_select),
        );

        // -- Installed Skins: import button + list of custom skins
        // with per-row Remove. Built-ins live in the picker only.
        let custom_skins: Vec<skins::Skin> = self
            .skins_store
            .list()
            .into_iter()
            .filter(|sk| sk.source == "user")
            .collect();
        let installed_body: gpui::Div = if custom_skins.is_empty() {
            div().text_size(px(12.0)).text_color(rgba(s.fg_secondary)).child(
                "No custom skins installed. Use Import to add a skin \
                 JSON file.",
            )
        } else {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .children(custom_skins.into_iter().map(|sk| self.skin_row(&sk, s, cx)))
        };
        let installed_card = div()
            .w_full()
            .bg(rgba(s.bg_panel))
            .border_1()
            .border_color(rgba(s.border_subtle))
            .rounded(px(s.radius_lg))
            .shadow_sm()
            .px_4()
            .py_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(15.0))
                            .text_color(rgba(s.fg_primary))
                            .child("Installed Skins"),
                    )
                    .child(import_button(
                        s,
                        "Import skin\u{2026}",
                        cx,
                        |this, window, cx| this.start_skin_import(window, cx),
                    )),
            )
            .child(installed_body);
        let installed_card = installed_card.opacity(self.filter_opacity("Installed Skins"));

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(app_skin_card)
            .child(self.filtered_card(
                s,
                "Appearance",
                Some(
                    "Light or dark variant of the active skin. \"Auto\" \
                     follows the system setting. Skins flagged dark-only \
                     ignore this and pin dark.",
                ),
                Select::new(&self.appearance_select),
            ))
            .child(installed_card)
            .child(self.filtered_card(
                s,
                "Terminal Font Size",
                Some(
                    "Pixel size of the terminal pane's monospace font. \
                     Leave blank to use the default (13). Changes the \
                     terminal grid only — chrome text size is fixed.",
                ),
                Input::new(&self.font_size).small().appearance(true),
            ))
            .child(self.filtered_card(
                s,
                "Scrollback",
                Some(
                    "Lines of terminal output retained for mouse-wheel \
                     scrollback. Higher values use more memory but let \
                     you scroll further back through chatty sessions. \
                     Leave blank to use the default (10000).",
                ),
                Input::new(&self.scrollback).small().appearance(true),
            ))
    }

    /// One row inside the Installed Skins card. Mirrors `theme_row`
    /// in shape: name + "Custom" tag (with "· dark-only" when the
    /// skin pinned dark) on the left, trash button on the right.
    /// Built-ins never reach this builder — the caller filters them
    /// out before iterating.
    fn skin_row(
        &self,
        skin: &skins::Skin,
        s: SkinTokens,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let skin_id = skin.id.clone();
        let trash_id = ElementId::Name(SharedString::from(format!(
            "trash-skin-{}",
            skin.id
        )));
        let tag = if !skin.supports_light {
            "Custom \u{00B7} dark-only".to_string()
        } else {
            "Custom".to_string()
        };
        div()
            .w_full()
            .bg(rgba(s.bg_input))
            .border_1()
            .border_color(rgba(s.border_subtle))
            .rounded(px(s.radius_md))
            .px_3()
            .py_2()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgba(s.fg_primary))
                            .child(SharedString::from(skin.name.clone())),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgba(s.fg_secondary))
                            .child(tag),
                    ),
            )
            .child(trash_button(
                s,
                trash_id,
                "Remove imported skin",
                cx,
                move |this, window, cx| {
                    this.delete_named_skin(skin_id.clone(), window, cx);
                },
            ))
    }

    /// Accessibility tab. Read-only summary of every OS-level
    /// accessibility preference Baudrun reacts to, with a one-
    /// line note on where the user changes it. Settings here are
    /// driven by the host OS rather than persisted in
    /// `settings.json` — Baudrun honours them automatically; this
    /// pane just makes that contract visible (and discoverable
    /// via the filter under "accessibility", "reduce motion",
    /// "a11y", etc.).
    fn accessibility_pane(&self, s: SkinTokens, cx: &mut Context<Self>) -> impl IntoElement {
        let reduce_motion = cx.global::<crate::ReduceMotion>().0;
        let status_label: SharedString = if reduce_motion {
            "On — Baudrun is skipping the reconnect-dot pulse and \
             holding the terminal cursor steady."
                .into()
        } else {
            "Off — Baudrun's optional animations (reconnect-dot pulse, \
             terminal cursor blink) are on."
                .into()
        };
        let os_path: &'static str = if cfg!(target_os = "macos") {
            "Change at System Settings → Accessibility → Display → \
             \u{201C}Reduce motion\u{201D}."
        } else if cfg!(target_os = "windows") {
            "Change at Settings → Accessibility → Visual effects → \
             \u{201C}Animation effects\u{201D}."
        } else {
            "Change via your desktop environment's accessibility / \
             animation settings."
        };
        let dot_color = if reduce_motion {
            s.success
        } else {
            s.fg_tertiary
        };
        let body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(rgba(dot_color)),
                    )
                    .child(div().text_sm().child(status_label)),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgba(s.fg_secondary))
                    .child(os_path),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgba(s.fg_tertiary))
                    .child(
                        "Detected once at app launch. Relaunch Baudrun \
                         after flipping the system setting if a value here \
                         looks stale.",
                    ),
            );
        div().flex().flex_col().gap_3().child(self.filtered_card(
            s,
            "Reduce Motion",
            Some(
                "Honours the OS preference for reduced motion. Read from \
                 the system at launch; no per-app override.",
            ),
            body,
        ))
    }

    /// Updates tab — current version, latest-from-GitHub status,
    /// and the user-facing knobs that gate the boot-time check.
    /// Mirrors the structure of `accessibility_pane`: a single
    /// status card up top + a settings card below.
    fn updates_pane(&self, s: SkinTokens, cx: &mut Context<Self>) -> impl IntoElement {
        let cur = &self.settings;
        let current_version = env!("CARGO_PKG_VERSION");
        // Clone the global out so it stops borrowing `cx` for the
        // duration of the render — every `import_button` /
        // `bool_field` call below grabs `cx` for its own listener,
        // which would conflict with a live borrow of the global.
        let available = cx
            .global::<crate::updater::UpdateState>()
            .available
            .clone();
        let dismissed_match = available
            .as_ref()
            .and_then(|a| {
                cur.dismissed_update_version
                    .as_deref()
                    .map(|d| d == a.version)
            })
            .unwrap_or(false);

        // -- status card --
        // Three states:
        //   1. Check disabled → grey dot + "checks off" message.
        //   2. No newer release / not yet checked → green dot +
        //      "up to date" message.
        //   3. Newer release found → amber dot + version pill +
        //      View Release / Dismiss buttons (or "Dismissed"
        //      label when the user has already clicked Dismiss).
        let (dot_color, status_label): (u32, SharedString) =
            if cur.disable_update_check {
                (s.fg_tertiary, "Update checks are disabled.".into())
            } else if let Some(a) = available.as_ref() {
                if dismissed_match {
                    (
                        s.fg_tertiary,
                        format!(
                            "v{} is available — you've dismissed this version.",
                            a.version
                        )
                        .into(),
                    )
                } else {
                    (
                        s.warn,
                        format!("v{} is available.", a.version).into(),
                    )
                }
            } else {
                (s.success, "Baudrun is up to date.".into())
            };

        let current_line: SharedString =
            format!("Currently running v{current_version}.").into();

        let action_row = available.clone().and_then(|a| {
            if dismissed_match || cur.disable_update_check {
                None
            } else {
                Some(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .items_center()
                        .child(import_button(
                            s,
                            "View release",
                            cx,
                            {
                                let url = a.html_url.clone();
                                move |_, _, cx| cx.open_url(&url)
                            },
                        ))
                        .child(import_button(s, "Dismiss this version", cx, {
                            let version = a.version.clone();
                            move |this, _, cx| {
                                this.dismiss_update(version.clone(), cx);
                            }
                        })),
                )
            }
        });

        let notes = available
            .as_ref()
            .filter(|_| !dismissed_match)
            .map(|a| {
                // First ~600 chars of the release notes — enough
                // for the headline, not the whole changelog.
                let raw = a.notes.trim();
                let truncated: String = raw.chars().take(600).collect();
                let display = if raw.chars().count() > 600 {
                    format!("{truncated}…")
                } else {
                    truncated
                };
                div()
                    .text_sm()
                    .text_color(rgba(s.fg_secondary))
                    .whitespace_normal()
                    .child(SharedString::from(display))
            });

        let status_body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(rgba(dot_color)),
                    )
                    .child(div().text_sm().child(status_label)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgba(s.fg_tertiary))
                    .child(current_line),
            )
            .children(notes)
            .children(action_row);

        // -- preferences card --
        let prefs_body = div()
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
            ));

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.filtered_card(
                s,
                "Updates",
                Some(
                    "Baudrun queries GitHub Releases once at app launch \
                     for a newer version. No automatic download — when \
                     an update is available the sidebar's gear icon \
                     gets an amber dot and this pane shows the new \
                     version with a button to open the Releases page.",
                ),
                status_body,
            ))
            .child(self.filtered_card(
                s,
                "Update Preferences",
                Some(
                    "Disable the check entirely, or opt into seeing \
                     pre-release tags (`-alpha.*` / `-beta.*` / `-rc.*`).",
                ),
                prefs_body,
            ))
    }

    fn advanced_pane(&self, s: SkinTokens, cx: &mut Context<Self>) -> impl IntoElement {
        let cur = &self.settings;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.filtered_card(
                s,
                "Session Log Directory",
                Some(
                    "Where profiles with \"Record session to file\" enabled \
                     write their logs. Leave blank to use the default.",
                ),
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&self.log_dir).small().appearance(true)),
                    )
                    .child(import_button(
                        s,
                        "Choose\u{2026}",
                        cx,
                        |this, window, cx| this.choose_log_dir(window, cx),
                    ))
                    .child(import_button(
                        s,
                        "Reset",
                        cx,
                        |this, window, cx| this.reset_log_dir(window, cx),
                    )),
            ))
            .child(self.filtered_card(
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
            .child(self.filtered_card(
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
            .child(self.filtered_card(
                s,
                "Window State",
                Some(
                    "Reopen Baudrun and Settings windows where you left \
                     them. Turn off to land at the centered default \
                     size each launch instead. The Reset buttons clear \
                     the saved geometry and resize the matching live \
                     windows immediately.",
                ),
                // Two rows: the on/off toggle on top, then a pair of
                // Reset buttons (Settings window vs main window) below.
                // The two-button split lets the user fix just the
                // window that drifted without touching the other.
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(bool_field(
                        "settings-restore-window-state",
                        "Restore window size and position on launch",
                        !cur.disable_window_state_restore,
                        cx,
                        SettingsView::set_restore_window_state,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(import_button(
                                s,
                                "Reset Settings window",
                                cx,
                                |this, window, cx| {
                                    this.reset_settings_window(window, cx)
                                },
                            ))
                            .child(import_button(
                                s,
                                "Reset main window",
                                cx,
                                |this, window, cx| {
                                    this.reset_main_window(window, cx)
                                },
                            )),
                    ),
            ))
            // (Updates moved to its own `SettingsTab::Updates`
            // pane — see `updates_pane`.)
            .child(
                // Custom card layout — section_card_with_desc puts
                // description directly under the title and a single
                // body block beneath that, but we want a Reveal
                // pill on the right of the title row (parallel to
                // how "Installed Themes" hosts its Import button).
                div()
                    .w_full()
                    .bg(rgba(s.bg_panel))
                    .border_1()
                    .border_color(rgba(s.border_subtle))
                    .rounded(px(s.radius_lg))
                    .shadow_sm()
                    .opacity(self.filter_opacity("Config Directory"))
                    .px_4()
                    .py_3()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    // `min_w_0` lets the description
                                    // wrap below its intrinsic min-
                                    // content width — without it the
                                    // long copy widens the column,
                                    // pushes the Reveal pill off-
                                    // screen, and pulls the rest of
                                    // the card with it.
                                    .min_w_0()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(15.0))
                                            .text_color(rgba(s.fg_primary))
                                            .child("Config Directory"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgba(s.fg_secondary))
                                            .whitespace_normal()
                                            .child(
                                                "Where Baudrun stores profiles, themes, \
                                                 skins, highlight packs, and \
                                                 settings.json. Choose a different \
                                                 location to share configs across \
                                                 machines (Dropbox, iCloud Drive, \
                                                 dotfile repo) or to test from a clean \
                                                 directory. Takes effect on the next \
                                                 launch.",
                                            ),
                                    ),
                            )
                            .child(import_button(
                                s,
                                "Reveal",
                                cx,
                                |this, window, cx| this.reveal_config_dir(window, cx),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .px_3()
                                    .py(px(6.0))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgba(s.border_subtle))
                                    .bg(rgba(s.bg_input))
                                    .text_size(px(12.0))
                                    .text_color(rgba(s.fg_secondary))
                                    .child(self.config_dir_display.clone()),
                            )
                            .child(import_button(
                                s,
                                "Choose\u{2026}",
                                cx,
                                |this, window, cx| this.choose_config_dir(window, cx),
                            ))
                            .child(import_button(
                                s,
                                "Reset",
                                cx,
                                |this, window, cx| this.reset_config_dir(window, cx),
                            )),
                    ),
            )
    }
}

/// Window-top header bar. Mirrors `form_header` — title at the
/// Baudrun `--font-size-h1` (24px), uppercase tag underneath, full
/// width with a subtle bottom border.
fn window_header(
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
                .child(
                    Input::new(filter_input).small().appearance(true),
                )
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
                                Tooltip::new(SharedString::from("Clear filter"))
                                    .build(window, cx)
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
fn rail(
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
        let hover_bg = s.bg_input;
        let opacity = if dimmed { 0.35 } else { 1.0 };
        let show_dot = update_pending && tab == SettingsTab::Updates;
        div()
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
                        this.child(
                            div()
                                .w(px(8.0))
                                .h(px(8.0))
                                .rounded_full()
                                .bg(rgba(s.warn)),
                        )
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
        .children(SettingsTab::ALL.iter().map(|&t| {
            item(t.label(), t, !lookup_match(t))
        }))
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

/// Pill-style "Import\u{2026}" button used at the bottom of each
/// asset section. Matches the visual weight of the surrounding
/// chrome — slightly raised on the panel's translucent bg, accent
/// border on hover for affordance. The on-click handler lives in
/// `SettingsView::start_*_import`.
/// Section title → extra keyword string for the Settings filter
/// box. Titles already match against themselves; this table is for
/// synonyms / related concepts so a user typing "ports" or
/// "voiceover" lands at the relevant section even when the title
/// itself doesn't contain the word. Keep entries lowercase — the
/// matcher normalises before comparing.
const SECTION_KEYWORDS: &[(&str, &str)] = &[
    ("App Skin", "chrome appearance theme accent radius font ui interface look feel \
                   style baudrun dracula synthwave crt monokai gruvbox solarized nord \
                   tokyo night brogrammer terminal panel sidebar widget"),
    ("Appearance", "light dark auto system mode follow os macos windows night day"),
    ("Installed Skins", "import custom skin manage delete remove add upload trash \
                         user imported personalize"),
    ("Terminal Font Size", "font size monospace terminal text glyph pixel zoom"),
    ("Scrollback", "scrollback lines buffer history wheel scroll mouse pixels page"),
    ("Default Theme", "theme palette ansi colour color terminal viewport baudrun \
                       dracula monokai gruvbox solarized nord onedark tomorrow brogrammer"),
    ("Installed Themes", "import itermcolors theme manage delete remove preview \
                          color colour palette terminal user custom"),
    ("Keyboard Shortcuts", "binding key hotkey shortcut clear send break suspend resume \
                            connect disconnect copy paste cmd ctrl meta combo capture \
                            override default"),
    ("Highlight Packs", "highlight pack rule regex syntax colour color match \
                         cisco junos arista mikrotik aruba routeros eos vendor \
                         network bgp ospf stp interface log error warning"),
    ("Session Log Directory", "log directory folder record session save path file \
                               record-to-file logfile capture record"),
    ("USB Driver Detection", "usb driver detection cp210x ftdi pl2303 ch340 banner \
                              adapter notice missing driver pop-up alert"),
    ("Copy on Select", "copy clipboard selection putty mouse drag autoselect auto \
                        copy on select autocopy"),
    ("Window State", "window position size remember restore launch persistence \
                      geometry frame bounds layout"),
    ("Updates", "update github release version check prerelease alpha beta rc \
                 changelog new available notification download"),
    ("Config Directory", "config directory folder path location store dropbox icloud \
                          override support portable share xdg appdata application support"),
    ("Reduce Motion", "reduce motion animation pulse blink cursor reconnect dot \
                       accessibility a11y wcag prefers prefer system os macos \
                       windows linux preference disable disabled steady still"),
];

const TAB_SECTIONS: &[(SettingsTab, &[&str])] = &[
    (SettingsTab::Appearance, &[
        "App Skin",
        "Appearance",
        "Installed Skins",
        "Terminal Font Size",
        "Scrollback",
    ]),
    (SettingsTab::Themes, &["Default Theme", "Installed Themes"]),
    (SettingsTab::Shortcuts, &["Keyboard Shortcuts"]),
    (SettingsTab::Highlighting, &["Highlight Packs"]),
    (SettingsTab::Accessibility, &["Reduce Motion"]),
    (SettingsTab::Updates, &["Updates", "Check for updates", "Pre-releases"]),
    (SettingsTab::Advanced, &[
        "Session Log Directory",
        "USB Driver Detection",
        "Copy on Select",
        "Window State",
        "Config Directory",
    ]),
];

/// Schedule a Notification to dismiss itself ~1.5 s after the user
/// clicks its action button. Without this the toast stays up for
/// the full autohide window (5 s) even after the user already
/// acted on it — feels like the click had no effect. The brief
/// linger gives time to read the follow-up "Restored …" toast
/// before the original "Removed …" one fades.
/// Schedule a Notification to dismiss itself ~1.5 s after the user
/// clicks its action button. Without this the toast stays up for
/// the full autohide window (5 s) even after the user already
/// acted on it — feels like the click had no effect. The brief
/// linger gives time to read the follow-up "Restored …" toast
/// before the original "Removed …" one fades.
fn dismiss_notification_after(notif: Entity<Notification>, cx: &mut gpui::App) {
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
fn encode_file_path(p: &std::path::Path) -> String {
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
fn resolve_config_dir_display() -> SharedString {
    match appdata::support_dir() {
        Ok(path) => SharedString::from(path.display().to_string()),
        Err(err) => SharedString::from(format!("(unavailable: {err})")),
    }
}

fn import_button<F>(
    s: SkinTokens,
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

/// Compact swatch strip — 8 normal-ANSI colors as small rounded tiles
/// fused into a single horizontal pill. Sits at the left of each
/// installed-themes row so the user can compare palettes at a glance
/// without opening the full preview dialog.
fn theme_swatches(theme: &themes::Theme) -> impl IntoElement {
    let palette = [
        &theme.black,
        &theme.red,
        &theme.green,
        &theme.yellow,
        &theme.blue,
        &theme.magenta,
        &theme.cyan,
        &theme.white,
    ];
    div()
        .flex()
        .flex_row()
        .overflow_hidden()
        .rounded(px(6.0))
        .children(palette.into_iter().map(|hex| {
            let color = parse_theme_color(hex).unwrap_or(0x808080FFu32);
            div().w(px(16.0)).h(px(28.0)).bg(rgba(color))
        }))
}

/// Sample lines for the theme-preview dialog. Each entry is one line
/// rendered as a horizontal flex of (palette-slot, text) spans. Slots
/// resolve against the theme being previewed so the snippet shows
/// every major colour the palette declares — `magenta` for keywords,
/// `green`/`red`/`yellow` for status, `cyan` for identifiers, plain
/// `fg` for prose.
fn preview_sample_lines() -> Vec<Vec<(&'static str, &'static str)>> {
    vec![
        vec![("fg", "RUGGEDCOM RS900G login: "), ("magenta", "admin")],
        vec![("fg", "Password: "), ("yellow", "********")],
        vec![],
        vec![
            ("fg", "Last login: Fri Apr 17 "),
            ("cyan", "11:47:05"),
            ("fg", " 2026 from "),
            ("cyan", "192.168.1.47"),
        ],
        vec![("fg", "RUGGEDCOM RS900G # show interfaces status")],
        vec![],
        vec![
            ("magenta", "Interface"),
            ("fg", "          Status  VLAN  "),
            ("magenta", "Duplex Speed"),
        ],
        vec![
            ("green", "GigabitEthernet0/1"),
            ("fg", "  "),
            ("green", "up"),
            ("fg", "      1     "),
            ("green", "full"),
            ("fg", "    1000"),
        ],
        vec![
            ("green", "GigabitEthernet0/2"),
            ("fg", "  "),
            ("red", "down"),
            ("fg", "    -     -       -"),
        ],
        vec![
            ("green", "Gi1/0/3"),
            ("fg", "             "),
            ("green", "up"),
            ("fg", "      10    "),
            ("green", "full"),
            ("fg", "    1000"),
        ],
        vec![
            ("green", "FastEthernet0/24"),
            ("fg", "    "),
            ("red", "err-disabled"),
            ("fg", "  -     -       -"),
        ],
        vec![
            ("green", "ge-0/0/1"),
            ("fg", "            "),
            ("green", "up"),
            ("fg", "      -     "),
            ("green", "full"),
            ("fg", "    10000"),
        ],
        vec![],
        vec![
            ("cyan", "11:47:12.120"),
            ("fg", " MAC "),
            ("magenta", "aa:bb:cc:dd:ee:ff"),
            ("fg", " learned on "),
            ("magenta", "Gi1/0/1"),
        ],
        vec![
            ("cyan", "11:47:13.440"),
            ("fg", " "),
            ("yellow", "WARNING"),
            ("fg", ": "),
            ("magenta", "STP"),
            ("fg", " "),
            ("yellow", "learning"),
            ("fg", " on "),
            ("magenta", "Port-channel1"),
        ],
        vec![
            ("cyan", "11:47:15.002"),
            ("fg", " Link "),
            ("red", "DOWN"),
            ("fg", " on "),
            ("magenta", "GigabitEthernet0/2"),
        ],
        vec![
            ("cyan", "11:47:18.881"),
            ("fg", " Route "),
            ("magenta", "192.168.1.47/24"),
            ("fg", " via "),
            ("green", "ge-0/0/1"),
            ("fg", " "),
            ("green", "established"),
        ],
        vec![],
        vec![
            ("cyan", "2026-04-17 11:48:00"),
            ("fg", " Session idle "),
            ("red", "timeout"),
            ("fg", ": "),
            ("cyan", "00:10:00"),
        ],
        vec![("fg", "RUGGEDCOM RS900G #")],
    ]
}

/// Build the body of the theme-preview dialog — a styled block of
/// fake network-gear output painted in the chosen theme's palette.
/// Each line is rendered as a horizontal flex so per-span colouring
/// just works without HTML-style markup.
fn theme_preview_block(theme: &themes::Theme) -> impl IntoElement {
    let bg = parse_theme_color(&theme.background).unwrap_or(0x000000FFu32);
    let fg = parse_theme_color(&theme.foreground).unwrap_or(0xE4E4E7FFu32);
    let pick = |slot: &str| -> u32 {
        let raw = match slot {
            "fg" => &theme.foreground,
            "bg" => &theme.background,
            "black" => &theme.black,
            "red" => &theme.red,
            "green" => &theme.green,
            "yellow" => &theme.yellow,
            "blue" => &theme.blue,
            "magenta" => &theme.magenta,
            "cyan" => &theme.cyan,
            "white" => &theme.white,
            _ => &theme.foreground,
        };
        parse_theme_color(raw).unwrap_or(0x808080FFu32)
    };
    let line_views: Vec<gpui::Div> = preview_sample_lines()
        .into_iter()
        .map(|spans| {
            // Empty line → render a thin spacer so blank lines in the
            // sample read as actual blank lines, not collapsed gaps.
            if spans.is_empty() {
                return div().h(px(8.0));
            }
            div()
                .flex()
                .flex_row()
                .children(spans.into_iter().map(|(slot, text)| {
                    div()
                        .text_color(rgba(pick(slot)))
                        .child(SharedString::from(text))
                }))
        })
        .collect();
    div()
        .px_4()
        .py_3()
        .rounded_md()
        .bg(rgba(bg))
        .text_color(rgba(fg))
        .text_size(px(13.0))
        .font_family("Menlo")
        .flex()
        .flex_col()
        .children(line_views)
}

/// Compact `#rrggbb` parser specialised for the theme JSON shape.
/// Themes never carry alpha, so we always pack `0xFF` and let the
/// caller treat it as `0xRRGGBBAA`. Returns `None` on any malformed
/// input — the caller falls back to a neutral grey so the preview
/// doesn't disappear on a single bad slot.
fn parse_theme_color(s: &str) -> Option<u32> {
    let hex = s.trim().strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | 0xFF)
}

/// Square icon-button rendering a trash glyph. Used next to import
/// buttons (skin / theme tabs) and per-row in the highlight tab to
/// remove a user-imported entry. Hover swaps to the danger token so
/// the destructive intent is obvious before the click.
fn trash_button<F>(
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
            .tooltip(move |window, cx| {
                Tooltip::new(tip_text.clone()).build(window, cx)
            })
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
pub(crate) const SHORTCUT_ACTIONS: &[&str] = &[
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

// Defaults differ from the Tauri shipping app in one spot:
// "new-profile" was Meta+N there because Tauri ran single-window,
// so Cmd+N had no conflicting "New Window" candidate. Now that
// every menubar gets a real File → New Window (statically bound to
// Cmd+N to match macOS convention), New Profile shifts to
// Cmd+Shift+P so the bindings don't collide. Users who imported a
// settings.json with the old Meta+N override still get their
// preference — `effective_shortcut` honours the override before
// falling back here.
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
        "new-profile" => "Meta+Shift+P",
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
        "new-profile" => "Control+Shift+P",
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
pub(crate) fn effective_shortcut(action: &str, overrides: &HashMap<String, String>) -> String {
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

/// Convert a stored W3C shortcut spec (`"Meta+Shift+K"`) into the
/// hyphen-joined lowercase form gpui's `KeyBinding::new` parser
/// expects (`"cmd-shift-k"`). Returns `None` for specs with no key
/// or only modifiers (gpui would reject those at parse time).
///
/// Mirrors `parse_spec` for the modifier-token vocabulary; the
/// difference is the output encoding — gpui uses `-` between
/// parts. Crucially, gpui's `cmd` / `super` / `win` tokens all set
/// `modifiers.platform`, which is the Cmd key on macOS but the
/// Windows / Super key on Windows / Linux — there is no
/// auto-translation. The portable token is `secondary-`, which
/// resolves to Cmd on macOS and Ctrl elsewhere. We can still emit
/// `cmd-` here because the upstream `default_for_action` is
/// `#[cfg]`-gated to only use `Meta+...` on macOS; on Windows /
/// Linux every shortcut feeds in as `Control+...` and we encode
/// that as `ctrl-...` (which IS portable to the real Control key
/// on every OS).
pub(crate) fn spec_to_gpui_binding(spec: &str) -> Option<String> {
    if spec.is_empty() {
        return None;
    }
    let mut parts: Vec<&str> = Vec::with_capacity(5);
    let mut control = false;
    let mut platform = false;
    let mut shift = false;
    let mut alt = false;
    let mut key: Option<String> = None;
    for raw in spec.split('+') {
        let tok = raw.trim();
        if tok.is_empty() {
            continue;
        }
        match tok.to_ascii_lowercase().as_str() {
            "control" | "ctrl" => control = true,
            "meta" | "cmd" | "command" | "super" | "win" => platform = true,
            "shift" => shift = true,
            "alt" | "option" => alt = true,
            _ => key = Some(canonical_key_for_display(tok)),
        }
    }
    let key = key?;
    // Order matches gpui's parser tolerance but we standardise on
    // ctrl → cmd → alt → shift → key for round-trip stability with
    // the rest of the codebase's KeyBinding strings.
    if control {
        parts.push("ctrl");
    }
    if platform {
        parts.push("cmd");
    }
    if alt {
        parts.push("alt");
    }
    if shift {
        parts.push("shift");
    }
    let mut out = parts.join("-");
    if !out.is_empty() {
        out.push('-');
    }
    out.push_str(&key);
    Some(out)
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
