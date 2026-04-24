//! USB serial bridge chipset identification. Given a VID (and
//! optionally PID + manufacturer string), identify the bridge chip
//! family (CP210x, FTDI, PL2303, ...) and the URL of the driver the
//! user needs to install, if any.
//!
//! Lookup strategy is three-tiered:
//!  1. Exact VID:PID rebrand table (vendors shipping a CP210x/FTDI
//!     under their own USB-IF VID).
//!  2. Plain VID → chipset name + driver URL.
//!  3. Manufacturer-string heuristic as a last resort — the chip's
//!     own iManufacturer descriptor often outs the underlying silicon
//!     even when the enclosing device has its own VID.

use serde::{Deserialize, Serialize};

const SILABS_DRIVER_URL: &str =
    "https://www.silabs.com/developers/usb-to-uart-bridge-vcp-drivers";

/// USB device whose VID/manufacturer points at a known serial
/// chipset but which isn't currently accessible as a serial port —
/// i.e. the user probably hasn't installed the vendor driver.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct USBSerialCandidate {
    pub vid: String,
    pub pid: String,
    pub chipset: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub manufacturer: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub product: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub serial_number: String,
    /// JSON tag keeps the Go-side spelling (`driverURL`) so existing
    /// frontend code and on-disk artifacts round-trip unchanged.
    #[serde(
        rename = "driverURL",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub driver_url: String,
    /// When set, replaces the default "driver not loaded" banner copy
    /// with a more specific explanation (e.g. Prolific legacy-chip
    /// warning).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
}

/// Chipset family name + driver download URL.
#[derive(Debug, Clone, Default)]
pub struct ChipsetInfo {
    pub name: String,
    pub driver_url: String,
}

impl ChipsetInfo {
    /// Whether this chipset needs user-installed drivers to work
    /// (i.e. we have something actionable to nudge the user about).
    pub fn needs_driver(&self) -> bool {
        !self.name.is_empty() && !self.driver_url.is_empty()
    }
}

/// Chipset family name for a well-known VID. Used to annotate
/// ports that are already accessible.
pub fn chipset_for_vid(vid: &str) -> &'static str {
    chipset_by_vid(&vid.to_ascii_lowercase())
}

/// Driver download URL for a VID. Empty string when no user-
/// installable driver is required (FTDI, CDC-ACM, etc.).
pub fn driver_url_for_vid(vid: &str) -> &'static str {
    driver_url_by_vid(&vid.to_ascii_lowercase())
}

/// Three-strategy chipset identification: rebrand VID:PID, bare VID,
/// then manufacturer heuristic. Returns `ChipsetInfo::default()` when
/// nothing matches.
pub fn identify(vid: &str, pid: &str, manufacturer: &str) -> ChipsetInfo {
    let vid_l = vid.to_ascii_lowercase();
    let pid_l = pid.to_ascii_lowercase();

    if let Some(info) = rebrand_for(&vid_l, &pid_l) {
        return info;
    }
    let name = chipset_by_vid(&vid_l);
    if !name.is_empty() {
        return ChipsetInfo {
            name: name.to_string(),
            driver_url: driver_url_by_vid(&vid_l).to_string(),
        };
    }
    chipset_from_manufacturer(manufacturer)
}

/// Whether a USB product string looks like a driver-issue placeholder
/// rather than a real product name. Common case: counterfeit-detecting
/// Prolific drivers enumerate the chip with a warning string as the
/// device name. The port appears drivered to the OS but serial I/O
/// won't work.
pub fn is_suspect_product(s: &str) -> bool {
    let lc = s.to_ascii_lowercase();
    lc.contains("please install")
        || lc.contains("please download")
        || lc.contains("support windows")
        || lc.contains("counterfeit")
        || lc.contains("not supported")
        || lc.contains("not support")
}

fn chipset_by_vid(vid_lower: &str) -> &'static str {
    match vid_lower {
        "10c4" => "CP210x (Silicon Labs)",
        "0403" => "FTDI",
        "067b" => "Prolific PL2303",
        "1a86" => "WCH CH340/CH341",
        "04d8" => "Microchip",
        "04b4" => "Cypress",
        "0557" => "ATEN",
        "0d28" => "ARM mbed (CDC-ACM)",
        "9710" => "MCS7810/20/40 (MosChip / ASIX)",
        "0711" => "MCTU232 (Magic Control)",
        "1393" => "Moxa UPort",
        "05d1" => "Brainboxes",
        _ => "",
    }
}

