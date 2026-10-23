#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
[ -n "$VERSION" ] || { echo 'Cannot read the application version.' >&2; exit 1; }

NODE_VERSION=v24.22.1
NODE_NAME="node-$NODE_VERSION-linux-x64-glibc-217"
NODE_BUILD="$ROOT/analysis/tools/$NODE_NAME"
NODE_BIN="$NODE_BUILD/bin"
PDFIUM="$ROOT/runtime/lib/libpdfium.so"
XKBCOMMON_X11="$ROOT/runtime/linux-x64/libxkbcommon-x11.so.0"
LIBXCB_XKB="$ROOT/runtime/linux-x64/libxcb-xkb.so.1"
LINUX_RUNTIME_LICENSE="$ROOT/runtime/linux-x64/LICENSE.xkbcommon"
LINUX_RUNTIME_SOURCE="$ROOT/runtime/linux-x64/source.json"
FONT=/usr/share/fonts/wqy-microhei/wqy-microhei.ttc
FONT_DOC=/usr/share/doc/wqy-microhei-fonts-0.2.0
PACKAGE_NAME="fpdf-$VERSION-linux-x64"
STAGE="$ROOT/dist/$PACKAGE_NAME"
ARCHIVE="$ROOT/dist/$PACKAGE_NAME.tar.gz"
RUN_FILE="$ROOT/dist/$PACKAGE_NAME.run"

mkdir -p "$ROOT/dist"

[ -f "$ROOT/runtime/lib/libpdfium.so" ] || "$ROOT/scripts/fetch-pdfium.sh"
[ -f "$XKBCOMMON_X11" ] && [ -f "$LIBXCB_XKB" ] || "$ROOT/scripts/fetch-linux-runtime.sh"
[ -x "$NODE_BIN/node" ] || "$ROOT/scripts/fetch-node.sh"
[ -f "$FONT" ] || { echo "Missing bundled font: $FONT" >&2; exit 1; }
[ -f "$FONT_DOC/LICENSE_Apache2.txt" ] || { echo "Missing font license files in $FONT_DOC" >&2; exit 1; }

export PATH="$NODE_BIN:$PATH"
if [ ! -d "$ROOT/ai-runtime/node_modules" ]; then
    npm ci --no-audit --no-fund --prefix "$ROOT/ai-runtime"
fi
(
    cd "$ROOT/ai-runtime"
    node --test bridge.test.mjs
)
FPDF_RUNTIME_NODE="$NODE_BIN/node" node "$ROOT/scripts/build-ai.mjs"

if [ "${FPDF_SKIP_TESTS:-0}" != 1 ]; then
    (
        cd "$ROOT"
        cargo fmt --check
        cargo clippy --locked --all-targets -- -D warnings
        cargo test --locked
    )
fi
(
    cd "$ROOT"
    cargo build --release --locked
)

python3 "$ROOT/scripts/collect-notices.py" --output "$ROOT/dist/THIRD_PARTY_NOTICES.txt"

