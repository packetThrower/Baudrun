//! Highlight-rule store. Four layers:
//!
//!   1. **Bundled preset packs** — read-only, shipped in the binary
//!      via `include_str!` from `src-tauri/resources/highlight/`.
//!      Includes the `baudrun-default` vendor-neutral set plus
//!      device-specific add-ons (cisco-ios, junos, aruba-cx,
//!      arista-eos, mikrotik-routeros, …).
//!   2. **User scratchpad pack** — editable JSON at
//!      `$SUPPORT_DIR/highlight-rules.json` (id `"user"`). Created
//!      from the `baudrun-default` preset on first run if absent.
//!   3. **Imported user packs** — one JSON file per pack under
//!      `$SUPPORT_DIR/highlight/<id>.json`. Users import shared packs
//!      (via Settings → Syntax Highlighting → Import) without touching
//!      the scratchpad. Each imported pack's id comes from its
//!      filename stem, which is sanitized to alphanum+hyphen+underscore
//!      at import time; collisions with bundled ids or the scratchpad
//!      id are rejected.
//!   4. **Active set per profile** — controlled by
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
    #[error("{0}")]
    Invalid(String),
    #[error("pack not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, HighlightError>;

const USER_FILE: &str = "highlight-rules.json";
const IMPORTS_DIR: &str = "highlight";
pub const USER_PACK_ID: &str = "user";