fn driver_url_by_vid(vid_lower: &str) -> &'static str {
    match vid_lower {
        "10c4" => SILABS_DRIVER_URL,
        "067b" => "https://www.prolific.com.tw",
        "1a86" => "https://www.wch-ic.com/downloads/CH34XSER_MAC_ZIP.html",
        "04d8" => "https://www.microchip.com/en-us/product/MCP2221A",
        "04b4" => "https://www.infineon.com/cms/en/design-support/tools/sdk/",
        "9710" => "https://www.asix.com.tw",
        "0711" => "https://www.mct.com.tw",
        "1393" => "https://www.moxa.com/en/support/product-support/software-and-documentation",
        "05d1" => "https://www.brainboxes.com",
        _ => "",
    }
}

/// Rebrands: VID:PID devices that ship a known bridge chip
/// reprogrammed with the vendor's own USB-IF VID.
fn rebrand_for(vid: &str, pid: &str) -> Option<ChipsetInfo> {
    // Siemens RUGGEDCOM USB Serial console (RST2228 family). Chip
    // inside is a CP210x but Siemens ships it with their own VID and
    // a non-CDC interface, so macOS's built-in CDC-ACM driver doesn't
    // bind; the SiLabs VCP driver is required.
    if vid == "0908" && pid == "01ff" {
        return Some(ChipsetInfo {
            name: "CP210x (Siemens RUGGEDCOM)".into(),
            driver_url: SILABS_DRIVER_URL.into(),
        });
    }
    None
}

fn chipset_from_manufacturer(s: &str) -> ChipsetInfo {
    if s.is_empty() {
        return ChipsetInfo::default();
    }
    let lc = s.to_ascii_lowercase();
    // Table of (substring, chipset-info). Order matters for
    // overlapping matches — most specific first.
    let table: &[(&str, &str, &str)] = &[
        ("silicon lab", "CP210x (Silicon Labs)", SILABS_DRIVER_URL),
        ("silabs", "CP210x (Silicon Labs)", SILABS_DRIVER_URL),
        ("prolific", "Prolific PL2303", "https://www.prolific.com.tw"),
        (
            "qinheng",
            "WCH CH340/CH341",
            "https://www.wch-ic.com/downloads/CH34XSER_MAC_ZIP.html",
        ),
        (
            "wch.cn",
            "WCH CH340/CH341",
            "https://www.wch-ic.com/downloads/CH34XSER_MAC_ZIP.html",
        ),
        (
            "moxa",
            "Moxa UPort",
            "https://www.moxa.com/en/support/product-support/software-and-documentation",
        ),
        ("brainboxes", "Brainboxes", "https://www.brainboxes.com"),
    ];
    for (needle, name, url) in table {
        if lc.contains(needle) {
            return ChipsetInfo {
                name: (*name).to_string(),
                driver_url: (*url).to_string(),
            };
        }
    }
    ChipsetInfo::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identify_by_vid() {
        let info = identify("10C4", "EA60", "");
        assert_eq!(info.name, "CP210x (Silicon Labs)");
        assert!(info.needs_driver());
    }

    #[test]
    fn identify_ftdi_has_no_driver_url() {
        let info = identify("0403", "6001", "");
        assert_eq!(info.name, "FTDI");
        assert!(!info.needs_driver(), "FTDI uses kernel CDC — no user driver");
    }

    #[test]
    fn identify_by_manufacturer_fallback() {
        let info = identify("abcd", "1234", "Silicon Labs Inc.");
        assert_eq!(info.name, "CP210x (Silicon Labs)");
    }

    #[test]
    fn identify_siemens_rebrand() {
        let info = identify("0908", "01ff", "Siemens");
        assert_eq!(info.name, "CP210x (Siemens RUGGEDCOM)");
        assert!(info.needs_driver());
    }

    #[test]
    fn identify_unknown_is_empty() {
        let info = identify("dead", "beef", "");
        assert!(!info.needs_driver());
        assert!(info.name.is_empty());
    }

    #[test]
    fn suspect_product_detection() {
        assert!(is_suspect_product("Please install corresponding PL2303 driver"));
        assert!(is_suspect_product("DEVICE NOT SUPPORT on newer driver"));
        assert!(!is_suspect_product("USB Serial Converter"));
    }
}
