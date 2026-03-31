use provider_common::component_v0_6::{
    DescribePayload, OperationDescriptor, RedactionRule, SchemaIr, canonical_cbor_bytes,
    decode_cbor, schema_hash,
};
use provider_common::helpers::{existing_config_from_answers, i18n, string_or_default};
use provider_common::qa_helpers::ApplyAnswersResult;
use serde::{Deserialize, Serialize};

mod bindings {
    wit_bindgen::generate!({
        path: "wit/state-provider-redis",
        world: "component-v0-v6-v0",
        generate_all
    });
}

const PROVIDER_ID: &str = "state-provider-redis";
const WORLD_ID: &str = "component-v0-v6-v0";

const I18N_KEYS: &[&str] = &[
    "state.redis.op.describe.title",
    "state.redis.op.describe.description",
    "state.redis.schema.input.title",
    "state.redis.schema.input.description",
    "state.redis.schema.output.title",
    "state.redis.schema.output.description",
    "state.redis.schema.output.ok.title",
    "state.redis.schema.output.ok.description",
    "state.redis.schema.config.title",
    "state.redis.schema.config.description",
    "state.redis.schema.config.redis_url.title",
    "state.redis.schema.config.redis_url.description",
    "state.redis.schema.config.redis_password.title",
    "state.redis.schema.config.redis_password.description",
    "state.redis.schema.config.redis_tls_enabled.title",
    "state.redis.schema.config.redis_tls_enabled.description",
    "state.redis.schema.config.key_prefix.title",
    "state.redis.schema.config.key_prefix.description",
    "state.redis.schema.config.default_ttl_seconds.title",
    "state.redis.schema.config.default_ttl_seconds.description",
    "state.redis.schema.config.connection_pool_size.title",
    "state.redis.schema.config.connection_pool_size.description",
    "state.redis.qa.default.title",
    "state.redis.qa.setup.title",
    "state.redis.qa.upgrade.title",
    "state.redis.qa.remove.title",
    "state.redis.qa.setup.redis_url",
    "state.redis.qa.setup.redis_password",
    "state.redis.qa.setup.redis_tls_enabled",
    "state.redis.qa.setup.key_prefix",
    "state.redis.qa.setup.default_ttl_seconds",
    "state.redis.qa.setup.connection_pool_size",
    // Flow-related i18n keys
    "state.redis.flow.default.title",
    "state.redis.flow.default.config_summary",
    "state.redis.flow.update.title",
    "state.redis.flow.update.collect",
    "state.redis.flow.update.complete",
    "state.redis.flow.remove.title",
    "state.redis.flow.remove.check_state",
    "state.redis.flow.remove.complete",
];

