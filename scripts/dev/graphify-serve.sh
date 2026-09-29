#!/usr/bin/env bash
# Start the graphify-rs MCP server so AI agents can query the PolyGlid graph.
#
# Usage: scripts/dev/graphify-serve.sh [extra graphify-rs args...]
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

exec graphify-rs serve "$@"
