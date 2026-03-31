//! Shared utility functions used by all messaging providers.
//!
//! These are pure functions extracted from the individual provider `lib.rs`
//! files to eliminate cross-provider duplication.

use crate::component_v0_6::{
    DescribePayload, I18nText, OperationDescriptor, QaQuestionSpec, QaSpec, SchemaField, SchemaIr,
    canonical_cbor_bytes, decode_cbor, default_en_i18n_messages,
};
use base64::Engine as _;
use greentic_types::ChannelMessageEnvelope;
use greentic_types::messaging::universal_dto::{SendPayloadInV1, SendPayloadResultV1};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// JSON serialization
// ---------------------------------------------------------------------------

/// Serialize a value to JSON bytes, returning `{}` on failure.
pub fn json_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec())
}

// ---------------------------------------------------------------------------
// I18n / descriptor helpers
// ---------------------------------------------------------------------------

/// Build an [`I18nText`] from a dotted key.
pub fn i18n(key: &str) -> I18nText {
    I18nText {
        key: key.to_string(),
    }
}

/// Build an [`OperationDescriptor`] from a name and i18n keys.
pub fn op(name: &str, title_key: &str, desc_key: &str) -> OperationDescriptor {
    OperationDescriptor {
        name: name.to_string(),
        title: i18n(title_key),
        description: i18n(desc_key),
    }
}

/// Build a [`QaQuestionSpec`] from a key, i18n text key, and required flag.
pub fn qa_q(key: &str, text_key: &str, required: bool) -> QaQuestionSpec {
    QaQuestionSpec {
        id: key.to_string(),
        label: i18n(text_key),
        help: None,
        error: None,
        kind: crate::component_v0_6::QuestionKind::Text,
        required,
        default: None,
        skip_if: None,
    }
}

/// Build a [`QaQuestionSpec`] for inline JSON input with optional schema validation.
pub fn qa_inline_json(key: &str, text_key: &str, required: bool) -> QaQuestionSpec {
    QaQuestionSpec {
        id: key.to_string(),
        label: i18n(text_key),
        help: None,
        error: None,
        kind: crate::component_v0_6::QuestionKind::InlineJson { schema: None },
        required,
        default: None,
        skip_if: None,
    }
}

/// Build a [`QaQuestionSpec`] for inline JSON input with JSON Schema validation.
pub fn qa_inline_json_with_schema(
    key: &str,
    text_key: &str,
    required: bool,
    schema: serde_json::Value,
) -> QaQuestionSpec {
    QaQuestionSpec {
        id: key.to_string(),
        label: i18n(text_key),
        help: None,
        error: None,
        kind: crate::component_v0_6::QuestionKind::InlineJson {
            schema: Some(schema),
        },
        required,
        default: None,
        skip_if: None,
    }
}

/// Build a [`QaQuestionSpec`] for asset file reference.
pub fn qa_asset_ref(
    key: &str,
    text_key: &str,
    required: bool,
    file_types: Vec<String>,
) -> QaQuestionSpec {
    QaQuestionSpec {
        id: key.to_string(),
        label: i18n(text_key),
        help: None,
        error: None,
        kind: crate::component_v0_6::QuestionKind::AssetRef {
            file_types,
            base_path: Some("assets/".to_string()),
            check_exists: true,
        },
        required,
        default: None,
        skip_if: None,
    }
}

/// Build a [`QaQuestionSpec`] for asset file reference with custom base path.
pub fn qa_asset_ref_with_base(
    key: &str,
    text_key: &str,
    required: bool,
    file_types: Vec<String>,
    base_path: Option<String>,
    check_exists: bool,
) -> QaQuestionSpec {
    QaQuestionSpec {
        id: key.to_string(),
        label: i18n(text_key),
        help: None,
        error: None,
        kind: crate::component_v0_6::QuestionKind::AssetRef {
            file_types,
            base_path,
            check_exists,
        },
        required,
        default: None,
        skip_if: None,
    }
}

