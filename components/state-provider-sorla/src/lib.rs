use provider_common::component_v0_6::{
    DescribePayload, OperationDescriptor, RedactionRule, SchemaIr, canonical_cbor_bytes,
    decode_cbor, schema_hash,
};
use provider_common::helpers::{existing_config_from_answers, i18n, string_or_default};
use provider_common::qa_helpers::ApplyAnswersResult;
use serde::{Deserialize, Serialize};

mod bindings {
    wit_bindgen::generate!({
        path: "wit/state-provider-sorla",
        world: "component-v0-v6-v0",
        generate_all
    });
}

const PROVIDER_ID: &str = "state-provider-sorla";
const WORLD_ID: &str = "component-v0-v6-v0";

const I18N_KEYS: &[&str] = &[
    "state.sorla.op.describe.title",
    "state.sorla.op.describe.description",
    "state.sorla.schema.input.title",
    "state.sorla.schema.input.description",
    "state.sorla.schema.output.title",
    "state.sorla.schema.output.description",
    "state.sorla.schema.output.ok.title",
    "state.sorla.schema.output.ok.description",
    "state.sorla.schema.config.title",
    "state.sorla.schema.config.description",
    "state.sorla.schema.config.endpoint.title",
    "state.sorla.schema.config.endpoint.description",
    "state.sorla.schema.config.token_ref.title",
    "state.sorla.schema.config.token_ref.description",
    "state.sorla.schema.config.key_prefix.title",
    "state.sorla.schema.config.key_prefix.description",
    "state.sorla.schema.config.default_ttl_seconds.title",
    "state.sorla.schema.config.default_ttl_seconds.description",
    "state.sorla.schema.config.request_timeout_ms.title",
    "state.sorla.schema.config.request_timeout_ms.description",
    "state.sorla.schema.config.cache_max_entries.title",
    "state.sorla.schema.config.cache_max_entries.description",
    "state.sorla.qa.default.title",
    "state.sorla.qa.setup.title",
    "state.sorla.qa.upgrade.title",
    "state.sorla.qa.remove.title",
    "state.sorla.qa.setup.endpoint",
    "state.sorla.qa.setup.token_ref",
    "state.sorla.qa.setup.key_prefix",
    "state.sorla.qa.setup.default_ttl_seconds",
    "state.sorla.qa.setup.request_timeout_ms",
    "state.sorla.qa.setup.cache_max_entries",
    // Flow-related i18n keys
    "state.sorla.flow.default.title",
    "state.sorla.flow.default.config_summary",
    "state.sorla.flow.update.title",
    "state.sorla.flow.update.collect",
    "state.sorla.flow.update.complete",
    "state.sorla.flow.remove.title",
    "state.sorla.flow.remove.check_state",
    "state.sorla.flow.remove.complete",
];

