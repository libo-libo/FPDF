#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
URL='https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8111/pdfium-linux-x64.tgz'
SHA256='bdc60126399f39fe768566a18d4be055a7b3681bc9e248e8a6656b4f6cae0aa2'
ARCHIVE="$ROOT/analysis/tools/pdfium-linux-x64.tgz"

mkdir -p "$ROOT/analysis/tools" "$ROOT/runtime"
if [ ! -f "$ARCHIVE" ] || [ "$(sha256sum "$ARCHIVE" | cut -d' ' -f1)" != "$SHA256" ]; then
    curl -fLsS --retry 2 --connect-timeout 20 --max-time 180 -o "$ARCHIVE" "$URL"
fi
[ "$(sha256sum "$ARCHIVE" | cut -d' ' -f1)" = "$SHA256" ] || {
    echo 'PDFium archive SHA-256 mismatch.' >&2
    exit 1
}
tar -xzf "$ARCHIVE" -C "$ROOT/runtime"
printf '{"version":"chromium/8111","url":"%s","sha256":"%s"}\n' "$URL" "$SHA256" \
    > "$ROOT/runtime/pdfium-source.json"
echo "PDFium ready: $ROOT/runtime/lib/libpdfium.so"
