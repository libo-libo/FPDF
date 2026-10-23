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

# The host libxkbcommon-x11 must match the host libxkbcommon; loading a
# bundled copy next to an older system libxkbcommon crashes XKB state setup.
# Only fall back to the bundled libraries when the host has no XKB support.
has_system_xkb() {
    if command -v ldconfig >/dev/null 2>&1 && \
        ldconfig -p 2>/dev/null | grep -q 'libxkbcommon-x11\.so\.0'; then
        return 0
    fi
    for dir in /usr/lib64 /usr/lib /lib64 /lib; do
        if [ -e "$dir/libxkbcommon-x11.so.0" ]; then
            return 0
        fi
    done
    return 1
}

[ -x "$SELF/fpdf-bin" ] || {
    echo "fpdf-bin is missing next to the launcher: $SELF" >&2
    exit 1
}

if [ -d "$SELF/lib" ] && ! has_system_xkb; then
    LD_LIBRARY_PATH="$SELF/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    export LD_LIBRARY_PATH
fi

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