/// Lowercase ascii-alphanum + hyphen + underscore. Used for pack ids
/// because the id becomes the filename and is also how the frontend
/// refers to the pack in Settings' enabled-presets list.
fn sanitize_pack_id(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

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
            (
                "mikrotik-routeros",
                include_str!("../resources/highlight/mikrotik-routeros.json"),
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
    imports_dir: PathBuf,
}

impl Store {
    pub fn new(support_dir: &Path) -> Result<Self> {
        fs::create_dir_all(support_dir)?;
        let imports_dir = support_dir.join(IMPORTS_DIR);
        fs::create_dir_all(&imports_dir)?;
        let store = Store {
            user_path: support_dir.join(USER_FILE),
            imports_dir,
        };
        // Seed the scratchpad with a copy of the default rules on
        // first run so users have something to edit instead of
        // facing an empty file.
        if !store.user_path.exists() {
            let seed = default_user_pack();
            store.save(&seed)?;
        }
        Ok(store)
    }

    /// Returns every pack the frontend should know about: bundled
    /// presets first, then imported user packs (alphabetical by id),
    /// then the user scratchpad. Re-read every call so external
    /// edits / new files show up without an app restart.
    pub fn list(&self) -> Vec<HighlightPack> {
        let mut out: Vec<HighlightPack> = bundled_packs().to_vec();
        let mut imports = self.load_imports();
        imports.sort_by(|a, b| a.id.cmp(&b.id));
        out.extend(imports);
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

    /// Read a JSON file from an arbitrary path, validate, and copy
    /// into `$SUPPORT/highlight/<id>.json`. The id comes from the
    /// pack's own `id` field in the JSON (sanitized); if that would
    /// collide with a bundled pack or the scratchpad, the import is
    /// rejected so users don't silently shadow a built-in.
    pub fn import_user_pack(&self, source_path: &Path) -> Result<HighlightPack> {
        // Read with the underlying filesystem error logged internally
        // but scrubbed from the error returned to the frontend — the
        // string returned via tauri::command surfaces in the UI and
        // (on some platforms) includes the absolute source path,
        // which is unhelpful and slightly leaky for hosted app
        // distributions where a stray screenshot might travel.
        let data = fs::read(source_path).map_err(|err| {
            log::warn!(
                "highlight: read import source {}: {}",
                source_path.display(),
                err
            );
            HighlightError::Invalid("couldn't read selected file".into())
        })?;
        let mut pack: HighlightPack = serde_json::from_slice(&data).map_err(|e| {
            HighlightError::Invalid(format!("invalid pack JSON: {}", e))
        })?;

        if pack.rules.is_empty() {
            return Err(HighlightError::Invalid(
                "pack has no rules — nothing to import".into(),
            ));
        }

        let id = sanitize_pack_id(&pack.id);
        if id.is_empty() {
            return Err(HighlightError::Invalid(
                "pack id must contain alphanumeric, hyphen, or underscore".into(),
            ));
        }
        if id == USER_PACK_ID {
            return Err(HighlightError::Invalid(format!(
                "id '{}' is reserved for the scratchpad — pick another id",
                USER_PACK_ID
            )));
        }
        if bundled_packs().iter().any(|p| p.id == id) {
            return Err(HighlightError::Invalid(format!(
                "id '{}' collides with a bundled pack — pick another id",
                id
            )));
        }

        pack.id = id.clone();
        pack.source = "user".into();

        let target = self.imports_dir.join(format!("{}.json", id));
        let body = serde_json::to_vec_pretty(&pack)?;
        let tmp = target.with_extension("json.tmp");
        fs::write(&tmp, body)?;
        fs::rename(&tmp, &target)?;
        Ok(pack)
    }

    /// Remove an imported user pack. The scratchpad (`"user"`) and
    /// bundled packs are not deletable — callers get `Invalid` /
    /// `NotFound` in those cases.
    pub fn delete_user_pack(&self, id: &str) -> Result<()> {
        if id == USER_PACK_ID {
            return Err(HighlightError::Invalid(
                "the scratchpad cannot be deleted — edit it via Settings or clear its rules on disk"
                    .into(),
            ));
        }
        if bundled_packs().iter().any(|p| p.id == id) {
            return Err(HighlightError::Invalid(format!(
                "'{}' is a bundled pack and cannot be deleted",
                id
            )));
        }
        let safe_id = sanitize_pack_id(id);
        if safe_id.is_empty() {
            return Err(HighlightError::Invalid("pack id required".into()));
        }
        let target = self.imports_dir.join(format!("{}.json", safe_id));
        if !target.exists() {
            return Err(HighlightError::NotFound(safe_id));
        }
        fs::remove_file(&target)?;
        Ok(())
    }

    fn load_imports(&self) -> Vec<HighlightPack> {
        let Ok(entries) = fs::read_dir(&self.imports_dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("json") {
                    return None;
                }
                // Derive the canonical id from the filename stem so
                // renaming the file renames the pack — otherwise users
                // end up with duplicate-looking entries when JSON.id
                // and filename drift apart.
                let stem = path.file_stem().and_then(|s| s.to_str())?;
                let id = sanitize_pack_id(stem);
                if id.is_empty() || id == USER_PACK_ID {
                    return None;
                }
                if bundled_packs().iter().any(|p| p.id == id) {
                    return None;
                }
                let data = fs::read(&path).ok()?;
                let mut pack: HighlightPack = serde_json::from_slice(&data).ok()?;
                pack.id = id;
                pack.source = "user".into();
                Some(pack)
            })
            .collect()
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

    use std::env;

    fn temp_support_dir(suffix: &str) -> PathBuf {
        let mut p = env::temp_dir();
        p.push(format!(
            "baudrun-highlight-test-{}-{}",
            suffix,
            std::process::id()
        ));
        // Start clean in case a previous run left it around.
        let _ = fs::remove_dir_all(&p);
        p
    }

    fn write_pack_file(path: &Path, pack: &HighlightPack) {
        fs::write(path, serde_json::to_vec_pretty(pack).unwrap()).unwrap();
    }

    fn sample_pack(id: &str) -> HighlightPack {
        HighlightPack {
            id: id.into(),
            name: format!("Sample {}", id),
            description: None,
            source: String::new(),
            rules: vec![HighlightRule {
                pattern: r"\bfoo\b".into(),
                color: "cyan".into(),
                ignore_case: false,
                group: None,
            }],
        }
    }

    #[test]
    fn bundled_packs_parse() {
        let packs = bundled_packs();
        assert!(!packs.is_empty());
        assert!(packs.iter().any(|p| p.id == "baudrun-default"));
        assert!(packs.iter().all(|p| !p.rules.is_empty()));
        assert!(packs.iter().all(|p| p.source == "builtin"));
    }

    #[test]
    fn sanitize_pack_id_filters_noise() {
        assert_eq!(sanitize_pack_id("my-lab_v2"), "my-lab_v2");
        assert_eq!(sanitize_pack_id("  foo bar  "), "foobar");
        // Path-traversal bytes are stripped; the remaining chars are
        // harmless inside the imports dir because the full filename
        // is always `<sanitized>.json`.
        assert_eq!(sanitize_pack_id("../evil.json"), "eviljson");
        assert_eq!(sanitize_pack_id("💀"), "");
    }

    #[test]
    fn import_accepts_fresh_pack() {
        let dir = temp_support_dir("import_accepts");
        let store = Store::new(&dir).unwrap();

        let pack = sample_pack("my-lab");
        let src = dir.join("incoming.json");
        write_pack_file(&src, &pack);

        let imported = store.import_user_pack(&src).unwrap();
        assert_eq!(imported.id, "my-lab");
        assert_eq!(imported.source, "user");

        let list = store.list();
        assert!(list.iter().any(|p| p.id == "my-lab" && p.source == "user"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn import_rejects_reserved_id() {
        let dir = temp_support_dir("import_reserved");
        let store = Store::new(&dir).unwrap();

        let pack = sample_pack(USER_PACK_ID);
        let src = dir.join("incoming.json");
        write_pack_file(&src, &pack);

        let err = store.import_user_pack(&src).unwrap_err();
        assert!(matches!(err, HighlightError::Invalid(_)));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn import_rejects_bundled_id() {
        let dir = temp_support_dir("import_bundled");
        let store = Store::new(&dir).unwrap();

        let pack = sample_pack("cisco-ios");
        let src = dir.join("incoming.json");
        write_pack_file(&src, &pack);

        let err = store.import_user_pack(&src).unwrap_err();
        assert!(matches!(err, HighlightError::Invalid(_)));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn import_rejects_empty_rules() {
        let dir = temp_support_dir("import_empty");
        let store = Store::new(&dir).unwrap();

        let mut pack = sample_pack("empty-pack");
        pack.rules.clear();
        let src = dir.join("incoming.json");
        write_pack_file(&src, &pack);

        let err = store.import_user_pack(&src).unwrap_err();
        assert!(matches!(err, HighlightError::Invalid(_)));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_user_pack_removes_file() {
        let dir = temp_support_dir("delete_removes");
        let store = Store::new(&dir).unwrap();

        let pack = sample_pack("removable");
        let src = dir.join("incoming.json");
        write_pack_file(&src, &pack);
        store.import_user_pack(&src).unwrap();

        assert!(store.list().iter().any(|p| p.id == "removable"));
        store.delete_user_pack("removable").unwrap();
        assert!(!store.list().iter().any(|p| p.id == "removable"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_rejects_scratchpad_and_bundled() {
        let dir = temp_support_dir("delete_reject");
        let store = Store::new(&dir).unwrap();

        let err = store.delete_user_pack(USER_PACK_ID).unwrap_err();
        assert!(matches!(err, HighlightError::Invalid(_)));

        let err = store.delete_user_pack("cisco-ios").unwrap_err();
        assert!(matches!(err, HighlightError::Invalid(_)));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_unknown_id_is_not_found() {
        let dir = temp_support_dir("delete_unknown");
        let store = Store::new(&dir).unwrap();

        let err = store.delete_user_pack("does-not-exist").unwrap_err();
        assert!(matches!(err, HighlightError::NotFound(_)));

        fs::remove_dir_all(&dir).ok();
    }
}
