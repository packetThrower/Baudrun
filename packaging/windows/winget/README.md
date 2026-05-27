# Winget manifest scaffold

Templates for getting Baudrun into the [Windows Package Manager
community repository](https://github.com/microsoft/winget-pkgs).

This directory is a **scaffold**, not an active install path —
winget doesn't read files from here. The actual install path is
a PR submitted upstream against `microsoft/winget-pkgs`. The
templates exist so the maintainer of that PR (us) doesn't have
to hand-write the YAML each release.

## Files

| File | Purpose |
|---|---|
| `packetThrower.Baudrun.locale.en-US.yaml` | Default-locale manifest. Mostly static across versions — copy as-is into each per-version dir, bump `PackageVersion` + `ReleaseNotesUrl`. |
| `packetThrower.Baudrun.yaml.template` | Version manifest. Substitute `${VERSION}`. |
| `packetThrower.Baudrun.installer.yaml.template` | Installer manifest. Substitute `${VERSION}`, `${RELEASE_DATE}`, `${SHA256_AMD64_MSI}`, `${SHA256_ARM64_MSI}`, `${PRODUCT_CODE_AMD64}`, `${PRODUCT_CODE_ARM64}`. |
| `rendered/<version>/` | Archived copy of the YAMLs submitted upstream for each version. Mirrors what's at `manifests/p/packetThrower/Baudrun/<version>/` in `microsoft/winget-pkgs`. |

## The submission, automated via CI (preferred)

Stable tags fire `.github/workflows/after_release.yml::publish_winget`,
which submits the winget-pkgs PR for us:

  1. release.yml publishes the GitHub Release (stable tag only —
     pre-release tags skip the MSI build and the winget submission).
  2. The Release-published event fires `after_release.yml`.
  3. The `publish_winget` job syncs the maintainer's
     `packetThrower/winget-pkgs` fork against upstream master, then
     runs `vedantmgoyal9/winget-releaser` to render the templates,
     commit them under `manifests/p/packetThrower/Baudrun/<version>/`
     on the fork, and open the PR against `microsoft/winget-pkgs`.
  4. The winget-pkgs validator runs on the PR; on success a
     moderator merges it.

The flow is hands-off after the tag push. If the release-event
trigger doesn't fire (rare GitHub Actions slow-publish race), use
the `workflow_dispatch` trigger from the Actions UI with the tag
name to re-run just the winget submission.

**One-time setup** (already done as of v0.13.0):

  * Personal access token (fine-grained) with **Contents: read +
    write** and **Pull requests: read + write** on the maintainer's
    `winget-pkgs` fork, stored as the `WINGET_TOKEN` repo secret.
  * The fork at `packetThrower/winget-pkgs` exists and is current
    enough for GitHub's `/merge-upstream` endpoint to fast-forward
    against upstream master.

## The submission, manually via wingetcreate

Fallback path if the automation is offline or for a non-release
emergency submission. `wingetcreate` is Microsoft's CLI for the
winget-pkgs repo:

```powershell
# Windows host. winget install Microsoft.WingetCreate (one-time).

$Version  = "0.12.0"
$MsiX64   = "https://github.com/packetThrower/Baudrun/releases/download/v$Version/Baudrun_${Version}_amd64_en-US.msi"
$MsiArm64 = "https://github.com/packetThrower/Baudrun/releases/download/v$Version/Baudrun_${Version}_arm64_en-US.msi"

# First-time submission: scaffolds the three YAMLs interactively
# from the .msi inspection (auto-detects both ProductCodes), then
# prompts for the locale fields.
wingetcreate new "$MsiX64,$MsiArm64"

# Subsequent version bumps: reuses the existing locale manifest
# and only updates URLs + SHA256 + ProductCode.
wingetcreate update packetThrower.Baudrun `
  --version $Version `
  --urls "$MsiX64,$MsiArm64" `
  --submit `
  --token $env:GITHUB_TOKEN
```

The `--submit` flag opens the PR for you. Without it, the YAMLs
land in a temp directory for review.

## The submission, manually via in-repo render

Targets the `winget-pkgs` fork's `manifests/p/packetThrower/Baudrun/<version>/` layout.

```bash
# From a fork of microsoft/winget-pkgs checked out locally:

VERSION=0.12.0
DATE=$(date -u +%Y-%m-%d)
DEST="manifests/p/packetThrower/Baudrun/$VERSION"
mkdir -p "$DEST"

# Pull the locale manifest in unchanged, then bump version fields.
cp /path/to/Baudrun/packaging/windows/winget/packetThrower.Baudrun.locale.en-US.yaml "$DEST/"
sed -i "s/^PackageVersion: .*/PackageVersion: $VERSION/" "$DEST/packetThrower.Baudrun.locale.en-US.yaml"

# SHA256s from the GitHub Release artifacts.
SHA_X64=$(curl -sL "https://github.com/packetThrower/Baudrun/releases/download/v$VERSION/Baudrun_${VERSION}_amd64_en-US.msi" | sha256sum | awk '{print $1}')
SHA_ARM64=$(curl -sL "https://github.com/packetThrower/Baudrun/releases/download/v$VERSION/Baudrun_${VERSION}_arm64_en-US.msi" | sha256sum | awk '{print $1}')

# ProductCodes: per-build GUIDs that change each release. Extract
# from each MSI with `msiextract --version` or `lessmsi list` on
# Linux/macOS; on Windows use the WindowsInstaller COM API.
# wingetcreate auto-detects if going via that path. The UpgradeCode
# is stable (defined in packaging/windows/wix/main.wxs); ProductCodes
# are not.
PRODUCT_CODE_X64='{REPLACE-WITH-X64-MSI-PRODUCT-GUID}'
PRODUCT_CODE_ARM64='{REPLACE-WITH-ARM64-MSI-PRODUCT-GUID}'

# Render the templates. The envsubst allowlist argument is
# load-bearing: the templates' `# yaml-language-server: $schema=…`
# line contains a literal `$schema` that bare envsubst would
# helpfully expand to nothing, breaking the schema header and
# failing winget validation with "schema header URL does not
# match the expected pattern" (caught on #376876's first
# validation run). The allowlist tells envsubst to substitute
# only our six known variables and pass `$schema` through.
ENVSUBST_VARS='${VERSION} ${RELEASE_DATE} ${SHA256_AMD64_MSI} ${SHA256_ARM64_MSI} ${PRODUCT_CODE_AMD64} ${PRODUCT_CODE_ARM64}'

VERSION="$VERSION" RELEASE_DATE="$DATE" \
  SHA256_AMD64_MSI="$SHA_X64" SHA256_ARM64_MSI="$SHA_ARM64" \
  PRODUCT_CODE_AMD64="$PRODUCT_CODE_X64" PRODUCT_CODE_ARM64="$PRODUCT_CODE_ARM64" \
  envsubst "$ENVSUBST_VARS" \
    < /path/to/Baudrun/packaging/windows/winget/packetThrower.Baudrun.yaml.template \
    > "$DEST/packetThrower.Baudrun.yaml"

VERSION="$VERSION" RELEASE_DATE="$DATE" \
  SHA256_AMD64_MSI="$SHA_X64" SHA256_ARM64_MSI="$SHA_ARM64" \
  PRODUCT_CODE_AMD64="$PRODUCT_CODE_X64" PRODUCT_CODE_ARM64="$PRODUCT_CODE_ARM64" \
  envsubst "$ENVSUBST_VARS" \
    < /path/to/Baudrun/packaging/windows/winget/packetThrower.Baudrun.installer.yaml.template \
    > "$DEST/packetThrower.Baudrun.installer.yaml"

# Validate before pushing — same checks the upstream CI runs.
winget validate --manifest "$DEST"

# Open a PR against microsoft/winget-pkgs.
```

## Notes

- **PackageIdentifier casing** is `packetThrower.Baudrun`. winget
  accepts mixed-case publishers (`git/git`, `Microsoft/PowerShell`
  both exist); we match the GitHub org casing.
- **Moniker `baudrun`** lets users run `winget install baudrun`
  instead of the full identifier.
- **Code signing** is not required for winget acceptance. Users
  will still see SmartScreen on first run (same UX as the manual
  installer) until reputation builds up.
- **Both architectures ship .msi** (since v0.13.0). The MSI build
  is driven by cargo-wix + system-installed WiX 3.14 in
  `.github/workflows/release.yml`; the WiX source is
  `packaging/windows/wix/main.wxs` (overridden from cargo-wix's
  default `wix/` discovery via `[package.metadata.wix].include`
  in Cargo.toml). v0.12.0 shipped arm64 as NSIS only because
  cargo-packager's bundled WiX 3.11 predated `-arch arm64`.
- **Schema version is 1.12.0** (current as of the 0.12.0 submission).
  winget-pkgs accepts older schemas back to 1.0; bump templates +
  rendered when 1.13+ ships.
