#!/bin/sh
set -eu

REPO="ejklock/active-collab-cli"
BIN_NAME="active-collab"

_os="$(uname -s)"
case "${_os}" in
  Linux)  _platform="linux" ;;
  Darwin) _platform="macos" ;;
  *)
    echo "Unsupported OS: ${_os}" >&2
    echo "Supported: Linux, Darwin (macOS)" >&2
    exit 1
    ;;
esac

_arch="$(uname -m)"
case "${_arch}" in
  x86_64|amd64) _arch_tag="x86_64" ;;
  arm64|aarch64)
    if [ "${_platform}" = "macos" ]; then
      _arch_tag="arm64"
    else
      echo "Unsupported arch for Linux: ${_arch} (only x86_64 is distributed)" >&2
      exit 1
    fi
    ;;
  *)
    echo "Unsupported architecture: ${_arch}" >&2
    echo "Supported: x86_64 (Linux + macOS), arm64 (macOS only)" >&2
    exit 1
    ;;
esac

_asset="${BIN_NAME}-${_platform}-${_arch_tag}"

if [ -n "${VERSION:-}" ]; then
  _tag="${VERSION}"
elif [ "${1:-}" != "" ]; then
  _tag="$1"
else
  _tag="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' \
    | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
  if [ -z "${_tag}" ]; then
    echo "Error: could not determine latest release tag." >&2
    exit 1
  fi
fi

_url="https://github.com/${REPO}/releases/download/${_tag}/${_asset}"

echo "Downloading ${_asset} (${_tag}) ..."
_tmp="$(mktemp)"
curl -fsSL -o "${_tmp}" "${_url}"

if [ -w "/usr/local/bin" ]; then
  _install_dir="/usr/local/bin"
else
  _install_dir="${HOME}/.local/bin"
  mkdir -p "${_install_dir}"
  case ":${PATH}:" in
    *":${_install_dir}:"*) ;;
    *)
      echo "Warning: ${_install_dir} is not on your PATH." >&2
      echo "  Add the following to your shell profile:" >&2
      echo "    export PATH=\"\${HOME}/.local/bin:\${PATH}\"" >&2
      ;;
  esac
fi

_dest="${_install_dir}/${BIN_NAME}"
mv "${_tmp}" "${_dest}"
chmod +x "${_dest}"

echo "Installed to ${_dest}"
"${_dest}" --help
