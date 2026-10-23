#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION='v24.22.1'
BASE="node-$VERSION-linux-x64"
TOOLS="$ROOT/analysis/tools"
RUNTIME_ARCHIVE="$TOOLS/$BASE.tar.xz"
BUILD_ARCHIVE="$TOOLS/$BASE-glibc-217.tar.xz"

download_checked() {
    archive=$1
    url=$2
    expected=$3
    if [ ! -f "$archive" ] || [ "$(sha256sum "$archive" | cut -d' ' -f1)" != "$expected" ]; then
        curl -fLsS --retry 3 --connect-timeout 20 --max-time 600 -o "$archive" "$url"
    fi
    [ "$(sha256sum "$archive" | cut -d' ' -f1)" = "$expected" ] || {
        echo "Node.js archive SHA-256 mismatch: $archive" >&2
        exit 1
    }
}

mkdir -p "$TOOLS"
download_checked "$RUNTIME_ARCHIVE" \
    "https://nodejs.org/dist/$VERSION/$BASE.tar.xz" \
    '6c172a9ab6718f7b801fb5a7bd486af2ee5e32758086a04b84fcce2aa77095e6'
download_checked "$BUILD_ARCHIVE" \
    "https://unofficial-builds.nodejs.org/download/release/$VERSION/$BASE-glibc-217.tar.xz" \
    'fb8ae2b650b593b2db2c6e01d69f3dd13661e21b333f8393d661be1e46ea5e82'

rm -rf "$TOOLS/$BASE" "$TOOLS/$BASE-glibc-217"
tar -xJf "$RUNTIME_ARCHIVE" -C "$TOOLS"
tar -xJf "$BUILD_ARCHIVE" -C "$TOOLS"
echo "Node.js runtime ready: $TOOLS/$BASE/bin/node"
echo "Node.js glibc 2.17 build tool ready: $TOOLS/$BASE-glibc-217/bin/node"
