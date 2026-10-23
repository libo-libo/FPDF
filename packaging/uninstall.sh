#!/bin/sh
set -eu

[ -n "${HOME:-}" ] || { echo 'HOME is not set.' >&2; exit 1; }
PREFIX=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
BIN_DIR=${FPDF_BIN_DIR:-"$HOME/.local/bin"}
DATA_DIR=${XDG_DATA_HOME:-"$HOME/.local/share"}
CONFIG_DIR=${XDG_CONFIG_HOME:-"$HOME/.config"}
PURGE=0

case "$DATA_DIR" in /*) ;; *) DATA_DIR="$HOME/.local/share" ;; esac
case "$CONFIG_DIR" in /*) ;; *) CONFIG_DIR="$HOME/.config" ;; esac

for arg in "$@"; do
    case "$arg" in
        --purge) PURGE=1 ;;
        --help|-h) echo "Usage: uninstall.sh [--purge]"; exit 0 ;;
        *) echo "Unknown option: $arg" >&2; exit 2 ;;
    esac
done

case "$PREFIX" in
    ''|'/'|"$HOME") echo "Refusing unsafe installation path: $PREFIX" >&2; exit 1 ;;
esac
[ -f "$PREFIX/fpdf" ] && [ -f "$PREFIX/libpdfium.so" ] || {
    echo "This does not look like an fpdf installation: $PREFIX" >&2
    exit 1
}

LAUNCHER="$BIN_DIR/fpdf"
if [ -L "$LAUNCHER" ] && [ "$(readlink "$LAUNCHER")" = "$PREFIX/fpdf" ]; then
    rm -f "$LAUNCHER"
fi
rm -f "$DATA_DIR/applications/fpdf.desktop"
rm -f "$DATA_DIR/icons/hicolor/256x256/apps/fpdf.png"
if [ "$PURGE" = 1 ]; then
    rm -rf "$CONFIG_DIR/fpdf"
fi
cd /
rm -rf "$PREFIX"
echo 'fpdf removed.'
[ "$PURGE" = 1 ] && echo 'Saved fpdf configuration was removed.'
exit 0
