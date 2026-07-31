#!/bin/sh
set -eu

REPO="ejklock/active-collab-cli"
BIN_NAME="active-collab"
ALIAS_NAME="ac"

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

_alias_dest="${_install_dir}/${ALIAS_NAME}"
_alias_linked=0

# The binary reports the name it was invoked as, so either name identifies it.
_is_this_cli() {
  _reported="$("$1" --version 2>/dev/null)" || return 1
  case "${_reported}" in
    "${BIN_NAME} "*|"${ALIAS_NAME} "*) return 0 ;;
    *) return 1 ;;
  esac
}

# A plain file named `ac` is replaced only when it identifies itself as this CLI
# (a copy left by an older install); anything else belongs to another program.
if [ -e "${_alias_dest}" ] && [ ! -L "${_alias_dest}" ]; then
  if _is_this_cli "${_alias_dest}"; then
    rm -f "${_alias_dest}"
  else
    echo "Warning: ${_alias_dest} already exists and is not this CLI; leaving it alone." >&2
    if ! "${_alias_dest}" --version >/dev/null 2>&1; then
      echo "  It does not run on this machine — a 'make install' build for another platform leaves exactly that." >&2
    fi
    echo "  Run '${BIN_NAME}', or remove ${_alias_dest} and re-run this installer." >&2
  fi
fi

if [ ! -e "${_alias_dest}" ] || [ -L "${_alias_dest}" ]; then
  ln -sf "${BIN_NAME}" "${_alias_dest}"
  echo "Linked ${_alias_dest} -> ${BIN_NAME}"
  _alias_linked=1
fi

# macOS ships /usr/sbin/ac (login accounting), so the alias is only usable when
# the install dir wins the PATH lookup.
if [ "${_alias_linked}" = "1" ]; then
  _on_path="$(command -v "${ALIAS_NAME}" 2>/dev/null || true)"
  if [ -n "${_on_path}" ] && [ "${_on_path}" != "${_alias_dest}" ]; then
    echo "Warning: '${ALIAS_NAME}' still resolves to ${_on_path}, which comes earlier on your PATH." >&2
    echo "  Put ${_install_dir} ahead of it, or run '${BIN_NAME}'." >&2
  fi
fi

"${_dest}" --help
