#!/bin/sh
# Start fpdf-bin with the bundled GLX compatibility shim loaded first.
set -eu

SELF=$0
while [ -L "$SELF" ]; do
    LINK=$(readlink "$SELF")
    case "$LINK" in
        /*) SELF=$LINK ;;
        *) SELF=$(dirname -- "$SELF")/$LINK ;;
    esac
done
SELF=$(CDPATH= cd -- "$(dirname -- "$SELF")" && pwd)

[ -x "$SELF/fpdf-bin" ] || {
    echo "fpdf-bin is missing next to the launcher: $SELF" >&2
    exit 1
}

COMPAT="$SELF/libfpdf-glx-compat.so"
if [ -f "$COMPAT" ]; then
    # glibc splits LD_PRELOAD on whitespace, so pass a file descriptor path
    # when the installation directory contains spaces.
    case $COMPAT in
        *[[:space:]]*)
            exec 3<"$COMPAT" || exit 1
            COMPAT=/proc/self/fd/3
            ;;
    esac
    case ${LD_PRELOAD:-} in
        '') LD_PRELOAD=$COMPAT ;;
        *) LD_PRELOAD="$COMPAT:$LD_PRELOAD" ;;
    esac
    export LD_PRELOAD
fi

exec "$SELF/fpdf-bin" "$@"
