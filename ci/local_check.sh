#!/usr/bin/env bash
set -euo pipefail

# Usage:
#   LOCAL_CHECK_ONLINE=1 LOCAL_CHECK_STRICT=1 LOCAL_CHECK_VERBOSE=1 ci/local_check.sh
# Defaults: offline, non-strict, quiet.

LOCAL_CHECK_ONLINE="${LOCAL_CHECK_ONLINE:-0}"
LOCAL_CHECK_STRICT="${LOCAL_CHECK_STRICT:-0}"
LOCAL_CHECK_VERBOSE="${LOCAL_CHECK_VERBOSE:-0}"
LOCAL_CHECK_SKIP_REDIS="${LOCAL_CHECK_SKIP_REDIS:-0}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
RUST_TOOLCHAIN_FILE="${PROJECT_ROOT}/rust-toolchain.toml"
if [[ -f "${RUST_TOOLCHAIN_FILE}" ]]; then
  TOOLCHAIN_CHANNEL="$(awk -F'"' '/channel/ {print $2; exit}' "${RUST_TOOLCHAIN_FILE}")"
  if [[ -n "${TOOLCHAIN_CHANNEL}" ]]; then
    export RUSTUP_TOOLCHAIN="${TOOLCHAIN_CHANNEL}"
    if [[ "${LOCAL_CHECK_VERBOSE}" != "0" ]]; then
      echo "Using Rust toolchain ${RUSTUP_TOOLCHAIN} (from ${RUST_TOOLCHAIN_FILE})"
    fi
  fi
fi
SKIP_CODE=99
REDIS_CONTAINER=""
REDIS_MANAGED=0
HOST_PACKAGES=(-p greentic-state -p provider-common -p greentic-messaging-renderer)

if [[ "${LOCAL_CHECK_VERBOSE}" != "0" ]]; then
  set -x
fi

need() {
  if command -v "$1" >/dev/null 2>&1; then
    return 0
  fi
  echo "[miss] $1"
  return 1
}

step() {
  echo ""
  echo "▶ $*"
}

run_or_skip() {
  local desc="$1"
  shift
  if "$@"; then
    return 0
  fi
  local status=$?
  if [[ $status -eq $SKIP_CODE ]]; then
    echo "[skip] $desc"
    return 0
  fi
  return $status
}

require_tool() {
  local tool="$1"
  if need "$tool"; then
    return 0
  fi
  if [[ "${LOCAL_CHECK_STRICT}" == "1" ]]; then
    echo "[fail] Missing required tool: ${tool}"
    return 1
  fi
  return "$SKIP_CODE"
}

require_online() {
  local desc="$1"
  if [[ "${LOCAL_CHECK_ONLINE}" == "1" ]]; then
    return 0
  fi
  echo "[offline] ${desc} (set LOCAL_CHECK_ONLINE=1 to run)"
  if [[ "${LOCAL_CHECK_STRICT}" == "1" ]]; then
    echo "[fail] Strict mode requires online step: ${desc}"
    return 1
  fi
  return "$SKIP_CODE"
}

stop_redis() {
  if [[ -n "${REDIS_CONTAINER}" && "${REDIS_MANAGED}" -eq 1 ]]; then
    docker stop "${REDIS_CONTAINER}" >/dev/null 2>&1 || true
  fi
  REDIS_CONTAINER=""
  REDIS_MANAGED=0
}

stop_redis_ci() {
  if ! command -v docker >/dev/null 2>&1; then
    return 0
  fi
  if ! docker ps --format '{{.Names}}' | grep -Fxq "redis-ci"; then
    return 0
  fi
  docker stop "redis-ci" >/dev/null 2>&1 || true
}

trap stop_redis EXIT

start_redis() {
  if [[ "${LOCAL_CHECK_SKIP_REDIS}" == "1" ]]; then
    echo "[skip] Redis provisioning disabled (LOCAL_CHECK_SKIP_REDIS=1)"
    return "$SKIP_CODE"
  fi

  if [[ -n "${REDIS_URL:-}" ]]; then
    echo "Using supplied REDIS_URL=${REDIS_URL}"
    return 0
  fi

  require_tool docker || return $?

  local name="greentic-state-redis-local-check"

  if ! docker ps --format '{{.Names}}' | grep -Fxq "${name}"; then
    if ! docker run -d --rm --name "${name}" -p 6379:6379 redis:7 >/dev/null; then
      echo "[fail] Unable to start Redis container"
      return 1
    fi
    REDIS_MANAGED=1
  else
    echo "Reusing running container ${name}"
    REDIS_MANAGED=0
  fi

  REDIS_CONTAINER="${name}"
  export REDIS_URL="redis://127.0.0.1:6379/"

  if ! timeout 30s bash -c "until docker exec \"${name}\" redis-cli ping >/dev/null 2>&1; do sleep 1; done"; then
    echo "[fail] Redis container failed health check"
    stop_redis
    return 1
  fi

  return 0
}

show_rust_versions() {
  require_tool rustc || return $?
  rustc --version
  require_tool cargo || return $?
  cargo --version
  return 0
}

show_optional_versions() {
  if command -v docker >/dev/null 2>&1; then
    docker --version
  fi
  if command -v jq >/dev/null 2>&1; then
    jq --version
  fi
  return 0
}

fmt_check() {
  if ! need cargo; then
    echo "[fail] cargo not found; install Rust toolchain to run rustfmt checks"
    return 1
  fi
  if ! need rustfmt; then
    echo "[fail] rustfmt component missing; run 'rustup component add rustfmt'"
    return 1
  fi
  cargo fmt --all -- --check
}

clippy_check() {
  require_tool cargo || return $?
  cargo clippy "${HOST_PACKAGES[@]}" --all-targets -- -D warnings
}

build_check() {
  require_tool cargo || return $?
  cargo build "${HOST_PACKAGES[@]}" --locked
}

state_packs_check() {
  require_tool cargo || return $?
  require_tool greentic-pack || return $?
  bash ./tools/build_state_packs.sh
}

tests_check() {
  require_tool cargo || return $?
  if ! start_redis; then
    return $?
  fi
  export REDIS_URL="${REDIS_URL:-redis://127.0.0.1:6379/}"
  cargo test "${HOST_PACKAGES[@]}" --all-features
}

deps_sanity() {
  require_tool cargo || return $?
  require_tool jq || return $?
  cargo metadata --format-version 1 --locked \
    | jq -r '.packages[] | select(.name=="greentic-state") | .dependencies[] | select(.source==null or (.source|startswith("git+"))) | .name' \
    | tee /dev/stderr \
    | (! grep .)
}

publish_dry_run() {
  require_online "cargo publish --dry-run" || return $?
  require_tool cargo || return $?
  stop_redis_ci
  cargo publish --dry-run
}

step "Toolchain versions"
run_or_skip "rustc/cargo versions" show_rust_versions
run_or_skip "docker/jq versions" show_optional_versions

step "Rustfmt"
run_or_skip "cargo fmt --all -- --check" fmt_check

step "Clippy"
run_or_skip "cargo clippy host packages" clippy_check

step "Build"
run_or_skip "cargo build host packages" build_check

step "Tests (with Redis)"
run_or_skip "cargo test host packages --all-features" tests_check

step "State Packs"
run_or_skip "build state-memory/state-redis gtpacks" state_packs_check

step "Dependency sanity"
run_or_skip "cargo metadata path/git dependency check" deps_sanity

step "Publish dry run"
run_or_skip "cargo publish --dry-run" publish_dry_run

echo ""
echo "Local checks completed"
