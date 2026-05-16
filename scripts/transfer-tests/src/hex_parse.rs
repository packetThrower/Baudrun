//! Hex parser lifted verbatim from `src/app_view.rs` (the **Send Hex…**
//! modal). Pure, no dependencies beyond `std` — the cleanest possible
//! "extract just this function" lift. Kept here as a separate module
//! (rather than #[path]-included from app_view.rs) because app_view.rs
//! depends on gpui and a Hex parser doesn't justify pulling that in.
//!
//! Whenever the canonical version changes, mirror the change here.
//! It's a 20-line pure function — drift is easy to spot in code review.

pub fn parse_hex_string(raw: &str) -> Result<Vec<u8>, &'static str> {
    let cleaned: String = raw
        .replace("0x", "")
        .replace("0X", "")
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',')
        .collect();
    if cleaned.is_empty() {
        return Ok(Vec::new());
    }
    if !cleaned.len().is_multiple_of(2) {
        return Err("odd number of hex digits");
    }
    if !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("non-hex characters");
    }
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    let bytes = cleaned.as_bytes();
    for chunk in bytes.chunks_exact(2) {
        let s = std::str::from_utf8(chunk).unwrap();
        out.push(u8::from_str_radix(s, 16).map_err(|_| "non-hex characters")?);
    }
    Ok(out)
}
