use provider_common::component_v0_6::{
    DescribePayload, OperationDescriptor, SchemaIr, canonical_cbor_bytes, decode_cbor, schema_hash,
};
use provider_common::helpers::{existing_config_from_answers, i18n};
use provider_common::qa_helpers::ApplyAnswersResult;
use serde::{Deserialize, Serialize};

mod bindings {
    wit_bindgen::generate!({
        path: "wit/state-provider-memory",
        world: "component-v0-v6-v0",
        generate_all
    });
}

const PROVIDER_ID: &str = "state-provider-memory";
const WORLD_ID: &str = "component-v0-v6-v0";

const I18N_KEYS: &[&str] = &[
    "state.memory.op.describe.title",
    "state.memory.op.describe.description",
    "state.memory.schema.input.title",
    "state.memory.schema.input.description",
    "state.memory.schema.output.title",
    "state.memory.schema.output.description",
    "state.memory.schema.output.ok.title",
    "state.memory.schema.output.ok.description",
    "state.memory.schema.config.title",
    "state.memory.schema.config.description",
    "state.memory.schema.config.max_entries.title",
    "state.memory.schema.config.max_entries.description",
    "state.memory.schema.config.default_ttl_seconds.title",
    "state.memory.schema.config.default_ttl_seconds.description",
    "state.memory.qa.default.title",
    "state.memory.qa.setup.title",
    "state.memory.qa.upgrade.title",
    "state.memory.qa.remove.title",
    "state.memory.qa.setup.max_entries",
    "state.memory.qa.setup.default_ttl_seconds",
    // Flow-related i18n keys
    "state.memory.flow.default.title",
    "state.memory.flow.default.config_summary",
];

const I18N_PAIRS: &[(&str, &str)] = &[
    ("state.memory.op.describe.title", "Describe"),
    (
        "state.memory.op.describe.description",
        "Describe in-memory state provider capabilities",
    ),
    ("state.memory.schema.input.title", "State input"),
    (
        "state.memory.schema.input.description",
        "Input for in-memory state provider",
    ),
    ("state.memory.schema.output.title", "State output"),
    (
        "state.memory.schema.output.description",
        "Result of in-memory state provider",
    ),
    ("state.memory.schema.output.ok.title", "Success"),
    (
        "state.memory.schema.output.ok.description",
        "Whether the operation succeeded",
    ),
    ("state.memory.schema.config.title", "Memory state config"),
    (
        "state.memory.schema.config.description",
        "In-memory state provider configuration",
    ),
    (
        "state.memory.schema.config.max_entries.title",
        "Maximum Entries",
    ),
    (
        "state.memory.schema.config.max_entries.description",
        "Maximum number of key-value entries to store (0 = unlimited)",
    ),
    (
        "state.memory.schema.config.default_ttl_seconds.title",
        "Default TTL (seconds)",
    ),
    (
        "state.memory.schema.config.default_ttl_seconds.description",
        "Default time-to-live for entries in seconds (0 = no expiry)",
    ),
    ("state.memory.qa.default.title", "Default"),
    ("state.memory.qa.setup.title", "Setup"),
    ("state.memory.qa.upgrade.title", "Upgrade"),
    ("state.memory.qa.remove.title", "Remove"),
    (
        "state.memory.qa.setup.max_entries",
        "Maximum entries (0 = unlimited)",
    ),
    (
        "state.memory.qa.setup.default_ttl_seconds",
        "Default TTL in seconds (0 = no expiry)",
    ),
];

struct Component;

impl bindings::exports::greentic::component::descriptor::Guest for Component {
    fn describe() -> Vec<u8> {
        canonical_cbor_bytes(&build_describe_payload())
    }
}

impl bindings::exports::greentic::component::runtime::Guest for Component {
    fn invoke(op: String, _input_cbor: Vec<u8>) -> Vec<u8> {
        // State dispatch is handled natively by the operator — this component
        // only provides QA/setup/describe.  Any runtime invoke returns an error
        // directing the caller to use the native state dispatch pipeline.
        canonical_cbor_bytes(&RunResult {
            ok: false,
            error: Some(format!(
                "state-provider-memory: runtime invoke not supported for op '{op}'; \
                 state operations are dispatched natively by the operator"
            )),
        })
    }
}

impl bindings::exports::greentic::component::qa::Guest for Component {
    fn qa_spec(mode: bindings::exports::greentic::component::qa::Mode) -> Vec<u8> {
        canonical_cbor_bytes(&build_qa_spec(mode))
    }

