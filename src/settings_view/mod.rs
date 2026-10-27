//! Standalone Settings window. Mirrors the Tauri layout — a separate
//! OS window so the user can change skins / themes / shortcuts and
//! watch them apply live in the main window without having to flip
//! back and forth past a modal sheet.
//!
//! The layout — page sidebar, search, grouped settings — is GPUI Kit's
//! `Settings`; each page is built from the cached `Settings` value and
//! commits through `SettingsBus`, same as before.

mod chrome;
mod shortcuts;
mod theme_preview;

use chrome::{
    action_button, dismiss_notification_after, encode_file_path, resolve_config_dir_display,
    trash_button,
};
use shortcuts::{any_modifier, format_spec, parse_spec, shortcut_label};
pub(crate) use shortcuts::{effective_shortcut, spec_to_gpui_binding, SHORTCUT_ACTIONS};
use theme_preview::{theme_preview_block, theme_swatches};

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, rgba, Anchor, App, AppContext, Axis, Context, Entity, FocusHandle,
    IntoElement, KeyDownEvent, PathPromptOptions, Render, SharedString, Subscription, Window,
};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    group_box::GroupBoxVariant,
    h_flex,
    input::{Input, InputEvent, InputState},
    kbd::Kbd,
    menu::{DropdownMenu, PopupMenuItem},
    notification::Notification,
    setting::{self, SettingField, SettingGroup, SettingItem, SettingPage},
    switch::Switch,
    v_flex, ActiveTheme, IconName, Sizable, TitleBar, WindowExt,
};
// Full Lucide set; the non-default icons used here are embedded via
// `ExtraIcons` in main.rs.
use gpui_kit::assets::IconName as Lucide;

