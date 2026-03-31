//! JSON bridge for QA ops dispatched through `schema-core-api invoke()`.
//!
//! The operator calls all provider ops through the `schema-core-api` JSON
//! `invoke(op, input_json)` path.  The WIT-native `qa` interface uses CBOR,
//! so this module bridges the two encodings.

use crate::component_v0_6::{canonical_cbor_bytes, decode_cbor};
use crate::helpers::{
    QaQuestionDef, i18n_bundle_from_pairs, i18n_keys_from, json_bytes, qa_spec_for_mode,
};
use serde_json::{Value, json};

/// Try to dispatch a QA op through the JSON invoke path.
///
/// Returns `Some(json_bytes)` if `op` is a QA op (`qa-spec`, `apply-answers`,
/// `i18n-keys`, or `i18n-bundle`), `None` otherwise so the caller can fall
/// through to its normal dispatch.
///
/// `apply_fn(mode_str, answers_cbor) -> result_cbor` is the provider-specific
/// apply-answers implementation that works with CBOR in/out.
pub fn dispatch_qa_ops(
    op: &str,
    input_json: &[u8],
    provider_name: &str,
    setup_questions: &[QaQuestionDef],
    default_keys: &[&str],
    i18n_keys_list: &[&str],
    apply_fn: impl FnOnce(&str, Vec<u8>) -> Vec<u8>,
) -> Option<Vec<u8>> {
    dispatch_qa_ops_with_i18n(
        op,
        input_json,
        provider_name,
        setup_questions,
        default_keys,
        i18n_keys_list,
        &[],
        apply_fn,
    )
}

/// Extended version of [`dispatch_qa_ops`] that also accepts `i18n_pairs`
/// for the `i18n-bundle` operation.
#[allow(clippy::too_many_arguments)]
pub fn dispatch_qa_ops_with_i18n(
    op: &str,
    input_json: &[u8],
    provider_name: &str,
    setup_questions: &[QaQuestionDef],
    default_keys: &[&str],
    i18n_keys_list: &[&str],
    i18n_pairs: &[(&str, &str)],
    apply_fn: impl FnOnce(&str, Vec<u8>) -> Vec<u8>,
) -> Option<Vec<u8>> {
    match op {
        "qa-spec" => Some(bridge_qa_spec(
            input_json,
            provider_name,
            setup_questions,
            default_keys,
        )),
        "apply-answers" => Some(bridge_apply_answers(input_json, apply_fn)),
        "i18n-keys" => Some(bridge_i18n_keys(i18n_keys_list)),
        "i18n-bundle" => Some(bridge_i18n_bundle(input_json, i18n_pairs)),
        _ => None,
    }
}

/// `qa-spec`: JSON `{"mode":"setup"}` → QaSpec as JSON.
fn bridge_qa_spec(
    input_json: &[u8],
    provider_name: &str,
    setup_questions: &[QaQuestionDef],
    default_keys: &[&str],
) -> Vec<u8> {
    let mode = extract_mode(input_json);
    let spec = qa_spec_for_mode(&mode, provider_name, setup_questions, default_keys);
    json_bytes(&spec)
}

/// `apply-answers`: JSON in → CBOR bridge → apply_fn → CBOR→JSON out.
///
/// Operator sends: `{"mode":"setup", "current_config":{…}, "answers":{…}}`
/// The provider's apply function expects CBOR-encoded answers with an
/// optional `existing_config` field inside.
fn bridge_apply_answers(
    input_json: &[u8],
    apply_fn: impl FnOnce(&str, Vec<u8>) -> Vec<u8>,
) -> Vec<u8> {
    let input: Value = match serde_json::from_slice(input_json) {
        Ok(v) => v,
        Err(e) => {
            return json_bytes(&json!({"ok": false, "error": format!("invalid input json: {e}")}));
        }
    };

    let mode = input.get("mode").and_then(Value::as_str).unwrap_or("setup");

    // Build the answers payload the same way the WIT path expects it:
    // a flat object with answer fields + optional "existing_config".
    let mut payload = input.get("answers").cloned().unwrap_or_else(|| json!({}));

    if let Some(current_config) = input.get("current_config")
        && let Some(obj) = payload.as_object_mut()
    {
        obj.insert("existing_config".to_string(), current_config.clone());
    }

    let answers_cbor = canonical_cbor_bytes(&payload);
    let result_cbor = apply_fn(mode, answers_cbor);

    match decode_cbor::<Value>(&result_cbor) {
        Ok(result) => json_bytes(&result),
        Err(e) => json_bytes(&json!({"ok": false, "error": format!("cbor decode error: {e}")})),
    }
}

