//! Data layer ported verbatim from `src-tauri/src/` during Phase 1
//! of the alacritty + gpui migration. These modules are pure
//! data + IO — config-dir resolution, profile / settings / theme /
//! skin / highlight-pack persistence, serial-port enumeration,
//! USB CP210x driver, XMODEM/YMODEM transfer, sanitization, an
//! event type. They have no UI dependency, so the same code runs
//! on the new gpui stack as did under Tauri.
//!
//! Currently only the modules are present; nothing in the gpui
//! UI code wires into them yet (Phase 2 brings up the sidebar +
//! profile form, and that's where most of these get pulled in).
//! Compiled into the binary now so we catch porting issues
//! immediately rather than discovering them under deadline pressure
//! once UI work depends on them.
//!
//! `mod` declarations only — re-exports stay verbatim inside each
//! submodule so call sites match what they were under
//! `src-tauri/src/`. When a module gets actively consumed, we'll
//! import it via `use crate::data::profiles;` etc.

#![allow(dead_code, unused_imports)]

pub mod appdata;
pub mod events;
pub mod highlight;
pub mod profiles;
pub mod sanitize;
pub mod serial;
pub mod settings;
pub mod skins;
pub mod state;
pub mod themes;
pub mod transfer;
pub mod usbserial;
