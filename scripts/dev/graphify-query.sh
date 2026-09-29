#!/usr/bin/env bash
# Query the PolyGlid knowledge graph built by scripts/dev/graphify-build.sh.
#
# Usage: scripts/dev/graphify-query.sh "question" [extra graphify-rs args...]
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repository_root"

if [ "$#" -eq 0 ]; then
  echo "usage: $0 \"question\" [extra graphify-rs args...]" >&2
  exit 2
fi

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: '$1' is required but was not found on PATH." >&2
    echo "Install it with: cargo install graphify-rs" >&2
    exit 1
  fi
}

require_tool graphify-rs

question="$1"
shift

exec graphify-rs query "$question" "$@"
