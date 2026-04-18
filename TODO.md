# TODO

## Distribution

- [ ] **Code sign + notarize macOS binary.** Requires enrollment in the
      Apple Developer Program ($99/yr). Steps once enrolled:
      1. Create a Developer ID Application certificate in Apple's
         developer portal; download as .p12.
      2. Add as GitHub Actions secrets: `APPLE_CERT_P12` (base64 of the
         .p12), `APPLE_CERT_PASSWORD`, `APPLE_ID`, `APPLE_APP_PASSWORD`
         (app-specific password), `APPLE_TEAM_ID`.
      3. In `build-macos` job: import the cert into the keychain, sign
         with `codesign`, submit to Apple for notarization via
         `xcrun notarytool`, then `xcrun stapler staple` the result.
- [ ] **Code sign Windows binary (Authenticode).** Certificate from
      DigiCert / Sectigo / SSL.com (~$200+/yr; EV cert is pricier but
      skips SmartScreen warmup). Add as secrets, sign the .exe with
      `signtool` in the `build-windows` job.
- [ ] **Public downloads for a private source repo.** Two-repo setup:
      1. Create public `otec-it/Seriesly-downloads` (empty, README points
         at Releases).
      2. Generate a fine-grained PAT scoped only to that repo with
         `Contents: Read and write`. Add to the private repo's Actions
         secrets as `RELEASES_REPO_TOKEN`.
      3. In `release.yml`, add `repository: otec-it/Seriesly-downloads`
         and `token: ${{ secrets.RELEASES_REPO_TOKEN }}` to the
         `softprops/action-gh-release` step.
      Do this **after** signing is in place so the first public release
      is already a trustworthy binary.