const I18N_PAIRS: &[(&str, &str)] = &[
    ("state.sorla.op.describe.title", "Describe"),
    (
        "state.sorla.op.describe.description",
        "Describe SoRLa state provider capabilities",
    ),
    ("state.sorla.schema.input.title", "State input"),
    (
        "state.sorla.schema.input.description",
        "Input for SoRLa state provider",
    ),
    ("state.sorla.schema.output.title", "State output"),
    (
        "state.sorla.schema.output.description",
        "Result of SoRLa state provider",
    ),
    ("state.sorla.schema.output.ok.title", "Success"),
    (
        "state.sorla.schema.output.ok.description",
        "Whether the operation succeeded",
    ),
    ("state.sorla.schema.config.title", "SoRLa state config"),
    (
        "state.sorla.schema.config.description",
        "SoRLa state provider configuration",
    ),
    (
        "state.sorla.schema.config.endpoint.title",
        "State Door Endpoint",
    ),
    (
        "state.sorla.schema.config.endpoint.description",
        "HTTPS base URL of the state door; read, write and delete are POSTed beneath it (e.g. https://admin.example/api/v1/ingest/state)",
    ),
    (
        "state.sorla.schema.config.token_ref.title",
        "State Token Reference",
    ),
    (
        "state.sorla.schema.config.token_ref.description",
        "Name of the secret that holds the per-unit state token. Never the token itself",
    ),
    ("state.sorla.schema.config.key_prefix.title", "Key Prefix"),
    (
        "state.sorla.schema.config.key_prefix.description",
        "Prefix for all keys to avoid collisions (default: greentic)",
    ),
    (
        "state.sorla.schema.config.default_ttl_seconds.title",
        "Default TTL (seconds)",
    ),
    (
        "state.sorla.schema.config.default_ttl_seconds.description",
        "Default time-to-live for entries in seconds (empty = server retention policy)",
    ),
    (
        "state.sorla.schema.config.request_timeout_ms.title",
        "Request Timeout (ms)",
    ),
    (
        "state.sorla.schema.config.request_timeout_ms.description",
        "Timeout for one call to the state door in milliseconds (default: 5000). A timed-out write fails the turn",
    ),
    (
        "state.sorla.schema.config.cache_max_entries.title",
        "Read Cache Size",
    ),
    (
        "state.sorla.schema.config.cache_max_entries.description",
        "Maximum number of entries in the host read cache (default: 10000)",
    ),
    ("state.sorla.qa.default.title", "Default"),
    ("state.sorla.qa.setup.title", "Setup"),
    ("state.sorla.qa.upgrade.title", "Upgrade"),
    ("state.sorla.qa.remove.title", "Remove"),
    (
        "state.sorla.qa.setup.endpoint",
        "State door endpoint (https)",
    ),
    (
        "state.sorla.qa.setup.token_ref",
        "Secret name of the state token",
    ),
    (
        "state.sorla.qa.setup.key_prefix",
        "Key prefix (default: greentic)",
    ),
    (
        "state.sorla.qa.setup.default_ttl_seconds",
        "Default TTL in seconds (optional)",
    ),
    (
        "state.sorla.qa.setup.request_timeout_ms",
        "Request timeout in milliseconds (default: 5000)",
    ),
    (
        "state.sorla.qa.setup.cache_max_entries",
        "Read cache size in entries (default: 10000)",
    ),
    ("state.sorla.flow.default.title", "Default setup"),
    (
        "state.sorla.flow.default.config_summary",
        "Configuration summary",
    ),
    ("state.sorla.flow.update.title", "Update configuration"),
    (
        "state.sorla.flow.update.collect",
        "Collect updated settings",
    ),
    ("state.sorla.flow.update.complete", "Update complete"),
    ("state.sorla.flow.remove.title", "Remove provider"),
    ("state.sorla.flow.remove.check_state", "Check state"),
    ("state.sorla.flow.remove.complete", "Remove complete"),
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
                "state-provider-sorla: runtime invoke not supported for op '{op}'; \
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
            "state.sorla",
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

const DEFAULT_KEY_PREFIX: &str = "greentic";
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_CACHE_MAX_ENTRIES: u64 = 10_000;

const MIN_REQUEST_TIMEOUT_MS: u64 = 100;
const MAX_REQUEST_TIMEOUT_MS: u64 = 60_000;
const MAX_CACHE_MAX_ENTRIES: u64 = 10_000_000;
/// Ten years. Anything longer is almost certainly a unit mistake.
const MAX_TTL_SECONDS: u64 = 315_360_000;
const MAX_ENDPOINT_LEN: usize = 2048;
const MAX_TOKEN_REF_LEN: usize = 256;
const MAX_KEY_PREFIX_LEN: usize = 128;
/// Prefix of a per-unit state token. A `token_ref` starting with it is a
/// pasted token, not a secret name.
const TOKEN_VALUE_PREFIX: &str = "gtm_";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderConfig {
    endpoint: String,
    /// Name of the secret holding the bearer token. Never the token value.
    #[serde(default)]
    token_ref: String,
    #[serde(default = "default_key_prefix")]
    key_prefix: String,
    #[serde(default)]
    default_ttl_seconds: Option<u64>,
    #[serde(default = "default_request_timeout_ms")]
    request_timeout_ms: u64,
    #[serde(default = "default_cache_max_entries")]
    cache_max_entries: u64,
}

fn default_key_prefix() -> String {
    DEFAULT_KEY_PREFIX.to_string()
}

const fn default_request_timeout_ms() -> u64 {
    DEFAULT_REQUEST_TIMEOUT_MS
}

const fn default_cache_max_entries() -> u64 {
    DEFAULT_CACHE_MAX_ENTRIES
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
            title: i18n("state.sorla.op.describe.title"),
            description: i18n("state.sorla.op.describe.description"),
        }],
        input_schema,
        output_schema,
        config_schema,
        redactions: vec![
            RedactionRule {
                path: "$.token_ref".to_string(),
                strategy: "replace".to_string(),
            },
            RedactionRule {
                path: "$.endpoint".to_string(),
                strategy: "replace".to_string(),
            },
        ],
        schema_hash: hash,
    }
}

