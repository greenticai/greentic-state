#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

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

DIST_DIR="${ROOT_DIR}/dist/packs"
mkdir -p "${DIST_DIR}"

command -v python3 >/dev/null 2>&1 || { echo "python3 is required" >&2; exit 1; }
command -v greentic-pack >/dev/null 2>&1 || { echo "greentic-pack is required" >&2; exit 1; }

bash "${ROOT_DIR}/tools/build_state_components.sh"

stamp_pack_files() {
  local pack_dir="$1"
  local component_id="$2"
  python3 - "$pack_dir" "$component_id" "$PACK_VERSION" <<'PY'
from pathlib import Path
import json
import sys

pack_dir = Path(sys.argv[1])
component_id = sys.argv[2]
version = sys.argv[3]

pack_yaml = pack_dir / "pack.yaml"
lines = pack_yaml.read_text().splitlines()
old_version = None
for line in lines:
    stripped = line.lstrip()
    if stripped.startswith("version:"):
        old_version = stripped.split(":", 1)[1].strip().strip("'\"")
        break

out = []
for line in lines:
    stripped = line.lstrip()
    if stripped.startswith("version:"):
        current = stripped.split(":", 1)[1].strip().strip("'\"")
        if current == old_version or current == version:
            prefix = line.split("version:")[0] + "version: "
            out.append(f"{prefix}{version}")
            continue
    out.append(line)
pack_yaml.write_text("\n".join(out) + "\n")

manifest_path = pack_dir / "pack.manifest.json"
manifest = json.loads(manifest_path.read_text())
manifest["version"] = version
for component in manifest.get("component_sources", []):
    if component.get("id") == component_id:
        component["version"] = version
for ext in manifest.get("extensions", {}).values():
    if isinstance(ext, dict):
        ext["version"] = version
        for offer in ext.get("inline", {}).get("offers", []):
            if isinstance(offer, dict):
                offer["version"] = version
manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")

component_manifest_path = pack_dir / "components" / component_id / "component.manifest.json"
component_manifest = json.loads(component_manifest_path.read_text())
component_manifest["version"] = version
component_manifest_path.write_text(json.dumps(component_manifest, indent=2) + "\n")
PY
}

build_pack() {
  local pack_name="$1"
  local component_id="$2"
  local artifact="${ROOT_DIR}/target/components/${component_id}.wasm"
  local pack_dir="${ROOT_DIR}/packs/${pack_name}"
  local pack_component_dir="${pack_dir}/components/${component_id}"
  local pack_out="${DIST_DIR}/${pack_name}.gtpack"

  [ -f "${artifact}" ] || { echo "missing built component: ${artifact}" >&2; exit 1; }
  [ -d "${pack_component_dir}" ] || { echo "missing pack component dir: ${pack_component_dir}" >&2; exit 1; }

  cp "${artifact}" "${pack_component_dir}/${component_id}.wasm"
  stamp_pack_files "${pack_dir}" "${component_id}"

  rm -f "${pack_out}"
  (cd "${pack_dir}" && greentic-pack resolve)
  (cd "${pack_dir}" && greentic-pack build --no-update --in . --gtpack-out "${pack_out}" --secrets-req ".secret_requirements.json")
  python3 "${ROOT_DIR}/tools/validate_pack_extensions.py" "${pack_out}"
}

build_pack "state-memory" "state-provider-memory"
build_pack "state-redis" "state-provider-redis"
