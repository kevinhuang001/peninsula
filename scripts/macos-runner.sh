#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
IDENTITY="${PENINSULA_SIGNING_IDENTITY:-Peninsula Local Signing}"
BIN="$1"
shift

# Only sign when the local identity already exists; never create one from a test/CI run.
if security find-identity -p codesigning 2>/dev/null | grep -q "\"$IDENTITY\""; then
    "$ROOT/scripts/sign-macos.sh" "$BIN" >/dev/null 2>&1 || true
fi

exec "$BIN" "$@"