const SETUP_QUESTIONS: &[provider_common::helpers::QaQuestionDef] = &[
    ("endpoint", "state.sorla.qa.setup.endpoint", true),
    ("token_ref", "state.sorla.qa.setup.token_ref", true),
    ("key_prefix", "state.sorla.qa.setup.key_prefix", false),
    (
        "default_ttl_seconds",
        "state.sorla.qa.setup.default_ttl_seconds",
        false,
    ),
    (
        "request_timeout_ms",
        "state.sorla.qa.setup.request_timeout_ms",
        false,
    ),
    (
        "cache_max_entries",
        "state.sorla.qa.setup.cache_max_entries",
        false,
    ),
];
const DEFAULT_KEYS: &[&str] = &["endpoint", "token_ref"];

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
        "state.sorla",
        SETUP_QUESTIONS,
        DEFAULT_KEYS,
    )
}

fn input_schema() -> SchemaIr {
    provider_common::helpers::schema_obj(
        "state.sorla.schema.input.title",
        "state.sorla.schema.input.description",
        vec![],
        false,
    )
}

fn output_schema() -> SchemaIr {
    provider_common::helpers::schema_obj(
        "state.sorla.schema.output.title",
        "state.sorla.schema.output.description",
        vec![(
            "ok",
            true,
            provider_common::helpers::schema_bool_ir(
                "state.sorla.schema.output.ok.title",
                "state.sorla.schema.output.ok.description",
            ),
        )],
        false,
    )
}

fn config_schema() -> SchemaIr {
    use provider_common::helpers::{schema_secret, schema_str};
    provider_common::helpers::schema_obj(
        "state.sorla.schema.config.title",
        "state.sorla.schema.config.description",
        vec![
            (
                "endpoint",
                true,
                schema_str(
                    "state.sorla.schema.config.endpoint.title",
                    "state.sorla.schema.config.endpoint.description",
                ),
            ),
            (
                "token_ref",
                true,
                schema_secret(
                    "state.sorla.schema.config.token_ref.title",
                    "state.sorla.schema.config.token_ref.description",
                ),
            ),
            (
                "key_prefix",
                false,
                schema_str(
                    "state.sorla.schema.config.key_prefix.title",
                    "state.sorla.schema.config.key_prefix.description",
                ),
            ),
            (
                "default_ttl_seconds",
                false,
                schema_str(
                    "state.sorla.schema.config.default_ttl_seconds.title",
                    "state.sorla.schema.config.default_ttl_seconds.description",
                ),
            ),
            (
                "request_timeout_ms",
                false,
                schema_str(
                    "state.sorla.schema.config.request_timeout_ms.title",
                    "state.sorla.schema.config.request_timeout_ms.description",
                ),
            ),
            (
                "cache_max_entries",
                false,
                schema_str(
                    "state.sorla.schema.config.cache_max_entries.title",
                    "state.sorla.schema.config.cache_max_entries.description",
                ),
            ),
        ],
        false,
    )
}