// ---------------------------------------------------------------------------
// QA answer extraction
// ---------------------------------------------------------------------------

/// Extract a string from `answers[key]`, falling back to `default`.
pub fn string_or_default(answers: &Value, key: &str, default: &str) -> String {
    answers
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_string()
}

/// Extract an optional non-empty string from `answers[key]`.
pub fn optional_string_from(answers: &Value, key: &str) -> Option<String> {
    let value = answers.get(key)?;
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        _ => None,
    }
}

/// Deserialize `answers.existing_config` or `answers.config` into `T`.
pub fn existing_config_from_answers<T: DeserializeOwned>(answers: &Value) -> Option<T> {
    answers
        .get("existing_config")
        .cloned()
        .or_else(|| answers.get("config").cloned())
        .and_then(|value| serde_json::from_value(value).ok())
}

// ---------------------------------------------------------------------------
// Pipeline error / success helpers
// ---------------------------------------------------------------------------

/// Return an encode error response.
pub fn encode_error(message: &str) -> Vec<u8> {
    json_bytes(&json!({"ok": false, "error": message}))
}

#[derive(Deserialize)]
struct EncodeMessageIn {
    message: ChannelMessageEnvelope,
}

/// Decode encode input across both legacy and current operator shapes.
///
/// Supported payloads:
/// - `{ "message": <ChannelMessageEnvelope>, ... }` (plan may be any JSON shape)
/// - `<ChannelMessageEnvelope>` (direct envelope fallback)
pub fn decode_encode_message(input_json: &[u8]) -> Result<ChannelMessageEnvelope, String> {
    match serde_json::from_slice::<EncodeMessageIn>(input_json) {
        Ok(value) => Ok(value.message),
        Err(message_err) => match serde_json::from_slice::<ChannelMessageEnvelope>(input_json) {
            Ok(envelope) => Ok(envelope),
            Err(envelope_err) => Err(format!(
                "invalid encode input: {message_err}; envelope fallback failed: {envelope_err}"
            )),
        },
    }
}

/// Return a send_payload error response.
pub fn send_payload_error(message: &str, retryable: bool) -> Vec<u8> {
    json_bytes(&SendPayloadResultV1 {
        ok: false,
        message: Some(message.to_string()),
        retryable,
    })
}

/// Return a send_payload success response.
pub fn send_payload_success() -> Vec<u8> {
    json_bytes(&SendPayloadResultV1 {
        ok: true,
        message: None,
        retryable: false,
    })
}

// ---------------------------------------------------------------------------
// Phase 2 — CBOR-JSON invoke bridge
// ---------------------------------------------------------------------------

/// Decode CBOR input, optionally remap `"run"` to `run_alias`, dispatch to
/// `dispatch_fn`, and re-encode the JSON result as CBOR.
///
/// This replaces the identical `runtime::Guest::invoke()` boilerplate found
/// in every provider except dummy.
pub fn cbor_json_invoke_bridge(
    op: &str,
    input_cbor: &[u8],
    run_alias: Option<&str>,
    dispatch_fn: impl FnOnce(&str, &[u8]) -> Vec<u8>,
) -> Vec<u8> {
    let input_value: Value = match decode_cbor(input_cbor) {
        Ok(value) => value,
        Err(err) => {
            return canonical_cbor_bytes(
                &json!({"ok": false, "error": format!("invalid input cbor: {err}")}),
            );
        }
    };
    let input_json = serde_json::to_vec(&input_value).unwrap_or_default();
    let effective_op = if op == "run" {
        run_alias.unwrap_or(op)
    } else {
        op
    };
    let output_json = dispatch_fn(effective_op, &input_json);
    let output_value: Value = serde_json::from_slice(&output_json)
        .unwrap_or_else(|_| json!({"ok": false, "error": "provider produced invalid json"}));
    canonical_cbor_bytes(&output_value)
}

