// Lightweight update checker. Hits the GitHub Releases API once per
// app launch, compares the newest tag to the running version, and
// returns a descriptor for the footer toast to render. No auto-update
// — the user pilots the actual upgrade.
//
// GitHub's REST API is CORS-friendly and the unauthenticated rate
// limit is 60 req/hr/IP, which is plenty for a once-per-launch check.
// The request runs from the renderer (no Rust involvement) so there's
// no new dependency or backend surface.
//
// The repo is hardcoded here — if Baudrun ever forks or the user
// runs a mirror, they can edit this one constant.

const REPO = "packetThrower/Baudrun";

export type AvailableUpdate = {
  /** Version string without the leading "v" — e.g. `"1.0.0"` or `"1.1.0-beta.2"`. */
  version: string;
  /** Full tag name — e.g. `"v1.0.0"`. */
  tag: string;
  /** True if the release has a `-pre` suffix (alpha / beta / rc). */
  prerelease: boolean;
  /** Browser-facing URL for the release notes. */
  url: string;
  /** The current running version, carried through for UI display. */
  currentVersion: string;
};

type Parsed = {
  major: number;
  minor: number;
  patch: number;
  /** Pre-release identifier (the part after the `-`), or `null` for stable. */
  pre: string | null;
};

function parseSemver(raw: string): Parsed | null {
  const cleaned = raw.trim().replace(/^v/i, "");
  const m = cleaned.match(/^(\d+)\.(\d+)\.(\d+)(?:-(.+))?$/);
  if (!m) return null;
  return {
    major: Number(m[1]),
    minor: Number(m[2]),
    patch: Number(m[3]),
    pre: m[4] ?? null,
  };
}

/**
 * Classic SemVer precedence: compare numeric parts in order; a version
 * WITHOUT a pre-release suffix is always greater than the same numeric
 * core WITH one (1.0.0 > 1.0.0-rc.2). Within pre-releases, compare
 * lexicographically after splitting on `.` — enough for the alpha.1 <
 * alpha.2 < beta.1 ordering Baudrun's release workflow uses.
 *
 * Returns positive if `a > b`, negative if `a < b`, 0 if equal.
 */
export function compareSemver(a: string, b: string): number {
  const pa = parseSemver(a);
  const pb = parseSemver(b);
  if (!pa && !pb) return 0;
  if (!pa) return -1;
  if (!pb) return 1;
  if (pa.major !== pb.major) return pa.major - pb.major;
  if (pa.minor !== pb.minor) return pa.minor - pb.minor;
  if (pa.patch !== pb.patch) return pa.patch - pb.patch;
  if (pa.pre === null && pb.pre === null) return 0;
  if (pa.pre === null) return 1;
  if (pb.pre === null) return -1;
  return comparePreReleaseIds(pa.pre, pb.pre);
}

function comparePreReleaseIds(a: string, b: string): number {
  const as = a.split(".");
  const bs = b.split(".");
  const n = Math.max(as.length, bs.length);
  for (let i = 0; i < n; i++) {
    const ai = as[i];
    const bi = bs[i];
    if (ai === undefined) return -1; // shorter comes first (1.0.0-alpha < 1.0.0-alpha.1)
    if (bi === undefined) return 1;
    const an = Number(ai);
    const bn = Number(bi);
    if (!Number.isNaN(an) && !Number.isNaN(bn)) {
      if (an !== bn) return an - bn;
    } else {
      const cmp = ai.localeCompare(bi);
      if (cmp !== 0) return cmp;
    }
  }
  return 0;
}

type Release = {
  tag_name: string;
  html_url: string;
  prerelease: boolean;
  draft: boolean;
};

/** Hard cap on a single GitHub API response. A real /releases/latest
 *  payload runs ~5 KB and a /releases?per_page=10 list runs ~50 KB.
 *  100 KB is well above the ceiling, well below the renderer-DoS
 *  threshold a hostile redirect could otherwise leverage. */