    fn apply_answers(
        mode: bindings::exports::greentic::component::qa::Mode,
        answers_cbor: Vec<u8>,
    ) -> Vec<u8> {
        use bindings::exports::greentic::component::qa::Mode;
        let mode_str = match mode {
            Mode::Default => "default",
            Mode::Setup => "setup",
            Mode::Upgrade => "upgrade",
            Mode::Remove => "remove",
        };
        apply_answers_impl(mode_str, answers_cbor)
    }
}

impl bindings::exports::greentic::component::component_i18n::Guest for Component {
    fn i18n_keys() -> Vec<String> {
        provider_common::helpers::i18n_keys_from(I18N_KEYS)
    }

    fn i18n_bundle(locale: String) -> Vec<u8> {
        provider_common::helpers::i18n_bundle_from_pairs(locale, I18N_PAIRS)
    }
}

// Backward-compatible schema-core-api export for operator v0.4.x
impl bindings::exports::greentic::provider_schema_core::schema_core_api::Guest for Component {
    fn describe() -> Vec<u8> {
        provider_common::helpers::schema_core_describe(&build_describe_payload())
    }

    fn validate_config(_config_json: Vec<u8>) -> Vec<u8> {
        provider_common::helpers::schema_core_validate_config()
    }

    fn healthcheck() -> Vec<u8> {
        provider_common::helpers::schema_core_healthcheck()
    }

    fn invoke(op: String, input_json: Vec<u8>) -> Vec<u8> {
        if let Some(result) = provider_common::qa_invoke_bridge::dispatch_qa_ops_with_i18n(
            &op,
            &input_json,
            "state.memory",
            SETUP_QUESTIONS,
            DEFAULT_KEYS,
            I18N_KEYS,
            I18N_PAIRS,
            apply_answers_impl,
        ) {
            return result;
        }
        serde_json::to_vec(&RunResult {
            ok: false,
            error: Some(format!("unsupported op: {op}")),
        })
        .unwrap_or_default()
    }
}

