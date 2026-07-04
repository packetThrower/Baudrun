//! JSON-backed profile store. Each profile captures every serial
//! parameter the user needs plus the ergonomic knobs around a session
//! (highlight, logging, paste safety, auto-reconnect).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub port_name: String,
    pub baud_rate: i32,
    pub data_bits: i32,
    pub parity: String,
    pub stop_bits: String,
    pub flow_control: String,
    pub line_ending: String,
    pub local_echo: bool,
    pub highlight: bool,
    pub theme_id: String,
    pub dtr_on_connect: String,
    pub rts_on_connect: String,
    pub dtr_on_disconnect: String,
    pub rts_on_disconnect: String,
    pub hex_view: bool,
    pub timestamps: bool,
    /// Prefix every new line with a sequential line number
    /// (`   42  `) for cross-referencing scrollback against
    /// external notes. New since the Tauri version; serialises
    /// with `#[serde(default)]` so existing profile JSON loads
    /// without an explicit `false`.
    #[serde(default)]
    pub line_numbers: bool,
    pub log_enabled: bool,
    pub auto_reconnect: bool,
    pub backspace_key: String,
    pub paste_warn_multiline: bool,
    pub paste_slow: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_char_delay_ms: Option<i32>,
    /// Per-profile override for the global `enabled_highlight_presets`
    /// list in Settings. `None` means inherit; `Some(_)` (even an empty
    /// vec) replaces the global selection for this profile's session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled_highlight_presets: Option<Vec<String>>,
    #[serde(default = "now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "now")]
    pub updated_at: DateTime<Utc>,
}

fn now() -> DateTime<Utc> {
    Utc::now()
}

