# PolyGlid Architecture

> This document distills the architecture rules from the reference workspace
> and adapts them to the current Rust workspace. The active product phase covers
> local project/workspace management and plugin execution.

## Repository Layout

```text
polyglid/
├── apps/
│   ├── desktop/          # GPUI native editor workbench
│   └── website/          # Dioxus Web site (GitHub Pages)
├── crates/
│   ├── client/           # UI-independent controllers and the local adapter
│   ├── config/           # Persisted configuration models and plugin registry
│   ├── core/             # Product policy, orchestration, and host use cases
│   ├── events/           # Shared event types between host, UI, and runtime
│   ├── plugin-api/       # Shared domain types (capabilities, reports, issues)
│   └── runtime/          # Wasmtime component loading, linking, execution
├── contracts/
│   └── polyglid.wit      # Canonical WIT contract for plugins
├── scripts/              # CI, verification, packaging, and release automation
└── .github/workflows/    # GitHub Actions (CI, release, pages)
```

## Crate Responsibilities

| Crate | Owns |
| --- | --- |
| `polyglid-core` | Use cases, policies, permissions, plugin orchestration, the `CoreEngine`, and the SQLite-backed store. |
| `polyglid-runtime` | Wasmtime component loading, linking, execution, and runtime error translation. |
| `polyglid-plugin-api` | Shared domain types: `Capability`, `CapabilityRequest`, `PluginManifest`, `PluginReport`, `Issue`, `Severity`. |
| `polyglid-config` | Config schema, defaults, validation, migrations, and persistence. |
| `polyglid-events` | Typed events emitted between the host, UI, runtime, and plugins. |
| `polyglid-client` | UI-independent controllers and the local gateway that the desktop shell calls. |
| `polyglid-desktop` | GPUI desktop window, editor buffers, navigation, state, and commands. |
| `polyglid-website` | Static marketing site rendered to GitHub Pages. |

The `contracts/polyglid.wit` file is the source of truth for the plugin ABI: the
`world security-tool` defines the exports plugins must provide (`execute`,
`metadata`, `required-capabilities`, `cli-panel`, `desktop-panel`) and the host
functions they may import (`dns.resolve`, `reports.write`).

## Architecture Rules

- **Contract first.** Change `contracts/polyglid.wit` before changing host or
  plugin behavior. Plugin output is structured data, not parsed terminal text.
  Keep exported report fields stable; add fields compatibly when possible.
  Regenerate or check host and plugin bindings after any contract change.
- **Deny by default.** Plugins are untrusted. Capabilities are denied unless
  explicitly granted by the permission store and the active security profile.
- **Least privilege.** Host-call capabilities (`dns`, `reports`) are scoped to
  the run target. Filesystem, process, environment, and raw network access are
  never exposed without a designed host capability and tests.
- **Safety.** Rust 2021 is used throughout and `unsafe_code` is forbidden at the
  workspace level (`Cargo.toml`). Runtime failures must not crash the host
  process.
- **Explicit deletion.** Permanent project-folder deletion requires explicit
  confirmation and is limited to a direct child of the registered workspace.

## Build Order

When adding or changing a feature, work top-down and verify at each layer:

1. **Contract** — update `contracts/polyglid.wit` if the ABI changes.
2. **Shared API types** — update `polyglid-plugin-api`.
3. **Runtime adapter** — update `polyglid-runtime`.
4. **Core use cases** — update `polyglid-core`, including permission and audit
   logic.
5. **Tests** — add positive and negative tests for capability grants, signature
   checks, and path validation.
6. **UI integration** — surface results and failures through `polyglid-client`
   into the desktop shell.
