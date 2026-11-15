#!/bin/sh
# Installs tendhostd on this machine, enrolls it with the tendhost server
# this script came from, and starts it as a service.
#
#   curl -fsSL __SERVER__/install.sh | sh -s -- --token <enrollment token> [--manage] [--no-agents] [--name <name>]
set -eu

SERVER="__SERVER__"
VERSION="__VERSION__"
RELEASES="https://github.com/goliajp/tendhost/releases/download/v$VERSION"
DEST=/usr/local/bin/tendhostd

fail() {
    echo "install: $1" >&2
    exit 1
}

case "$(uname -s)" in
    Linux) os=linux ;;
    Darwin) os=macos ;;
    *) fail "unsupported system $(uname -s); tendhostd runs on Linux and macOS" ;;
esac
case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) fail "unsupported processor $(uname -m)" ;;
esac
[ "$os-$arch" != macos-x86_64 ] || fail "no build is published for Intel Macs; build tendhostd from source"
case " $* " in
    *" --token "*) ;;
    *) fail "pass the enrollment token: sh -s -- --token <token>" ;;
esac

if [ "$(id -u)" = 0 ]; then
    as_root=""
else
    command -v sudo >/dev/null 2>&1 || fail "run this as root, or install sudo"
    as_root="sudo"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
asset="tendhostd-$os-$arch"
echo "Downloading $asset $VERSION"
curl -fsSL "$RELEASES/$asset" -o "$tmp/tendhostd" || fail "could not download $RELEASES/$asset"
curl -fsSL "$RELEASES/$asset.sha256" -o "$tmp/sha256" || fail "could not download the checksum of $asset"
want=$(cut -d ' ' -f 1 "$tmp/sha256")
if command -v sha256sum >/dev/null 2>&1; then
    got=$(sha256sum "$tmp/tendhostd" | cut -d ' ' -f 1)
else
    got=$(shasum -a 256 "$tmp/tendhostd" | cut -d ' ' -f 1)
fi
[ "$want" = "$got" ] || fail "the download does not match its checksum; nothing was installed"

$as_root mkdir -p "$(dirname "$DEST")"
$as_root install -m 755 "$tmp/tendhostd" "$DEST"

# on Linux the daemon is a system service and sees every user's agents;
# on a Mac it runs as the person installing it
if [ "$os" = linux ]; then
    $as_root "$DEST" enroll --server "$SERVER" "$@"
    $as_root "$DEST" install
else
    "$DEST" enroll --server "$SERVER" "$@"
    "$DEST" install
fi
echo "Done. This machine now reports to $SERVER."