/// schema-core-api `describe()` — JSON-serialize a [`DescribePayload`].
pub fn schema_core_describe(payload: &DescribePayload) -> Vec<u8> {
    serde_json::to_vec(payload).unwrap_or_default()
}

/// schema-core-api `validate_config()` — always returns `{"ok": true}`.
pub fn schema_core_validate_config() -> Vec<u8> {
    json_bytes(&json!({"ok": true}))
}

/// schema-core-api `healthcheck()` — always returns `{"status": "healthy"}`.
pub fn schema_core_healthcheck() -> Vec<u8> {
    json_bytes(&json!({"status": "healthy"}))
}

// ---------------------------------------------------------------------------
// send_payload dispatch wrapper
// ---------------------------------------------------------------------------

/// Decode a [`SendPayloadInV1`], verify `provider_type`, base64-decode the
/// payload body, and forward to `send_fn`.  Returns success/error bytes.
///
/// This replaces the identical `send_payload()` boilerplate found in every
/// provider except dummy.
pub fn send_payload_dispatch(
    input_json: &[u8],
    provider_type: &str,
    send_fn: impl FnOnce(&[u8]) -> Vec<u8>,
) -> Vec<u8> {
    let send_in = match serde_json::from_slice::<SendPayloadInV1>(input_json) {
        Ok(value) => value,
        Err(err) => {
            return send_payload_error(&format!("invalid send_payload input: {err}"), false);
        }
    };
    if send_in.provider_type != provider_type {
        return send_payload_error("provider type mismatch", false);
    }
    let payload_bytes =
        match base64::engine::general_purpose::STANDARD.decode(&send_in.payload.body_b64) {
            Ok(bytes) => bytes,
            Err(err) => {
                return send_payload_error(&format!("payload decode failed: {err}"), false);
            }
        };
    let payload: Value = serde_json::from_slice(&payload_bytes).unwrap_or(Value::Null);
    let payload_bytes = serde_json::to_vec(&payload).unwrap_or_else(|_| b"{}".to_vec());
    let result_bytes = send_fn(&payload_bytes);
    let result_value: Value = serde_json::from_slice(&result_bytes).unwrap_or(Value::Null);
    let ok = result_value
        .get("ok")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if ok {
        send_payload_success()
    } else {
        let message = result_value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("send_payload failed");
        send_payload_error(message, false)
    }
}

// ---------------------------------------------------------------------------
// I18n boilerplate helpers
// ---------------------------------------------------------------------------

/// Convert a `&[&str]` key list to `Vec<String>` for `i18n_keys()`.
pub fn i18n_keys_from(keys: &[&str]) -> Vec<String> {
    keys.iter().map(|k| (*k).to_string()).collect()
}

/// Build a default English i18n bundle CBOR blob for `i18n_bundle()`.
pub fn i18n_bundle_default(locale: String, keys: &[&str]) -> Vec<u8> {
    let locale = if locale.trim().is_empty() {
        "en".to_string()
    } else {
        locale
    };
    let messages = default_en_i18n_messages(keys);
    canonical_cbor_bytes(&json!({"locale": locale, "messages": Value::Object(messages)}))
}

/// Build an i18n bundle CBOR blob from explicit `(key, value)` pairs.
pub fn i18n_bundle_from_pairs(locale: String, pairs: &[(&str, &str)]) -> Vec<u8> {
    let locale = if locale.trim().is_empty() {
        "en".to_string()
    } else {
        locale
    };
    let messages: serde_json::Map<String, Value> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), Value::String((*v).to_string())))
        .collect();
    canonical_cbor_bytes(&json!({"locale": locale, "messages": Value::Object(messages)}))
}

// ---------------------------------------------------------------------------
// Schema builder helpers
// ---------------------------------------------------------------------------

