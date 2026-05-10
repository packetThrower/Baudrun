//! App-wide observable Settings entity. Wraps `data::settings::Store`
//! so writes go through one place that both persists and emits an
//! `Updated` event — anything subscribed (the main `AppView`, the
//! standalone Settings window, future menus) gets a notify on change.
//!
//! Cross-window propagation is the whole reason this exists: gpui
//! entities live at the App level (not per-window), so the same
//! Entity<SettingsBus> handed to two windows lets one window write
//! and the other re-render without us reaching across raw window
//! boundaries.

use std::rc::Rc;

use gpui::{Context, EventEmitter};

use crate::data::settings::{self, Settings};

/// Single broadcast event. Carries the post-save Settings snapshot
/// so subscribers don't have to re-read the store. The store value
/// and the snapshot agree by construction (we only emit after
/// `Store::update` succeeded).
#[derive(Clone)]
pub enum SettingsEvent {
    Updated(Settings),
}

pub struct SettingsBus {
    store: Rc<settings::Store>,
    /// Last successfully-saved value. Reads from here are O(1) and
    /// don't lock the store's RwLock; the store is the source of
    /// truth on disk while this field is the source of truth for
    /// live UI reads.
    cached: Settings,
}

impl SettingsBus {
    pub fn new(store: Rc<settings::Store>) -> Self {
        let cached = store.get();
        Self { store, cached }
    }

    pub fn current(&self) -> &Settings {
        &self.cached
    }

    /// Persist `next` through the store and notify subscribers on
    /// success. On failure (disk full, perm denied, …) the cache
    /// stays put, no event fires, and the error bubbles up so
    /// callers can surface it. Mirrors the `commit` semantics
    /// SettingsView used to own internally.
    pub fn replace(
        &mut self,
        next: Settings,
        cx: &mut Context<Self>,
    ) -> Result<(), settings::SettingsError> {
        let saved = self.store.update(next)?;
        self.cached = saved.clone();
        cx.emit(SettingsEvent::Updated(saved));
        cx.notify();
        Ok(())
    }
}

impl EventEmitter<SettingsEvent> for SettingsBus {}
