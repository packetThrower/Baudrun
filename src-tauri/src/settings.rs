//! Global settings store — theme/skin defaults, font size, log
//! directory, appearance overrides, scrollback size, editable
//! keyboard shortcuts. Persists as a single JSON file. Defaults
//! match the Go Wails version so existing `settings.json` files
//! round-trip without change.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default)]
    pub default_theme_id: String,

    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub font_size: i32,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub log_dir: String,

    #[serde(default, skip_serializing_if = "is_false")]
    pub disable_driver_detection: bool,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub skin_id: String,

    /// "auto" (follow system), "light", or "dark".
    /// Empty / missing is treated as "auto" by consumers.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub appearance: String,

    /// PuTTY-style auto-copy on mouse selection release.
    #[serde(default, skip_serializing_if = "is_false")]
    pub copy_on_select: bool,

    /// xterm.js screen-reader mode (ARIA live region for incoming
    /// output). Small perf cost on heavy output; off by default.
    #[serde(default, skip_serializing_if = "is_false")]
    pub screen_reader_mode: bool,

    /// xterm scrollback lines. Default 10000.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub scrollback_lines: i32,

    /// Keyboard-shortcut overrides keyed by action id, values are
    /// W3C KeyboardEvent modifier+key strings (`"Meta+K"`). Unset
    /// falls back to a platform-appropriate default picked by the
    /// frontend — None here does NOT mean "disabled".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcuts: Option<HashMap<String, String>>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            default_theme_id: "baudrun".into(),
            font_size: 13,
            log_dir: String::new(),
            disable_driver_detection: false,
            skin_id: "baudrun".into(),
            appearance: "auto".into(),
            copy_on_select: false,
            screen_reader_mode: false,
            scrollback_lines: 10_000,
            shortcuts: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("serialize settings: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, SettingsError>;

pub struct Store {
    path: PathBuf,
    inner: RwLock<Settings>,
}

impl Store {
    pub fn new(support_dir: &Path) -> Result<Self> {
        fs::create_dir_all(support_dir)?;
        let path = support_dir.join("settings.json");
        let settings = load(&path)?;
        Ok(Store {
            path,
            inner: RwLock::new(settings),
        })
    }

    pub fn get(&self) -> Settings {
        self.inner.read().unwrap().clone()
    }

    pub fn update(&self, new_settings: Settings) -> Result<Settings> {
        let mut guard = self.inner.write().unwrap();
        let prev = guard.clone();
        *guard = new_settings;
        if let Err(err) = save(&self.path, &guard) {
            *guard = prev;
            return Err(err);
        }
        Ok(guard.clone())
    }
}

fn load(path: &Path) -> Result<Settings> {
    match fs::read(path) {
        Ok(data) if data.is_empty() => Ok(Settings::default()),
        Ok(data) => Ok(serde_json::from_slice(&data)?),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(err) => Err(err.into()),
    }
}

fn save(path: &Path, s: &Settings) -> Result<()> {
    let data = serde_json::to_vec_pretty(s)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn is_false(v: &bool) -> bool {
    !v
}

fn is_zero_i32(v: &i32) -> bool {
    *v == 0
}