fn default_config_out() -> ProviderConfig {
    ProviderConfig {
        endpoint: String::new(),
        token_ref: String::new(),
        key_prefix: default_key_prefix(),
        default_ttl_seconds: None,
        request_timeout_ms: DEFAULT_REQUEST_TIMEOUT_MS,
        cache_max_entries: DEFAULT_CACHE_MAX_ENTRIES,
    }
}

/// Host part of an `http(s)://authority[/path]` URL, or an error naming what
/// is wrong. Rejects userinfo, query and fragment: the door base is a plain
/// address and credentials never belong in it.
fn host_of(url: &str, scheme_len: usize) -> Result<&str, String> {
    let rest = &url[scheme_len..];
    if rest.contains(['?', '#']) {
        return Err("endpoint must not contain a query or fragment".to_string());
    }
    let authority = rest.split('/').next().unwrap_or("");
    if authority.contains('@') {
        return Err("endpoint must not contain credentials".to_string());
    }
    let host = if let Some(stripped) = authority.strip_prefix('[') {
        // IPv6 literal: [::1] or [::1]:port
        let end = stripped
            .find(']')
            .ok_or_else(|| "endpoint has an unterminated IPv6 host".to_string())?;
        &authority[..end + 2]
    } else {
        authority.split(':').next().unwrap_or("")
    };
    if host.is_empty() {
        return Err("endpoint must name a host".to_string());
    }
    Ok(host)
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

fn validate_endpoint(endpoint: &str) -> Result<(), String> {
    if endpoint.trim().is_empty() {
        return Err("config validation failed: endpoint is required".to_string());
    }
    if endpoint.len() > MAX_ENDPOINT_LEN {
        return Err(format!(
            "config validation failed: endpoint is longer than {MAX_ENDPOINT_LEN} characters"
        ));
    }
    if endpoint
        .chars()
        .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err("config validation failed: endpoint must not contain whitespace".to_string());
    }
    let lowered = endpoint.to_ascii_lowercase();
    if lowered.starts_with("https://") {
        host_of(endpoint, "https://".len())
            .map(|_| ())
            .map_err(|e| format!("config validation failed: {e}"))
    } else if lowered.starts_with("http://") {
        let host = host_of(endpoint, "http://".len())
            .map_err(|e| format!("config validation failed: {e}"))?;
        if is_loopback_host(&host.to_ascii_lowercase()) {
            Ok(())
        } else {
            Err(
                "config validation failed: endpoint must use https (http is allowed only for localhost)"
                    .to_string(),
            )
        }
    } else {
        Err("config validation failed: endpoint must start with https://".to_string())
    }
}

