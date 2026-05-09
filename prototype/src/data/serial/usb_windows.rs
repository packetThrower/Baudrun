//! Windows implementation of [`super::detect_missing_drivers`]. Uses
//! PowerShell's `Get-PnpDevice` to find USB devices the system has
//! enumerated but failed to bind a driver to (Status != "OK", usually
//! Problem code 28 = CM_PROB_FAILED_INSTALL).

use std::collections::HashSet;
use std::process::Command;
use std::sync::OnceLock;

use regex::Regex;
use serde::Deserialize;

use super::chipsets::{self, USBSerialCandidate};
use super::detect::detect_suspect_enumerated_ports;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PnpLine {
    #[serde(rename = "InstanceId")]
    instance_id: String,
    // Get-PnpDevice fills in FriendlyName from the firmware iProduct
    // descriptor when no driver is bound; for some devices it's $null
    // and ConvertTo-Json emits literal `null`. Same for Manufacturer
    // (often $null on Win11 ARM for unbound devices). `#[serde(default)]`
    // alone only handles a *missing* field — a present-but-null field
    // fails deserialization into String and silently drops the whole
    // row. Coerce null → "" so we still see the device.
    #[serde(default, deserialize_with = "string_or_null")]
    friendly_name: String,
    #[serde(default, deserialize_with = "string_or_null")]
    manufacturer: String,
}

fn string_or_null<'de, D>(de: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(de)?.unwrap_or_default())
}

pub fn detect_missing_drivers() -> Result<Vec<USBSerialCandidate>, String> {
    let mut drivered: HashSet<String> = HashSet::new();
    if let Ok(ports) = serialport::available_ports() {
        for entry in ports {
            if let serialport::SerialPortType::UsbPort(u) = entry.port_type {
                drivered.insert(format!("{:04x}:{:04x}", u.vid, u.pid));
            }
        }
    }

    let script = "\
        Get-PnpDevice -PresentOnly \
        | Where-Object { $_.InstanceId -like 'USB\\VID_*' -and $_.Status -ne 'OK' } \
        | ForEach-Object { $_ | Select-Object InstanceId,FriendlyName,Manufacturer | ConvertTo-Json -Compress }\
    ";

    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    hide_console(&mut cmd);
    let out = cmd
        .output()
        .map_err(|e| format!("Get-PnpDevice: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "Get-PnpDevice exited {:?}",
            out.status.code()
        ));
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut missing = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let parsed: PnpLine = match serde_json::from_str(line) {
            Ok(p) => p,
            // Log the parse failure so silently-dropped rows don't
            // hide bugs the way the null-Manufacturer issue did. The
            // truncated `line` keeps the log readable on long rows.
            Err(e) => {
                let truncated: String = line.chars().take(160).collect();
                log::warn!(
                    "skip Get-PnpDevice row, JSON parse failed: {} (line: {}{})",
                    e,
                    truncated,
                    if line.len() > 160 { "…" } else { "" }
                );
                continue;
            }
        };
        let caps = match instance_id_re().captures(&parsed.instance_id) {
            Some(c) => c,
            None => continue,
        };
        let vid = caps
            .get(1)
            .map(|m| m.as_str().to_ascii_lowercase())
            .unwrap_or_default();
        let pid = caps
            .get(2)
            .map(|m| m.as_str().to_ascii_lowercase())
            .unwrap_or_default();
        let serial = caps
            .get(3)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();

        let info = chipsets::identify(&vid, &pid, &parsed.manufacturer);
        if !info.needs_driver() {
            continue;
        }
        let key = format!("{}:{}:{}", vid, pid, serial);
        if !seen.insert(key) {
            continue;
        }
        let vidpid = format!("{}:{}", vid, pid);
        if drivered.contains(&vidpid) {
            continue;
        }
        let mut candidate = USBSerialCandidate {
            vid: vid.clone(),
            pid: pid.clone(),
            chipset: info.name,
            manufacturer: parsed.manufacturer,
            product: parsed.friendly_name,
            serial_number: serial,
            driver_url: info.driver_url,
            reason: String::new(),
        };
        retarget_for_arm64(&mut candidate);
        missing.push(candidate);
    }

    for mut c in detect_suspect_enumerated_ports() {
        let key = format!("{}:{}:{}", c.vid, c.pid, c.serial_number);
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        retarget_for_arm64(&mut c);
        missing.push(c);
    }
    Ok(missing)
}

/// Whether the host CPU (not the build target) is ARM64. Baudrun
/// ships both x64 and native ARM64 Windows builds, so a user on a
/// Windows-on-ARM machine could be running either:
///
///   * The native arm64-setup.exe — `PROCESSOR_ARCHITECTURE` = "ARM64".
///   * The x64-setup.exe under Microsoft's x64-on-ARM emulator —
///     `PROCESSOR_ARCHITECTURE` = "AMD64", with the real host arch
///     stashed in `PROCESSOR_ARCHITEW6432` = "ARM64".
///
/// Checking either env var for "ARM64" covers both cases regardless
/// of how Baudrun itself was compiled.
fn is_windows_on_arm64() -> bool {
    let native = std::env::var("PROCESSOR_ARCHITEW6432").unwrap_or_default();
    let proc_arch = std::env::var("PROCESSOR_ARCHITECTURE").unwrap_or_default();
    native.eq_ignore_ascii_case("ARM64") || proc_arch.eq_ignore_ascii_case("ARM64")
}