/// Build a `SchemaIr::String` (non-secret, no format).
pub fn schema_str(title: &str, desc: &str) -> SchemaIr {
    SchemaIr::String {
        title: i18n(title),
        description: i18n(desc),
        format: None,
        secret: false,
    }
}

/// Build a `SchemaIr::String` with a format (e.g. `"uri"`).
pub fn schema_str_fmt(title: &str, desc: &str, format: &str) -> SchemaIr {
    SchemaIr::String {
        title: i18n(title),
        description: i18n(desc),
        format: Some(format.to_string()),
        secret: false,
    }
}

/// Build a secret `SchemaIr::String` (no format).
pub fn schema_secret(title: &str, desc: &str) -> SchemaIr {
    SchemaIr::String {
        title: i18n(title),
        description: i18n(desc),
        format: None,
        secret: true,
    }
}

/// Build a `SchemaIr::Bool`.
pub fn schema_bool_ir(title: &str, desc: &str) -> SchemaIr {
    SchemaIr::Bool {
        title: i18n(title),
        description: i18n(desc),
    }
}

/// Build a `SchemaIr::Object` from a list of `(name, required, schema)`.
pub fn schema_obj(
    title: &str,
    desc: &str,
    field_defs: Vec<(&str, bool, SchemaIr)>,
    additional_properties: bool,
) -> SchemaIr {
    let mut fields = BTreeMap::new();
    for (name, required, schema) in field_defs {
        fields.insert(name.to_string(), SchemaField { required, schema });
    }
    SchemaIr::Object {
        title: i18n(title),
        description: i18n(desc),
        fields,
        additional_properties,
    }
}

// ---------------------------------------------------------------------------
// QA spec builder
// ---------------------------------------------------------------------------

/// Question definition: `(key, i18n_text_key, required_in_setup)`.
pub type QaQuestionDef<'a> = (&'a str, &'a str, bool);

/// Build a [`QaSpec`] for the given mode from shared question definitions.
///
/// - `prefix`: provider prefix (e.g. `"slack"`)
/// - `setup_questions`: full list of `(key, text_key, required)` for Setup
/// - `default_keys`: subset of keys that appear in Default mode (all required)
///
/// Upgrade mode reuses Setup questions with `required = false`.
/// Remove mode returns an empty question list.
pub fn qa_spec_for_mode(
    mode: &str,
    prefix: &str,
    setup_questions: &[QaQuestionDef],
    default_keys: &[&str],
) -> QaSpec {
    match mode {
        "default" => {
            let questions = default_keys
                .iter()
                .filter_map(|dk| {
                    setup_questions
                        .iter()
                        .find(|(k, _, _)| k == dk)
                        .map(|(k, t, _)| qa_q(k, t, true))
                })
                .collect();
            QaSpec {
                mode: "default".to_string(),
                title: i18n(&format!("{prefix}.qa.default.title")),
                description: None,
                questions,
                defaults: Default::default(),
            }
        }
        "setup" => QaSpec {
            mode: "setup".to_string(),
            title: i18n(&format!("{prefix}.qa.setup.title")),
            description: None,
            questions: setup_questions
                .iter()
                .map(|(k, t, r)| qa_q(k, t, *r))
                .collect(),
            defaults: Default::default(),
        },
        "upgrade" => QaSpec {
            mode: "upgrade".to_string(),
            title: i18n(&format!("{prefix}.qa.upgrade.title")),
            description: None,
            questions: setup_questions
                .iter()
                .map(|(k, t, _)| qa_q(k, t, false))
                .collect(),
            defaults: Default::default(),
        },
        _ => QaSpec {
            mode: "remove".to_string(),
            title: i18n(&format!("{prefix}.qa.remove.title")),
            description: None,
            questions: Vec::new(),
            defaults: Default::default(),
        },
    }
}

// ---------------------------------------------------------------------------
// Config loader
// ---------------------------------------------------------------------------

