#!/bin/sh
set -eu

[ -n "${HOME:-}" ] || { echo 'HOME is not set.' >&2; exit 1; }
SOURCE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PREFIX=${FPDF_PREFIX:-"$HOME/.local/opt/fpdf"}
BIN_DIR=${FPDF_BIN_DIR:-"$HOME/.local/bin"}
DATA_DIR=${XDG_DATA_HOME:-"$HOME/.local/share"}
CONFIG_DIR=${XDG_CONFIG_HOME:-"$HOME/.config"}

case "$DATA_DIR" in /*) ;; *) DATA_DIR="$HOME/.local/share" ;; esac
case "$CONFIG_DIR" in /*) ;; *) CONFIG_DIR="$HOME/.config" ;; esac

while [ "$#" -gt 0 ]; do
    case "$1" in
        --prefix)
            [ "$#" -ge 2 ] || { echo '--prefix requires a directory.' >&2; exit 2; }
            PREFIX=$2
            shift 2
            ;;
        --bin-dir)
            [ "$#" -ge 2 ] || { echo '--bin-dir requires a directory.' >&2; exit 2; }
            BIN_DIR=$2
            shift 2
            ;;
        --help|-h)
            echo "Usage: ./install.sh [--prefix DIR] [--bin-dir DIR]"
            echo "Installs for the current user; root access is not required."
            exit 0
            ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
done

for required in fpdf fpdf-bin libfpdf-glx-compat.so libpdfium.so libxkbcommon-x11.so.0 libxcb-xkb.so.1 ai/bridge.mjs ai/node fonts/wqy-microhei.ttc fpdf.desktop.in fpdf.png; do
    [ -f "$SOURCE/$required" ] || {
        echo "Installation files are incomplete: $SOURCE/$required" >&2
        exit 1
    }
done
case "$PREFIX" in
    ''|'/'|"$HOME")
        echo "Refusing unsafe installation prefix: $PREFIX" >&2
        exit 1
        ;;
esac
case "$PREFIX" in
    *'
'*|*'"'*|*'%'*)
        echo 'The installation path cannot contain a newline, double quote, or percent sign.' >&2
        exit 1
        ;;
esac

LAUNCHER="$BIN_DIR/fpdf"
if [ -e "$LAUNCHER" ] && [ ! -L "$LAUNCHER" ]; then
    echo "Cannot replace existing file: $LAUNCHER" >&2
    exit 1
fi

mkdir -p "$(dirname -- "$PREFIX")" "$BIN_DIR" "$DATA_DIR/applications" \
    "$DATA_DIR/icons/hicolor/256x256/apps"
STAGE="$PREFIX.installing.$$"
OLD="$PREFIX.previous.$$"
rm -rf "$STAGE" "$OLD"
mkdir -p "$STAGE"
cp -a "$SOURCE"/. "$STAGE"/

if [ -e "$PREFIX" ] || [ -L "$PREFIX" ]; then
    mv "$PREFIX" "$OLD"
fi
if ! mv "$STAGE" "$PREFIX"; then
    [ ! -e "$OLD" ] || mv "$OLD" "$PREFIX"
    exit 1
fi
rm -rf "$OLD"

ln -sfn "$PREFIX/fpdf" "$LAUNCHER"

ICON="$DATA_DIR/icons/hicolor/256x256/apps/fpdf.png"
cp -f "$PREFIX/fpdf.png" "$ICON"
EXEC_PATH=$(printf '%s' "$PREFIX/fpdf" | sed 's/[\\&|]/\\&/g')
ICON_PATH=$(printf '%s' "$ICON" | sed 's/[\\&|]/\\&/g')
sed -e "s|%EXEC%|$EXEC_PATH|g" -e "s|%ICON%|$ICON_PATH|g" \
    "$PREFIX/fpdf.desktop.in" > "$DATA_DIR/applications/fpdf.desktop"
chmod 644 "$DATA_DIR/applications/fpdf.desktop" "$ICON"

echo "fpdf installed to $PREFIX"
echo "Launcher: $LAUNCHER"
echo "Configuration: $CONFIG_DIR/fpdf"
echo "Run: $LAUNCHER /path/to/document.pdf"
echo "If ~/.local/bin is not on PATH, run: export PATH=\"\$HOME/.local/bin:\$PATH\""