bindings::export!(Component with_types_in bindings);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunResult {
    ok: bool,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderConfig {
    #[serde(default = "default_max_entries")]
    max_entries: u32,
    #[serde(default)]
    default_ttl_seconds: u32,
}

const fn default_max_entries() -> u32 {
    10000
}

fn build_describe_payload() -> DescribePayload {
    let input_schema = input_schema();
    let output_schema = output_schema();
    let config_schema = config_schema();
    let hash = schema_hash(&input_schema, &output_schema, &config_schema);

    DescribePayload {
        provider: PROVIDER_ID.to_string(),
        world: WORLD_ID.to_string(),
        operations: vec![OperationDescriptor {
            name: "state.dispatch".to_string(),
            title: i18n("state.memory.op.describe.title"),
            description: i18n("state.memory.op.describe.description"),
        }],
        input_schema,
        output_schema,
        config_schema,
        redactions: vec![],
        schema_hash: hash,
    }
}

const SETUP_QUESTIONS: &[provider_common::helpers::QaQuestionDef] = &[
    ("max_entries", "state.memory.qa.setup.max_entries", false),
    (
        "default_ttl_seconds",
        "state.memory.qa.setup.default_ttl_seconds",
        false,
    ),
];
const DEFAULT_KEYS: &[&str] = &[];

fn build_qa_spec(
    mode: bindings::exports::greentic::component::qa::Mode,
) -> provider_common::component_v0_6::QaSpec {
    use bindings::exports::greentic::component::qa::Mode;
    let mode_str = match mode {
        Mode::Default => "default",
        Mode::Setup => "setup",
        Mode::Upgrade => "upgrade",
        Mode::Remove => "remove",
    };
    provider_common::helpers::qa_spec_for_mode(
        mode_str,
        "state.memory",
        SETUP_QUESTIONS,
        DEFAULT_KEYS,
    )
}

fn input_schema() -> SchemaIr {
    provider_common::helpers::schema_obj(
        "state.memory.schema.input.title",
        "state.memory.schema.input.description",
        vec![],
        false,
    )
}

fn output_schema() -> SchemaIr {
    provider_common::helpers::schema_obj(
        "state.memory.schema.output.title",
        "state.memory.schema.output.description",
        vec![(
            "ok",
            true,
            provider_common::helpers::schema_bool_ir(
                "state.memory.schema.output.ok.title",
                "state.memory.schema.output.ok.description",
            ),
        )],
        false,
    )
}

fn config_schema() -> SchemaIr {
    provider_common::helpers::schema_obj(
        "state.memory.schema.config.title",
        "state.memory.schema.config.description",
        vec![
            (
                "max_entries",
                false,
                provider_common::helpers::schema_str(
                    "state.memory.schema.config.max_entries.title",
                    "state.memory.schema.config.max_entries.description",
                ),
            ),
            (
                "default_ttl_seconds",
                false,
                provider_common::helpers::schema_str(
                    "state.memory.schema.config.default_ttl_seconds.title",
                    "state.memory.schema.config.default_ttl_seconds.description",
                ),
            ),
        ],
        false,
    )
}

fn default_config_out() -> ProviderConfig {
    ProviderConfig {
        max_entries: 10000,
        default_ttl_seconds: 0,
    }
}

fn apply_answers_impl(mode: &str, answers_cbor: Vec<u8>) -> Vec<u8> {
    let answers: serde_json::Value = match decode_cbor(&answers_cbor) {
        Ok(value) => value,
        Err(err) => {
            return canonical_cbor_bytes(&ApplyAnswersResult::<ProviderConfig>::decode_error(
                format!("invalid answers cbor: {err}"),
            ));
        }
    };

    if mode == "remove" {
        return canonical_cbor_bytes(&ApplyAnswersResult::<ProviderConfig>::remove(vec![
            "delete_config_key".to_string(),
            "delete_provider_state_namespace".to_string(),
        ]));
    }

    let mut merged = existing_config_from_answers(&answers).unwrap_or_else(default_config_out);
    let answer_obj = answers.as_object();
    let has = |key: &str| answer_obj.is_some_and(|obj| obj.contains_key(key));

    if mode == "setup" || mode == "default" {
        if let Some(v) = answers
            .get("max_entries")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u32>().ok())
        {
            merged.max_entries = v;
        }
        if let Some(v) = answers
            .get("default_ttl_seconds")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u32>().ok())
        {
            merged.default_ttl_seconds = v;
        }
    }

    if mode == "upgrade" {
        if has("max_entries")
            && let Some(v) = answers
                .get("max_entries")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u32>().ok())
        {
            merged.max_entries = v;
        }
        if has("default_ttl_seconds")
            && let Some(v) = answers
                .get("default_ttl_seconds")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u32>().ok())
        {
            merged.default_ttl_seconds = v;
        }
    }

    canonical_cbor_bytes(&ApplyAnswersResult::success(merged))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bindings::exports::greentic::component::qa::Guest as QaGuest;
    use bindings::exports::greentic::component::qa::Mode;

    #[test]
    fn describe_payload_serializes() {
        let payload = build_describe_payload();
        assert_eq!(payload.provider, PROVIDER_ID);
        assert_eq!(payload.world, WORLD_ID);
        assert_eq!(payload.operations.len(), 1);
        assert_eq!(payload.operations[0].name, "state.dispatch");
        let bytes = canonical_cbor_bytes(&payload);
        assert!(!bytes.is_empty());
    }

    #[test]
    fn qa_spec_default_mode() {
        let spec = build_qa_spec(Mode::Default);
        assert_eq!(spec.mode, "default");
    }

    #[test]
    fn qa_spec_setup_mode() {
        let spec = build_qa_spec(Mode::Setup);
        assert_eq!(spec.mode, "setup");
    }

    #[test]
    fn qa_spec_remove_mode() {
        let spec = build_qa_spec(Mode::Remove);
        assert_eq!(spec.mode, "remove");
    }

    #[test]
    fn apply_answers_setup_produces_config() {
        let answers = serde_json::json!({
            "max_entries": "5000",
            "default_ttl_seconds": "300"
        });
        let out =
            <Component as QaGuest>::apply_answers(Mode::Setup, canonical_cbor_bytes(&answers));
        let out_json: serde_json::Value = decode_cbor(&out).expect("decode apply output");
        assert_eq!(out_json.get("ok"), Some(&serde_json::Value::Bool(true)));
        let config = out_json.get("config").expect("config object");
        assert_eq!(config.get("max_entries"), Some(&serde_json::json!(5000)));
        assert_eq!(
            config.get("default_ttl_seconds"),
            Some(&serde_json::json!(300))
        );
    }

    #[test]
    fn apply_answers_remove_returns_cleanup() {
        let answers = serde_json::json!({});
        let out =
            <Component as QaGuest>::apply_answers(Mode::Remove, canonical_cbor_bytes(&answers));
        let out_json: serde_json::Value = decode_cbor(&out).expect("decode remove output");
        assert!(out_json.get("remove").is_some());
    }

    #[test]
    fn i18n_keys_nonempty() {
        use bindings::exports::greentic::component::component_i18n::Guest as I18nGuest;
        let keys = <Component as I18nGuest>::i18n_keys();
        assert!(!keys.is_empty());
        assert!(keys.contains(&"state.memory.qa.setup.max_entries".to_string()));
    }
}
