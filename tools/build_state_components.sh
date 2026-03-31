#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

bash "${ROOT_DIR}/tools/build_components/state-provider-memory.sh"
bash "${ROOT_DIR}/tools/build_components/state-provider-redis.sh"