use crate::data::appdata;
use crate::data::highlight;
use crate::data::settings::Settings;
use crate::data::skins;
use crate::data::themes;
use crate::settings_bus::{SettingsBus, SettingsEvent};
use crate::skin_tokens::SkinTokens;

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
    /// Skin store: the App Skin options and the Installed Skins list.
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
    /// Theme store: the Default Theme options and Installed Themes list.
    themes_store: Rc<themes::Store>,
    settings: Settings,

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

    // -- Appearance: plain inputs, committed on blur / Enter; empty
    // means "use the default" (stored as 0). --
    font_size: Entity<InputState>,
    _font_size_sub: Subscription,
    scrollback: Entity<InputState>,
    _scrollback_sub: Subscription,

    // -- Advanced tab field state --
    log_dir: Entity<InputState>,
    /// Subscription handles held to keep the subscriptions alive
    /// for the SettingsView's lifetime; dropping them cancels the
    /// subscription. Underscore prefix marks "owned solely for
    /// drop-time effect, never read."
    _log_dir_sub: Subscription,

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

        // -- Appearance: terminal font size + scrollback --

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
        let font_size_sub = cx.subscribe(&font_size, |this, input, event: &InputEvent, cx| {
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
        let scrollback_sub = cx.subscribe(&scrollback, |this, input, event: &InputEvent, cx| {
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

        // -- Advanced tab widgets --

        let log_dir = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(t!("settings.log_dir_placeholder"))
                .default_value(current.log_dir.as_str())
        });

        // Persist log_dir on Blur (focus loss). Live save per
        // keystroke would write the JSON file on every typed
        // character — wasteful, and noisy for `fs::write`.
        let log_dir_sub = cx.subscribe(&log_dir, |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
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
            font_size,
            _font_size_sub: font_size_sub,
            scrollback,
            _scrollback_sub: scrollback_sub,
            capturing_shortcut: None,
            capture_focus: cx.focus_handle(),
            log_dir,
            _log_dir_sub: log_dir_sub,
            config_dir_display: resolve_config_dir_display(),
        }
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
                // Previously this was a bare `log::error!` and the user
                // never saw the failure — the toggle would appear to
                // take but the next launch would resurrect the old
                // value because the on-disk write had silently failed.
                // Defer the toast through a no-op spawn so we can pick
                // up `window` (the `&mut Context<Self>` we're handed
                // here doesn't carry one).
                log::error!("save settings: {err}");
                let msg = t!("settings.toast.save_failed", err = err);
                cx.spawn(async move |weak, cx_async| {
                    let _ = weak.update_in(cx_async, |_, window, view_cx| {
                        window.push_notification(
                            Notification::error(SharedString::from(msg)),
                            view_cx,
                        );
                    });
                })
                .detach();
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
                            Notification::error(SharedString::from(t!(
                                "settings.toast.config_open_failed",
                                err = err
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
            prompt: Some(t!("settings.prompt.choose_config_dir").into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update_in(cx, |this, window, view_cx| {
                if let Err(err) = appdata::write_override(Some(&path)) {
                    log::error!("write config dir override: {err}");
                    window.push_notification(
                        Notification::error(SharedString::from(t!(
                            "settings.toast.config_set_failed",
                            err = err
                        ))),
                        view_cx,
                    );
                    return;
                }
                this.config_dir_display = resolve_config_dir_display();
                view_cx.notify();
                window.push_notification(
                    Notification::success(SharedString::from(t!("settings.toast.config_set"))),
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
                Notification::error(SharedString::from(t!(
                    "settings.toast.config_reset_failed",
                    err = err
                ))),
                cx,
            );
            return;
        }
        self.config_dir_display = resolve_config_dir_display();
        cx.notify();
        window.push_notification(
            Notification::success(SharedString::from(t!("settings.toast.config_reset"))),
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
            prompt: Some(t!("settings.prompt.choose_log_dir").into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let path_str = path.display().to_string();
            let _ = this.update_in(cx, |this, window, view_cx| {
                this.log_dir.update(view_cx, |state, view_cx| {
                    state.set_value(path_str.clone(), window, view_cx);
                });
                if this.settings.log_dir != path_str {
                    let mut next = this.settings.clone();
                    next.log_dir = path_str;
                    this.commit(next, view_cx);
                    window.push_notification(
                        Notification::success(SharedString::from(t!(
                            "settings.toast.log_dir_updated"
                        ))),
                        view_cx,
                    );
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
            window.push_notification(
                Notification::success(SharedString::from(t!("settings.toast.log_dir_reset"))),
                cx,
            );
        }
    }

    /// Reset the Settings window's geometry — clears the saved
    /// `settings_window` from settings.json AND resizes the live
    /// Settings window (the one this button lives in) back to its
    /// default centered size. Live resize so the user sees the
    /// effect immediately rather than having to close + reopen.
    fn reset_settings_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        let (w, h) = crate::app_view::SETTINGS_WINDOW_SIZE;
        window.resize(gpui::size(px(w), px(h)));
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
                let Some(Some(root)) = window.root::<gpui_kit::component::Root>() else {
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

    fn start_capture(&mut self, action: &'static str, window: &mut Window, cx: &mut Context<Self>) {
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
        if matches!(
            key,
            "shift" | "ctrl" | "control" | "cmd" | "alt" | "platform"
        ) {
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
            prompt: Some(t!("settings.prompt.import_skin").into()),
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
                    let _ = this.update_in(cx, |_, window, view_cx| {
                        view_cx.notify();
                        window.push_notification(
                            Notification::success(SharedString::from(t!(
                                "settings.toast.skin_imported",
                                name = name
                            ))),
                            view_cx,
                        );
                    });
                }
                Err(err) => {
                    log::error!("import skin: {err}");
                    let msg = t!("settings.toast.skin_import_failed", err = err);
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
            prompt: Some(t!("settings.prompt.import_theme").into()),
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
                    let _ = this.update_in(cx, |_, window, view_cx| {
                        view_cx.notify();
                        window.push_notification(
                            Notification::success(SharedString::from(t!(
                                "settings.toast.theme_imported",
                                name = name
                            ))),
                            view_cx,
                        );
                    });
                }
                Err(err) => {
                    log::error!("import theme: {err}");
                    let msg = t!("settings.toast.theme_import_failed", err = err);
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
    fn delete_named_skin(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(skin) = self.skins_store.get(&id) else {
            return;
        };
        if skin.source != "user" {
            return;
        }
        let snapshot = skin.clone();
        let name = skin.name.clone();
        if let Err(err) = self.skins_store.delete(&id) {
            log::error!("delete skin {id}: {err}");
            window.push_notification(
                Notification::error(SharedString::from(t!(
                    "settings.toast.skin_delete_failed",
                    err = err
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
        cx.notify();

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
            Notification::success(SharedString::from(t!(
                "settings.toast.skin_removed",
                name = name
            )))
            .action(move |_, _, ctx| {
                let app = app.clone();
                let snapshot = snapshot.clone();
                let prev_active_id = prev_active_id.clone();
                let notif = ctx.entity();
                Button::new("undo-skin-delete")
                    .label(t!("settings.undo"))
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
                Notification::error(SharedString::from(t!(
                    "settings.toast.skin_restore_failed",
                    err = err
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
        cx.notify();
        window.push_notification(
            Notification::success(SharedString::from(t!(
                "settings.toast.skin_restored",
                name = name
            ))),
            cx,
        );
    }

    /// Per-row delete on the Themes tab. Refuses non-custom themes.
    /// If the theme being deleted is the currently-selected default,
    /// also resets `default_theme_id` to the built-in baudrun palette
    /// so the live terminal doesn't end up pointing at a dead id.
    fn delete_named_theme(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(theme) = self.themes_store.get(&id) else {
            return;
        };
        if theme.source != "user" {
            return;
        }
        let snapshot = theme.clone();
        let name = theme.name.clone();
        if let Err(err) = self.themes_store.delete(&id) {
            log::error!("delete theme {id}: {err}");
            window.push_notification(
                Notification::error(SharedString::from(t!(
                    "settings.toast.theme_delete_failed",
                    err = err
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
        cx.notify();
        let app = cx.entity();
        let prev_active_id = if was_active {
            Some(snapshot.id.clone())
        } else {
            None
        };
        window.push_notification(
            Notification::success(SharedString::from(t!(
                "settings.toast.theme_removed",
                name = name
            )))
            .action(move |_, _, ctx| {
                let app = app.clone();
                let snapshot = snapshot.clone();
                let prev_active_id = prev_active_id.clone();
                let notif = ctx.entity();
                Button::new("undo-theme-delete")
                    .label(t!("settings.undo"))
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
                Notification::error(SharedString::from(t!(
                    "settings.toast.theme_restore_failed",
                    err = err
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
        cx.notify();
        window.push_notification(
            Notification::success(SharedString::from(t!(
                "settings.toast.theme_restored",
                name = name
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
        let Some(theme) = self.themes_store.get(&theme_id) else {
            return;
        };
        let title = SharedString::from(theme.name.clone());
        let subtitle = SharedString::from(if theme.source == "user" {
            t!("settings.theme_preview.subtitle_imported")
        } else {
            t!("settings.theme_preview.subtitle_builtin")
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
    fn delete_highlight_pack(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pack) = self.highlight_store.list().into_iter().find(|p| p.id == id) else {
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
                Notification::error(SharedString::from(t!(
                    "settings.toast.pack_delete_failed",
                    err = err
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
            Notification::success(SharedString::from(t!(
                "settings.toast.pack_removed",
                name = name
            )))
            .action(move |_, _, ctx| {
                let app = app.clone();
                let snapshot = snapshot.clone();
                let notif = ctx.entity();
                Button::new("undo-pack-delete")
                    .label(t!("settings.undo"))
                    .on_click(move |_, window, cx| {
                        let app = app.clone();
                        let snapshot = snapshot.clone();
                        let notif = notif.clone();
                        app.update(cx, |this, view_cx| {
                            this.restore_highlight_pack(snapshot, was_enabled, window, view_cx);
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
                Notification::error(SharedString::from(t!(
                    "settings.toast.pack_restore_failed",
                    err = err
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
            Notification::success(SharedString::from(t!(
                "settings.toast.pack_restored",
                name = name
            ))),
            cx,
        );
    }

    fn start_highlight_import(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t!("settings.prompt.import_pack").into()),
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
                            Notification::success(SharedString::from(t!(
                                "settings.toast.pack_imported",
                                name = name
                            ))),
                            view_cx,
                        );
                    });
                }
                Err(err) => {
                    log::error!("import highlight: {err}");
                    let msg = t!("settings.toast.pack_import_failed", err = err);
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
}

/// Pull skin options out of the store + tag user-imported entries
/// with "(custom)". Used at SettingsView::new and on rebuild after
/// import.
fn build_skin_opts(store: &skins::Store) -> Vec<(SharedString, SharedString)> {
    store
        .list()
        .into_iter()
        .map(|s| {
            let title = if s.source == "user" {
                format!("{} (custom)", s.name)
            } else {
                s.name
            };
            opt(&s.id, title)
        })
        .collect()
}

fn build_theme_opts(store: &themes::Store) -> Vec<(SharedString, SharedString)> {
    store
        .list()
        .into_iter()
        .map(|t| {
            let title = if t.source == "user" {
                format!("{} (custom)", t.name)
            } else {
                t.name
            };
            opt(&t.id, title)
        })
        .collect()
}

// Chrome colours used to live as `const`s here, but Phase 4 slice 3
// moved them into the `SkinTokens` global so skin picks live-apply.
// SettingsView's render reads `cx.global::<SkinTokens>()` once and
// hands the value to the per-section helpers.

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *cx.global::<SkinTokens>();
        let view = cx.entity();
        div()
            .size_full()
            .flex()
            .flex_col()
            // `window_background()` picks up the skin's `--shell-bg`
            // gradient when present, so the Settings window's frosted
            // look matches the main one.
            .bg(s.window_background())
            .text_color(rgba(s.fg_primary))
            .text_size(px(13.0))
            // Title bar — transparent so the skin's window background
            // reads through, plus a per-skin `border_subtle` bottom rule
            // (dark-on-dark skins still get a visible separator). See the
            // matching branch in `app_view.rs`.
            .child(
                TitleBar::new()
                    .h(px(s.titlebar_height_px))
                    .bg(gpui::transparent_black())
                    .border_color(rgba(s.border_subtle)),
            )
            // GPUI Kit's Settings owns the page sidebar, the search box
            // (titles, descriptions and each item's `keywords`) and the
            // scrolling page body.
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(setting::Settings::new("settings").pages(self.pages(&view, s, cx))),
            )
    }
}

impl SettingsView {
    fn pages(&self, view: &Entity<Self>, s: SkinTokens, cx: &App) -> Vec<SettingPage> {
        vec![
            self.appearance_page(view),
            self.themes_page(view),
            self.shortcuts_page(view),
            self.highlighting_page(view),
            accessibility_page(s, cx),
            self.updates_page(view, s, cx),
            self.advanced_page(view),
        ]
    }

    fn appearance_page(&self, view: &Entity<Self>) -> SettingPage {
        let skin_opts = build_skin_opts(&self.skins_store);
        let appearance_opts = vec![
            opt("auto", t!("settings.appearance_auto")),
            opt("light", t!("settings.appearance_light")),
            opt("dark", t!("settings.appearance_dark")),
        ];
        let mut locale_opts = vec![opt("", t!("settings.language_auto"))];
        locale_opts.extend(
            crate::i18n::SUPPORTED
                .iter()
                .map(|(code, name)| opt(code, *name)),
        );

        let custom_skins: Vec<skins::Skin> = self
            .skins_store
            .list()
            .into_iter()
            .filter(|sk| sk.source == "user")
            .collect();
        let skin_rows: Vec<SettingItem> = if custom_skins.is_empty() {
            vec![SettingItem::render(|_, _, cx| {
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("settings.no_custom_skins"))
            })]
        } else {
            custom_skins
                .into_iter()
                .map(|sk| {
                    let tag = if sk.supports_light {
                        t!("settings.tag_custom")
                    } else {
                        t!("settings.tag_custom_dark_only")
                    };
                    let (view, id) = (view.clone(), sk.id.clone());
                    SettingItem::new(
                        sk.name.clone(),
                        SettingField::render(move |_, _, _| {
                            let (view, id) = (view.clone(), id.clone());
                            trash_button(
                                format!("trash-skin-{id}"),
                                t!("settings.tip.remove_skin"),
                                move |window, cx| {
                                    view.update(cx, |this, cx| {
                                        this.delete_named_skin(id.clone(), window, cx)
                                    })
                                },
                            )
                        }),
                    )
                    .description(tag.to_string())
                })
                .collect()
        };

        SettingPage::new(t!("settings.tab.appearance"))
            .icon(IconName::Palette)
            .default_open(true)
            .groups(
                Section::new()
                    .item(
                        SettingItem::new(
                            t!("settings.app_skin_title"),
                            dropdown(
                                skin_opts,
                                true,
                                read(view, |st| {
                                    if st.skin_id.is_empty() {
                                        SharedString::from(skins::DEFAULT_SKIN_ID)
                                    } else {
                                        SharedString::from(st.skin_id.clone())
                                    }
                                }),
                                write(view, |st, v: SharedString| st.skin_id = v.to_string()),
                            ),
                        )
                        .description(t!("settings.app_skin_desc").to_string())
                        .keywords([
                            "chrome theme accent radius font ui interface look feel style \
                             dracula synthwave crt monokai gruvbox solarized nord tokyo night",
                        ]),
                    )
                    .item(
                        SettingItem::new(
                            t!("settings.appearance_title"),
                            dropdown(
                                appearance_opts,
                                false,
                                read(view, |st| {
                                    if st.appearance.is_empty() {
                                        SharedString::from("auto")
                                    } else {
                                        SharedString::from(st.appearance.clone())
                                    }
                                }),
                                write(view, |st, v: SharedString| st.appearance = v.to_string()),
                            ),
                        )
                        .description(t!("settings.appearance_desc").to_string())
                        .keywords(["light dark auto system mode follow os night day"]),
                    )
                    .item(
                        SettingItem::new(
                            t!("settings.language_title"),
                            dropdown(
                                locale_opts,
                                false,
                                read(view, |st| SharedString::from(st.locale.clone())),
                                write(view, |st, v: SharedString| st.locale = v.to_string()),
                            ),
                        )
                        .description(t!("settings.language_desc").to_string())
                        .keywords(["language locale translation i18n"]),
                    ),
            )
            .groups(
                Section::new()
                    .item(
                        SettingItem::new(
                            t!("settings.font_size_title"),
                            text_input(view, |this| this.font_size.clone()),
                        )
                        .description(t!("settings.font_size_desc").to_string())
                        .keywords(["font size monospace terminal text glyph zoom"]),
                    )
                    .item(
                        SettingItem::new(
                            t!("settings.scrollback_title"),
                            text_input(view, |this| this.scrollback.clone()),
                        )
                        .description(t!("settings.scrollback_desc").to_string())
                        .keywords(["scrollback lines buffer history scroll"]),
                    ),
            )
            .groups(
                Section::new()
                    .title(t!("settings.installed_skins_title"))
                    .footer({
                        let view = view.clone();
                        move |_, _| {
                            let view = view.clone();
                            action_button(
                                "import-skin",
                                t!("settings.import_skin"),
                                move |w, cx| {
                                    view.update(cx, |this, cx| this.start_skin_import(w, cx))
                                },
                            )
                        }
                    })
                    .items(skin_rows),
            )
    }

    fn themes_page(&self, view: &Entity<Self>) -> SettingPage {
        let theme_rows: Vec<SettingItem> = self
            .themes_store
            .list()
            .into_iter()
            .map(|theme| {
                let is_custom = theme.source == "user";
                let tag = if is_custom {
                    t!("settings.tag_custom")
                } else {
                    t!("settings.tag_builtin")
                };
                let view = view.clone();
                let theme = Rc::new(theme);
                SettingItem::new(
                    theme.name.clone(),
                    SettingField::render(move |_, _, _| {
                        let (view, id) = (view.clone(), theme.id.clone());
                        h_flex()
                            .gap_2()
                            .child(theme_swatches(&theme))
                            .when(is_custom, |row| {
                                let (view, id) = (view.clone(), id.clone());
                                row.child(trash_button(
                                    format!("trash-theme-{id}"),
                                    t!("settings.tip.delete_theme"),
                                    move |window, cx| {
                                        view.update(cx, |this, cx| {
                                            this.delete_named_theme(id.clone(), window, cx)
                                        })
                                    },
                                ))
                            })
                            .child(action_button(
                                format!("preview-theme-{id}"),
                                t!("settings.preview"),
                                move |window, cx| {
                                    view.update(cx, |this, cx| {
                                        this.open_theme_preview(id.clone(), window, cx)
                                    })
                                },
                            ))
                    }),
                )
                .description(tag.to_string())
            })
            .collect();

        SettingPage::new(t!("settings.tab.themes"))
            .icon(Lucide::SwatchBook)
            .groups(
                Section::new().item(
                    SettingItem::new(
                        t!("settings.default_theme_title"),
                        dropdown(
                            build_theme_opts(&self.themes_store),
                            true,
                            read(view, |st| {
                                if st.default_theme_id.is_empty() {
                                    SharedString::from(themes::DEFAULT_THEME_ID)
                                } else {
                                    SharedString::from(st.default_theme_id.clone())
                                }
                            }),
                            write(view, |st, v: SharedString| {
                                st.default_theme_id = v.to_string()
                            }),
                        ),
                    )
                    .description(t!("settings.default_theme_desc").to_string())
                    .keywords(["theme palette ansi colour color terminal viewport"]),
                ),
            )
            .groups(
                Section::new()
                    .title(t!("settings.installed_themes_title"))
                    .footer({
                        let view = view.clone();
                        move |_, _| {
                            let view = view.clone();
                            action_button(
                                "import-theme",
                                t!("settings.import_itermcolors"),
                                move |w, cx| {
                                    view.update(cx, |this, cx| this.start_theme_import(w, cx))
                                },
                            )
                        }
                    })
                    .items(theme_rows),
            )
    }

    fn shortcuts_page(&self, view: &Entity<Self>) -> SettingPage {
        let rows = SHORTCUT_ACTIONS.iter().map(|&action| {
            let (cell_view, dirty_view, reset_view) = (view.clone(), view.clone(), view.clone());
            SettingItem::new(
                shortcut_label(action),
                SettingField::render(move |_, window, cx| {
                    shortcut_cell(&cell_view, action, window, cx)
                })
                .on_reset(
                    move |cx| {
                        dirty_view
                            .read(cx)
                            .settings
                            .shortcuts
                            .as_ref()
                            .is_some_and(|o| o.contains_key(action))
                    },
                    move |_, cx| reset_view.update(cx, |this, cx| this.reset_shortcut(action, cx)),
                ),
            )
            .keywords(["binding key hotkey shortcut combo capture"])
        });
        SettingPage::new(t!("settings.tab.shortcuts"))
            .icon(Lucide::Keyboard)
            .resettable(true)
            .groups(
                Section::new()
                    .title(t!("settings.keyboard_shortcuts_title"))
                    .description(t!("settings.keyboard_shortcuts_desc"))
                    .items(rows),
            )
    }

    fn highlighting_page(&self, view: &Entity<Self>) -> SettingPage {
        let rows: Vec<SettingItem> = self
            .highlight_store
            .list()
            .into_iter()
            .map(|p| {
                let is_custom = p.source == "user" || p.source == "import";
                let label = if is_custom {
                    format!("{} (custom)", p.name)
                } else {
                    p.name.clone()
                };
                let desc = p.description.clone().filter(|d| !d.is_empty());
                let (get_view, set_view, id) = (view.clone(), view.clone(), p.id.clone());
                let toggle = SettingField::switch(
                    {
                        let id = id.clone();
                        move |cx| get_view.read(cx).enabled_packs().contains(&id)
                    },
                    {
                        let id = id.clone();
                        move |on, cx| {
                            set_view.update(cx, |this, cx| {
                                this.toggle_highlight_pack(id.clone(), on, cx)
                            })
                        }
                    },
                );
                let item = if is_custom {
                    // Custom packs get a trash button beside the switch,
                    // so the field becomes a small rendered row.
                    let view = view.clone();
                    SettingItem::new(
                        label,
                        SettingField::render(move |_, _, cx| {
                            let on = view.read(cx).enabled_packs().contains(&id);
                            let (toggle_view, trash_view, id) =
                                (view.clone(), view.clone(), id.clone());
                            let trash_id = id.clone();
                            h_flex()
                                .gap_2()
                                .child(trash_button(
                                    format!("trash-pack-{id}"),
                                    t!("settings.tip.delete_pack"),
                                    move |window, cx| {
                                        trash_view.update(cx, |this, cx| {
                                            this.delete_highlight_pack(trash_id.clone(), window, cx)
                                        })
                                    },
                                ))
                                .child(
                                    Switch::new(SharedString::from(format!("pack-{id}")))
                                        .checked(on)
                                        .on_click(move |on: &bool, _, cx| {
                                            toggle_view.update(cx, |this, cx| {
                                                this.toggle_highlight_pack(id.clone(), *on, cx)
                                            })
                                        }),
                                )
                        }),
                    )
                } else {
                    SettingItem::new(label, toggle)
                };
                match desc {
                    Some(d) => item.description(d),
                    None => item,
                }
                .keywords(["highlight pack rule regex syntax colour color vendor"])
            })
            .collect();

        SettingPage::new(t!("settings.tab.highlighting"))
            .icon(Lucide::Highlighter)
            .groups(
                Section::new()
                    .title(t!("settings.highlight_packs_title"))
                    .description(t!("settings.highlight_packs_desc"))
                    .footer({
                        let view = view.clone();
                        move |_, _| {
                            let view = view.clone();
                            action_button(
                                "import-highlight",
                                t!("settings.import_pack"),
                                move |w, cx| {
                                    view.update(cx, |this, cx| this.start_highlight_import(w, cx))
                                },
                            )
                        }
                    })
                    .items(rows),
            )
    }

    /// Current version, latest-from-GitHub status, and the knobs that
    /// gate the boot-time check. The page title carries an amber dot
    /// while an undismissed release is available.
    fn updates_page(&self, view: &Entity<Self>, s: SkinTokens, cx: &App) -> SettingPage {
        let available = cx.global::<crate::updater::UpdateState>().available.clone();
        let cur = &self.settings;
        let dismissed_match = available
            .as_ref()
            .zip(cur.dismissed_update_version.as_deref())
            .is_some_and(|(a, d)| d == a.version);
        let pending = available.is_some() && !dismissed_match;

        // Three states: checks off (grey), newer release (amber, or
        // grey once dismissed), otherwise up to date (green).
        let (dot, status): (u32, SharedString) = if cur.disable_update_check {
            (s.fg_tertiary, t!("settings.updates.checks_disabled").into())
        } else if let Some(a) = available.as_ref() {
            if dismissed_match {
                (
                    s.fg_tertiary,
                    t!("settings.updates.available_dismissed", version = a.version).into(),
                )
            } else {
                (
                    s.warn,
                    t!("settings.updates.available", version = a.version).into(),
                )
            }
        } else {
            (s.success, t!("settings.updates.up_to_date").into())
        };
        let checks_on = !cur.disable_update_check;
        let status_view = view.clone();
        let status_item = SettingItem::render(move |_, _, cx| {
            let notes = available.as_ref().filter(|_| !dismissed_match).map(|a| {
                // First ~600 chars — enough for the headline.
                let raw = a.notes.trim();
                let mut display: String = raw.chars().take(600).collect();
                if raw.chars().count() > 600 {
                    display.push('…');
                }
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .whitespace_normal()
                    .child(display)
            });
            let actions = available.clone().filter(|_| pending && checks_on).map(|a| {
                let (url, version, view) = (a.html_url, a.version, status_view.clone());
                h_flex()
                    .gap_2()
                    .child(action_button(
                        "updates-view-release",
                        t!("settings.updates.view_release"),
                        move |_, cx| cx.open_url(&url),
                    ))
                    .child(action_button(
                        "updates-dismiss",
                        t!("settings.updates.dismiss"),
                        move |_, cx| {
                            view.update(cx, |this, cx| this.dismiss_update(version.clone(), cx))
                        },
                    ))
            });
            v_flex()
                .gap_2()
                .child(status_line(dot, status.clone()))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(t!(
                            "settings.updates.current_version",
                            version = env!("CARGO_PKG_VERSION")
                        )),
                )
                .children(notes)
                .children(actions)
        })
        .keywords(["update github release version check changelog new available"]);

        let mut page = SettingPage::new(t!("settings.tab.updates")).icon(IconName::RefreshCw);
        if pending {
            page =
                page.title_suffix(move |_, _| div().size(px(8.)).rounded_full().bg(rgba(s.warn)));
        }
        page.groups(
            Section::new()
                .title(t!("settings.updates_title"))
                .description(t!("settings.updates_desc"))
                .item(status_item),
        )
        .groups(
            Section::new()
                .title(t!("settings.update_prefs_title"))
                .description(t!("settings.update_prefs_desc"))
                .item(SettingItem::new(
                    t!("settings.updates.check_on_launch"),
                    switch(view, |st| !st.disable_update_check, Self::set_check_updates),
                ))
                .item(
                    SettingItem::new(
                        t!("settings.updates.include_prerelease"),
                        switch(
                            view,
                            |st| st.include_prerelease_updates,
                            Self::set_include_prerelease,
                        ),
                    )
                    .keywords(["prerelease alpha beta rc"]),
                ),
        )
    }

    fn advanced_page(&self, view: &Entity<Self>) -> SettingPage {
        let log_view = view.clone();
        let config_view = view.clone();
        let window_view = view.clone();
        SettingPage::new(t!("settings.tab.advanced"))
            .icon(Lucide::SlidersHorizontal)
            .groups(
                Section::new()
                    .item(
                        SettingItem::new(
                            t!("settings.log_dir_title"),
                            // Keeps its own InputState: the path commits on
                            // blur / Enter, not on every keystroke.
                            SettingField::render(move |_, _, cx| {
                                let input = log_view.read(cx).log_dir.clone();
                                let (choose, reset) = (log_view.clone(), log_view.clone());
                                h_flex()
                                    .gap_2()
                                    .w(px(360.))
                                    .child(div().flex_1().child(Input::new(&input).small()))
                                    .child(action_button(
                                        "log-dir-choose",
                                        t!("settings.choose"),
                                        move |w, cx| {
                                            choose.update(cx, |this, cx| this.choose_log_dir(w, cx))
                                        },
                                    ))
                                    .child(action_button(
                                        "log-dir-reset",
                                        t!("settings.reset"),
                                        move |w, cx| {
                                            reset.update(cx, |this, cx| this.reset_log_dir(w, cx))
                                        },
                                    ))
                            }),
                        )
                        .description(t!("settings.log_dir_desc").to_string())
                        .layout(Axis::Vertical)
                        .keywords(["log directory folder record session save path file"]),
                    )
                    .item(
                        SettingItem::new(
                            t!("settings.usb_driver_title"),
                            switch(
                                view,
                                |st| !st.disable_driver_detection,
                                Self::set_detect_drivers,
                            ),
                        )
                        .description(t!("settings.usb_driver_desc").to_string())
                        .keywords(["usb driver detection cp210x ftdi pl2303 ch340 adapter"]),
                    )
                    .item(
                        SettingItem::new(
                            t!("settings.copy_on_select_title"),
                            switch(view, |st| st.copy_on_select, Self::set_copy_on_select),
                        )
                        .description(t!("settings.copy_on_select_desc").to_string())
                        .keywords(["copy clipboard selection putty mouse drag"]),
                    ),
            )
            .groups(
                Section::new()
                    .title(t!("settings.window_state_title"))
                    .description(t!("settings.window_state_desc"))
                    .item(
                        SettingItem::new(
                            t!("settings.window_state_toggle"),
                            switch(
                                view,
                                |st| !st.disable_window_state_restore,
                                Self::set_restore_window_state,
                            ),
                        )
                        .keywords(["window position size remember restore geometry"]),
                    )
                    // Two resets so the user can fix just the window
                    // that drifted.
                    .item(SettingItem::render(move |_, _, _| {
                        let (settings_w, main_w) = (window_view.clone(), window_view.clone());
                        h_flex()
                            .gap_2()
                            .child(action_button(
                                "reset-settings-window",
                                t!("settings.reset_settings_window"),
                                move |w, cx| {
                                    settings_w
                                        .update(cx, |this, cx| this.reset_settings_window(w, cx))
                                },
                            ))
                            .child(action_button(
                                "reset-main-window",
                                t!("settings.reset_main_window"),
                                move |w, cx| {
                                    main_w.update(cx, |this, cx| this.reset_main_window(w, cx))
                                },
                            ))
                    })),
            )
            .groups(
                Section::new()
                    .title(t!("settings.config_dir_title"))
                    .description(t!("settings.config_dir_desc"))
                    .item(
                        SettingItem::render(move |_, _, cx| {
                            let path = config_view.read(cx).config_dir_display.clone();
                            let (reveal, choose, reset) = (
                                config_view.clone(),
                                config_view.clone(),
                                config_view.clone(),
                            );
                            h_flex()
                                .gap_2()
                                // Single-line path that keeps the meaningful
                                // tail ("…Application Support/Baudrun").
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis_start()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(path),
                                )
                                .child(action_button(
                                    "config-dir-reveal",
                                    t!("settings.reveal"),
                                    move |w, cx| {
                                        reveal.update(cx, |this, cx| this.reveal_config_dir(w, cx))
                                    },
                                ))
                                .child(action_button(
                                    "config-dir-choose",
                                    t!("settings.choose"),
                                    move |w, cx| {
                                        choose.update(cx, |this, cx| this.choose_config_dir(w, cx))
                                    },
                                ))
                                .child(action_button(
                                    "config-dir-reset",
                                    t!("settings.reset"),
                                    move |w, cx| {
                                        reset.update(cx, |this, cx| this.reset_config_dir(w, cx))
                                    },
                                ))
                        })
                        .keywords([
                            "config directory folder path location dropbox icloud portable xdg",
                        ]),
                    ),
            )
    }
}

/// Read-only summary of the OS reduce-motion preference Baudrun
/// honours. Driven by the host OS rather than `settings.json`; the
/// page just makes the contract visible (and searchable).
fn accessibility_page(s: SkinTokens, cx: &App) -> SettingPage {
    let reduce_motion = cx.global::<crate::ReduceMotion>().0;
    let (dot, status): (u32, SharedString) = if reduce_motion {
        (s.success, t!("settings.reduce_motion.status_on").into())
    } else {
        (
            s.fg_tertiary,
            t!("settings.reduce_motion.status_off").into(),
        )
    };
    let os_path = if cfg!(target_os = "macos") {
        t!("settings.reduce_motion.path_macos")
    } else if cfg!(target_os = "windows") {
        t!("settings.reduce_motion.path_windows")
    } else {
        t!("settings.reduce_motion.path_other")
    };
    SettingPage::new(t!("settings.tab.accessibility"))
        .icon(Lucide::Accessibility)
        .groups(
            Section::new()
                .title(t!("settings.reduce_motion_title"))
                .description(t!("settings.reduce_motion_desc"))
                .item(
                    SettingItem::render(move |_, _, cx| {
                        v_flex()
                            .gap_2()
                            .child(status_line(dot, status.clone()))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(os_path.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t!("settings.reduce_motion.stale_note")),
                            )
                    })
                    .keywords([
                        "reduce motion animation pulse blink cursor accessibility a11y wcag",
                    ]),
                ),
        )
}

fn opt(id: &str, title: impl Into<SharedString>) -> (SharedString, SharedString) {
    (SharedString::from(id.to_string()), title.into())
}

type FooterFn = Rc<dyn Fn(&mut Window, &mut App) -> gpui::AnyElement>;

/// `SettingGroup` builder that emits one outlined group per item, so
/// each control stays visibly tied to its label while scrolling. The
/// section title and description sit above the first box (keeping the
/// section in the page sidebar); the footer sits under the last.
#[derive(Default)]
struct Section {
    title: Option<SharedString>,
    description: Option<SharedString>,
    footer: Option<FooterFn>,
    items: Vec<SettingItem>,
}

impl Section {
    fn new() -> Self {
        Self::default()
    }

    fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    fn footer<E: IntoElement>(
        mut self,
        footer: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.footer = Some(Rc::new(move |window, cx| {
            footer(window, cx).into_any_element()
        }));
        self
    }

    fn item(mut self, item: SettingItem) -> Self {
        self.items.push(item);
        self
    }

    fn items(mut self, items: impl IntoIterator<Item = SettingItem>) -> Self {
        self.items.extend(items);
        self
    }
}

impl IntoIterator for Section {
    type Item = SettingGroup;
    type IntoIter = std::vec::IntoIter<SettingGroup>;

    fn into_iter(self) -> Self::IntoIter {
        let last = self.items.len().saturating_sub(1);
        let groups: Vec<SettingGroup> = self
            .items
            .into_iter()
            .enumerate()
            .map(|(ix, item)| {
                // Right padding keeps the box border clear of the
                // page's overlay scrollbar.
                let mut group = SettingGroup::new()
                    .variant(GroupBoxVariant::Outline)
                    .pr_3()
                    .item(item);
                if ix == 0 {
                    if let Some(title) = self.title.clone() {
                        group = group.title(title);
                    }
                    if let Some(description) = self.description.clone() {
                        group = group.description(description);
                    }
                }
                if ix == last {
                    if let Some(footer) = self.footer.clone() {
                        group = group.footer(move |window, cx| footer(window, cx));
                    }
                }
                group
            })
            .collect();
        groups.into_iter()
    }
}

/// GPUI Kit's dropdown field anchors its menu to the button's right
/// edge; this is the same button + menu, opening down from its left.
fn dropdown(
    options: Vec<(SharedString, SharedString)>,
    scrollable: bool,
    get: impl Fn(&App) -> SharedString + 'static,
    set: impl Fn(SharedString, &mut App) + 'static,
) -> SettingField<SharedString> {
    let (options, set) = (Rc::new(options), Rc::new(set));
    SettingField::render(move |opts, _, cx| {
        let current = get(cx);
        let label = options
            .iter()
            .find(|(value, _)| *value == current)
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| current.clone());
        let (options, set) = (options.clone(), set.clone());
        Button::new("dropdown")
            .when(opts.layout() == Axis::Vertical, |b| b.w_full())
            .label(label)
            .dropdown_caret(true)
            .outline()
            .with_size(opts.size())
            .dropdown_menu_with_anchor(Anchor::TopLeft, move |menu, _, _| {
                options
                    .iter()
                    .fold(menu, |menu, (value, label)| {
                        let (value, set) = (value.clone(), set.clone());
                        menu.item(
                            PopupMenuItem::new(label.clone())
                                .checked(value == current)
                                .on_click(move |_, _, cx| set(value.clone(), cx)),
                        )
                    })
                    .scrollable(scrollable)
            })
    })
}

/// Getter for a typed setting field: reads the view's cached Settings.
fn read<T: 'static>(
    view: &Entity<SettingsView>,
    f: fn(&Settings) -> T,
) -> impl Fn(&App) -> T + 'static {
    let view = view.clone();
    move |cx| f(&view.read(cx).settings)
}

/// Setter for a typed setting field: applies `f` to a copy of the
/// Settings and commits it (persist + cross-window broadcast).
fn write<T: 'static>(
    view: &Entity<SettingsView>,
    f: fn(&mut Settings, T),
) -> impl Fn(T, &mut App) + 'static {
    let view = view.clone();
    move |v, cx| {
        view.update(cx, |this, cx| {
            let mut next = this.settings.clone();
            f(&mut next, v);
            this.commit(next, cx);
        })
    }
}

/// One of the view's own `InputState`s as a setting field. The view's
/// subscription commits it, so it isn't written on every keystroke.
fn text_input(
    view: &Entity<SettingsView>,
    input: fn(&SettingsView) -> Entity<InputState>,
) -> SettingField<SharedString> {
    let view = view.clone();
    // Fixed width: an empty input (font size left at its default)
    // otherwise shrinks to a sliver and hides its placeholder.
    SettingField::render(move |_, _, cx| Input::new(&input(view.read(cx))).small().w(px(120.0)))
}

/// Switch bound to one of the view's existing `set_*` handlers.
fn switch(
    view: &Entity<SettingsView>,
    get: fn(&Settings) -> bool,
    set: fn(&mut SettingsView, bool, &mut Context<SettingsView>),
) -> SettingField<bool> {
    let set_view = view.clone();
    SettingField::switch(read(view, get), move |on, cx| {
        set_view.update(cx, |this, cx| set(this, on, cx))
    })
}

/// The binding cell of one Shortcuts row: the current combo as a `Kbd`
/// (or "—"), or, while capturing, a focused "Press a key…" cell that
/// eats the next key press.
fn shortcut_cell(
    view: &Entity<SettingsView>,
    action: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    let this = view.read(cx);
    if this.capturing_shortcut == Some(action) {
        return div()
            .min_w(px(150.0))
            .px_3()
            .py(px(6.0))
            .rounded_md()
            .border_1()
            .border_color(cx.theme().primary)
            .text_size(px(12.0))
            .track_focus(&this.capture_focus)
            .on_key_down(window.listener_for(view, SettingsView::handle_capture_key))
            .child(t!("settings.capture_prompt"))
            .into_any_element();
    }
    let overrides = this.settings.shortcuts.clone().unwrap_or_default();
    let is_overridden = overrides.contains_key(action);
    let label: gpui::AnyElement = match parse_spec(&effective_shortcut(action, &overrides)) {
        Some(stroke) => Kbd::new(stroke).appearance(true).into_any_element(),
        None => div().child("\u{2014}").into_any_element(),
    };
    let (capture_view, reset_view) = (view.clone(), view.clone());
    h_flex()
        .gap_1()
        .child(
            Button::new(SharedString::from(format!("shortcut-capture-{action}")))
                .outline()
                .small()
                .min_w(px(150.0))
                .child(label)
                .on_click(move |_, window, cx| {
                    capture_view.update(cx, |this, cx| this.start_capture(action, window, cx))
                }),
        )
        // Per-row reset (the page header's reset clears every row).
        .when(is_overridden, |row| {
            row.child(
                Button::new(SharedString::from(format!("shortcut-reset-{action}")))
                    .ghost()
                    .small()
                    .icon(IconName::Undo2)
                    .tooltip(t!("settings.reset"))
                    .on_click(move |_, _, cx| {
                        reset_view.update(cx, |this, cx| this.reset_shortcut(action, cx))
                    }),
            )
        })
        .into_any_element()
}

/// Coloured status dot + label, shared by the Updates and
/// Accessibility status rows.
fn status_line(dot: u32, label: SharedString) -> impl IntoElement {
    h_flex()
        // Dot pins to the first line when the label wraps.
        .items_start()
        .gap_2()
        .child(
            div()
                .mt(px(5.0))
                .size(px(8.0))
                .flex_shrink_0()
                .rounded_full()
                .bg(rgba(dot)),
        )
        .child(div().flex_1().min_w_0().text_sm().child(label))
}
