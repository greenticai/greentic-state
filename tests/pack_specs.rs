use serde_json::Value as JsonValue;
use serde_yaml_bw::Value as YamlValue;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn pack_dirs() -> Vec<PathBuf> {
    ["state-memory", "state-redis"]
        .into_iter()
        .map(|name| repo_root().join("packs").join(name))
        .collect()
}

fn yaml(path: &Path) -> YamlValue {
    serde_yaml_bw::from_str(&fs::read_to_string(path).expect("read yaml")).expect("parse yaml")
}

fn json(path: &Path) -> JsonValue {
    serde_json::from_slice(&fs::read(path).expect("read json")).expect("parse json")
}

#[test]
fn state_packs_have_setup_specs() {
    for pack_dir in pack_dirs() {
        let setup = pack_dir.join("assets").join("setup.yaml");
        assert!(setup.exists(), "missing {}", setup.display());
        let _: YamlValue = yaml(&setup);
    }
}

#[test]
fn pack_manifest_matches_pack_yaml_identity() {
    for pack_dir in pack_dirs() {
        let pack_yaml = yaml(&pack_dir.join("pack.yaml"));
        let manifest = json(&pack_dir.join("pack.manifest.json"));

        let pack_id = pack_yaml
            .get("pack_id")
            .and_then(YamlValue::as_str)
            .expect("pack_id");
        let version = pack_yaml
            .get("version")
            .and_then(YamlValue::as_str)
            .expect("version");

        assert_eq!(
            manifest.get("name").and_then(JsonValue::as_str),
            Some(pack_id)
        );
        assert_eq!(
            manifest.get("version").and_then(JsonValue::as_str),
            Some(version)
        );
    }
}

#[test]
fn component_manifest_paths_exist_for_each_pack() {
    for pack_dir in pack_dirs() {
        let manifest = json(&pack_dir.join("pack.manifest.json"));
        let components = manifest
            .get("component_sources")
            .and_then(JsonValue::as_array)
            .expect("component_sources");

        for component in components {
            let wasm = component
                .get("wasm")
                .and_then(JsonValue::as_str)
                .expect("wasm");
            let component_manifest = component
                .get("manifest")
                .and_then(JsonValue::as_str)
                .expect("manifest");

            assert!(
                pack_dir.join(wasm).exists(),
                "missing component wasm {}",
                pack_dir.join(wasm).display()
            );
            assert!(
                pack_dir.join(component_manifest).exists(),
                "missing component manifest {}",
                pack_dir.join(component_manifest).display()
            );
        }
    }
}

#[test]
fn redis_pack_declares_redis_secret_requirement() {
    let manifest = json(&repo_root().join("packs/state-redis/pack.manifest.json"));
    let requirements = manifest
        .get("secret_requirements")
        .and_then(JsonValue::as_array)
        .expect("secret_requirements");

    assert!(
        requirements.iter().any(|req| {
            req.get("name").and_then(JsonValue::as_str) == Some("redis_url")
                && req.get("scope").and_then(JsonValue::as_str) == Some("tenant")
        }),
        "state-redis should declare redis_url tenant secret requirement"
    );
}
