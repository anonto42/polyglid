#!/usr/bin/env bash
# Build the PolyGlid knowledge graph with graphify-rs.
#
# Usage: scripts/dev/graphify-build.sh [extra graphify-rs args...]
#
# Produces the interactive graph under ~/.graphify-rs/<project>-<hash>/:
#   graph.html      interactive visualization
#   graph.json      queryable graph data
#   GRAPH_REPORT.md analysis report
#
# The first pass (AST extraction) is local and free. Pass an Anthropic API key
# via ANTHROPIC_API_KEY or configure [llm] in graphify-rs.toml to add semantic
# extraction; otherwise use --no-llm to skip it explicitly.
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repository_root"

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: '$1' is required but was not found on PATH." >&2
    echo "Install it with: cargo install graphify-rs" >&2
    exit 1
  fi
}

require_tool graphify-rs

exec graphify-rs build "$@"
