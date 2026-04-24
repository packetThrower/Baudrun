//! OS-idiomatic config directory resolution, plus a tiny "override
//! file" indirection that lets users relocate Baudrun's data to an
//! arbitrary path (dotfile-repo, iCloud Drive, Dropbox, etc.).
//!
//!   - macOS:   ~/Library/Application Support/Baudrun
//!   - Windows: %APPDATA%\Baudrun
//!   - Linux:   $XDG_CONFIG_HOME/Baudrun (or ~/.config/Baudrun)
//!
//! The override file (`config_dir_override`) always lives inside the
//! platform default so it remains findable even after the real config
//! has moved. Deleting it reverts to the default on next launch.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

const APP_DIR_NAME: &str = "Baudrun";
const OVERRIDE_FILE_NAME: &str = "config_dir_override";

#[derive(Debug, Error)]
pub enum AppDataError {
    #[error("user config directory unavailable on this platform")]
    NoConfigDir,
    #[error("config directory path must be absolute")]
    NotAbsolute,
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub type Result<T> = std::result::Result<T, AppDataError>;

/// Default platform support directory, ignoring any override.
pub fn default_support_dir() -> Result<PathBuf> {
    let base = dirs::config_dir().ok_or(AppDataError::NoConfigDir)?;
    Ok(base.join(APP_DIR_NAME))
}

/// Path of the bootstrap redirect file. A single-line text file
/// containing the absolute path of the user's chosen config
/// directory, or missing when the default is in use.
pub fn override_file() -> Result<PathBuf> {
    Ok(default_support_dir()?.join(OVERRIDE_FILE_NAME))
}

/// Active config directory — honors the override file when present,
/// falls back to [`default_support_dir`] otherwise.
pub fn support_dir() -> Result<PathBuf> {
    match read_override() {
        Ok(Some(path)) => Ok(path),
        _ => default_support_dir(),
    }
}

/// Point future launches at `dir`. Pass `None` to clear the override
/// and revert to the default on next start. Absolute paths only; the
/// target directory is created if it doesn't already exist.
pub fn write_override(dir: Option<&Path>) -> Result<()> {
    let override_path = override_file()?;
    if let Some(parent) = override_path.parent() {
        fs::create_dir_all(parent)?;
    }

    match dir {
        None => match fs::remove_file(&override_path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        },
        Some(path) => {
            if !path.is_absolute() {
                return Err(AppDataError::NotAbsolute);
            }
            fs::create_dir_all(path)?;
            let mut contents = path.to_string_lossy().into_owned();
            contents.push('\n');
            fs::write(&override_path, contents)?;
            Ok(())
        }
    }
}

fn read_override() -> Result<Option<PathBuf>> {
    let path = override_file()?;
    match fs::read_to_string(&path) {
        Ok(data) => {
            let trimmed = data.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(PathBuf::from(trimmed)))
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}