fn validate_token_ref(token_ref: &str) -> Result<(), String> {
    if token_ref.trim().is_empty() {
        return Err("config validation failed: token_ref is required".to_string());
    }
    if token_ref.len() > MAX_TOKEN_REF_LEN {
        return Err(format!(
            "config validation failed: token_ref is longer than {MAX_TOKEN_REF_LEN} characters"
        ));
    }
    if token_ref
        .chars()
        .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err("config validation failed: token_ref must not contain whitespace".to_string());
    }
    if token_ref.starts_with(TOKEN_VALUE_PREFIX) {
        return Err(
            "config validation failed: token_ref looks like a token value; \
             give the NAME of the secret that holds it"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_key_prefix(prefix: &str) -> Result<(), String> {
    if prefix.trim().is_empty() {
        return Err("config validation failed: key_prefix must not be empty".to_string());
    }
    if prefix.len() > MAX_KEY_PREFIX_LEN {
        return Err(format!(
            "config validation failed: key_prefix is longer than {MAX_KEY_PREFIX_LEN} characters"
        ));
    }
    if prefix.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("config validation failed: key_prefix must not contain whitespace".to_string());
    }
    Ok(())
}

fn validate_config_out(config: &ProviderConfig) -> Result<(), String> {
    validate_endpoint(&config.endpoint)?;
    validate_token_ref(&config.token_ref)?;
    validate_key_prefix(&config.key_prefix)?;
    if let Some(ttl) = config.default_ttl_seconds
        && ttl > MAX_TTL_SECONDS
    {
        return Err(format!(
            "config validation failed: default_ttl_seconds must be at most {MAX_TTL_SECONDS}"
        ));
    }
    if !(MIN_REQUEST_TIMEOUT_MS..=MAX_REQUEST_TIMEOUT_MS).contains(&config.request_timeout_ms) {
        return Err(format!(
            "config validation failed: request_timeout_ms must be between \
             {MIN_REQUEST_TIMEOUT_MS} and {MAX_REQUEST_TIMEOUT_MS}"
        ));
    }
    if !(1..=MAX_CACHE_MAX_ENTRIES).contains(&config.cache_max_entries) {
        return Err(format!(
            "config validation failed: cache_max_entries must be between 1 and {MAX_CACHE_MAX_ENTRIES}"
        ));
    }
    Ok(())
}

/// Read a numeric answer given as a JSON number or a numeric string. `Ok(None)`
/// when the key is absent or an empty string; an unparsable value is an error
/// rather than being ignored, so a typo cannot silently keep the old value.
fn number_answer(answers: &serde_json::Value, key: &str) -> Result<Option<u64>, String> {
    match answers.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => n
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("config validation failed: {key} must be a whole number")),
        Some(serde_json::Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            trimmed
                .parse::<u64>()
                .map(Some)
                .map_err(|_| format!("config validation failed: {key} must be a whole number"))
        }
        Some(_) => Err(format!(
            "config validation failed: {key} must be a whole number"
        )),
    }
}

