//! Stub [`super::detect_missing_drivers`] for platforms without
//! bespoke USB enumeration logic (currently Linux + BSDs). sysfs +
//! udev enumeration would work here — unimplemented until someone
//! needs it.

use super::chipsets::USBSerialCandidate;

pub fn detect_missing_drivers() -> Result<Vec<USBSerialCandidate>, String> {
    Ok(Vec::new())
}
