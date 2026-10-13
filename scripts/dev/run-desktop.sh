#!/usr/bin/env bash
# Run the desktop application without the live-reload tooling.
#
# Usage: scripts/dev/run-desktop.sh [extra cargo run args...]
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repository_root"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/polyglid-workbench-target}"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"

exec cargo run -p polyglid-desktop "$@"