impl Profile {
    /// Defaults mirror `profiles.Defaults()` in the Go version —
    /// 9600-8N1 no flow control, CR line ending, paste safety on,
    /// auto-reconnect on.
    pub fn defaults() -> Self {
        Profile {
            id: String::new(),
            name: String::new(),
            port_name: String::new(),
            baud_rate: 9600,
            data_bits: 8,
            parity: "none".into(),
            stop_bits: "1".into(),
            flow_control: "none".into(),
            line_ending: "cr".into(),
            local_echo: false,
            highlight: true,
            theme_id: String::new(),
            dtr_on_connect: "default".into(),
            rts_on_connect: "default".into(),
            dtr_on_disconnect: "default".into(),
            rts_on_disconnect: "default".into(),
            hex_view: false,
            timestamps: false,
            line_numbers: false,
            log_enabled: false,
            auto_reconnect: true,
            backspace_key: "del".into(),
            paste_warn_multiline: true,
            paste_slow: true,
            paste_char_delay_ms: Some(10),
            enabled_highlight_presets: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("name required")]
    NameRequired,
    #[error("port required")]
    PortRequired,
    #[error("baud rate must be positive")]
    InvalidBaud,
    #[error("data bits must be 5-8")]
    InvalidDataBits,
    #[error("invalid parity: {0}")]
    InvalidParity(String),
    #[error("invalid stop bits: {0}")]
    InvalidStopBits(String),
    #[error("invalid flow control: {0}")]
    InvalidFlowControl(String),
    #[error("invalid line ending: {0}")]
    InvalidLineEnding(String),
    #[error("invalid {field}: {value}")]
    InvalidLinePolicy { field: &'static str, value: String },
    #[error("invalid backspaceKey: {0}")]
    InvalidBackspaceKey(String),
    #[error("profile id required")]
    IdRequired,
    #[error("profile {0} not found")]
    NotFound(String),
    #[error("profile {0} already exists")]
    IdConflict(String),
    #[error("serialize profiles: {0}")]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub type Result<T> = std::result::Result<T, ProfileError>;

pub struct Store {
    path: PathBuf,
    inner: RwLock<Vec<Profile>>,
}

impl Store {
    pub fn new(support_dir: &Path) -> Result<Self> {
        fs::create_dir_all(support_dir)?;
        let path = support_dir.join("profiles.json");
        let profiles = load(&path)?;
        Ok(Store {
            path,
            inner: RwLock::new(profiles),
        })
    }

    pub fn list(&self) -> Vec<Profile> {
        self.inner.read().unwrap().clone()
    }

    pub fn get(&self, id: &str) -> Option<Profile> {
        self.inner
            .read()
            .unwrap()
            .iter()
            .find(|p| p.id == id)
            .cloned()
    }

    pub fn create(&self, mut profile: Profile) -> Result<Profile> {
        validate(&profile)?;
        let mut guard = self.inner.write().unwrap();
        let now = Utc::now();
        profile.id = Uuid::new_v4().to_string();
        profile.created_at = now;
        profile.updated_at = now;
        guard.push(profile.clone());
        if let Err(err) = save(&self.path, &guard) {
            guard.pop();
            return Err(err);
        }
        Ok(profile)
    }

    pub fn update(&self, mut profile: Profile) -> Result<Profile> {
        if profile.id.is_empty() {
            return Err(ProfileError::IdRequired);
        }
        validate(&profile)?;
        let mut guard = self.inner.write().unwrap();
        let idx = guard
            .iter()
            .position(|p| p.id == profile.id)
            .ok_or_else(|| ProfileError::NotFound(profile.id.clone()))?;
        profile.created_at = guard[idx].created_at;
        profile.updated_at = Utc::now();
        let prev = std::mem::replace(&mut guard[idx], profile.clone());
        if let Err(err) = save(&self.path, &guard) {
            guard[idx] = prev;
            return Err(err);
        }
        Ok(profile)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let mut guard = self.inner.write().unwrap();
        let idx = guard
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| ProfileError::NotFound(id.to_string()))?;
        let prev = guard.remove(idx);
        if let Err(err) = save(&self.path, &guard) {
            guard.insert(idx, prev);
            return Err(err);
        }
        Ok(())
    }

    /// Move profile `id` so it sits immediately before the profile
    /// with id `before`; `None` moves it to the end of the list.
    /// profiles.json's array order IS the sidebar order, so saving
    /// the vec persists the ordering. Dropping a row onto itself
    /// (`before == Some(id)`) is a no-op; a `before` id that no
    /// longer exists (deleted mid-drag by another window) degrades
    /// to move-to-end rather than erroring.
    pub fn reorder(&self, id: &str, before: Option<&str>) -> Result<()> {
        if before == Some(id) {
            return Ok(());
        }
        let mut guard = self.inner.write().unwrap();
        let from = guard
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| ProfileError::NotFound(id.to_string()))?;
        let profile = guard.remove(from);
        let dest = match before {
            Some(b) => guard.iter().position(|p| p.id == b).unwrap_or(guard.len()),
            None => guard.len(),
        };
        guard.insert(dest, profile);
        if let Err(err) = save(&self.path, &guard) {
            // Undo the move so the in-memory order still matches
            // what's on disk.
            let profile = guard.remove(dest);
            guard.insert(from, profile);
            return Err(err);
        }
        Ok(())
    }

    /// Re-insert a previously-deleted profile, preserving its id +
    /// `created_at` + `updated_at`. Used by the Undo path on the
    /// session-header / sidebar profile-delete toast within the
    /// 5 s window after a delete. Errors if a profile with the
    /// same id already exists (would shadow either an in-flight
    /// new profile or a separately-restored copy).
    pub fn restore(&self, profile: Profile) -> Result<Profile> {
        if profile.id.is_empty() {
            return Err(ProfileError::IdRequired);
        }
        let mut guard = self.inner.write().unwrap();
        if guard.iter().any(|p| p.id == profile.id) {
            return Err(ProfileError::IdConflict(profile.id.clone()));
        }
        guard.push(profile.clone());
        if let Err(err) = save(&self.path, &guard) {
            guard.pop();
            return Err(err);
        }
        Ok(profile)
    }
}

fn load(path: &Path) -> Result<Vec<Profile>> {
    match fs::read(path) {
        Ok(data) if data.is_empty() => Ok(Vec::new()),
        Ok(data) => Ok(serde_json::from_slice(&data)?),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err.into()),
    }
}

