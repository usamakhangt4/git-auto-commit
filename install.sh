#!/bin/sh
set -eu

REPO="usamakhangt4/git-auto-commit"
BIN_DIR="${GIT_AUTO_COMMIT_INSTALL_DIR:-$HOME/.local/bin}"

case "$(uname -s)" in
  Linux) os="unknown-linux-gnu" ;;
  Darwin) os="apple-darwin" ;;
  *) echo "Unsupported operating system: $(uname -s)" >&2; exit 1 ;;
esac

case "$(uname -m)" in
  x86_64|amd64) arch="x86_64" ;;
  arm64|aarch64) arch="aarch64" ;;
  *) echo "Unsupported CPU architecture: $(uname -m)" >&2; exit 1 ;;
esac

target="$arch-$os"
url="https://github.com/$REPO/releases/latest/download/git-auto-commit-$target.tar.gz"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT INT TERM

echo "Downloading git-auto-commit for $target..."
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$url" -o "$tmp_dir/archive.tar.gz"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$tmp_dir/archive.tar.gz" "$url"
else
  echo "curl or wget is required." >&2
  exit 1
fi

tar -xzf "$tmp_dir/archive.tar.gz" -C "$tmp_dir"
mkdir -p "$BIN_DIR"
install -m 755 "$tmp_dir/git-auto-commit" "$BIN_DIR/git-auto-commit"

echo "Installed git-auto-commit to $BIN_DIR/git-auto-commit"
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "Add $BIN_DIR to your PATH, then run: git-auto-commit --help" ;;
esac

