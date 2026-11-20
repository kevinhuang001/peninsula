#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
BIN="$1"
shift
"$ROOT/scripts/sign-macos.sh" "$BIN" >/dev/null 2>&1 || true
exec "$BIN" "$@"
