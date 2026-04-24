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
    #[serde(default)]
    friendly_name: String,
    #[serde(default)]
    manufacturer: String,
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
            Err(_) => continue,
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
        missing.push(USBSerialCandidate {
            vid,
            pid,
            chipset: info.name,
            manufacturer: parsed.manufacturer,
            product: parsed.friendly_name,
            serial_number: serial,
            driver_url: info.driver_url,
            reason: String::new(),
        });
    }

    for c in detect_suspect_enumerated_ports() {
        let key = format!("{}:{}:{}", c.vid, c.pid, c.serial_number);
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        missing.push(c);
    }
    Ok(missing)
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
