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
    MouseUpEvent, Render, SharedString, Subscription, Window,
};
use gpui_component::{
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState},
    select::{Select, SelectEvent, SelectItem, SelectState},
    IndexPath, Root, Sizable,
};

use crate::data::highlight;
use crate::data::settings::{self, Settings};
use crate::data::skins;
use crate::data::themes;

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
    settings_store: Rc<settings::Store>,
    #[allow(dead_code)] // re-read on render via the entity cache; kept
                        // around for live-apply (skin import etc.)
    skins_store: Rc<skins::Store>,
    /// Highlight pack store. Re-read on every render of the
    /// Highlighting pane so the toggle list reflects packs the user
    /// imported in the current session (post-launch import refreshes
    /// without restarting Settings).
    highlight_store: Rc<highlight::Store>,
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
        settings_store: Rc<settings::Store>,
        skins_store: Rc<skins::Store>,
        highlight_store: Rc<highlight::Store>,
        themes_store: Rc<themes::Store>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let current = settings_store.get();

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
            settings_store,
            skins_store,
            highlight_store,
            themes_store,
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
            SettingsTab::Appearance => self.appearance_pane().into_any_element(),
            SettingsTab::Themes => self.themes_pane().into_any_element(),
            SettingsTab::Highlighting => self.highlighting_pane(cx).into_any_element(),
            SettingsTab::Advanced => self.advanced_pane(cx).into_any_element(),
            other => placeholder_pane(other.label()).into_any_element(),
        }
    }

    fn themes_pane(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(section_card_with_desc(
                "Default Theme",
                Some(
                    "Terminal palette new sessions start with — affects \
                     the 16 ANSI colours, default foreground/background, \
                     selection, and cursor. Per-profile overrides go on \
                     the profile form. Built-in themes ship with the \
                     app; user themes load from $SUPPORT_DIR/themes/.",
                ),
                Select::new(&self.theme_select).small(),
            ))
    }

    fn highlighting_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = self.enabled_packs();
        let packs = self.highlight_store.list();
        let rows = packs.into_iter().map(|p| {
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
                        .text_color(rgba(FG_SECONDARY))
                        .whitespace_normal()
                        .child(desc),
                )
        });

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(section_card_with_desc(
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

    fn appearance_pane(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(section_card_with_desc(
                "App Skin",
                Some(
                    "How the app chrome (sidebar, panes, buttons) looks. \
                     Doesn't affect the terminal palette — that's the \
                     Themes tab. Imported user skins live next to the \
                     built-ins after a launch.",
                ),
                Select::new(&self.skin_select).small(),
            ))
            .child(section_card_with_desc(
                "Appearance",
                Some(
                    "Light or dark variant of the active skin. \"Auto\" \
                     follows the system setting. Skins flagged dark-only \
                     ignore this and pin dark.",
                ),
                Select::new(&self.appearance_select).small(),
            ))
            .child(section_card_with_desc(
                "Terminal Font Size",
                Some(
                    "Pixel size of the terminal pane's monospace font. \
                     Leave blank to use the default (13). Changes the \
                     terminal grid only — chrome text size is fixed.",
                ),
                Input::new(&self.font_size).small().appearance(true),
            ))
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
