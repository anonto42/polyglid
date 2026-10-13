# PolyGlid Project Memory

## Current Product Phase

PolyGlid is a local-first, cross-platform Rust desktop application. The active
desktop is an editor-first GPUI workbench with:

- traversal-safe workspace browsing;
- document tabs, editable buffers, dirty state, and save support;
- project, target, execution, report, and plugin snapshot views;
- command and shell navigation foundations.

Additional product mutations must be connected through `DesktopControllers`
one complete workflow at a time.

## Workspace Shape

- `apps/desktop` — GPUI 0.2 native editor workbench.
- `apps/website` — Dioxus 0.7 Web product website and Pages bundle.
- `crates/client` — UI-independent gateway, models, and feature controllers.
- `crates/core` — domain services, SQLite stores, security, and execution.
- `crates/config` — persisted configuration and plugin registry.
- `crates/events` — shared event types.
- `crates/plugin-api` — plugin contracts and capability types.
- `crates/runtime` — Wasmtime component runtime.
- `contracts/polyglid.wit` — host/plugin component contract.
- `scripts` — Moon project for CI, site, packaging, and release automation.

Rust 2021 is used across the workspace and unsafe code is forbidden.

## Runtime Data

- `POLYGLID_DATA_DIR` overrides the application data folder.
- The default data folder is `~/.polyglid` on Unix-like systems and
  `%LOCALAPPDATA%/PolyGlid` on Windows.
- `POLYGLID_WORKSPACE_ROOT` overrides the default project workspace.
- The default workspace is `~/polyglid-projects`.

## Branch Roles

- `main` — active PolyGlid development.
- `reference/legacy-main` — legacy code, data, and ideas.
- `reference/zed-base` — Zed/GPUI design and implementation reference.

Do not develop product features directly on a reference branch.
