//! Highlight-rule store. Three layers:
//!
//!   1. **Bundled preset packs** — read-only, shipped in the binary
//!      via `include_str!` from `src-tauri/resources/highlight/`.
//!      Includes the `baudrun-default` vendor-neutral set plus
//!      device-specific add-ons (cisco-ios, junos, aruba-cx,
//!      arista-eos, …).
//!   2. **User pack** — editable JSON at
//!      `$SUPPORT_DIR/highlight-rules.json`. Created from the
//!      `baudrun-default` preset on first run if absent. Users edit
//!      the file directly or via Settings → Advanced → Highlights.
//!   3. **Active set per profile** — controlled by
//!      `Settings.enabled_highlight_presets`; the frontend composes
//!      enabled bundled packs + the user pack into the regex engine
//!      at runtime.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// JSON shape of a single rule. `pattern` is a regex string the
/// frontend feeds straight into `new RegExp(pattern, flags)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HighlightRule {
    pub pattern: String,
    /// Named color. Frontend maps to ANSI escape sequences.
    /// One of: `red`, `green`, `yellow`, `blue`, `magenta`,
    /// `cyan`, `dim`. Unknown values fall back to `dim`.
    pub color: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_case: bool,
    /// Optional category tag for display — not used by the matcher.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// A named set of rules. The user pack and each bundled preset
/// share this shape so users can copy / paste between them.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HighlightPack {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the pack came from the embedded resources
    /// (read-only) or from the user file (editable). Set by the
    /// store at load time; not serialized into preset JSON.
    #[serde(default)]
    pub source: String,
    pub rules: Vec<HighlightRule>,
}

#[derive(Debug, Error)]
pub enum HighlightError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("serialize highlight rules: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, HighlightError>;

const USER_FILE: &str = "highlight-rules.json";
pub const USER_PACK_ID: &str = "user";

/// Bundled preset packs, parsed once on first access. Order is the
/// order Settings → Advanced lists them; default first so it's the
/// natural pick.
pub fn bundled_packs() -> &'static [HighlightPack] {
    static PACKS: OnceLock<Vec<HighlightPack>> = OnceLock::new();
    PACKS.get_or_init(|| {
        let raw_packs: &[(&str, &str)] = &[
            (
                "baudrun-default",
                include_str!("../resources/highlight/baudrun-default.json"),
            ),
            (
                "cisco-ios",
                include_str!("../resources/highlight/cisco-ios.json"),
            ),
            ("junos", include_str!("../resources/highlight/junos.json")),
            (
                "aruba-cx",
                include_str!("../resources/highlight/aruba-cx.json"),
            ),
            (
                "arista-eos",
                include_str!("../resources/highlight/arista-eos.json"),
            ),
        ];
        raw_packs
            .iter()
            .map(|(id, raw)| {
                let mut p: HighlightPack = serde_json::from_str(raw)
                    .unwrap_or_else(|e| panic!("bundled highlight pack {} invalid: {}", id, e));
                p.source = "builtin".into();
                p
            })
            .collect()
    })
}

/// Default user pack contents — a copy of `baudrun-default` with
/// id rewritten to `user`. Used to seed the user file on first run.
fn default_user_pack() -> HighlightPack {
    let default = bundled_packs()
        .iter()
        .find(|p| p.id == "baudrun-default")
        .expect("baudrun-default pack present");
    HighlightPack {
        id: USER_PACK_ID.into(),
        name: "User overrides".into(),
        description: Some(
            "Editable copy of the Baudrun default rules. Add your own patterns here \
             — they apply alongside any bundled presets you've enabled."
                .into(),
        ),
        source: "user".into(),
        rules: default.rules.clone(),
    }
}

pub struct Store {
    user_path: PathBuf,
}

impl Store {
    pub fn new(support_dir: &Path) -> Result<Self> {
        fs::create_dir_all(support_dir)?;
        let store = Store {
            user_path: support_dir.join(USER_FILE),
        };
        // Seed the file with a copy of the default rules on first
        // run so users have something to edit instead of facing an
        // empty file.
        if !store.user_path.exists() {
            let seed = default_user_pack();
            store.save(&seed)?;
        }
        Ok(store)
    }

    /// Returns every pack the frontend should know about: bundled
    /// presets first, then the user pack. The user pack is loaded
    /// fresh each call so external edits to the JSON file are
    /// picked up without an app restart.
    pub fn list(&self) -> Vec<HighlightPack> {
        let mut out: Vec<HighlightPack> = bundled_packs().to_vec();
        if let Ok(user) = self.load_user() {
            out.push(user);
        }
        out
    }

    pub fn update_user_pack(&self, mut pack: HighlightPack) -> Result<HighlightPack> {
        // The id + source fields are owned by the store; users
        // can edit name / description / rules but the slot itself
        // is fixed.
        pack.id = USER_PACK_ID.into();
        pack.source = "user".into();
        self.save(&pack)?;
        Ok(pack)
    }

    fn load_user(&self) -> Result<HighlightPack> {
        let data = fs::read(&self.user_path)?;
        let mut pack: HighlightPack = serde_json::from_slice(&data)?;
        pack.id = USER_PACK_ID.into();
        pack.source = "user".into();
        Ok(pack)
    }

    fn save(&self, pack: &HighlightPack) -> Result<()> {
        let data = serde_json::to_vec_pretty(pack)?;
        let tmp = self.user_path.with_extension("json.tmp");
        fs::write(&tmp, data)?;
        fs::rename(&tmp, &self.user_path)?;
        Ok(())
    }
}

fn is_false(v: &bool) -> bool {
    !v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_packs_parse() {
        let packs = bundled_packs();
        assert!(!packs.is_empty());
        assert!(packs.iter().any(|p| p.id == "baudrun-default"));
        assert!(packs.iter().all(|p| !p.rules.is_empty()));
        assert!(packs.iter().all(|p| p.source == "builtin"));
    }
}
