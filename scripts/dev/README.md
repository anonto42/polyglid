# Development Scripts

Convenience wrappers used by the root `Makefile` and the Moon tasks. All
scripts resolve the repository root themselves, so they can be run from
anywhere. Run them inside the pinned environment: `nix develop`.

## Desktop Development

| Script | Purpose |
| --- | --- |
| `serve-desktop.sh` | `dx serve` with Dioxus live reload; Rust hot-patching (Subsecond) on by default, `--no-hot-patch` to disable, `--open` to open a window, extra args pass through to `dx serve` |
| `run-desktop.sh` | plain `cargo run -p polyglid-desktop` without live-reload tooling |

### Live-Reload Behavior (Dioxus 0.7)

- RSX markup edits (elements, text, string attributes) apply instantly.
- Rust logic changes need hot-patching (`--hot-patch`, default here) or a
  rebuild; press `r` in the dx TUI to force a full rebuild.
- Hot-patching only tracks the tip crate (`apps/desktop`). Changes in
  `crates/*` recompile normally.
- Styles in `apps/desktop/assets/*.css` are embedded via `include_str!`, so CSS
  edits recompile instead of hot-swapping.

## Knowledge Graph (graphify-rs)

`graphify-rs` builds a queryable codebase knowledge graph (AST extraction,
clustering, PageRank, cycle detection) and can serve it to AI agents over MCP.
Install it once with `cargo install graphify-rs`.

| Script | Purpose |
| --- | --- |
| `graphify-build.sh` | build the graph; extra args pass through (`--no-llm` skips semantic extraction) |
| `graphify-query.sh "question"` | query the built graph |
| `graphify-serve.sh` | start the MCP server for agent access |

Output lands in `~/.graphify-rs/<project>-<hash>/` (`graph.html`,
`graph.json`, `GRAPH_REPORT.md`). AST extraction is local and free; LLM
semantic extraction requires `ANTHROPIC_API_KEY` or an `[llm]` section in
`graphify-rs.toml`.

### Recommended Workflow

Rebuild the graph when architecture changes (new crate, moved boundary), then
ask it questions instead of re-reading the tree:

```sh
scripts/dev/graphify-build.sh --no-llm
scripts/dev/graphify-query.sh "how does permanent project deletion validate paths?"
```