/// Wrapper that reads the runtime arch and dispatches to the pure
/// policy fn below. Split this way so the policy is unit-testable
/// without process-global env-var manipulation.
fn retarget_for_arm64(c: &mut USBSerialCandidate) {
    apply_arm64_retargeting(c, is_windows_on_arm64());
}

/// Tweak the candidate's `reason` and `driver_url` when running on
/// Windows-on-ARM, where vendor-specific guidance changes:
///
/// * Prolific PL2303 — for the legacy chip rev there's no ARM64
///   driver path at all, so steering the user at prolific.com.tw is
///   a dead end. Point them at the adapters doc instead, which lays
///   out the chip-rev story and recommends an FTDI / CP210x cable.
/// * FTDI — ARM64 driver exists but Windows Update doesn't push it;
///   manual install is required. Mention it.
/// * CP210x and others — no change; their normal driver pages still apply.
fn apply_arm64_retargeting(c: &mut USBSerialCandidate, on_arm64: bool) {
    if !on_arm64 {
        return;
    }
    if c.vid == "067b" {
        c.reason = "On Windows 11 ARM the legacy PL2303 chip in this cable (e.g. TRENDnet TU-S9) has no working driver: Prolific's modern driver rejects it, and the legacy driver is x64-only and unsigned for ARM64. Replace the cable with an FTDI or Silicon Labs CP210x adapter — see the Baudrun adapters guide.".into();
        c.driver_url = "https://packetthrower.github.io/Baudrun/usage/adapters/".into();
    } else if c.vid == "0403" {
        c.reason = "Windows 11 ARM needs FTDI's ARM64 VCP driver; the standard installer isn't ARM64-compatible, so download the ARM64 driver package from FTDI and install it manually via Device Manager.".into();
    }
}

fn instance_id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Matches: USB\VID_10C4&PID_EA60\0001 (last group is the device
    // instance id / serial; present on most but not all devices).
    RE.get_or_init(|| {
        Regex::new(r"(?i)USB\\VID_([0-9A-F]{4})&PID_([0-9A-F]{4})(?:\\(.*))?").unwrap()
    })
}

/// CREATE_NO_WINDOW flag — keeps a console window from flashing
/// while PowerShell runs. Matches the behavior of the Go version's
/// `internal/winconsole` helper.
#[cfg(target_os = "windows")]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn hide_console(_cmd: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reproduces the Win11-ARM PL2303HXA case: PowerShell emits
    /// `Manufacturer: null` for an unbound USB device, and previously
    /// that took the whole row down the `Err(_) => continue` chute,
    /// hiding the device from the missing-driver banner.
    #[test]
    fn pnp_line_accepts_null_string_fields() {
        let json = r#"{"InstanceId":"USB\\VID_067B&PID_2303\\5&341269D7&0&7","FriendlyName":"USB-Serial Controller D","Manufacturer":null}"#;
        let line: PnpLine = serde_json::from_str(json).expect("should accept null Manufacturer");
        assert_eq!(line.instance_id, "USB\\VID_067B&PID_2303\\5&341269D7&0&7");
        assert_eq!(line.friendly_name, "USB-Serial Controller D");
        assert_eq!(line.manufacturer, "");
    }

    #[test]
    fn pnp_line_missing_fields_default_empty() {
        let json = r#"{"InstanceId":"USB\\VID_10C4&PID_EA60\\0001"}"#;
        let line: PnpLine = serde_json::from_str(json).expect("should accept missing fields");
        assert_eq!(line.friendly_name, "");
        assert_eq!(line.manufacturer, "");
    }

    #[test]
    fn pnp_line_both_null() {
        let json = r#"{"InstanceId":"USB\\VID_067B&PID_2303","FriendlyName":null,"Manufacturer":null}"#;
        let line: PnpLine = serde_json::from_str(json).expect("should accept both null");
        assert_eq!(line.friendly_name, "");
        assert_eq!(line.manufacturer, "");
    }

    fn fixture(vid: &str) -> USBSerialCandidate {
        USBSerialCandidate {
            vid: vid.into(),
            pid: "2303".into(),
            chipset: "Prolific PL2303".into(),
            manufacturer: String::new(),
            product: "USB-Serial Controller D".into(),
            serial_number: String::new(),
            driver_url: "https://www.prolific.com.tw".into(),
            reason: String::new(),
        }
    }

    #[test]
    fn arm64_retargeting_off_is_noop() {
        let mut c = fixture("067b");
        apply_arm64_retargeting(&mut c, false);
        assert_eq!(c.driver_url, "https://www.prolific.com.tw");
        assert!(c.reason.is_empty());
    }

    #[test]
    fn arm64_retargeting_prolific_points_at_adapters_guide() {
        let mut c = fixture("067b");
        apply_arm64_retargeting(&mut c, true);
        assert!(c.driver_url.contains("packetthrower.github.io/Baudrun/usage/adapters"));
        assert!(c.reason.contains("Windows 11 ARM"));
        assert!(c.reason.to_lowercase().contains("ftdi"));
    }

    #[test]
    fn arm64_retargeting_ftdi_mentions_manual_install() {
        let mut c = fixture("0403");
        apply_arm64_retargeting(&mut c, true);
        assert!(c.reason.contains("ARM64"));
        assert!(c.reason.to_lowercase().contains("manual"));
    }

    #[test]
    fn arm64_retargeting_silabs_unchanged() {
        let mut c = fixture("10c4");
        let url_before = c.driver_url.clone();
        apply_arm64_retargeting(&mut c, true);
        // SiLabs ships ARM64 via Windows Update, no special handling.
        assert_eq!(c.driver_url, url_before);
        assert!(c.reason.is_empty());
    }
}
