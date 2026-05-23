//! Data layer ported verbatim from `src-tauri/src/` during Phase 1
//! of the alacritty + gpui migration. These modules are pure
//! data + IO — config-dir resolution, profile / settings / theme /
//! skin / highlight-pack persistence, serial-port enumeration,
//! USB CP210x driver, XMODEM/YMODEM transfer, sanitization, an
//! event type. They have no UI dependency, so the same code runs
//! on the new gpui stack as did under Tauri.
//!
//! Module status notes — used at file level rather than as a
//! blanket `#![allow(dead_code, unused_imports)]` here, so future
//! orphans surface as clippy warnings instead of being silently
//! absorbed:
//!
//!   - `events`, `state` — Tauri-era residue (event bus, AppState).
//!     gpui replaces both wholesale. Candidates for full deletion.
//!   - `serial::session`, `serial::direct`, `serial::usb_darwin`,
//!     `serial::chipsets`, `usbserial::*` — libusb-direct serial
//!     path + the chipset-identification table that backs it on
//!     macOS / Windows only. The gpui code runs `serialport`-only;
//!     `chipsets` specifically is fully dead on Linux because its
//!     callers (`detect`, `usb_darwin`, `usb_windows`) are all
//!     cfg-gated to non-Linux. Kept compiled as a reference for
//!     the eventual libusb-fallback resurrection.
//!   - Other modules (`profiles`, `settings`, `skins`, `themes`,
//!     `highlight`, `sanitize`, `transfer`, `hex`, `appdata`,
//!     `serial::ports`) are actively wired into the gpui UI on
//!     all platforms.

pub mod appdata;
pub mod events;
pub mod hex;
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