rm -rf "$STAGE"
mkdir -p "$STAGE/ai" "$STAGE/fonts" "$STAGE/licenses/PDFium" "$STAGE/licenses/WQY-Micro-Hei" "$STAGE/licenses/X11"
install -m 755 "$ROOT/target/release/fpdf" "$STAGE/fpdf"
install -m 755 "$PDFIUM" "$STAGE/libpdfium.so"
install -m 755 "$XKBCOMMON_X11" "$STAGE/libxkbcommon-x11.so.0"
install -m 755 "$LIBXCB_XKB" "$STAGE/libxcb-xkb.so.1"
cp -a "$ROOT/runtime/ai/." "$STAGE/ai/"
install -m 644 "$FONT" "$STAGE/fonts/wqy-microhei.ttc"
install -m 644 "$ROOT/assets/fpdf.png" "$STAGE/fpdf.png"
install -m 644 "$ROOT/assets/fpdf.svg" "$STAGE/fpdf.svg"
install -m 644 "$ROOT/LICENSE" "$STAGE/LICENSE"
install -m 644 "$ROOT/agent.example.txt" "$STAGE/agent.example.txt"
install -m 644 "$ROOT/docs/linux.md" "$STAGE/README.md"
install -m 755 "$ROOT/packaging/install.sh" "$STAGE/install.sh"
install -m 755 "$ROOT/packaging/uninstall.sh" "$STAGE/uninstall.sh"
install -m 644 "$ROOT/packaging/fpdf.desktop.in" "$STAGE/fpdf.desktop.in"
install -m 644 "$ROOT/dist/THIRD_PARTY_NOTICES.txt" "$STAGE/THIRD_PARTY_NOTICES.txt"
install -m 644 "$ROOT/runtime/LICENSE" "$STAGE/licenses/PDFium/PDFium-LICENSE.txt"
cp -a "$ROOT/runtime/licenses/." "$STAGE/licenses/PDFium/"
install -m 644 "$ROOT/runtime/VERSION" "$STAGE/licenses/PDFium/VERSION"
install -m 644 "$ROOT/runtime/pdfium-source.json" "$STAGE/licenses/PDFium/source.json"
install -m 644 "$FONT_DOC/LICENSE_Apache2.txt" "$STAGE/licenses/WQY-Micro-Hei/LICENSE-Apache-2.0.txt"
install -m 644 "$FONT_DOC/LICENSE_GPLv3.txt" "$STAGE/licenses/WQY-Micro-Hei/LICENSE-GPLv3.txt"
install -m 644 "$FONT_DOC/README.txt" "$STAGE/licenses/WQY-Micro-Hei/README.txt"
install -m 644 "$LINUX_RUNTIME_LICENSE" "$STAGE/licenses/X11/libxkbcommon-LICENSE.txt"
install -m 644 "$LINUX_RUNTIME_SOURCE" "$STAGE/licenses/X11/source.json"
printf '%s\n' "$VERSION" > "$STAGE/VERSION"
cat > "$STAGE/licenses/Node.js-source.json" <<EOF
{"version":"$NODE_VERSION","archive":"$NODE_NAME.tar.xz","url":"https://unofficial-builds.nodejs.org/download/release/$NODE_VERSION/$NODE_NAME.tar.xz","sha256":"fb8ae2b650b593b2db2c6e01d69f3dd13661e21b333f8393d661be1e46ea5e82","note":"glibc 2.17 compatible build used for the EL8 baseline"}
EOF
chmod 755 "$STAGE/fpdf" "$STAGE/ai/node" "$STAGE/install.sh" "$STAGE/uninstall.sh"
chmod 644 "$STAGE/fpdf.png" "$STAGE/fpdf.svg" "$STAGE/fpdf.desktop.in"

if [ -n "$(find "$STAGE" -type f \( -name agent.txt -o -name githubtoken.txt -o -name .env -o -name '.env.*' \) -print)" ]; then
    echo 'Refusing to package a private configuration file.' >&2
    exit 1
fi

python3 "$ROOT/scripts/check-linux-abi.py" "$STAGE/fpdf" "$STAGE/libpdfium.so" "$STAGE/ai/node" \
    "$STAGE/libxkbcommon-x11.so.0" "$STAGE/libxcb-xkb.so.1"
readelf -d "$STAGE/fpdf" | grep -F 'Library rpath: [$ORIGIN]' >/dev/null || {
    echo 'fpdf is missing the transitive $ORIGIN runtime library search path.' >&2
    exit 1
}
printf '\nDynamic dependencies of fpdf:\n'
ldd "$STAGE/fpdf" || true

if command -v desktop-file-validate >/dev/null 2>&1; then
    DESKTOP_CHECK=$(mktemp --suffix=.desktop)
    sed -e "s|%EXEC%|$STAGE/fpdf|g" -e "s|%ICON%|$STAGE/fpdf.png|g" \
        "$STAGE/fpdf.desktop.in" > "$DESKTOP_CHECK"
    desktop-file-validate "$DESKTOP_CHECK"
    rm -f "$DESKTOP_CHECK"
fi

tar -czf "$ARCHIVE" -C "$ROOT/dist" "$PACKAGE_NAME"
RUN_HEADER="$ROOT/dist/.fpdf-run-header.$$"
cat > "$RUN_HEADER" <<'EOF'
#!/bin/sh
set -eu
[ -n "${TMPDIR:-}" ] || TMPDIR=/tmp
TMP=$(mktemp -d "$TMPDIR/fpdf-install.XXXXXX")
cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT HUP INT TERM
sed '1,/^__FPDF_PAYLOAD__$/d' "$0" | tar -xzf - -C "$TMP"
PACKAGE=
for candidate in "$TMP"/fpdf-*-linux-x64; do
    if [ -d "$candidate" ]; then PACKAGE=$candidate; break; fi
done
[ -n "$PACKAGE" ] || { echo 'Installer payload is incomplete.' >&2; exit 1; }
"$PACKAGE/install.sh" "$@"
exit $?
EOF
printf '__FPDF_PAYLOAD__\n' >> "$RUN_HEADER"
cat "$RUN_HEADER" "$ARCHIVE" > "$RUN_FILE"
chmod 755 "$RUN_FILE"
rm -f "$RUN_HEADER"

(
    cd "$ROOT/dist"
    sha256sum "$PACKAGE_NAME.tar.gz" "$PACKAGE_NAME.run" > "$PACKAGE_NAME.sha256"
)
echo "Release directory: $STAGE"
echo "Archive: $ARCHIVE"
echo "Self-extracting installer: $RUN_FILE"
echo "Checksums: $ROOT/dist/$PACKAGE_NAME.sha256"
