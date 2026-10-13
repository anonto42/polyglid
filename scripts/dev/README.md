# Development Scripts

Convenience wrappers used by the root `Makefile` and the Moon tasks. All
scripts resolve the repository root themselves, so they can be run from
anywhere. Run them inside the pinned environment: `nix develop`.

## Desktop Development

| Script | Purpose |
| --- | --- |
| `serve-desktop.sh` | run the native GPUI editor workbench; extra arguments pass through to `cargo run` |
| `dev-desktop.sh` | watch desktop, crate, and contract sources and restart the workbench after changes |
| `run-desktop.sh` | run the same `polyglid-desktop` package directly |

GPUI is a native Rust build and does not use the Dioxus hot-reload server. Use
`make dev` for automatic rebuilds and restarts, or restart `make serve`
manually after source changes.

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
