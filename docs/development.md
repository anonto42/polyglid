# Development Guide

## Prerequisites

PolyGlid is a Rust workspace built with Nix + Moon + Cargo. On macOS, Xcode
Command Line Tools remain a host requirement.

Install [Nix](https://nixos.org/download/) with flakes enabled, then enter the
reproducible development environment:

```bash
nix develop
```

The shell provides the pinned Rust toolchain (1.96.0, edition 2021), the
`wasm32-unknown-unknown` target, Moon 2.4.6, Dioxus CLI 0.7.9 for the website,
Go, Git, and the GPUI Linux libraries required by the desktop build. It supports Linux
x86-64/ARM64 and macOS Apple Silicon.

## Common Commands

Run from the repository root unless noted.

```bash
# Desktop application
cargo run -p polyglid-desktop

# Same, via the Makefile wrapper over scripts/dev/serve-desktop.sh
make serve

# Knowledge graph of the codebase (graphify-rs, wrappers in scripts/dev/)
make graph
make graph-query Q="how does permanent project deletion validate paths?"
make graph-serve

# Whole workspace
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check

# Moon orchestrates the same checks without replacing Cargo
moon run :format
moon run :check
moon run :test
moon run :build
moon run website:bundle
```

The exact verification used by CI is available as:

```bash
scripts/ci/verify-rust.sh
```

### Desktop Development Notes

- GPUI is compiled as a native Rust application; restart `make serve` after
  source changes.
- `Ctrl+P` opens commands, `Ctrl+B` toggles the explorer, `Ctrl+W` closes a
  clean document, and `Ctrl+S` saves the active document.
- Use `POLYGLID_DATA_DIR` and `POLYGLID_WORKSPACE_ROOT` to isolate development
  data and select the workspace displayed by the editor.

### Knowledge Graph

`graphify-rs` (install once with `cargo install graphify-rs`) builds a
queryable knowledge graph of the codebase — useful for architecture questions
and for AI agents. Wrappers live in `scripts/dev/` (see
`scripts/dev/README.md`). `make graph` performs the free AST-only pass; LLM
semantic extraction additionally requires `ANTHROPIC_API_KEY` or an `[llm]`
section in `graphify-rs.toml`. Graph output lands in
`~/.graphify-rs/<project>-<hash>/`.

## Quality Gates

- Formatting: `cargo fmt --all -- --check`
- Checks: `cargo check --workspace` (run via `moon ci --downstream deep`)
- Tests: `cargo test --workspace`
- Lint: `cargo clippy --workspace -- -D warnings` (where available)

The workspace forbids unsafe code (`unsafe_code = "forbid"`), so clippy must not
regress that guarantee. Changes to a shared crate also validate all downstream
consumers — Moon runs affected tasks and their dependents.

## Releases

Pull request titles declare release intent using Conventional Commits:

| Pull request title | Version result |
| --- | --- |
| `feat!: change a public contract` | Major |
| `feat(desktop): add project search` | Minor |
| `fix(core): reject an unsafe path` | Patch |
| `docs: explain workspace setup` | No product release |

After changes land on `main` and CI succeeds, Release Please opens or updates a
release pull request containing the workspace version, lockfile, and changelog
updates. Merging it creates a tag and a draft GitHub release; GitHub Actions then
build and publish the Linux, Windows, and macOS archives with SHA-256 checksums.

Use squash merges and configure GitHub to use the pull request title as the
squash commit message so the validated intent is preserved.
