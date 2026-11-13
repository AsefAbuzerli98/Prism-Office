# Releasing Prism Office

Releases are built by `.github/workflows/release.yml` when a tag `v*` is
pushed (or by running the workflow manually). It produces a draft GitHub
release with:

| Platform | Artifacts |
|----------|-----------|
| macOS (Apple Silicon + Intel, universal) | `.dmg`, `.app.tar.gz` (+ `.sig`) |
| Windows x64 | `.msi`, `-setup.exe` (NSIS) (+ `.sig`) |
| Windows ARM64 | `.msi`, `-setup.exe` (NSIS) (+ `.sig`) |
| all | `latest.json` (updater manifest) |

The version comes from `src-tauri/tauri.conf.json` → `version`; keep it in
step with the tag.

## One-time setup (repository secrets)

Everything is optional — without a secret the matching step is skipped and
the artifacts are unsigned.

### macOS signing and notarization

1. Export your *Developer ID Application* certificate as `.p12`.
2. Secrets: `APPLE_CERTIFICATE` (base64 of the `.p12`),
   `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`
   (e.g. `Developer ID Application: Name (TEAMID)`).
3. Notarization: `APPLE_ID`, `APPLE_PASSWORD` (an app-specific password),
   `APPLE_TEAM_ID`.

The app is built with the hardened runtime and `src-tauri/Entitlements.plist`
(camera, microphone, local network server for Live Presenting).

### Windows Authenticode

Secrets: `WINDOWS_CERTIFICATE` (base64 of a code-signing `.pfx`),
`WINDOWS_CERTIFICATE_PASSWORD`. The workflow imports it and signs with
SHA-256 and an RFC 3161 timestamp. (EV certificates on hardware tokens or
Azure Trusted Signing need a `bundle.windows.signCommand` instead.)

### Updater keys

The updater only installs packages signed with your key.

```bash
pnpm tauri signer generate -w ~/.tauri/prism-office.key
```

- Put the **public** key into `src-tauri/tauri.conf.json` →
  `plugins.updater.pubkey` and commit it.
- Store the **private** key as the `TAURI_SIGNING_PRIVATE_KEY` secret (and its
  password as `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`). Never commit it.

Until a public key is configured, Settings → Updates reports that updates
are not configured and nothing is downloaded.

## Checklist

1. `cargo test --workspace`, `pnpm test`, `cargo run -p prism-fuzz --release -- all 600`.
2. Bump `version` in `src-tauri/tauri.conf.json`, `Cargo.toml`, `package.json`.
3. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`.
4. Check the draft release, install on each platform, publish.
