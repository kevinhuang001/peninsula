#!/bin/sh
set -eu

if [ "$(uname -s)" != "Darwin" ]; then
    echo "This installer only runs on macOS." >&2
    exit 1
fi

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APP="$HOME/Applications/灵动岛.app"
STAGE="$ROOT/target/macos-app-stage"

cd "$ROOT"
cargo build --locked --release
rm -rf "$STAGE" "$APP"
mkdir -p "$STAGE/Contents/MacOS" "$HOME/Applications"
cp target/release/island-lab "$STAGE/Contents/MacOS/island-lab"
cat > "$STAGE/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>island-lab</string>
<key>CFBundleIdentifier</key><string>dev.peninsula.islandlab</string>
<key>CFBundleName</key><string>灵动岛</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>0.1.0</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
mv "$STAGE" "$APP"
open -n "$APP"
echo "Installed and launched: $APP"
