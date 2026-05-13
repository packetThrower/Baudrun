//! Boot-time update check against the project's GitHub Releases.
//!
//! Posture by design: **detection only**. We never download a
//! replacement bundle or rewrite anything on disk. The check
//! discovers the latest tag, compares it against
//! `CARGO_PKG_VERSION`, and if a newer release exists publishes
//! a [`UpdateState`] global that the chrome reads to paint amber
//! indicators (Settings rail's `Updates` row + sidebar gear icon
//! in `app_view::sidebar_header`). The user opens the Updates
//! pane in Settings to see the version + release-notes link and
//! click through to the Releases page in their browser.
//!
//! Why no auto-install:
//!   * macOS code-signing + notarization haven't shipped yet.
//!     Auto-replacing a running unsigned bundle would trip
//!     Gatekeeper on every launch.
//!   * The "swap the live binary on quit" dance (Sparkle-style
//!     relauncher helper) is real work that's hard to verify on
//!     all three platforms.
//!   * Detection-only keeps the user in control — they pick
//!     whether and when to install.
//!
//! The fetch is sync (`ureq`), runs on a single
//! `gpui::BackgroundExecutor` task at boot. Worst case the user
//! is offline and the future resolves to `None` — no UI on the
//! amber-indicator path, exactly as if no update existed.

use std::time::Duration;

use serde::Deserialize;

/// Live state of the boot-time update check. Installed as a
/// gpui `Global` so any render path can read it cheaply (a
/// `.clone()` of a small Option). `None` means "no check has
/// completed yet" OR "the check ran and we're already on the
/// newest release" — render code treats both identically (no
/// amber dot painted).
#[derive(Debug, Clone, Default)]
pub struct UpdateState {
    /// `Some` when a newer release is available. Fields are the
    /// pieces of [`Release`] the chrome actually reads.
    pub available: Option<UpdateAvailable>,
}

impl gpui::Global for UpdateState {}

/// Available-update payload exposed to the chrome.
#[derive(Debug, Clone)]
pub struct UpdateAvailable {
    /// Bare tag string (no leading `v`), e.g. `"0.9.8"` or
    /// `"0.9.8-beta.2"`. Compared against `dismissed_update_version`
    /// for re-prompting suppression.
    pub version: String,
    /// Browser URL for the GitHub Releases page entry. Opened
    /// when the user clicks the Updates pane's "View release"
    /// button.
    pub html_url: String,
    /// Optional release-notes body (Markdown). Truncated for
    /// in-app display — the full notes are on the html_url.
    pub notes: String,
}

/// GitHub Releases API response shape — we only deserialize the
/// fields we actually use. `#[serde(default)]` on optional
/// strings so partial responses don't fail the whole parse.
#[derive(Debug, Deserialize)]
struct Release {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

/// Query the project's GitHub Releases API for the newest release
/// that the user's `include_prerelease` preference allows.
///
/// Returns `Ok(None)` when no release is newer than
/// `current_version`, when the network call fails, or when the
/// response shape doesn't parse. Returns `Err` only for
/// programmer-error paths (the current version string didn't
/// parse as semver — should never happen since it comes from
/// `env!("CARGO_PKG_VERSION")` which Cargo validates at compile
/// time).
///
/// Blocking — call from a `BackgroundExecutor` task, never from
/// the render thread.
pub fn check_for_update(
    current_version: &str,
    include_prerelease: bool,
) -> Result<Option<UpdateAvailable>, semver::Error> {
    let current = semver::Version::parse(current_version)?;
    let releases = match fetch_releases(include_prerelease) {
        Some(r) => r,
        None => return Ok(None),
    };
    let newest = releases
        .into_iter()
        // Drafts are unpublished and shouldn't show up via the
        // public API, but filter defensively in case GitHub's
        // pagination ever leaks one.
        .filter(|r| !r.draft)
        // Hide pre-releases unless the user opted in.
        .filter(|r| include_prerelease || !r.prerelease)
        // Parse each tag's semver; tags that don't parse get
        // silently skipped. Most repo tags are `vX.Y.Z[-pre]`
        // — strip the leading `v`.
        .filter_map(|r| {
            let bare = r.tag_name.strip_prefix('v').unwrap_or(&r.tag_name);
            let ver = semver::Version::parse(bare).ok()?;
            Some((ver, r))
        })
        // Pick the maximum by version, NOT by API-order, because
        // GitHub sorts by created_at and a backported patch on
        // an older line could land newest-by-date.
        .max_by(|(a, _), (b, _)| a.cmp(b));
    let Some((newest_ver, release)) = newest else {
        return Ok(None);
    };
    if newest_ver <= current {
        return Ok(None);
    }
    Ok(Some(UpdateAvailable {
        version: newest_ver.to_string(),
        html_url: release.html_url,
        notes: release.body,
    }))
}

/// Hit the GitHub Releases API. Returns `None` on any error —
/// the caller treats that the same as "no update found", so the
/// user never sees a "check failed" diagnostic. Errors land in
/// the standard log sink at `info!` level (not `warn!` /
/// `error!` — a transient offline state isn't a problem to
/// surface).
fn fetch_releases(include_prerelease: bool) -> Option<Vec<Release>> {
    // Stable-only path uses `/releases/latest` (single object).
    // Pre-release path needs `/releases` (paginated; first page
    // is 30 entries which is plenty since we only want the
    // newest). Wrapping the singleton in a Vec keeps the
    // downstream filter chain uniform.
    let url = if include_prerelease {
        "https://api.github.com/repos/packetThrower/Baudrun/releases?per_page=30"
    } else {
        "https://api.github.com/repos/packetThrower/Baudrun/releases/latest"
    };

    // 5s connect + 5s read — long enough that a slow Wi-Fi hop
    // doesn't false-negative, short enough that a captive-portal
    // hang doesn't keep the worker thread blocked forever.
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(5))
        .user_agent(concat!(
            "Baudrun/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/packetThrower/Baudrun)"
        ))
        .build();
    let response = match agent.get(url).call() {
        Ok(r) => r,
        Err(err) => {
            log::info!("update check: HTTP {err}");
            return None;
        }
    };
    let parsed = if include_prerelease {
        response.into_json::<Vec<Release>>()
    } else {
        response.into_json::<Release>().map(|r| vec![r])
    };
    match parsed {
        Ok(v) => Some(v),
        Err(err) => {
            log::info!("update check: parse failed: {err}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_version_parses_as_semver() {
        // Sanity that the build-time-substituted version string
        // is parseable — `env!("CARGO_PKG_VERSION")` is what the
        // boot-time check feeds into `check_for_update`.
        let raw = env!("CARGO_PKG_VERSION");
        semver::Version::parse(raw).expect("CARGO_PKG_VERSION must be valid semver");
    }
}
