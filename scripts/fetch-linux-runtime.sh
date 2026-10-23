#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DEST="$ROOT/runtime/linux-x64"
CACHE="$ROOT/analysis/tools/linux-runtime"
REPO='https://repo.almalinux.org/almalinux/8/AppStream/x86_64/os/Packages'
XKBCOMMON_X11='libxkbcommon-x11-0.9.1-1.el8.x86_64.rpm'
XKBCOMMON_X11_SHA256='3573869075b9a6b5146b78e789476aa7c2bcc3b06124743979730e52d2b6f1a8'
LIBXCB='libxcb-1.13.1-1.el8.x86_64.rpm'
LIBXCB_SHA256='bf18a9ee30ac538bee0a737f91aaaf160e8570be11e1795ecf5082032ee14fdc'
XKBCOMMON='libxkbcommon-0.9.1-1.el8.x86_64.rpm'
XKBCOMMON_SHA256='9ba313c55750dc3c0faee5f706dd0d051ffd06d1a544544b425ddb7c8c33f905'

download_checked() {
    archive=$1
    url=$2
    expected=$3
    if [ ! -f "$archive" ] || [ "$(sha256sum "$archive" | cut -d' ' -f1)" != "$expected" ]; then
        curl -fLsS --retry 3 --connect-timeout 20 --max-time 180 -o "$archive" "$url"
    fi
    [ "$(sha256sum "$archive" | cut -d' ' -f1)" = "$expected" ] || {
        echo "Linux runtime RPM SHA-256 mismatch: $archive" >&2
        exit 1
    }
}

mkdir -p "$CACHE" "$DEST"
download_checked "$CACHE/$XKBCOMMON_X11" "$REPO/$XKBCOMMON_X11" "$XKBCOMMON_X11_SHA256"
download_checked "$CACHE/$LIBXCB" "$REPO/$LIBXCB" "$LIBXCB_SHA256"
download_checked "$CACHE/$XKBCOMMON" "$REPO/$XKBCOMMON" "$XKBCOMMON_SHA256"

EXTRACT=$(mktemp -d)
trap 'rm -rf "$EXTRACT"' EXIT HUP INT TERM
rpm2cpio "$CACHE/$XKBCOMMON_X11" | (cd "$EXTRACT" && cpio -idm --quiet)
rpm2cpio "$CACHE/$LIBXCB" | (cd "$EXTRACT" && cpio -idm --quiet)
rpm2cpio "$CACHE/$XKBCOMMON" | (cd "$EXTRACT" && cpio -idm --quiet)

install -m 755 "$EXTRACT/usr/lib64/libxkbcommon-x11.so.0.0.0" "$DEST/"
ln -sfn libxkbcommon-x11.so.0.0.0 "$DEST/libxkbcommon-x11.so.0"
install -m 755 "$EXTRACT/usr/lib64/libxcb-xkb.so.1.0.0" "$DEST/"
ln -sfn libxcb-xkb.so.1.0.0 "$DEST/libxcb-xkb.so.1"
install -m 644 "$EXTRACT/usr/share/licenses/libxkbcommon/LICENSE" "$DEST/LICENSE.xkbcommon"
cat > "$DEST/source.json" <<JSON
{"baseline":"AlmaLinux 8 x86_64","packages":[{"name":"$XKBCOMMON_X11","url":"$REPO/$XKBCOMMON_X11","sha256":"$XKBCOMMON_X11_SHA256"},{"name":"$LIBXCB","url":"$REPO/$LIBXCB","sha256":"$LIBXCB_SHA256"},{"name":"$XKBCOMMON","url":"$REPO/$XKBCOMMON","sha256":"$XKBCOMMON_SHA256"}],"purpose":"X11 keyboard support loaded dynamically by winit; bundled for hosts without libxkbcommon-x11 or libxcb-xkb"}
JSON
echo "Linux X11 runtime ready: $DEST"