const I18N_PAIRS: &[(&str, &str)] = &[
    ("state.redis.op.describe.title", "Describe"),
    (
        "state.redis.op.describe.description",
        "Describe Redis state provider capabilities",
    ),
    ("state.redis.schema.input.title", "State input"),
    (
        "state.redis.schema.input.description",
        "Input for Redis state provider",
    ),
    ("state.redis.schema.output.title", "State output"),
    (
        "state.redis.schema.output.description",
        "Result of Redis state provider",
    ),
    ("state.redis.schema.output.ok.title", "Success"),
    (
        "state.redis.schema.output.ok.description",
        "Whether the operation succeeded",
    ),
    ("state.redis.schema.config.title", "Redis state config"),
    (
        "state.redis.schema.config.description",
        "Redis state provider configuration",
    ),
    (
        "state.redis.schema.config.redis_url.title",
        "Redis Connection URL",
    ),
    (
        "state.redis.schema.config.redis_url.description",
        "Full Redis URL including host, port, and database (e.g. redis://localhost:6379/0)",
    ),
    (
        "state.redis.schema.config.redis_password.title",
        "Redis Password",
    ),
    (
        "state.redis.schema.config.redis_password.description",
        "Password for Redis authentication (optional)",
    ),
    (
        "state.redis.schema.config.redis_tls_enabled.title",
        "Enable TLS",
    ),
    (
        "state.redis.schema.config.redis_tls_enabled.description",
        "Enable TLS encryption for Redis connection",
    ),
    ("state.redis.schema.config.key_prefix.title", "Key Prefix"),
    (
        "state.redis.schema.config.key_prefix.description",
        "Prefix for all Redis keys to avoid collisions (default: greentic)",
    ),
    (
        "state.redis.schema.config.default_ttl_seconds.title",
        "Default TTL (seconds)",
    ),
    (
        "state.redis.schema.config.default_ttl_seconds.description",
        "Default time-to-live for entries in seconds (0 = no expiry)",
    ),
    (
        "state.redis.schema.config.connection_pool_size.title",
        "Connection Pool Size",
    ),
    (
        "state.redis.schema.config.connection_pool_size.description",
        "Number of Redis connections in the pool (default: 5)",
    ),
    ("state.redis.qa.default.title", "Default"),
    ("state.redis.qa.setup.title", "Setup"),
    ("state.redis.qa.upgrade.title", "Upgrade"),
    ("state.redis.qa.remove.title", "Remove"),
    ("state.redis.qa.setup.redis_url", "Redis connection URL"),
    (
        "state.redis.qa.setup.redis_password",
        "Redis password (optional)",
    ),
    ("state.redis.qa.setup.redis_tls_enabled", "Enable TLS"),
    (
        "state.redis.qa.setup.key_prefix",
        "Key prefix (default: greentic)",
    ),
    (
        "state.redis.qa.setup.default_ttl_seconds",
        "Default TTL in seconds (0 = no expiry)",
    ),
    (
        "state.redis.qa.setup.connection_pool_size",
        "Connection pool size (default: 5)",
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
        canonical_cbor_bytes(&RunResult {
            ok: false,
            error: Some(format!(
                "state-provider-redis: runtime invoke not supported for op '{op}'; \
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
            "state.redis",
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
    redis_url: String,
    #[serde(default)]
    redis_password: String,
    #[serde(default)]
    redis_tls_enabled: bool,
    #[serde(default = "default_key_prefix")]
    key_prefix: String,
    #[serde(default)]
    default_ttl_seconds: u32,
    #[serde(default = "default_pool_size")]
    connection_pool_size: u32,
}

fn default_key_prefix() -> String {
    "greentic".to_string()
}

const fn default_pool_size() -> u32 {
    5
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
            title: i18n("state.redis.op.describe.title"),
            description: i18n("state.redis.op.describe.description"),
        }],
        input_schema,
        output_schema,
        config_schema,
        redactions: vec![
            RedactionRule {
                path: "$.redis_password".to_string(),
                strategy: "replace".to_string(),
            },
            RedactionRule {
                path: "$.redis_url".to_string(),
                strategy: "replace".to_string(),
            },
        ],
        schema_hash: hash,
    }
}

const SETUP_QUESTIONS: &[provider_common::helpers::QaQuestionDef] = &[
    ("redis_url", "state.redis.qa.setup.redis_url", true),
    (
        "redis_password",
        "state.redis.qa.setup.redis_password",
        false,
    ),
    (
        "redis_tls_enabled",
        "state.redis.qa.setup.redis_tls_enabled",
        false,
    ),
    ("key_prefix", "state.redis.qa.setup.key_prefix", false),
    (
        "default_ttl_seconds",
        "state.redis.qa.setup.default_ttl_seconds",
        false,
    ),
    (
        "connection_pool_size",
        "state.redis.qa.setup.connection_pool_size",
        false,
    ),
];
const DEFAULT_KEYS: &[&str] = &["redis_url"];

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
        "state.redis",
        SETUP_QUESTIONS,
        DEFAULT_KEYS,
    )
}

fn input_schema() -> SchemaIr {
    provider_common::helpers::schema_obj(
        "state.redis.schema.input.title",
        "state.redis.schema.input.description",
        vec![],
        false,
    )
}

fn output_schema() -> SchemaIr {
    provider_common::helpers::schema_obj(
        "state.redis.schema.output.title",
        "state.redis.schema.output.description",
        vec![(
            "ok",
            true,
            provider_common::helpers::schema_bool_ir(
                "state.redis.schema.output.ok.title",
                "state.redis.schema.output.ok.description",
            ),
        )],
        false,
    )
}