/// Load a provider config from input JSON.
///
/// Tries `input["config"]` first, then falls back to extracting top-level
/// fields listed in `keys`.  Returns `Err` if neither source yields a valid
/// `T`.
pub fn load_config_generic<T: DeserializeOwned>(input: &Value, keys: &[&str]) -> Result<T, String> {
    if let Some(cfg) = input.get("config") {
        return serde_json::from_value::<T>(cfg.clone())
            .map_err(|e| format!("invalid config: {e}"));
    }
    let mut partial = serde_json::Map::new();
    for key in keys {
        if let Some(v) = input.get(*key) {
            partial.insert((*key).to_string(), v.clone());
        }
    }
    if !partial.is_empty() {
        return serde_json::from_value::<T>(Value::Object(partial))
            .map_err(|e| format!("invalid config: {e}"));
    }
    Err("missing config: expected `config` or top-level config fields".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_bytes_produces_valid_json() {
        let bytes = json_bytes(&json!({"ok": true}));
        let parsed: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed["ok"], true);
    }

    #[test]
    fn string_or_default_returns_value_when_present() {
        let answers = json!({"name": "Alice"});
        assert_eq!(string_or_default(&answers, "name", "Bob"), "Alice");
    }

    #[test]
    fn string_or_default_returns_default_when_missing() {
        let answers = json!({});
        assert_eq!(string_or_default(&answers, "name", "Bob"), "Bob");
    }

    #[test]
    fn string_or_default_returns_default_when_empty() {
        let answers = json!({"name": "  "});
        assert_eq!(string_or_default(&answers, "name", "Bob"), "Bob");
    }

    #[test]
    fn optional_string_from_returns_some() {
        let answers = json!({"name": "Alice"});
        assert_eq!(optional_string_from(&answers, "name"), Some("Alice".into()));
    }

    #[test]
    fn optional_string_from_returns_none_for_empty() {
        let answers = json!({"name": ""});
        assert_eq!(optional_string_from(&answers, "name"), None);
    }

    #[test]
    fn optional_string_from_returns_none_for_missing() {
        let answers = json!({});
        assert_eq!(optional_string_from(&answers, "name"), None);
    }

    #[test]
    fn existing_config_from_answers_prefers_existing_config() {
        let answers = json!({
            "existing_config": {"a": 1},
            "config": {"a": 2}
        });
        let val: Option<Value> = existing_config_from_answers(&answers);
        assert_eq!(val.unwrap()["a"], 1);
    }

    #[test]
    fn existing_config_from_answers_falls_back_to_config() {
        let answers = json!({"config": {"a": 2}});
        let val: Option<Value> = existing_config_from_answers(&answers);
        assert_eq!(val.unwrap()["a"], 2);
    }

    #[test]
    fn send_payload_error_serializes_correctly() {
        let bytes = send_payload_error("bad", true);
        let val: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["ok"], false);
        assert_eq!(val["message"], "bad");
        assert_eq!(val["retryable"], true);
    }

    #[test]
    fn send_payload_success_serializes_correctly() {
        let bytes = send_payload_success();
        let val: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["ok"], true);
    }

    #[test]
    fn op_builds_descriptor() {
        let desc = op("send", "p.send.title", "p.send.desc");
        assert_eq!(desc.name, "send");
        assert_eq!(desc.title.key, "p.send.title");
        assert_eq!(desc.description.key, "p.send.desc");
    }

    #[test]
    fn qa_q_builds_question() {
        let q = qa_q("name", "p.qa.name", true);
        assert_eq!(q.id, "name");
        assert_eq!(q.label.key, "p.qa.name");
        assert!(q.required);
    }

    #[test]
    fn schema_core_validate_config_returns_ok() {
        let bytes = schema_core_validate_config();
        let val: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["ok"], true);
    }

    #[test]
    fn schema_core_healthcheck_returns_healthy() {
        let bytes = schema_core_healthcheck();
        let val: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["status"], "healthy");
    }
}
