//! Terminal color-scheme store. 13 built-in themes are embedded at
//! compile time as JSON (see `src-tauri/resources/builtin_themes.json`
//! — generated from the Go `themes/builtins.go` via
//! `go run ./scripts/dump-builtins`). User themes import from iTerm2
//! `.itermcolors` files and live on disk at
//! `$SUPPORT_DIR/themes/<id>.json`.

mod parse;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use parse::parse_iterm_colors;

pub const DEFAULT_THEME_ID: &str = "baudrun";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub id: String,
    pub name: String,
    /// "builtin" or "user"
    pub source: String,

    pub background: String,
    pub foreground: String,
    pub cursor: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cursor_accent: String,
    pub selection: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub selection_foreground: String,

    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
    pub bright_black: String,
    pub bright_red: String,
    pub bright_green: String,
    pub bright_yellow: String,
    pub bright_blue: String,
    pub bright_magenta: String,
    pub bright_cyan: String,
    pub bright_white: String,
}

#[derive(Debug, Error)]
pub enum ThemeError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("serialize theme: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("parse iTerm colors: {0}")]
    Parse(String),
    #[error("user theme {0} not found")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, ThemeError>;

pub struct Store {
    dir: PathBuf,
    user: RwLock<Vec<Theme>>,
}

impl Store {
    pub fn new(support_dir: &Path) -> Result<Self> {
        let dir = support_dir.join("themes");
        fs::create_dir_all(&dir)?;
        let user = load_user(&dir);
        Ok(Store {
            dir,
            user: RwLock::new(user),
        })
    }

    pub fn list(&self) -> Vec<Theme> {
        let user = self.user.read().unwrap();
        let mut out = Vec::with_capacity(builtins().len() + user.len());
        out.extend(builtins().iter().cloned());
        out.extend(user.iter().cloned());
        out
    }

    pub fn get(&self, id: &str) -> Option<Theme> {
        if let Some(t) = builtins().iter().find(|t| t.id == id) {
            return Some(t.clone());
        }
        self.user
            .read()
            .unwrap()
            .iter()
            .find(|t| t.id == id)
            .cloned()
    }

    /// Returns the theme with the given id, falling back to the
    /// default and finally the first builtin.
    pub fn resolve(&self, id: &str) -> Theme {
        self.get(id)
            .or_else(|| self.get(DEFAULT_THEME_ID))
            .unwrap_or_else(|| builtins()[0].clone())
    }

    /// Import an `.itermcolors` XML plist from `path` and persist it
    /// as a user theme. The display name (and slug source for the ID)
    /// comes from the filename without extension. Collisions with
    /// existing IDs get a `-2`, `-3`, ... suffix.
    pub fn import(&self, path: &Path) -> Result<Theme> {
        let data = fs::read(path)?;
        let base = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "theme".into());
        let mut theme = parse_iterm_colors(&data, &base).map_err(ThemeError::Parse)?;
        theme.source = "user".into();

        let mut user = self.user.write().unwrap();
        let base_id = theme.id.clone();
        let mut suffix = 2;
        while id_exists(&user, &theme.id) {
            theme.id = format!("{}-{}", base_id, suffix);
            suffix += 1;
        }

        persist_user(&self.dir, &theme)?;
        user.push(theme.clone());
        Ok(theme)
    }

    /// Remove a user-imported theme. Returns an error on unknown ids
    /// and silently refuses to touch builtins (they don't appear in
    /// the user vec).
    pub fn delete(&self, id: &str) -> Result<()> {
        let mut user = self.user.write().unwrap();
        let idx = user
            .iter()
            .position(|t| t.id == id)
            .ok_or_else(|| ThemeError::NotFound(id.to_string()))?;
        let _ = fs::remove_file(self.dir.join(format!("{}.json", id)));
        user.remove(idx);
        Ok(())
    }
}

/// Slice of embedded built-in themes, parsed once on first access.
pub fn builtins() -> &'static [Theme] {
    static BUILTINS: OnceLock<Vec<Theme>> = OnceLock::new();
    BUILTINS.get_or_init(|| {
        let raw = include_str!("../../resources/builtin_themes.json");
        serde_json::from_str::<Vec<Theme>>(raw).expect("invalid builtin themes JSON")
    })
}

fn load_user(dir: &Path) -> Vec<Theme> {
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
            Err(_) => continue,
        };
        let mut theme: Theme = match serde_json::from_slice(&data) {
            Ok(t) => t,
            Err(_) => continue,
        };
        theme.source = "user".into();
        out.push(theme);
    }
    out
}

fn persist_user(dir: &Path, theme: &Theme) -> Result<()> {
    let tmp = dir.join(format!("{}.json.tmp", theme.id));
    let final_path = dir.join(format!("{}.json", theme.id));
    let data = serde_json::to_vec_pretty(theme)?;
    fs::write(&tmp, data)?;
    fs::rename(&tmp, final_path)?;
    Ok(())
}

fn id_exists(user: &[Theme], id: &str) -> bool {
    if builtins().iter().any(|t| t.id == id) {
        return true;
    }
    user.iter().any(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_parse() {
        let list = builtins();
        assert!(!list.is_empty(), "expected at least one builtin theme");
        assert!(list.iter().any(|t| t.id == DEFAULT_THEME_ID));
        assert!(list.iter().all(|t| t.source == "builtin"));
    }

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Dracula", "theme"), "dracula");
        assert_eq!(slugify("Tomorrow Night", "theme"), "tomorrow-night");
        assert_eq!(slugify("!!!", "theme"), "theme");
        assert_eq!(slugify("Solarized  Dark--!", "theme"), "solarized-dark");
    }
}

/// slugify mirrors the Go implementation: lowercase, collapse
/// `[ \-_.]` runs to a single `-`, strip non-alnum, trim trailing
/// dashes, fall back to "theme" when empty.
pub(crate) fn slugify(input: &str, fallback: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_dash = true;
    for ch in input.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if matches!(ch, ' ' | '-' | '_' | '.') && !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out = fallback.into();
    }
    out
}
