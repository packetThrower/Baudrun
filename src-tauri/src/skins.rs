//! App-chrome skin store — named sets of CSS custom-property values
//! the frontend applies to `document.documentElement`. Distinct from
//! terminal themes (see [`crate::themes`]) which only color the xterm
//! palette. 14 built-in skins are embedded at compile time as JSON;
//! user skins import from hand-authored JSON files and persist under
//! `$SUPPORT_DIR/skins/<id>.json`.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_SKIN_ID: &str = "baudrun";

/// A skin's `vars` are always applied; `dark_vars` overlay on top
/// when the app is in dark appearance, `light_vars` when in light.
/// `supports_light = false` means the skin is dark-only (CRT,
/// synthwave, etc.); the applier pins dark regardless of the user's
/// global appearance preference in that case.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
    pub id: String,
    pub name: String,
    /// "builtin" or "user"
    pub source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub vars: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub dark_vars: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub light_vars: HashMap<String, String>,
    pub supports_light: bool,
}

#[derive(Debug, Error)]
pub enum SkinError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("parse skin json: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("skin name required")]
    NameRequired,
    #[error("skin has no variables")]
    NoVars,
    #[error("skin var {0:?} must start with --")]
    BadVarName(String),
    #[error("user skin {0} not found")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, SkinError>;

pub struct Store {
    dir: PathBuf,
    user: RwLock<Vec<Skin>>,
}

impl Store {
    pub fn new(support_dir: &Path) -> Result<Self> {
        let dir = support_dir.join("skins");
        fs::create_dir_all(&dir)?;
        let user = load_user(&dir);
        Ok(Store {
            dir,
            user: RwLock::new(user),
        })
    }

    pub fn list(&self) -> Vec<Skin> {
        let user = self.user.read().unwrap();
        let mut out = Vec::with_capacity(builtins().len() + user.len());
        out.extend(builtins().iter().cloned());
        out.extend(user.iter().cloned());
        out
    }

    pub fn get(&self, id: &str) -> Option<Skin> {
        if let Some(s) = builtins().iter().find(|s| s.id == id) {
            return Some(s.clone());
        }
        self.user
            .read()
            .unwrap()
            .iter()
            .find(|s| s.id == id)
            .cloned()
    }

    pub fn resolve(&self, id: &str) -> Skin {
        self.get(id)
            .or_else(|| self.get(DEFAULT_SKIN_ID))
            .unwrap_or_else(|| builtins()[0].clone())
    }

    /// Read a user skin JSON from disk and persist it as a user
    /// import. Collisions with existing IDs get a `-2`, `-3`, ...
    /// suffix.
    pub fn import(&self, path: &Path) -> Result<Skin> {
        let data = fs::read(path)?;
        let mut skin: Skin = serde_json::from_slice(&data)?;
        validate(&skin)?;
        skin.source = "user".into();

        let mut user = self.user.write().unwrap();
        let base = if skin.id.is_empty() {
            let slug = crate::themes::slugify(&skin.name, "skin");
            skin.id = slug.clone();
            slug
        } else {
            skin.id.clone()
        };
        let mut suffix = 2;
        while id_exists(&user, &skin.id) {
            skin.id = format!("{}-{}", base, suffix);
            suffix += 1;
        }

        persist_user(&self.dir, &skin)?;
        user.push(skin.clone());
        Ok(skin)
    }

    /// Remove a user-imported skin. Builtins are immutable; unknown
    /// ids return `NotFound`.
    pub fn delete(&self, id: &str) -> Result<()> {
        let mut user = self.user.write().unwrap();
        let idx = user
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| SkinError::NotFound(id.to_string()))?;
        let _ = fs::remove_file(self.dir.join(format!("{}.json", id)));
        user.remove(idx);
        Ok(())
    }
}

pub fn builtins() -> &'static [Skin] {
    static BUILTINS: OnceLock<Vec<Skin>> = OnceLock::new();
    BUILTINS.get_or_init(|| {
        let raw = include_str!("../resources/builtin_skins.json");
        serde_json::from_str::<Vec<Skin>>(raw).expect("invalid builtin skins JSON")
    })
}

fn load_user(dir: &Path) -> Vec<Skin> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        if path.extension().map(|e| e != "json").unwrap_or(true) {
            continue;
        }
        let data = match fs::read(&path) {
            Ok(d) => d,
            Err(err) => {
                log::warn!(
                    "skin: skipping unreadable file {}: {}",
                    path.display(),
                    err
                );
                continue;
            }
        };
        let mut skin: Skin = match serde_json::from_slice(&data) {
            Ok(s) => s,
            Err(err) => {
                log::warn!(
                    "skin: skipping malformed JSON at {}: {}",
                    path.display(),
                    err
                );
                continue;
            }
        };
        skin.source = "user".into();
        out.push(skin);
    }
    out
}

fn persist_user(dir: &Path, skin: &Skin) -> Result<()> {
    let tmp = dir.join(format!("{}.json.tmp", skin.id));
    let final_path = dir.join(format!("{}.json", skin.id));
    let data = serde_json::to_vec_pretty(skin)?;
    fs::write(&tmp, data)?;
    fs::rename(&tmp, final_path)?;
    Ok(())
}

fn id_exists(user: &[Skin], id: &str) -> bool {
    if builtins().iter().any(|s| s.id == id) {
        return true;
    }
    user.iter().any(|s| s.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_parse() {
        let list = builtins();
        assert!(!list.is_empty(), "expected at least one builtin skin");
        assert!(list.iter().any(|s| s.id == DEFAULT_SKIN_ID));
        assert!(list.iter().all(|s| s.source == "builtin"));
        for sk in list {
            for k in sk.vars.keys() {
                assert!(
                    k.starts_with("--"),
                    "builtin skin {:?} var {:?} missing -- prefix",
                    sk.id,
                    k
                );
            }
        }
    }
}

fn validate(sk: &Skin) -> Result<()> {
    if sk.name.is_empty() {
        return Err(SkinError::NameRequired);
    }
    if sk.vars.is_empty() && sk.dark_vars.is_empty() && sk.light_vars.is_empty() {
        return Err(SkinError::NoVars);
    }
    for map in [&sk.vars, &sk.dark_vars, &sk.light_vars] {
        for key in map.keys() {
            if !key.starts_with("--") {
                return Err(SkinError::BadVarName(key.clone()));
            }
        }
    }
    Ok(())
}