fn save(path: &Path, profiles: &[Profile]) -> Result<()> {
    let data = serde_json::to_vec_pretty(profiles)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn validate(p: &Profile) -> Result<()> {
    if p.name.is_empty() {
        return Err(ProfileError::NameRequired);
    }
    if p.port_name.is_empty() {
        return Err(ProfileError::PortRequired);
    }
    if p.baud_rate <= 0 {
        return Err(ProfileError::InvalidBaud);
    }
    if !(5..=8).contains(&p.data_bits) {
        return Err(ProfileError::InvalidDataBits);
    }
    match p.parity.as_str() {
        "none" | "odd" | "even" | "mark" | "space" => {}
        other => return Err(ProfileError::InvalidParity(other.to_string())),
    }
    match p.stop_bits.as_str() {
        "1" | "1.5" | "2" => {}
        other => return Err(ProfileError::InvalidStopBits(other.to_string())),
    }
    match p.flow_control.as_str() {
        "none" | "rtscts" | "xonxoff" => {}
        other => return Err(ProfileError::InvalidFlowControl(other.to_string())),
    }
    match p.line_ending.as_str() {
        "cr" | "lf" | "crlf" => {}
        other => return Err(ProfileError::InvalidLineEnding(other.to_string())),
    }
    for (field, value) in [
        ("dtrOnConnect", &p.dtr_on_connect),
        ("rtsOnConnect", &p.rts_on_connect),
        ("dtrOnDisconnect", &p.dtr_on_disconnect),
        ("rtsOnDisconnect", &p.rts_on_disconnect),
    ] {
        match value.as_str() {
            "" | "default" | "assert" | "deassert" => {}
            other => {
                return Err(ProfileError::InvalidLinePolicy {
                    field,
                    value: other.to_string(),
                })
            }
        }
    }
    match p.backspace_key.as_str() {
        "" | "del" | "bs" => {}
        other => return Err(ProfileError::InvalidBackspaceKey(other.to_string())),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with(names: &[&str]) -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path()).unwrap();
        for n in names {
            let mut p = Profile::defaults();
            p.name = n.to_string();
            p.port_name = "/dev/null".into();
            store.create(p).unwrap();
        }
        (dir, store)
    }

    fn order(store: &Store) -> Vec<String> {
        store.list().into_iter().map(|p| p.name).collect()
    }

    #[test]
    fn reorder_moves_before_and_to_end() {
        let (_dir, store) = store_with(&["a", "b", "c"]);
        let ids: Vec<String> = store.list().into_iter().map(|p| p.id).collect();

        // c before a → [c, a, b]
        store.reorder(&ids[2], Some(&ids[0])).unwrap();
        assert_eq!(order(&store), ["c", "a", "b"]);

        // c to end → [a, b, c]
        store.reorder(&ids[2], None).unwrap();
        assert_eq!(order(&store), ["a", "b", "c"]);

        // self-drop is a no-op
        store.reorder(&ids[0], Some(&ids[0])).unwrap();
        assert_eq!(order(&store), ["a", "b", "c"]);

        // vanished `before` degrades to move-to-end
        store.reorder(&ids[0], Some("gone")).unwrap();
        assert_eq!(order(&store), ["b", "c", "a"]);

        // unknown moved id errors
        assert!(store.reorder("gone", None).is_err());

        // order survives a reload from disk
        let reloaded = Store::new(_dir.path()).unwrap();
        assert_eq!(order(&reloaded), ["b", "c", "a"]);
    }
}
