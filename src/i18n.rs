//! UI-language resolution + locale install.
//!
//! Phase A.1 of the localization work (see TODO.md "Localization
//! (i18n)"): this module resolves which locale the app should run in
//! and installs it via `gpui_component::set_locale`, which drives the
//! `rust-i18n` global that gpui-component's own widget chrome
//! (dialogs, pagination, calendar, etc.) reads. gpui-component bundles
//! `en`, `zh-CN`, `zh-HK`, and `it` translations, so this alone gives
//! a Chinese-locale user translated widget chrome before Baudrun's own
//! strings are extracted (that's Phase A.2, which adds `rust-i18n` +
//! `locales/*.yml` and wraps Baudrun's ~180 strings in `t!()` — the
//! same global locale set here will cover both layers).
//!
//! Requested in issue #72.

/// Locale codes Baudrun ships (or intends to ship) UI translations
/// for, each paired with its endonym (name in its own language) so
/// the future language picker reads correctly to a user already in
/// the "wrong" locale. English first, then by endonym.
///
/// `zh-CN` (Simplified Chinese) is the first non-English target —
/// issue #72's reporter writes Simplified Chinese, and gpui-component
/// already ships matching `zh-CN` widget strings. Traditional
/// (`zh-TW` / `zh-HK`) is intentionally absent until someone asks:
/// see `match_os_locale`, which routes Traditional OS locales to
/// English rather than silently showing a Traditional user
/// Simplified text.
pub const SUPPORTED: &[(&str, &str)] = &[("en", "English"), ("zh-CN", "简体中文")];

/// Default / fallback locale. Must be a code gpui-component itself
/// ships, and the language every Baudrun string is authored in.
pub const DEFAULT: &str = "en";

/// Resolve the active locale and install it via
/// `gpui_component::set_locale`. Called once at boot from `main::run`
/// with the persisted `Settings.locale`. Returns the installed code
/// for logging.
pub fn init(override_code: &str) -> &'static str {
    let resolved = resolve(override_code);
    gpui_component::set_locale(resolved);
    log::info!("i18n: locale set to {resolved} (settings.locale={override_code:?})");
    resolved
}

/// Pure resolution — no side effects, unit-testable. Precedence:
///
/// 1. `override_code` — the persisted `Settings.locale` when the user
///    picked one. Empty string means "unset / follow the OS".
/// 2. The OS locale via `sys-locale`, matched against `SUPPORTED`
///    with Simplified/Traditional Chinese awareness (see
///    `match_os_locale`).
/// 3. `DEFAULT` (English).
pub fn resolve(override_code: &str) -> &'static str {
    // 1. Explicit user choice from settings.
    if !override_code.is_empty() {
        if let Some(code) = supported(override_code) {
            return code;
        }
        log::warn!("i18n: settings.locale {override_code:?} not in SUPPORTED — ignoring");
    }

    // 2. OS locale. `sys-locale` returns the platform's best guess:
    // `defaultLocale` on macOS, `LANG`/`LC_ALL` on Linux,
    // `GetUserDefaultLocaleName` on Windows.
    if let Some(os) = sys_locale::get_locale() {
        if let Some(code) = match_os_locale(&os) {
            return code;
        }
    }

    // 3. Fallback.
    DEFAULT
}

/// Map a raw OS locale string to a `SUPPORTED` code, or `None`.
///
/// The important case is Chinese. PortFinder (the sibling app whose
/// i18n this is modelled on) strips every OS locale to its bare
/// language subtag — `zh-Hans-CN` → `zh` — and matches that. For its
/// seven Latin-script languages that's fine, but bare `zh` never
/// matches a region-qualified `zh-CN`, so a mainland user would fall
/// through to English. Baudrun keeps the script/region subtags and
/// decides Simplified vs Traditional from them.
fn match_os_locale(os: &str) -> Option<&'static str> {
    let lower = os.to_ascii_lowercase();
    // BCP-47 uses `-`; some Unix locales use `_`; a trailing
    // `.UTF-8`/`.utf8` codeset is `.`-separated. Split on all three.
    let mut subtags = lower.split(['-', '_', '.']).filter(|s| !s.is_empty());
    let lang = subtags.next()?;
    let rest: Vec<&str> = subtags.collect();

    if lang == "zh" {
        // Traditional if the script (`Hant`) or a Traditional-using
        // region (Taiwan / Hong Kong / Macau) is present. Everything
        // else — `Hans`, `CN`, `SG`, or a bare `zh` — is Simplified,
        // which is what mainland systems report and the majority
        // variant to default an ambiguous `zh` to.
        let traditional = rest
            .iter()
            .any(|t| matches!(*t, "hant" | "tw" | "hk" | "mo"));
        if traditional {
            // No Traditional catalog yet — prefer English over
            // showing a Traditional reader Simplified text. Reorders
            // to a real match automatically once `zh-TW`/`zh-HK`
            // join SUPPORTED.
            return supported("zh-tw").or_else(|| supported("zh-hk"));
        }
        return supported("zh-cn");
    }

    // Non-Chinese: match the OS language subtag against the language
    // portion of each SUPPORTED code (so `en-US` → `en`).
    SUPPORTED
        .iter()
        .find(|(code, _)| {
            code.split('-')
                .next()
                .map(str::to_ascii_lowercase)
                .as_deref()
                == Some(lang)
        })
        .map(|(code, _)| *code)
}

/// Case-insensitive lookup of a `SUPPORTED` code, returning the
/// canonically-cased entry (`"zh-cn"` → `"zh-CN"`).
fn supported(code: &str) -> Option<&'static str> {
    SUPPORTED
        .iter()
        .find(|(c, _)| c.eq_ignore_ascii_case(code))
        .map(|(c, _)| *c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_override_wins_and_is_case_insensitive() {
        assert_eq!(resolve("zh-CN"), "zh-CN");
        assert_eq!(resolve("zh-cn"), "zh-CN");
        assert_eq!(resolve("en"), "en");
    }

    #[test]
    fn simplified_chinese_os_locales_resolve_to_zh_cn() {
        // The bug PortFinder's bare-language strip would hit: every
        // one of these must reach zh-CN, not fall through to English.
        for os in [
            "zh",
            "zh-CN",
            "zh_CN",
            "zh-Hans",
            "zh-Hans-CN",
            "zh-CN.UTF-8",
            "zh-SG",
        ] {
            assert_eq!(match_os_locale(os), Some("zh-CN"), "{os}");
        }
    }

    #[test]
    fn traditional_chinese_falls_through_until_shipped() {
        // No Traditional catalog yet → None (caller uses English)
        // rather than silently serving Simplified.
        for os in ["zh-TW", "zh-Hant", "zh-Hant-TW", "zh-HK", "zh-MO"] {
            assert_eq!(match_os_locale(os), None, "{os}");
        }
    }

    #[test]
    fn unknown_os_locale_is_none() {
        assert_eq!(match_os_locale("de-DE"), None);
        assert_eq!(match_os_locale("fr"), None);
    }

    #[test]
    fn english_os_locales_match() {
        assert_eq!(match_os_locale("en-US"), Some("en"));
        assert_eq!(match_os_locale("en_GB.UTF-8"), Some("en"));
    }
}
