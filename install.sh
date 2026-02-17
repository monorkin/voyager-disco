#!/bin/sh
set -eu

REPO="monorkin/voyager-disco"
BINARY="voyager-disco"

# Detect OS
OS="$(uname -s)"
case "$OS" in
  Linux*)  OS="linux" ;;
  Darwin*) OS="darwin" ;;
  FreeBSD*|OpenBSD*|NetBSD*) OS="freebsd" ;;
  *)
    echo "Error: unsupported OS: $OS" >&2
    exit 1
    ;;
esac

# Detect architecture
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|amd64)   ARCH="amd64" ;;
  aarch64|arm64)   ARCH="arm64" ;;
  *)
    echo "Error: unsupported architecture: $ARCH" >&2
    exit 1
    ;;
esac

# Install directory
case "$OS" in
  linux|freebsd) INSTALL_DIR="/usr/bin" ;;
  *)             INSTALL_DIR="/usr/local/bin" ;;
esac

echo "Detected: ${OS}/${ARCH}"
echo "Install directory: ${INSTALL_DIR}"

# Fetch latest release tag
echo "Fetching latest release..."
TAG="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed 's/.*"tag_name": *"//;s/".*//')"

if [ -z "$TAG" ]; then
  echo "Error: could not determine latest release" >&2
  exit 1
fi

echo "Latest release: ${TAG}"

# Download binary
ASSET="${BINARY}-${OS}-${ARCH}"
URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"

echo "Downloading ${URL}..."
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

curl -fSL -o "${TMPDIR}/${BINARY}" "$URL"
chmod +x "${TMPDIR}/${BINARY}"

# Install
if [ -w "$INSTALL_DIR" ]; then
  mv "${TMPDIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
else
  echo "Need elevated permissions to install to ${INSTALL_DIR}"
  sudo mv "${TMPDIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
fi

echo "Installed ${BINARY} ${TAG} to ${INSTALL_DIR}/${BINARY}"
