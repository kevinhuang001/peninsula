#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
IDENTITY="${PENINSULA_SIGNING_IDENTITY:-Peninsula Local Signing}"

"$ROOT/scripts/setup-macos-signing.sh" >/dev/null 2>&1
case "$1" in
    *.app) codesign --force --sign "$IDENTITY" "$@" ;;
    *) codesign --force --sign "$IDENTITY" --identifier dev.peninsula.islandlab "$@" ;;
esac
codesign --verify --strict "$@"
