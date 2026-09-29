#!/usr/bin/env bash
# Run the desktop application without the live-reload tooling.
#
# Usage: scripts/dev/run-desktop.sh [extra cargo run args...]
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repository_root"

exec cargo run -p polyglid-desktop "$@"