const MAX_RESPONSE_BYTES = 100 * 1024;
const FETCH_TIMEOUT_MS = 10_000;

async function fetchReleases(includePrereleases: boolean): Promise<Release[]> {
  // `/releases/latest` returns a single JSON object and skips pre-releases + drafts.
  // `/releases?per_page=10` returns an array newest-first, including pre-releases.
  // Fetching 10 covers typical release cadence with headroom.
  const url = includePrereleases
    ? `https://api.github.com/repos/${REPO}/releases?per_page=10`
    : `https://api.github.com/repos/${REPO}/releases/latest`;

  // AbortController gives us a hard deadline AND a way to short-circuit
  // a slow / hung fetch — important on flaky networks where the
  // updater check would otherwise spin until the OS-level TCP timeout.
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);

  let resp: Response;
  try {
    resp = await fetch(url, {
      signal: controller.signal,
      headers: {
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
      },
    });
  } finally {
    clearTimeout(timer);
  }

  if (!resp.ok) throw new Error(`GitHub API ${resp.status}`);

  // Fast path: trust the Content-Length header when present. GitHub
  // sets it on every JSON response and an attacker who controls the
  // body but not the headers (e.g. a TLS MITM that can't forge certs
  // — i.e. nobody on a properly-secured connection) doesn't matter
  // here. Belt-and-suspenders is the byte-count loop below.
  const declaredSize = Number(resp.headers.get("content-length"));
  if (Number.isFinite(declaredSize) && declaredSize > MAX_RESPONSE_BYTES) {
    throw new Error(
      `GitHub API response declares ${declaredSize} bytes, capped at ${MAX_RESPONSE_BYTES}`,
    );
  }

  // Read incrementally so a server that lies about Content-Length (or
  // omits it on a chunked response) can't blow past the cap.
  const reader = resp.body?.getReader();
  if (!reader) throw new Error("GitHub API response had no body");
  const chunks: Uint8Array[] = [];
  let total = 0;
  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    if (!value) continue;
    total += value.byteLength;
    if (total > MAX_RESPONSE_BYTES) {
      reader.cancel();
      throw new Error(
        `GitHub API response exceeded ${MAX_RESPONSE_BYTES} bytes`,
      );
    }
    chunks.push(value);
  }
  const merged = new Uint8Array(total);
  let offset = 0;
  for (const c of chunks) {
    merged.set(c, offset);
    offset += c.byteLength;
  }
  const body = new TextDecoder().decode(merged);
  const data = JSON.parse(body);
  if (Array.isArray(data)) return data as Release[];
  return [data as Release];
}

/**
 * Query GitHub for the newest release eligible under the given policy
 * and return an {@link AvailableUpdate} if it's ahead of `currentVersion`.
 * Returns `null` when there's nothing newer, when the request fails,
 * or when parsing falls over — callers treat it as "no update".
 */
export async function checkForUpdate(
  currentVersion: string,
  includePrereleases: boolean,
): Promise<AvailableUpdate | null> {
  try {
    const releases = await fetchReleases(includePrereleases);
    const eligible = releases
      .filter((r) => !r.draft)
      .filter((r) => includePrereleases || !r.prerelease);
    if (eligible.length === 0) return null;

    // Sort newest-first by semver so `/releases?per_page=10` order is
    // normalized even if GitHub's default sort drifts or a maintainer
    // re-tags an older release.
    eligible.sort((a, b) =>
      compareSemver(b.tag_name, a.tag_name),
    );
    const newest = eligible[0];
    const trimmed = newest.tag_name.replace(/^v/i, "");

    if (compareSemver(trimmed, currentVersion) <= 0) return null;

    return {
      version: trimmed,
      tag: newest.tag_name,
      prerelease: newest.prerelease,
      url: newest.html_url,
      currentVersion,
    };
  } catch (err) {
    console.warn("update check failed:", err);
    return null;
  }
}
