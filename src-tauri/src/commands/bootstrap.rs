//! Single-IPC bootstrap helper for new windows.
//!
//! Each Tauri window's renderer runs `App.svelte` and on mount used
//! to fire 8 separate IPCs to populate its local stores: list_profiles,
//! list_themes, list_skins, get_settings, list_highlight_packs,
//! default_log_directory, get_config_directory, get_default_config_directory.
//!
//! On Windows each `ipc.localhost` call routes through WebView2's
//! URL loader + Tauri's protocol handler — single-digit-ms latency
//! per call individually, but enough to add up. More importantly,
//! per-call latency competes with the main window's IPC traffic
//! (e.g. its sidebar refresh after a session migration), so a slow
//! sibling command can tail-block these otherwise-trivial reads.
//! v0.9.5-alpha.x trace data showed exactly this on Windows: a
//! profile-window open stalled ~500ms after Promise.all because
//! the main window's `list_ports` was still in flight, gating the
//! response delivery thread for the new window's follow-up calls.
//!
//! Collapsing all 8 reads into one IPC removes that exposure: the
//! response is a single dispatch, with one URL-loader request and
//! one protocol-handler hop. The Settings / dir state is all
//! cheap reads from `AppState` (HashMap clones, atomic loads), so
//! the merged payload doesn't add measurable backend cost.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::appdata;
use crate::highlight::HighlightPack;
use crate::profiles::Profile;
use crate::settings::Settings;
use crate::skins::Skin;
use crate::state::AppState;
use crate::themes::Theme;

/// Snapshot of every piece of state a fresh window needs at mount.
/// Ordering matches the original onMount sequence in `App.svelte` so
/// the diff stays readable. Field names use camelCase on the wire to
/// match what the JS stores already expect.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapPayload {
    pub profiles: Vec<Profile>,
    pub themes: Vec<Theme>,
    pub skins: Vec<Skin>,
    pub settings: Settings,
    pub highlight_packs: Vec<HighlightPack>,
    pub default_log_dir: String,
    pub config_dir: String,
    pub default_config_dir: String,
}

/// Build a `BootstrapPayload` for the calling window. All reads are
/// O(n) over already-loaded in-memory collections, so the command
/// returns synchronously without touching disk; running it through
/// the blocking pool would add latency for no benefit.
#[tauri::command]
pub fn bootstrap_window_state(
    state: State<'_, Arc<AppState>>,
) -> Result<BootstrapPayload, String> {
    let support = appdata::support_dir().map_err(|e| e.to_string())?;
    let default_support = appdata::default_support_dir().map_err(|e| e.to_string())?;

    Ok(BootstrapPayload {
        profiles: state.profiles.list(),
        themes: state.themes.list(),
        skins: state.skins.list(),
        settings: state.settings.get(),
        highlight_packs: state.highlight.list(),
        default_log_dir: support.join("logs").to_string_lossy().into_owned(),
        config_dir: support.to_string_lossy().into_owned(),
        default_config_dir: default_support.to_string_lossy().into_owned(),
    })
}