fn config_schema() -> SchemaIr {
    provider_common::helpers::schema_obj(
        "state.redis.schema.config.title",
        "state.redis.schema.config.description",
        vec![
            (
                "redis_url",
                true,
                provider_common::helpers::schema_secret(
                    "state.redis.schema.config.redis_url.title",
                    "state.redis.schema.config.redis_url.description",
                ),
            ),
            (
                "redis_password",
                false,
                provider_common::helpers::schema_secret(
                    "state.redis.schema.config.redis_password.title",
                    "state.redis.schema.config.redis_password.description",
                ),
            ),
            (
                "redis_tls_enabled",
                false,
                provider_common::helpers::schema_bool_ir(
                    "state.redis.schema.config.redis_tls_enabled.title",
                    "state.redis.schema.config.redis_tls_enabled.description",
                ),
            ),
            (
                "key_prefix",
                false,
                provider_common::helpers::schema_str(
                    "state.redis.schema.config.key_prefix.title",
                    "state.redis.schema.config.key_prefix.description",
                ),
            ),
            (
                "default_ttl_seconds",
                false,
                provider_common::helpers::schema_str(
                    "state.redis.schema.config.default_ttl_seconds.title",
                    "state.redis.schema.config.default_ttl_seconds.description",
                ),
            ),
            (
                "connection_pool_size",
                false,
                provider_common::helpers::schema_str(
                    "state.redis.schema.config.connection_pool_size.title",
                    "state.redis.schema.config.connection_pool_size.description",
                ),
            ),
        ],
        false,
    )
}

fn default_config_out() -> ProviderConfig {
    ProviderConfig {
        redis_url: String::new(),
        redis_password: String::new(),
        redis_tls_enabled: false,
        key_prefix: "greentic".to_string(),
        default_ttl_seconds: 0,
        connection_pool_size: 5,
    }
}

