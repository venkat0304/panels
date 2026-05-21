#!/usr/bin/env sh
# panels installer — downloads a prebuilt binary from the GitHub release
# and drops it into $PREFIX/bin (default: ~/.local/bin).
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/venkat0304/panels/master/install.sh | sh
#   PANELS_VERSION=v0.5.9 ./install.sh     # pin a specific version
#   PREFIX=/usr/local sudo ./install.sh    # install to /usr/local/bin
set -eu

REPO="venkat0304/panels"
BIN="panels"
PREFIX="${PREFIX:-$HOME/.local}"
INSTALL_DIR="$PREFIX/bin"
VERSION="${PANELS_VERSION:-latest}"

log() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

# ---- detect platform ---------------------------------------------------------
uname_s=$(uname -s)
uname_m=$(uname -m)

case "$uname_s" in
    Darwin) os=macos ;;
    Linux)  os=linux ;;
    *) die "unsupported OS: $uname_s (panels ships macOS and Linux binaries)" ;;
esac

case "$uname_m" in
    arm64|aarch64) arch=aarch64 ;;
    x86_64|amd64)  arch=x86_64 ;;
    *) die "unsupported arch: $uname_m" ;;
esac

asset="${BIN}-${os}-${arch}"

# ---- resolve version ---------------------------------------------------------
if [ "$VERSION" = "latest" ]; then
    log "resolving latest release tag"
    # Use the redirect from /releases/latest to avoid API rate limits.
    if command -v curl >/dev/null 2>&1; then
        VERSION=$(curl -fsSLI -o /dev/null -w '%{url_effective}' \
            "https://github.com/${REPO}/releases/latest" \
            | sed 's|.*/tag/||')
    else
        die "curl is required"
    fi
    [ -n "$VERSION" ] || die "could not resolve latest version"
fi

url="https://github.com/${REPO}/releases/download/${VERSION}/${asset}"
log "downloading ${asset} ${VERSION}"
log "from ${url}"

# ---- download ----------------------------------------------------------------
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
out="$tmp/$BIN"

if ! curl -fsSL --retry 3 -o "$out" "$url"; then
    die "download failed — does the release have a ${asset} asset?
     see https://github.com/${REPO}/releases/${VERSION}"
fi
chmod +x "$out"

# ---- install -----------------------------------------------------------------
mkdir -p "$INSTALL_DIR"
mv "$out" "$INSTALL_DIR/$BIN"
log "installed ${BIN} to ${INSTALL_DIR}/${BIN}"

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        printf '\n\033[1;33mnote:\033[0m %s is not on your $PATH yet.\n' "$INSTALL_DIR"
        printf '     add this to your shell rc:\n'
        printf '       export PATH="%s:$PATH"\n\n' "$INSTALL_DIR"
        ;;
esac

# ---- verify ------------------------------------------------------------------
if "$INSTALL_DIR/$BIN" --version >/dev/null 2>&1; then
    "$INSTALL_DIR/$BIN" --version
fi

log "run: $BIN"
