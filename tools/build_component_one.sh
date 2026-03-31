#!/usr/bin/env bash
set -euo pipefail

PACKAGE_NAME="${1:-}"
if [ -z "${PACKAGE_NAME}" ]; then
  echo "Usage: $0 <package-name>" >&2
  exit 1
fi

source "$(dirname "${BASH_SOURCE[0]}")/build_component_env.sh"

ARTIFACT_NAME="${PACKAGE_NAME//-/_}.wasm"
ARTIFACT_PATH="${TARGET_DIR_OVERRIDE}/release/${ARTIFACT_NAME}"
NESTED_ARTIFACT_PATH="${TARGET_DIR_OVERRIDE}/${BUILD_TARGET}/release/${ARTIFACT_NAME}"
NESTED_WASIP1_PATH="${TARGET_DIR_OVERRIDE}/wasm32-wasip1/release/${ARTIFACT_NAME}"

cargo +"${RUSTUP_TOOLCHAIN}" build --release --package "${PACKAGE_NAME}" --target "${BUILD_TARGET}" --target-dir "${TARGET_DIR_OVERRIDE}"

if [ ! -f "${ARTIFACT_PATH}" ] && [ -f "${NESTED_ARTIFACT_PATH}" ]; then
  ARTIFACT_PATH="${NESTED_ARTIFACT_PATH}"
fi

if [ ! -f "${ARTIFACT_PATH}" ] && [ -f "${NESTED_WASIP1_PATH}" ]; then
  echo "Found wasm32-wasip1 artifact for ${PACKAGE_NAME} (${NESTED_WASIP1_PATH}). Expected wasm32-wasip2 output." >&2
  exit 1
fi

if [ ! -f "${ARTIFACT_PATH}" ]; then
  echo "Expected artifact not found: ${ARTIFACT_PATH}" >&2
  exit 1
fi

cp "${ARTIFACT_PATH}" "${TARGET_DIR}/${PACKAGE_NAME}.wasm"
if [ "${PACKAGE_NAME}" = "questions" ]; then
  cp "${ARTIFACT_PATH}" "${ROOT_DIR}/components/questions/questions.wasm"
fi
if [ "${HAS_WASM_TOOLS}" -eq 1 ] && [ "${SKIP_WASM_TOOLS_VALIDATION}" -eq 0 ]; then
  if ! "${WASM_TOOLS_BIN}" component wit "${TARGET_DIR}/${PACKAGE_NAME}.wasm" | grep -q "wasi:cli/"; then
    echo "Artifact ${PACKAGE_NAME} does not appear to target WASI preview 2 (missing wasi:cli import)" >&2
    exit 1
  fi
  "${WASM_TOOLS_BIN}" validate "${TARGET_DIR}/${PACKAGE_NAME}.wasm" >/dev/null
fi

echo "Built ${TARGET_DIR}/${PACKAGE_NAME}.wasm"
