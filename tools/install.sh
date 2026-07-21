#!/bin/sh
# reflow2 installer — BL-15's no-checkout path.
#
#   curl -fsSL https://raw.githubusercontent.com/sligara7/reflow2/main/tools/install.sh | sh
#
# Downloads the prebuilt reflow2-mcp binary and the consumer kit from GitHub
# Releases, installs the binary onto PATH and the kit beside it, and says
# exactly what to run next. No Rust toolchain, no ~14-minute RocksDB build.
#
# While the repo is PRIVATE, unauthenticated downloads fail — so this prefers
# `gh release download` (which uses your GitHub auth) and falls back to plain
# curl, which is the path that simply works the day the repo goes public.
# "I could not download" is reported as exactly that, never as a half-install.
#
# Overrides:
#   REFLOW2_VERSION      tag to install (default: latest release)
#   REFLOW2_BIN_DIR      where the binary goes   (default: ~/.local/bin)
#   REFLOW2_KIT_DIR      where the kit goes      (default: ~/.local/share/reflow2)
#   REFLOW2_REPO         owner/repo              (default: sligara7/reflow2)

set -eu

REPO="${REFLOW2_REPO:-sligara7/reflow2}"
BIN_DIR="${REFLOW2_BIN_DIR:-$HOME/.local/bin}"
KIT_DIR="${REFLOW2_KIT_DIR:-$HOME/.local/share/reflow2}"
VERSION="${REFLOW2_VERSION:-latest}"

say()  { printf '%s\n' "$*"; }
fail() { printf 'error: %s\n' "$*" >&2; exit 1; }

# ---- platform ---------------------------------------------------------------
os="$(uname -s)"
arch="$(uname -m)"
case "$os/$arch" in
  Linux/x86_64)             target="linux-x86_64" ;;
  Darwin/arm64)             target="macos-arm64" ;;
  Darwin/x86_64)            target="macos-x86_64" ;;
  *) fail "no prebuilt binary for $os/$arch — build from source instead:
  git clone https://github.com/$REPO && cd reflow2 && cargo build --release -p reflow2-mcp" ;;
esac

bin_asset="reflow2-mcp-${target}.tar.gz"
kit_asset="reflow2-kit.tar.gz"

# ---- download ---------------------------------------------------------------
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

download() {
  # $1 = asset name; lands in $tmp/$1
  if command -v gh > /dev/null 2>&1; then
    if [ "$VERSION" = "latest" ]; then
      gh release download --repo "$REPO" --pattern "$1" --dir "$tmp" \
        || fail "could not download $1 from the latest release of $REPO (gh)"
    else
      gh release download "$VERSION" --repo "$REPO" --pattern "$1" --dir "$tmp" \
        || fail "could not download $1 from release $VERSION of $REPO (gh)"
    fi
  else
    if [ "$VERSION" = "latest" ]; then
      url="https://github.com/$REPO/releases/latest/download/$1"
    else
      url="https://github.com/$REPO/releases/download/$VERSION/$1"
    fi
    curl -fsSL -o "$tmp/$1" "$url" \
      || fail "could not download $url
If the repository is private, install the GitHub CLI (gh) and authenticate — this
script uses it automatically. 'Could not download' never means 'up to date'."
  fi
}

# Like download(), but returns nonzero instead of exiting — for an OPTIONAL
# asset. download()'s fail exits the whole script even when the call sits in an
# `if` condition, so using it for checksums.txt silently killed the install on
# any release without one, with the error swallowed by the caller's 2>/dev/null
# (BL-55). The honest-skip branch below was unreachable dead code.
try_download() {
  # $1 = asset name; lands in $tmp/$1
  if command -v gh > /dev/null 2>&1; then
    if [ "$VERSION" = "latest" ]; then
      gh release download --repo "$REPO" --pattern "$1" --dir "$tmp" 2> /dev/null
    else
      gh release download "$VERSION" --repo "$REPO" --pattern "$1" --dir "$tmp" 2> /dev/null
    fi
  else
    if [ "$VERSION" = "latest" ]; then
      curl -fsSL -o "$tmp/$1" "https://github.com/$REPO/releases/latest/download/$1" 2> /dev/null
    else
      curl -fsSL -o "$tmp/$1" "https://github.com/$REPO/releases/download/$VERSION/$1" 2> /dev/null
    fi
  fi
}

say "reflow2: downloading $bin_asset and $kit_asset ($VERSION) from $REPO ..."
download "$bin_asset"
download "$kit_asset"

# ---- verify (best effort, honest about skipping) ----------------------------
if try_download "checksums.txt"; then
  (
    cd "$tmp"
    if command -v sha256sum > /dev/null 2>&1; then
      grep -E "($bin_asset|$kit_asset)" checksums.txt | sha256sum -c - > /dev/null \
        || fail "checksum mismatch — the download is corrupt or tampered with; nothing was installed"
    elif command -v shasum > /dev/null 2>&1; then
      grep -E "($bin_asset|$kit_asset)" checksums.txt | shasum -a 256 -c - > /dev/null \
        || fail "checksum mismatch — the download is corrupt or tampered with; nothing was installed"
    else
      say "note: no sha256 tool found — checksums NOT verified"
    fi
  )
else
  say "note: checksums.txt not present on this release — checksums NOT verified"
fi

# ---- install ----------------------------------------------------------------
mkdir -p "$BIN_DIR"
tar -C "$tmp" -xzf "$tmp/$bin_asset"
install -m 755 "$tmp/reflow2-mcp" "$BIN_DIR/reflow2-mcp"

mkdir -p "$KIT_DIR"
rm -rf "$KIT_DIR/kit.new"
mkdir -p "$KIT_DIR/kit.new"
tar -C "$KIT_DIR/kit.new" --strip-components=1 -xzf "$tmp/$kit_asset"
rm -rf "$KIT_DIR/kit"
mv "$KIT_DIR/kit.new" "$KIT_DIR/kit"

installed_version="$("$BIN_DIR/reflow2-mcp" --version 2> /dev/null || true)"
# A binary that cannot execute (wrong arch, glibc too new) must not report
# success with a blank version — that is a swallowed error (BL-55).
if [ -z "$installed_version" ]; then
  fail "the installed binary failed to run ('$BIN_DIR/reflow2-mcp --version' produced nothing) —
likely a platform mismatch (wrong architecture, or the binary needs a newer glibc).
Build from source instead:
  git clone https://github.com/$REPO && cd reflow2 && cargo build --release -p reflow2-mcp"
fi

say ""
say "installed:"
say "  binary  $BIN_DIR/reflow2-mcp  ($installed_version)"
say "  kit     $KIT_DIR/kit"

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) say ""
     say "NOTE: $BIN_DIR is not on your PATH. Add it, e.g.:"
     say "  export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac

say ""
say "Next — set up a project (creates the design environment, touches nothing else):"
say "  python3 $KIT_DIR/kit/tools/reflow2_init.py <your-project-dir> --binary $BIN_DIR/reflow2-mcp"
say ""
say "To update later: re-run this installer. Your design graphs are never touched."
