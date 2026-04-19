# Arch Linux packaging

`PKGBUILD` for the Arch User Repository. The package is
`seriesly-bin` — it downloads the pre-built `.deb` from the GitHub
release and extracts it, so the install is fast and no Go / Node /
Wails toolchain is needed on the user's machine.

## Release workflow

After a new tag ships:

1. **Bump `_tag`** in `PKGBUILD` to the new release.
2. **Update the SHA-256 sums.** Download the two `.deb` files from
   the GitHub release and compute sums:
   ```bash
   sha256sum seriesly_<VERSION>_amd64.deb seriesly_<VERSION>_arm64.deb
   ```
   Replace the `'SKIP'` entries with the real sums in the
   corresponding `sha256sums_<arch>` arrays. `SKIP` during local
   testing is fine; AUR requires real sums.
3. **Regenerate `.SRCINFO`** — AUR uses this as the source of truth
   for package metadata:
   ```bash
   makepkg --printsrcinfo > .SRCINFO
   ```
4. **Test the build locally** (on an Arch or container):
   ```bash
   makepkg -si
   ```
5. **Commit to the AUR** — AUR hosts each package in its own git
   repo. First time:
   ```bash
   git clone ssh://aur@aur.archlinux.org/seriesly-bin.git aur-seriesly-bin
   cp PKGBUILD .SRCINFO aur-seriesly-bin/
   cd aur-seriesly-bin
   git add PKGBUILD .SRCINFO
   git commit -m "seriesly-bin <version>"
   git push origin master
   ```

## License line

`license=('custom:unknown')` is a placeholder until the project
picks a LICENSE. When the license is chosen, update to the SPDX
identifier that `licensecheck` + `namcap` accept (e.g. `MIT`,
`Apache-2.0`, `GPL-3.0-or-later`).

## Why `-bin` and not a from-source build

A from-source PKGBUILD would need `go`, `nodejs`, `npm`, and the
Wails CLI installed to build, plus it would cover the webkit /
gtk3 dev packages as makedepends. That's legitimate and some users
prefer it, but it raises the install time from seconds to several
minutes and adds a much larger build-deps footprint.

The `-bin` pattern is canonical for Wails apps on the AUR — Wails'
own reference apps follow it. If demand for a from-source package
shows up, a second PKGBUILD (plain `seriesly`) can live next to
this one.

## Namcap

Before pushing to AUR, run:
```bash
namcap PKGBUILD
namcap seriesly-bin-<version>-x86_64.pkg.tar.zst
```

Expect a warning about the binary not being stripped (we pass
`options=(!strip)` because Wails already strips the release
build). Any other warning should be investigated.
