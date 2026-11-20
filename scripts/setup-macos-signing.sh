#!/bin/sh
set -eu

NAME="${PENINSULA_SIGNING_IDENTITY:-Peninsula Local Signing}"
DIR="$HOME/Library/Application Support/PeninsulaSigning"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

mkdir -p "$DIR"
chmod 700 "$DIR"

if security find-identity -p codesigning 2>/dev/null | grep -q "\"$NAME\""; then
    if [ -f "$DIR/cert.pem" ]; then
        security add-trusted-cert -r trustRoot -p codeSign -k "$KEYCHAIN" "$DIR/cert.pem" >/dev/null 2>&1 || true
    fi
    exit 0
fi

if [ ! -f "$DIR/cert.pem" ] || [ ! -f "$DIR/key.pem" ] || [ ! -f "$DIR/identity.p12" ]; then
    PASS=$(/usr/bin/openssl rand -hex 24)
    printf '%s' "$PASS" > "$DIR/password"
    chmod 600 "$DIR/password"
    /usr/bin/openssl req -x509 -newkey rsa:2048 -sha256 -days 3650 -nodes \
        -keyout "$DIR/key.pem" -out "$DIR/cert.pem" \
        -subj "/CN=$NAME/O=Peninsula/C=US" \
        -addext "basicConstraints=critical,CA:false" \
        -addext "keyUsage=critical,digitalSignature" \
        -addext "extendedKeyUsage=critical,codeSigning" >/dev/null 2>&1
    chmod 600 "$DIR/key.pem" "$DIR/cert.pem"
    /usr/bin/openssl pkcs12 -export \
        -inkey "$DIR/key.pem" -in "$DIR/cert.pem" \
        -name "$NAME" -out "$DIR/identity.p12" \
        -passout "pass:$PASS" >/dev/null 2>&1
    chmod 600 "$DIR/identity.p12"
fi

PASS=$(cat "$DIR/password")
security import "$DIR/identity.p12" -k "$KEYCHAIN" -P "$PASS" \
    -T /usr/bin/codesign -T /usr/bin/security >/dev/null

# Trust the self-signed leaf for code signing in the user domain. Without this macOS
# reports CSSMERR_TP_NOT_TRUSTED and tends to ask for the login password on every read.
security add-trusted-cert -r trustRoot -p codeSign -k "$KEYCHAIN" "$DIR/cert.pem" >/dev/null 2>&1 || true

security find-identity -p codesigning 2>/dev/null | grep -q "\"$NAME\""
echo "Installed macOS signing identity: $NAME ($DIR)" >&2
