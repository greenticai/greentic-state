#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

OCI_REGISTRY="${OCI_REGISTRY:-ghcr.io}"
OCI_ORG="${OCI_ORG:-${GITHUB_REPOSITORY_OWNER:-greenticai}}"
OCI_REPO_PREFIX="${OCI_REPO_PREFIX:-packs/state}"
PUBLISH_LATEST="${PUBLISH_LATEST:-1}"

PACK_VERSION="${PACK_VERSION:-}"
if [ -z "${PACK_VERSION}" ]; then
  PACK_VERSION="$(python3 - <<'PY'
from pathlib import Path
import tomllib

data = tomllib.loads(Path("Cargo.toml").read_text())
print(data.get("package", {}).get("version") or data.get("workspace", {}).get("package", {}).get("version", "0.0.0"))
PY
)"
fi

command -v oras >/dev/null 2>&1 || { echo "oras is required" >&2; exit 1; }

bash "${ROOT_DIR}/tools/build_state_packs.sh"

push_pack() {
  local pack_name="$1"
  local pack_path="${ROOT_DIR}/dist/packs/${pack_name}.gtpack"
  local pack_dir
  local ref_base="${OCI_REGISTRY}/${OCI_ORG}/${OCI_REPO_PREFIX}/${pack_name}"
  local readme_path="${ROOT_DIR}/packs/${pack_name}/README.md"
  local pack_title pack_desc

  [ -f "${pack_path}" ] || { echo "missing pack artifact: ${pack_path}" >&2; exit 1; }

  pack_title="$(python3 - <<'PY' "${ROOT_DIR}/packs/${pack_name}/pack.manifest.json"
from pathlib import Path
import json, sys
data = json.loads(Path(sys.argv[1]).read_text())
print(data.get("name", ""))
PY
)"
  pack_desc="$(python3 - <<'PY' "${ROOT_DIR}/packs/${pack_name}/pack.manifest.json"
from pathlib import Path
import json, sys
data = json.loads(Path(sys.argv[1]).read_text())
print(data.get("description", ""))
PY
)"

  pack_dir="$(dirname "${pack_path}")"
  cp "${readme_path}" "${pack_dir}/README.md"
  (
    cd "${pack_dir}"
    oras push "${ref_base}:${PACK_VERSION}" \
      --artifact-type application/vnd.greentic.gtpack.v1+zip \
      --annotation "org.opencontainers.image.source=${GITHUB_SERVER_URL:-https://github.com}/${GITHUB_REPOSITORY:-greenticai/greentic-state}" \
      --annotation "org.opencontainers.image.revision=${GITHUB_SHA:-unknown}" \
      --annotation "org.opencontainers.image.version=${PACK_VERSION}" \
      --annotation "org.opencontainers.image.title=${pack_title}" \
      --annotation "org.opencontainers.image.description=${pack_desc}" \
      "${pack_name}.gtpack:application/vnd.greentic.gtpack.v1+zip" \
      "README.md:text/markdown"

    if [[ "${PUBLISH_LATEST}" =~ ^(1|true|TRUE|yes|YES)$ ]]; then
      oras push "${ref_base}:latest" \
        --artifact-type application/vnd.greentic.gtpack.v1+zip \
        --annotation "org.opencontainers.image.source=${GITHUB_SERVER_URL:-https://github.com}/${GITHUB_REPOSITORY:-greenticai/greentic-state}" \
        --annotation "org.opencontainers.image.revision=${GITHUB_SHA:-unknown}" \
        --annotation "org.opencontainers.image.version=${PACK_VERSION}" \
        --annotation "org.opencontainers.image.title=${pack_title}" \
        --annotation "org.opencontainers.image.description=${pack_desc}" \
        "${pack_name}.gtpack:application/vnd.greentic.gtpack.v1+zip" \
        "README.md:text/markdown" >/dev/null
    fi
  )
  rm -f "${pack_dir}/README.md"
}

push_pack "state-memory"
push_pack "state-redis"
