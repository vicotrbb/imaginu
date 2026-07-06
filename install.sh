#!/bin/sh
# imaginu installer — downloads the matching prebuilt release binary.
#
#   curl -fsSL https://raw.githubusercontent.com/vicotrbb/imaginu/main/install.sh | sh
#
# Options (env vars):
#   IMAGINU_VERSION=v0.1.0   install a specific tag (default: latest release)
#   IMAGINU_INSTALL_DIR=DIR  install location (default: /usr/local/bin,
#                            falling back to ~/.local/bin if not writable)
#
# No Rust required. `ffmpeg` is only needed later for video output.
set -eu

REPO="vicotrbb/imaginu"
BIN="imaginu"

say()  { printf '\033[1;36m==>\033[0m %s\n' "$1"; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$1" >&2; }
err()  { printf '\033[1;31merror:\033[0m %s\n' "$1" >&2; exit 1; }

command -v curl >/dev/null 2>&1 || err "curl is required"
command -v tar  >/dev/null 2>&1 || err "tar is required"

# --- detect target triple ---------------------------------------------------
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux)  os_part="unknown-linux-musl" ;;
  Darwin) os_part="apple-darwin" ;;
  *) err "unsupported OS '$os' — on Windows use the .zip from the Releases page, or 'cargo install imaginu'" ;;
esac
case "$arch" in
  x86_64|amd64)  arch_part="x86_64" ;;
  arm64|aarch64) arch_part="aarch64" ;;
  *) err "unsupported architecture '$arch'" ;;
esac
target="${arch_part}-${os_part}"

# --- resolve version --------------------------------------------------------
version="${IMAGINU_VERSION:-}"
if [ -z "$version" ]; then
  say "Resolving latest release..."
  version="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -n1 | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')"
  [ -n "$version" ] || err "could not determine the latest release tag (set IMAGINU_VERSION=vX.Y.Z)"
fi

archive="${BIN}-${version}-${target}.tar.gz"
base="https://github.com/${REPO}/releases/download/${version}"
say "Installing ${BIN} ${version} for ${target}"

# --- download + verify ------------------------------------------------------
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
curl -fSL --progress-bar "${base}/${archive}" -o "${tmp}/${archive}" \
  || err "download failed: ${base}/${archive}"

if curl -fsSL "${base}/${archive}.sha256" -o "${tmp}/${archive}.sha256" 2>/dev/null; then
  say "Verifying checksum..."
  expected="$(awk '{print $1}' "${tmp}/${archive}.sha256")"
  if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "${tmp}/${archive}" | awk '{print $1}')"
  elif command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "${tmp}/${archive}" | awk '{print $1}')"
  else
    actual=""; warn "no sha256 tool found; skipping checksum verification"
  fi
  if [ -n "$actual" ] && [ "$expected" != "$actual" ]; then
    err "checksum mismatch! expected ${expected}, got ${actual}"
  fi
else
  warn "no published checksum for ${archive}; skipping verification"
fi

tar -xzf "${tmp}/${archive}" -C "$tmp"
[ -f "${tmp}/${BIN}" ] || err "archive did not contain '${BIN}'"
chmod +x "${tmp}/${BIN}"

# --- install ----------------------------------------------------------------
dir="${IMAGINU_INSTALL_DIR:-/usr/local/bin}"
if [ ! -d "$dir" ] || [ ! -w "$dir" ]; then
  if [ "$dir" = "/usr/local/bin" ]; then
    dir="${HOME}/.local/bin"
    mkdir -p "$dir"
    warn "/usr/local/bin not writable; installing to ${dir}"
  else
    err "install dir '$dir' is not a writable directory"
  fi
fi
mv "${tmp}/${BIN}" "${dir}/${BIN}"
say "Installed ${dir}/${BIN}"

case ":${PATH}:" in
  *":${dir}:"*) ;;
  *) warn "${dir} is not on your PATH — add it, e.g.:  export PATH=\"${dir}:\$PATH\"" ;;
esac

"${dir}/${BIN}" --version 2>/dev/null || true
say "Done. Try:  ${BIN} generate '{\"kind\":\"tree\",\"style\":\"oak\"}' -o tree.glb --preview"
