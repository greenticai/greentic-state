#!/usr/bin/env bash
set -euo pipefail

if [ "${COMPONENT_BUILD_ENV_READY:-0}" = "1" ]; then
  return 0
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${TARGET_DIR:-${ROOT_DIR}/target/components}"
BUILD_TARGET="wasm32-wasip2"
TARGET_DIR_OVERRIDE="${TARGET_DIR_OVERRIDE:-${ROOT_DIR}/target/${BUILD_TARGET}}"
WASM_TOOLS_BIN="${WASM_TOOLS_BIN:-wasm-tools}"
SKIP_WASM_TOOLS_VALIDATION="${SKIP_WASM_TOOLS_VALIDATION:-0}"
HAS_WASM_TOOLS=0
RUST_TOOLCHAIN_FILE="${ROOT_DIR}/rust-toolchain.toml"

export XDG_CACHE_HOME="${XDG_CACHE_HOME:-${ROOT_DIR}/.cache}"
mkdir -p "${XDG_CACHE_HOME}"

if [[ -f "${RUST_TOOLCHAIN_FILE}" ]]; then
  TOOLCHAIN_CHANNEL="$(awk -F'"' '/channel/ {print $2; exit}' "${RUST_TOOLCHAIN_FILE}")"
fi
if [[ -z "${TOOLCHAIN_CHANNEL:-}" ]]; then
  TOOLCHAIN_CHANNEL="$(rustc --version | awk '{print $2}')"
fi
RUSTC_HOST_TRIPLE="$(
  rustc -vV | awk '
    /^host: / { host = $2 }
    END {
      if (host == "") exit 1
      print host
    }
  '
)"
if [[ -z "${RUSTC_HOST_TRIPLE}" ]]; then
  echo "failed to determine rustc host triple" >&2
  exit 1
fi
FULL_RUSTUP_TOOLCHAIN="${TOOLCHAIN_CHANNEL}-${RUSTC_HOST_TRIPLE}"
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-${FULL_RUSTUP_TOOLCHAIN}}"

if ! rustup target list --toolchain "${RUSTUP_TOOLCHAIN}" --installed | grep -q "${BUILD_TARGET}"; then
  echo "Installing Rust target ${BUILD_TARGET}..."
  rustup target add --toolchain "${RUSTUP_TOOLCHAIN}" "${BUILD_TARGET}" || {
    # Retry once — a parallel worker may have raced us.
    sleep 1
    rustup target list --toolchain "${RUSTUP_TOOLCHAIN}" --installed | grep -q "${BUILD_TARGET}" \
      || rustup target add --toolchain "${RUSTUP_TOOLCHAIN}" "${BUILD_TARGET}"
  }
fi

if command -v "${WASM_TOOLS_BIN}" >/dev/null 2>&1; then
  HAS_WASM_TOOLS=1
else
  echo "wasm-tools not found; skipping WASI preview 2 validation checks (install wasm-tools to enable)" >&2
fi

mkdir -p "${TARGET_DIR}"
mkdir -p "${TARGET_DIR_OVERRIDE}"
mkdir -p "${TARGET_DIR_OVERRIDE}/wasm32-wasip1/release/deps"
mkdir -p "${TARGET_DIR_OVERRIDE}/wasm32-wasip1/debug/deps"
mkdir -p "${TARGET_DIR_OVERRIDE}/wasm32-wasip2/release/deps"
mkdir -p "${TARGET_DIR_OVERRIDE}/wasm32-wasip2/debug/deps"

export ROOT_DIR TARGET_DIR BUILD_TARGET TARGET_DIR_OVERRIDE WASM_TOOLS_BIN SKIP_WASM_TOOLS_VALIDATION HAS_WASM_TOOLS RUSTUP_TOOLCHAIN
export COMPONENT_BUILD_ENV_READY=1
