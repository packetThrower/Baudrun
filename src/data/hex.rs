//! Send-Hex byte parser. Accepts the four input shapes Baudrun's
//! **Send Hex…** modal documents — space-separated (`41 42 43`),
//! comma-separated (`41,42,43`), compact (`414243`), and `0x`-prefixed
//! (`0x41 0x42 0x43`) — and any mixture thereof. Mixed-case hex
//! digits are fine.
//!
//! Pure, no IO, no UI dependency. Lives under `data/` (rather than
//! inline in `app_view.rs`) so the transfer-test harness can lift it
//! verbatim via `#[path]` instead of carrying a drift-prone copy,
//! and so the validation cases are reachable from `cargo test`
//! without standing up gpui.
//!
//! Empty input parses to `Ok(Vec::new())` rather than an error — the
//! UI layer decides whether "send 0 bytes" is a useful operation
//! (it isn't, and the modal surfaces "empty" inline), but the parser
//! itself reports only true syntax problems.

/// Parse a Send-Hex input string into raw bytes.
///
/// Returns `Err("odd number of hex digits")` if the cleaned input has
/// an odd character count, `Err("non-hex characters")` for any
/// non-`[0-9a-fA-F]` character that survives cleaning, and
/// `Ok(Vec::new())` for input that's empty or strips to empty.
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

#[cfg(test)]
mod tests {
    use super::*;

    // T1 from TESTING.md: ASCII passthrough.
    #[test]
    fn ascii_hello() {
        assert_eq!(parse_hex_string("48 65 6c 6c 6f").unwrap(), b"Hello");
    }

    // T3 from TESTING.md: control bytes, 0xff, DEL.
    #[test]
    fn binary_and_control_bytes() {
        assert_eq!(
            parse_hex_string("00 01 02 ff fe 7f").unwrap(),
            vec![0x00, 0x01, 0x02, 0xff, 0xfe, 0x7f]
        );
    }

    // T2 from TESTING.md: three encodings of "ABC" must all match.
    #[test]
    fn input_formats_are_equivalent() {
        let spaced = parse_hex_string("41 42 43").unwrap();
        let compact = parse_hex_string("414243").unwrap();
        let prefixed = parse_hex_string("0x41 0x42 0x43").unwrap();
        assert_eq!(spaced, b"ABC");
        assert_eq!(spaced, compact);
        assert_eq!(spaced, prefixed);
    }

    #[test]
    fn upper_x_prefix_strips() {
        assert_eq!(parse_hex_string("0X41").unwrap(), vec![0x41]);
    }

    #[test]
    fn commas_are_separators() {
        assert_eq!(parse_hex_string("41,42,43").unwrap(), b"ABC");
    }

    #[test]
    fn mixed_case_hex() {
        assert_eq!(
            parse_hex_string("DeAdBeEf").unwrap(),
            vec![0xde, 0xad, 0xbe, 0xef]
        );
    }

    #[test]
    fn leading_and_trailing_whitespace() {
        assert_eq!(parse_hex_string("  41  ").unwrap(), vec![0x41]);
    }

    // T4 from TESTING.md: validation paths.
    #[test]
    fn empty_input_is_ok_empty() {
        // Parser-level concern: empty input is syntactically valid.
        // The Send-Hex modal rejects 0-byte sends at a higher layer.
        assert_eq!(parse_hex_string("").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn bare_prefix_strips_to_empty() {
        // `0x` with nothing after strips to empty input.
        assert_eq!(parse_hex_string("0x").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn odd_digit_count_rejected() {
        assert_eq!(parse_hex_string("abc"), Err("odd number of hex digits"));
        // Pure-junk odd-length input hits the length check before the
        // non-hex check — parser reports the first problem in source
        // order, which is length. Documented so a future reader doesn't
        // assume "xyz" returns the more informative non-hex error.
        assert_eq!(parse_hex_string("xyz"), Err("odd number of hex digits"));
    }

    #[test]
    fn non_hex_characters_rejected() {
        // Even-length non-hex input clears the length check and lands
        // on the non-hex branch.
        assert_eq!(parse_hex_string("wxyz"), Err("non-hex characters"));
        // Length 2, single non-hex char.
        assert_eq!(parse_hex_string("0z"), Err("non-hex characters"));
    }
}
