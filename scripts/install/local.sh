#!/usr/bin/env bash
# Build Codex from this checkout and install the `codexium` binary locally.
#
# Usage:
#   ./scripts/install/local.sh [--profile release|debug] [--skip-build]
#
# Environment:
#   CODEX_INSTALL_DIR   Destination directory (default: ~/.local/bin)
#   CODEX_BUILD_PROFILE release or debug (default: release; overridden by --profile)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CRATE_DIR="$REPO_ROOT/codex-rs"
BIN_DIR="${CODEX_INSTALL_DIR:-$HOME/.local/bin}"
PROFILE="${CODEX_BUILD_PROFILE:-release}"
SKIP_BUILD=0

step() {
  printf '==> %s\n' "$1"
}

warn() {
  printf 'WARNING: %s\n' "$1" >&2
}

usage() {
  cat <<EOF
Usage: local.sh [--profile release|debug] [--skip-build]

Build Codex from this checkout and install the binary to \$CODEX_INSTALL_DIR (default: ~/.local/bin).

Options:
  --profile release|debug   Build profile (default: release)
  --skip-build              Install an existing build without recompiling
  -h, --help                Show this help

Environment:
  CODEX_INSTALL_DIR         Install directory (default: ~/.local/bin)
  CODEX_BUILD_PROFILE       Same as --profile when no flag is passed

After install, run: codexium
Compose mode: Shift+Tab or /compose -- see README.md
EOF
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --profile)
        [ "$#" -ge 2 ] || {
          echo "--profile requires a value." >&2
          exit 1
        }
        PROFILE="$2"
        shift
        ;;
      --skip-build)
        SKIP_BUILD=1
        ;;
      -h | --help)
        usage
        exit 0
        ;;
      *)
        echo "Unknown argument: $1" >&2
        usage >&2
        exit 1
        ;;
    esac
    shift
  done

  case "$PROFILE" in
    release | debug) ;;
    *)
      echo "Invalid profile: $PROFILE (expected release or debug)." >&2
      exit 1
      ;;
  esac
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "$1 is required." >&2
    exit 1
  fi
}

ensure_rust_toolchain() {
  require_command cargo
  require_command rustc
}

build_codex() {
  step "Building codexium ($PROFILE) in $CRATE_DIR"
  (
    cd "$CRATE_DIR"
    if [ "$PROFILE" = "release" ]; then
      cargo build --release -p codex-cli --bin codexium
    else
      cargo build -p codex-cli --bin codexium
    fi
  )
}

binary_path() {
  printf '%s/target/%s/codexium\n' "$CRATE_DIR" "$PROFILE"
}

install_binary() {
  local source_path="$1"
  local dest_path="$BIN_DIR/codexium"

  if [ ! -x "$source_path" ]; then
    echo "Built binary not found at $source_path" >&2
    exit 1
  fi

  mkdir -p "$BIN_DIR"
  step "Installing $dest_path"
  ln -sf "$source_path" "$dest_path"
}

path_hint() {
  case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
      warn "$BIN_DIR is not on PATH."
      step "Add to this shell: export PATH=\"$BIN_DIR:\$PATH\""
      step "Add permanently: echo 'export PATH=\"$BIN_DIR:\$PATH\"' >> ~/.bashrc  # or ~/.zshrc"
      ;;
  esac
}

parse_args "$@"
ensure_rust_toolchain

if [ "$SKIP_BUILD" -eq 0 ]; then
  build_codex
else
  step "Skipping build (--skip-build)"
fi

install_binary "$(binary_path)"
path_hint

step "Installed $( "$BIN_DIR/codexium" --version 2>/dev/null || echo 'codexium' )"
step "Launch: codexium"
step "Compose mode: Shift+Tab or /compose (see README.md)"