/// `i18n-keys`: returns JSON array of key strings.
fn bridge_i18n_keys(i18n_keys_list: &[&str]) -> Vec<u8> {
    let keys = i18n_keys_from(i18n_keys_list);
    json_bytes(&keys)
}

/// `i18n-bundle`: returns JSON `{"locale":"…","messages":{key:value}}`.
///
/// The input is a JSON string with the locale (e.g. `"en"`).
/// Uses the provider's custom `I18N_PAIRS` for translations.
fn bridge_i18n_bundle(input_json: &[u8], i18n_pairs: &[(&str, &str)]) -> Vec<u8> {
    let locale: String = serde_json::from_slice(input_json)
        .ok()
        .and_then(|v: Value| v.as_str().map(String::from))
        .unwrap_or_else(|| "en".to_string());
    let cbor = i18n_bundle_from_pairs(locale, i18n_pairs);
    // Decode CBOR to JSON for the operator
    match decode_cbor::<Value>(&cbor) {
        Ok(val) => json_bytes(&val),
        Err(e) => json_bytes(&json!({"error": format!("cbor decode error: {e}")})),
    }
}

/// Extract the mode string from JSON input, defaulting to `"setup"`.
fn extract_mode(input_json: &[u8]) -> String {
    serde_json::from_slice::<Value>(input_json)
        .ok()
        .and_then(|v| v.get("mode")?.as_str().map(String::from))
        .unwrap_or_else(|| "setup".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_mode_parses_setup() {
        let input = br#"{"mode":"setup"}"#;
        assert_eq!(extract_mode(input), "setup");
    }

    #[test]
    fn extract_mode_defaults_to_setup() {
        assert_eq!(extract_mode(b"{}"), "setup");
        assert_eq!(extract_mode(b"invalid"), "setup");
    }

    #[test]
    fn dispatch_returns_none_for_unknown_op() {
        let result = dispatch_qa_ops("send", b"{}", "test", &[], &[], &[], |_, _| vec![]);
        assert!(result.is_none());
    }

    #[test]
    fn dispatch_qa_spec_returns_json() {
        let questions: &[QaQuestionDef] = &[("token", "t.qa.token", true)];
        let result = dispatch_qa_ops(
            "qa-spec",
            br#"{"mode":"setup"}"#,
            "test",
            questions,
            &["token"],
            &[],
            |_, _| vec![],
        )
        .unwrap();
        let parsed: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(parsed["mode"], "setup");
        assert!(parsed["questions"].as_array().unwrap().len() == 1);
    }

    #[test]
    fn dispatch_i18n_keys_returns_json_array() {
        let keys = &["a.key", "b.key"];
        let result =
            dispatch_qa_ops("i18n-keys", b"{}", "test", &[], &[], keys, |_, _| vec![]).unwrap();
        let parsed: Vec<String> = serde_json::from_slice(&result).unwrap();
        assert_eq!(parsed, vec!["a.key", "b.key"]);
    }

    #[test]
    fn dispatch_apply_answers_bridges_json_cbor() {
        let input =
            br#"{"mode":"setup","current_config":{"enabled":true},"answers":{"token":"abc"}}"#;
        let result = dispatch_qa_ops(
            "apply-answers",
            input,
            "test",
            &[],
            &[],
            &[],
            |mode, cbor| {
                assert_eq!(mode, "setup");
                // Verify the CBOR payload contains both answers and existing_config
                let payload: Value = decode_cbor(&cbor).unwrap();
                assert_eq!(payload["token"], "abc");
                assert_eq!(payload["existing_config"]["enabled"], true);
                // Return a simple CBOR success response
                canonical_cbor_bytes(&json!({"ok": true, "config": {"token": "abc"}}))
            },
        )
        .unwrap();
        let parsed: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(parsed["ok"], true);
        assert_eq!(parsed["config"]["token"], "abc");
    }
}
