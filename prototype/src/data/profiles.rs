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
