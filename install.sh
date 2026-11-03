#!/bin/sh
# Install tabbify from a GitHub release:
#
#   curl -fsSL https://github.com/Trojann/tabbify/releases/latest/download/install.sh | sh
#
#   TABBIFY_INSTALL_DIR  where to put tabbify (default: ~/.local/bin)
#   TABBIFY_VERSION      install another release (e.g. 1.0.5)
set -eu

REPO="Trojann/tabbify"
VERSION="${TABBIFY_VERSION:-@VERSION@}"
VERSION="${VERSION#v}"
DIR="${TABBIFY_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf 'tabbify: %s\n' "$*"; }
fail() { printf 'tabbify: %s\n' "$*" >&2; exit 1; }

case "$VERSION" in
    @*) fail "this copy of the script has no version; run the one from a release, or set TABBIFY_VERSION" ;;
esac
case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) fail "there's no prebuilt tabbify for $(uname -m); build it from source instead" ;;
esac
case "$(uname -s)" in
    Linux) triple="$arch-unknown-linux-musl" ;;
    Darwin) triple="$arch-apple-darwin" ;;
    *) fail "there's no prebuilt tabbify for $(uname -s); build it from source instead" ;;
esac

if command -v curl >/dev/null 2>&1; then
    fetch() { curl -fsSL -o "$2" "$1"; }
elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -qO "$2" "$1"; }
else
    fail "curl or wget is needed"
fi

name="tabbify-$VERSION-$triple"
url="https://github.com/$REPO/releases/download/v$VERSION/$name.tar.gz"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

say "downloading tabbify $VERSION ($arch)"
fetch "$url" "$tmp/$name.tar.gz" || fail "couldn't download $url"
if command -v sha256sum >/dev/null 2>&1; then
    check() { sha256sum -c --quiet "$1"; }
elif command -v shasum >/dev/null 2>&1; then
    check() { shasum -a 256 -s -c "$1"; }
fi
if command -v check >/dev/null 2>&1; then
    fetch "$url.sha256" "$tmp/$name.tar.gz.sha256" || fail "couldn't download the checksum"
    (cd "$tmp" && check "$name.tar.gz.sha256") || fail "the download doesn't match its checksum"
else
    say "neither sha256sum nor shasum is installed; skipping the checksum"
fi
tar -xzf "$tmp/$name.tar.gz" -C "$tmp"

mkdir -p "$DIR"
cp "$tmp/$name/tabbify" "$DIR/tabbify.new"
chmod 755 "$DIR/tabbify.new"
mv "$DIR/tabbify.new" "$DIR/tabbify"
say "installed $("$DIR/tabbify" --version) in $DIR"

case ":$PATH:" in
    *":$DIR:"*) ;;
    *) say "$DIR isn't in your PATH; add it in your shell's config" ;;
esac
say "next: run \`tabbify install\` to set it up in your shell, then open a new terminal"
