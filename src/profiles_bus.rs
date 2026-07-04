//! App-wide "profiles changed" broadcast. `profiles::Store` is a
//! plain `Rc<RwLock<…>>` with no observability, so a profile
//! create / edit / delete / reorder in one window used to leave
//! every other window's sidebar stale until something else
//! happened to re-render it. This is the profile-side analogue of
//! `SettingsBus`, pared down to a pure signal (zorite's
//! `DocSignal` pattern): the entity carries no state — render
//! paths already re-read `profile_store.list()` fresh each frame,
//! so subscribers only need a `cx.notify()` kick, not a payload.
//!
//! Handed around via the `GlobalProfilesBus` global (gpui entities
//! live at App scope, so one entity serves every window) rather
//! than threading another parameter through `open_app_window`.

use gpui::{Entity, EventEmitter, Global};

/// The (payload-free) event. Fired after any successful mutation
/// of the profile store.
pub struct ProfilesChanged;

/// Zero-state entity that exists only to be subscribed to.
pub struct ProfilesBus;

impl EventEmitter<ProfilesChanged> for ProfilesBus {}

/// App-scoped handle to the single ProfilesBus entity. Installed
/// by `main::run` before the first window opens; read by
/// `AppView::new` to subscribe.
pub struct GlobalProfilesBus(pub Entity<ProfilesBus>);

impl Global for GlobalProfilesBus {}