fn validate_config_out(config: &ProviderConfig) -> Result<(), String> {
    if config.redis_url.trim().is_empty() {
        return Err("config validation failed: redis_url is required".to_string());
    }
    if !(config.redis_url.starts_with("redis://") || config.redis_url.starts_with("rediss://")) {
        return Err(
            "config validation failed: redis_url must start with redis:// or rediss://".to_string(),
        );
    }
    Ok(())
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
            "delete_provenance_key".to_string(),
            "delete_provider_state_namespace".to_string(),
            "best_effort_revoke_tokens".to_string(),
        ]));
    }

    let mut merged = existing_config_from_answers(&answers).unwrap_or_else(default_config_out);
    let answer_obj = answers.as_object();
    let has = |key: &str| answer_obj.is_some_and(|obj| obj.contains_key(key));

    if mode == "setup" || mode == "default" {
        merged.redis_url = string_or_default(&answers, "redis_url", &merged.redis_url);
        merged.redis_password =
            string_or_default(&answers, "redis_password", &merged.redis_password);
        merged.redis_tls_enabled = answers
            .get("redis_tls_enabled")
            .and_then(|v| v.as_bool().or_else(|| v.as_str().map(|s| s == "true")))
            .unwrap_or(merged.redis_tls_enabled);
        merged.key_prefix = string_or_default(&answers, "key_prefix", &merged.key_prefix);
        if let Some(v) = answers
            .get("default_ttl_seconds")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u32>().ok())
        {
            merged.default_ttl_seconds = v;
        }
        if let Some(v) = answers
            .get("connection_pool_size")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u32>().ok())
        {
            merged.connection_pool_size = v;
        }
    }

    if mode == "upgrade" {
        if has("redis_url") {
            merged.redis_url = string_or_default(&answers, "redis_url", &merged.redis_url);
        }
        if has("redis_password") {
            merged.redis_password =
                string_or_default(&answers, "redis_password", &merged.redis_password);
        }
        if has("redis_tls_enabled") {
            merged.redis_tls_enabled = answers
                .get("redis_tls_enabled")
                .and_then(|v| v.as_bool().or_else(|| v.as_str().map(|s| s == "true")))
                .unwrap_or(merged.redis_tls_enabled);
        }
        if has("key_prefix") {
            merged.key_prefix = string_or_default(&answers, "key_prefix", &merged.key_prefix);
        }
        if has("default_ttl_seconds")
            && let Some(v) = answers
                .get("default_ttl_seconds")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u32>().ok())
        {
            merged.default_ttl_seconds = v;
        }
        if has("connection_pool_size")
            && let Some(v) = answers
                .get("connection_pool_size")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u32>().ok())
        {
            merged.connection_pool_size = v;
        }
    }

    if let Err(error) = validate_config_out(&merged) {
        return canonical_cbor_bytes(&ApplyAnswersResult::<ProviderConfig>::validation_error(
            error,
        ));
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
        assert_eq!(payload.redactions.len(), 2);
        let bytes = canonical_cbor_bytes(&payload);
        assert!(!bytes.is_empty());
    }

    #[test]
    fn qa_spec_default_returns_redis_url_only() {
        let spec = build_qa_spec(Mode::Default);
        assert_eq!(spec.mode, "default");
        let question_ids: Vec<&str> = spec.questions.iter().map(|q| q.id.as_str()).collect();
        assert!(question_ids.contains(&"redis_url"));
    }

    #[test]
    fn qa_spec_setup_has_all_fields() {
        let spec = build_qa_spec(Mode::Setup);
        assert_eq!(spec.mode, "setup");
        assert!(spec.questions.len() >= 4);
    }

    #[test]
    fn apply_answers_setup_validates_url() {
        let answers = serde_json::json!({
            "redis_url": "not-a-redis-url"
        });
        let out =
            <Component as QaGuest>::apply_answers(Mode::Setup, canonical_cbor_bytes(&answers));
        let out_json: serde_json::Value = decode_cbor(&out).expect("decode");
        assert_eq!(out_json.get("ok"), Some(&serde_json::Value::Bool(false)));
        let err = out_json.get("error").and_then(|v| v.as_str()).unwrap_or("");
        assert!(err.contains("redis_url"));
    }

    #[test]
    fn apply_answers_setup_accepts_valid_url() {
        let answers = serde_json::json!({
            "redis_url": "redis://localhost:6379/0",
            "key_prefix": "myapp",
            "connection_pool_size": "10"
        });
        let out =
            <Component as QaGuest>::apply_answers(Mode::Setup, canonical_cbor_bytes(&answers));
        let out_json: serde_json::Value = decode_cbor(&out).expect("decode");
        assert_eq!(out_json.get("ok"), Some(&serde_json::Value::Bool(true)));
        let config = out_json.get("config").expect("config");
        assert_eq!(
            config.get("redis_url"),
            Some(&serde_json::json!("redis://localhost:6379/0"))
        );
        assert_eq!(config.get("key_prefix"), Some(&serde_json::json!("myapp")));
        assert_eq!(
            config.get("connection_pool_size"),
            Some(&serde_json::json!(10))
        );
    }

    #[test]
    fn apply_answers_remove_returns_cleanup() {
        let answers = serde_json::json!({});
        let out =
            <Component as QaGuest>::apply_answers(Mode::Remove, canonical_cbor_bytes(&answers));
        let out_json: serde_json::Value = decode_cbor(&out).expect("decode");
        assert!(out_json.get("remove").is_some());
    }

    #[test]
    fn apply_answers_upgrade_preserves_unspecified() {
        let answers = serde_json::json!({
            "existing_config": {
                "redis_url": "redis://original:6379/0",
                "redis_password": "secret",
                "redis_tls_enabled": false,
                "key_prefix": "greentic",
                "default_ttl_seconds": 0,
                "connection_pool_size": 5
            },
            "key_prefix": "updated"
        });
        let out =
            <Component as QaGuest>::apply_answers(Mode::Upgrade, canonical_cbor_bytes(&answers));
        let out_json: serde_json::Value = decode_cbor(&out).expect("decode");
        assert_eq!(out_json.get("ok"), Some(&serde_json::Value::Bool(true)));
        let config = out_json.get("config").expect("config");
        assert_eq!(
            config.get("redis_url"),
            Some(&serde_json::json!("redis://original:6379/0"))
        );
        assert_eq!(
            config.get("key_prefix"),
            Some(&serde_json::json!("updated"))
        );
    }

    #[test]
    fn i18n_keys_nonempty() {
        use bindings::exports::greentic::component::component_i18n::Guest as I18nGuest;
        let keys = <Component as I18nGuest>::i18n_keys();
        assert!(!keys.is_empty());
        assert!(keys.contains(&"state.redis.qa.setup.redis_url".to_string()));
    }
}
