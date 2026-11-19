# Releasing pin

## Publishing a release

1. Bump `version` in the root `Cargo.toml` and merge it into `main`.
2. Tag the merge and push the tag:

   ```sh
   git tag v0.1.2
   git push origin v0.1.2
   ```

`.github/workflows/release.yml` then builds a universal `Pin.app` (Apple Silicon and Intel)
with `scripts/bundle-macos.sh` and attaches `Pin.zip` to a GitHub release.

Build the same locally with `scripts/bundle-macos.sh`, or
`TARGETS=aarch64-apple-darwin scripts/bundle-macos.sh` for this Mac only.

## App icon

The icon is drawn in `assets/icon.svg` (the README logo is `assets/logo.svg`). After changing
it, run `scripts/make-icon.sh` and commit the new `packaging/macos/Pin.icns`.

## Signing

macOS remembers keychain access per app signature. Ad-hoc signed releases get a new signature
every version, so users are asked for their password once after each update. A signing
certificate of your own keeps the signature stable:

1. In Keychain Access, choose **Certificate Assistant → Create a Certificate**. Pick a name
   (for example `Pin Release`), Identity Type **Self-Signed Root**, Certificate Type
   **Code Signing**.
2. Export the certificate with its private key as a `.p12` file with a password.
3. Add three repository secrets: `MACOS_CERT_P12` (the file as base64:
   `base64 -i cert.p12 | pbcopy`), `MACOS_CERT_PASSWORD` and `MACOS_CERT_NAME` (the name
   from step 1).

The release workflow signs with it whenever these secrets exist and falls back to ad-hoc
signing otherwise.