/// Fold the answers that are present into `merged`. An absent key keeps the
/// existing value, which is what makes `upgrade` partial.
fn merge_answers(merged: &mut ProviderConfig, answers: &serde_json::Value) -> Result<(), String> {
    merged.endpoint = string_or_default(answers, "endpoint", &merged.endpoint);
    merged.token_ref = string_or_default(answers, "token_ref", &merged.token_ref);
    merged.key_prefix = string_or_default(answers, "key_prefix", &merged.key_prefix);
    if let Some(v) = number_answer(answers, "default_ttl_seconds")? {
        merged.default_ttl_seconds = Some(v);
    }
    if let Some(v) = number_answer(answers, "request_timeout_ms")? {
        merged.request_timeout_ms = v;
    }
    if let Some(v) = number_answer(answers, "cache_max_entries")? {
        merged.cache_max_entries = v;
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
    let outcome = merge_answers(&mut merged, &answers).and_then(|()| validate_config_out(&merged));
    if let Err(error) = outcome {
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

    fn apply(mode: Mode, answers: serde_json::Value) -> serde_json::Value {
        let out = <Component as QaGuest>::apply_answers(mode, canonical_cbor_bytes(&answers));
        decode_cbor(&out).expect("decode")
    }

    fn error_of(out: &serde_json::Value) -> String {
        assert_eq!(out.get("ok"), Some(&serde_json::Value::Bool(false)));
        out.get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    fn valid() -> serde_json::Value {
        serde_json::json!({
            "endpoint": "https://admin.example/api/v1/ingest/state",
            "token_ref": "state/door_token",
        })
    }

    #[test]
    fn describe_payload_serializes() {
        let payload = build_describe_payload();
        assert_eq!(payload.provider, PROVIDER_ID);
        assert_eq!(payload.world, WORLD_ID);
        assert_eq!(payload.operations.len(), 1);
        assert_eq!(payload.operations[0].name, "state.dispatch");
        assert_eq!(payload.redactions.len(), 2);
        let paths: Vec<&str> = payload.redactions.iter().map(|r| r.path.as_str()).collect();
        assert!(paths.contains(&"$.token_ref"));
        assert!(!canonical_cbor_bytes(&payload).is_empty());
    }

    #[test]
    fn i18n_keys_and_pairs_agree() {
        let pair_keys: Vec<&str> = I18N_PAIRS.iter().map(|(k, _)| *k).collect();
        assert_eq!(pair_keys, I18N_KEYS);
        assert!(I18N_KEYS.iter().all(|k| k.starts_with("state.sorla.")));
    }

    #[test]
    fn i18n_keys_nonempty() {
        use bindings::exports::greentic::component::component_i18n::Guest as I18nGuest;
        let keys = <Component as I18nGuest>::i18n_keys();
        assert!(keys.contains(&"state.sorla.qa.setup.endpoint".to_string()));
    }

    #[test]
    fn qa_spec_default_asks_endpoint_and_token_ref() {
        let spec = build_qa_spec(Mode::Default);
        assert_eq!(spec.mode, "default");
        let ids: Vec<&str> = spec.questions.iter().map(|q| q.id.as_str()).collect();
        assert!(ids.contains(&"endpoint"));
        assert!(ids.contains(&"token_ref"));
    }

    #[test]
    fn qa_spec_setup_has_all_fields() {
        let spec = build_qa_spec(Mode::Setup);
        assert_eq!(spec.mode, "setup");
        assert_eq!(spec.questions.len(), 6);
    }

    #[test]
    fn setup_accepts_valid_answers_and_applies_defaults() {
        let out = apply(Mode::Setup, valid());
        assert_eq!(out.get("ok"), Some(&serde_json::Value::Bool(true)));
        let config = out.get("config").expect("config");
        assert_eq!(
            config.get("key_prefix"),
            Some(&serde_json::json!("greentic"))
        );
        assert_eq!(
            config.get("request_timeout_ms"),
            Some(&serde_json::json!(5000))
        );
        assert_eq!(
            config.get("cache_max_entries"),
            Some(&serde_json::json!(10000))
        );
        assert!(
            config
                .get("default_ttl_seconds")
                .is_none_or(|v| v.is_null())
        );
    }

    #[test]
    fn setup_accepts_numbers_as_strings_or_numbers() {
        let mut answers = valid();
        answers["default_ttl_seconds"] = serde_json::json!("3600");
        answers["request_timeout_ms"] = serde_json::json!(2500);
        let out = apply(Mode::Setup, answers);
        let config = out.get("config").expect("config");
        assert_eq!(
            config.get("default_ttl_seconds"),
            Some(&serde_json::json!(3600))
        );
        assert_eq!(
            config.get("request_timeout_ms"),
            Some(&serde_json::json!(2500))
        );
    }

    #[test]
    fn endpoint_is_required() {
        let out = apply(
            Mode::Setup,
            serde_json::json!({ "token_ref": "state/door_token" }),
        );
        assert!(error_of(&out).contains("endpoint is required"));
    }

    #[test]
    fn endpoint_must_be_https() {
        let mut answers = valid();
        answers["endpoint"] = serde_json::json!("http://admin.example/state");
        assert!(error_of(&apply(Mode::Setup, answers)).contains("https"));
        let mut answers = valid();
        answers["endpoint"] = serde_json::json!("ftp://admin.example/state");
        assert!(error_of(&apply(Mode::Setup, answers)).contains("https://"));
    }

    #[test]
    fn http_is_allowed_for_loopback_only() {
        for ok in [
            "http://localhost:8080/state",
            "http://127.0.0.1:8080/state",
            "http://[::1]:8080/state",
        ] {
            assert!(validate_endpoint(ok).is_ok(), "{ok}");
        }
        // A lookalike host must not pass as loopback.
        for bad in [
            "http://localhost.evil.example/state",
            "http://127.0.0.1.evil.example/state",
            "http://evil.example/state",
        ] {
            assert!(validate_endpoint(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn endpoint_rejects_credentials_query_and_whitespace() {
        for bad in [
            "https://user:pw@admin.example/state",
            "https://admin.example/state?token=x",
            "https://admin.example/state#frag",
            "https://admin.example/sta te",
            "https:///state",
            "https://",
        ] {
            assert!(validate_endpoint(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn token_ref_is_required_and_never_a_token_value() {
        let out = apply(
            Mode::Setup,
            serde_json::json!({ "endpoint": "https://admin.example/state" }),
        );
        assert!(error_of(&out).contains("token_ref is required"));

        let mut answers = valid();
        answers["token_ref"] = serde_json::json!("gtm_abcdef0123456789");
        assert!(error_of(&apply(Mode::Setup, answers)).contains("token value"));

        let mut answers = valid();
        answers["token_ref"] = serde_json::json!("two words");
        assert!(error_of(&apply(Mode::Setup, answers)).contains("whitespace"));
    }

    #[test]
    fn numbers_are_bounded() {
        for (key, value) in [
            ("request_timeout_ms", serde_json::json!(50)),
            ("request_timeout_ms", serde_json::json!(60_001)),
            ("cache_max_entries", serde_json::json!(0)),
            ("cache_max_entries", serde_json::json!(10_000_001)),
            (
                "default_ttl_seconds",
                serde_json::json!(MAX_TTL_SECONDS + 1),
            ),
        ] {
            let mut answers = valid();
            answers[key] = value.clone();
            let out = apply(Mode::Setup, answers);
            assert!(error_of(&out).contains(key), "{key}={value}");
        }
    }

    #[test]
    fn unparsable_numbers_are_rejected_not_ignored() {
        for value in [
            serde_json::json!("abc"),
            serde_json::json!(-5),
            serde_json::json!(1.5),
        ] {
            let mut answers = valid();
            answers["request_timeout_ms"] = value.clone();
            let out = apply(Mode::Setup, answers);
            assert!(error_of(&out).contains("whole number"), "{value}");
        }
    }

    #[test]
    fn key_prefix_is_validated() {
        let mut answers = valid();
        answers["key_prefix"] = serde_json::json!("has space");
        assert!(error_of(&apply(Mode::Setup, answers)).contains("key_prefix"));
    }

    #[test]
    fn remove_returns_cleanup() {
        let out = apply(Mode::Remove, serde_json::json!({}));
        assert!(out.get("remove").is_some());
    }

    #[test]
    fn upgrade_preserves_unspecified_and_revalidates() {
        let answers = serde_json::json!({
            "existing_config": {
                "endpoint": "https://admin.example/api/v1/ingest/state",
                "token_ref": "state/door_token",
                "key_prefix": "greentic",
                "request_timeout_ms": 5000,
                "cache_max_entries": 10000
            },
            "key_prefix": "updated"
        });
        let out = apply(Mode::Upgrade, answers);
        assert_eq!(out.get("ok"), Some(&serde_json::Value::Bool(true)));
        let config = out.get("config").expect("config");
        assert_eq!(
            config.get("endpoint"),
            Some(&serde_json::json!(
                "https://admin.example/api/v1/ingest/state"
            ))
        );
        assert_eq!(
            config.get("key_prefix"),
            Some(&serde_json::json!("updated"))
        );

        // An upgrade may not downgrade the endpoint to plain http.
        let answers = serde_json::json!({
            "existing_config": {
                "endpoint": "https://admin.example/state",
                "token_ref": "state/door_token"
            },
            "endpoint": "http://admin.example/state"
        });
        assert!(error_of(&apply(Mode::Upgrade, answers)).contains("https"));
    }
}
