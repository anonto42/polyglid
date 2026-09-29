# Project Scripts

Repository automation lives here instead of being embedded in workflow YAML.

- `dev/` contains development wrappers (desktop live-reload serving, plain
  runs, and `graphify-rs` knowledge-graph tasks) used by the root `Makefile`
  and Moon tasks. See `dev/README.md`.
- `ci/` contains validation (including release-title tests), pinned tool setup,
  platform dependency setup, and workspace verification.
- `release/` contains release lockfile synchronization, platform packaging,
  package validation, AppImage metadata validation, and GitHub release
  publication. Every platform archive bundles the runtime-directory guidance
  used by first-run setup. The Windows release also produces a WiX v4 MSI with
  Start Menu registration and major-upgrade handling. The macOS release also
  produces an ARM64 `.app` bundle inside a `.dmg` with an Applications alias.
  Publishing generates `polyglid-update.json` beside `SHA256SUMS` for future
  automatic update clients.
  The manifest and checksum set are verified before any GitHub upload.
- `site/` contains the reproducible Dioxus Web production build.

Run scripts from the repository root unless a script says otherwise.
