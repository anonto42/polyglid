# PolyGlid

PolyGlid is a local-first, cross-platform desktop application for organizing
and managing local project workspaces. It is built in Rust with a native GPUI
desktop and a Dioxus website, and is
developed one working product phase at a time.

[![GitHub repository](https://img.shields.io/badge/GitHub-anonto42%2Fpolyglid-181717?logo=github)](https://github.com/anonto42/polyglid)
[![Rust](https://img.shields.io/badge/Rust-2021-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![GPUI](https://img.shields.io/badge/GPUI-0.2-4B7BEC)](https://crates.io/crates/gpui)
[![License](https://img.shields.io/badge/License-MIT%20or%20Apache--2.0-blue)](LICENSE-APACHE)

## Current Functionality

- Open the local PolyGlid workspace.
- Discover persisted projects.
- Create, select, and rename projects.
- Remove a project from the catalog.
- Permanently delete a project folder after explicit confirmation.
- Switch between configured local workspaces.

Future product areas are intentionally absent and will be introduced only when
their development phase begins.

## Repository Layout

```text
apps/desktop/       GPUI desktop editor workbench
apps/website/       Dioxus website and GitHub Pages bundle
crates/client/      UI-independent controllers and local adapter
crates/config/      persisted configuration models
crates/core/        local project and application services
crates/events/      shared client event types
crates/plugin-api/  plugin contracts and capability types
crates/runtime/     Wasmtime component runtime
contracts/          host/plugin component contract
scripts/            CI, packaging, site, and release automation
```

## Development Setup

Install [Nix](https://nixos.org/download/) with flakes enabled, then enter the
reproducible development environment:

```bash
nix develop
```

The shell provides Rust, Moon, the Dioxus CLI for the website, Go, Git, and the
native Linux libraries required by GPUI. It supports Linux x86-64/ARM64 and macOS
Apple Silicon. On macOS, Xcode Command Line Tools are also required.

Run the desktop application:

```bash
cargo run -p polyglid-desktop
```

Run workspace checks:

```bash
cargo check --workspace
cargo test --workspace
```

Moon can orchestrate formatting, checks, tests, builds, and the website bundle:

```bash
moon run :format
moon run :check
moon run :test
moon run :build
moon run website:bundle
```

The full Rust verification used by CI is also available locally:

```bash
scripts/ci/verify-rust.sh
```

## Runtime Data

PolyGlid stores its local data in `~/.polyglid` on Unix-like systems and
`%LOCALAPPDATA%/PolyGlid` on Windows. Use these environment variables to
override the defaults when developing or testing:

| Variable | Purpose |
| --- | --- |
| `POLYGLID_DATA_DIR` | Application data directory |
| `POLYGLID_WORKSPACE_ROOT` | Default project workspace |

## CI and Releases

Every branch push and pull request to `main` runs formatting, checks, tests,
builds, and automation validation for affected projects. Pull request titles
use Conventional Commits to declare release intent:

| Pull request title | Version result |
| --- | --- |
| `feat!: change a public contract` | Major |
| `feat(desktop): add project search` | Minor |
| `fix(core): reject an unsafe path` | Patch |
| `docs: explain workspace setup` | No product release |

After changes land on `main`, Release Please prepares the release. Approved
releases are packaged for Linux, Windows, and macOS with checksums. Native
artifacts are currently unsigned pending release-key setup.

The website is built and deployed to GitHub Pages after successful `main` CI.

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md),
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), and [SECURITY.md](SECURITY.md)
before opening an issue or pull request.

## License

PolyGlid is dual-licensed under [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE), at your option.
