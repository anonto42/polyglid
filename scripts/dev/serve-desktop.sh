#!/usr/bin/env bash
# Run the native GPUI desktop application.
#
# Usage: scripts/dev/serve-desktop.sh [extra cargo run args...]
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repository_root"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: 'cargo' is required but was not found on PATH." >&2
  echo "Run this script from inside the development shell: nix develop" >&2
  exit 1
fi

if [ "${POLYGLID_DATA_DIR:-}" = "" ]; then
  echo "info: POLYGLID_DATA_DIR is unset; the app will use the default data folder." >&2
  echo "info: set it (e.g. target/dev-data) to isolate dev-run data from real installs." >&2
fi

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/polyglid-workbench-target}"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"

echo "Starting the native PolyGlid GPUI workbench..." >&2
exec cargo run --package polyglid-desktop "$@"
