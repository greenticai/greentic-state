//! Declarative macros that generate the standard test suite shared by all
//! messaging providers.  Each provider invokes the macro in its own
//! `#[cfg(test)] mod tests` block, passing provider-specific values.

/// Generate the 7 standard provider tests that are identical (or
/// parameterizable) across all messaging providers.
///
/// # Required parameters
///
/// | Parameter | Example |
/// |-----------|---------|
/// | `describe_fn` | `build_describe_payload` |
/// | `qa_spec_fn` | `build_qa_spec` |
/// | `i18n_keys` | `I18N_KEYS` |
/// | `world_id` | `WORLD_ID` |
/// | `provider_id` | `PROVIDER_ID` |
/// | `schema_hash` | `"6eee..."` |
/// | `qa_default_keys` | `["public_base_url", "bot_token"]` |
/// | `mode_type` | `bindings::exports::greentic::component::qa::Mode` |
/// | `component_type` | `Component` |
/// | `qa_guest_path` | `bindings::exports::greentic::component::qa::Guest` |
/// | `validation_answers` | `{"public_base_url": "not-a-url", "bot_token": "t"}` |
/// | `validation_field` | `"public_base_url"` |
#[macro_export]
macro_rules! standard_provider_tests {
    (
        describe_fn: $describe_fn:expr,
        qa_spec_fn: $qa_spec_fn:expr,
        i18n_keys: $i18n_keys:expr,
        world_id: $world_id:expr,
        provider_id: $provider_id:expr,
        schema_hash: $schema_hash:expr,
        qa_default_keys: [$($qa_key:expr),* $(,)?],
        mode_type: $Mode:ty,
        component_type: $Comp:ty,
        qa_guest_path: $QaGuest:path,
        validation_answers: $val_answers:tt,
        validation_field: $val_field:expr $(,)?
    ) => {
        #[test]
        fn schema_hash_is_stable() {
            let describe = $describe_fn();
            assert_eq!(describe.schema_hash, $schema_hash);
        }

        #[test]
        fn describe_passes_strict_rules() {
            let describe = $describe_fn();
            assert!(!describe.operations.is_empty());
            assert_eq!(
                describe.schema_hash,
                provider_common::component_v0_6::schema_hash(
                    &describe.input_schema,
                    &describe.output_schema,
                    &describe.config_schema,
                )
            );
            assert_eq!(describe.world, $world_id);
            assert_eq!(describe.provider, $provider_id);
        }

        #[test]
        fn i18n_keys_cover_qa_specs() {
            let keyset = $i18n_keys
                .iter()
                .map(|v| (*v).to_string())
                .collect::<std::collections::BTreeSet<_>>();
            for mode in [
                <$Mode>::Default,
                <$Mode>::Setup,
                <$Mode>::Upgrade,
                <$Mode>::Remove,
            ] {
                let spec = $qa_spec_fn(mode);
                assert!(
                    keyset.contains(&spec.title.key),
                    "missing i18n key for QA title: {}",
                    spec.title.key,
                );
                for question in spec.questions {
                    assert!(
                        keyset.contains(&question.label.key),
                        "missing i18n key for question: {}",
                        question.label.key,
                    );
                }
            }
        }

        #[test]
        fn qa_default_asks_required_minimum() {
            let spec = $qa_spec_fn(<$Mode>::Default);
            let keys: Vec<String> = spec
                .questions
                .into_iter()
                .map(|q| q.id)
                .collect();
            let expected: Vec<String> = vec![$($qa_key.to_string()),*];
            assert_eq!(keys, expected);
        }

        #[test]
        fn apply_answers_remove_returns_cleanup_plan() {
            use $QaGuest as QaGuest;
            let out = <$Comp as QaGuest>::apply_answers(
                <$Mode>::Remove,
                provider_common::component_v0_6::canonical_cbor_bytes(
                    &serde_json::json!({}),
                ),
            );
            let out_json: serde_json::Value =
                provider_common::component_v0_6::decode_cbor(&out)
                    .expect("decode apply output");
            assert_eq!(
                out_json.get("ok"),
                Some(&serde_json::Value::Bool(true)),
            );
            assert_eq!(
                out_json.get("config"),
                Some(&serde_json::Value::Null),
            );
            let cleanup = out_json
                .get("remove")
                .and_then(|v| v.get("cleanup"))
                .and_then(serde_json::Value::as_array)
                .expect("cleanup steps");
            assert!(!cleanup.is_empty());
        }

        #[test]
        fn apply_answers_validates_field() {
            use $QaGuest as QaGuest;
            let answers = serde_json::json!($val_answers);
            let out = <$Comp as QaGuest>::apply_answers(
                <$Mode>::Default,
                provider_common::component_v0_6::canonical_cbor_bytes(&answers),
            );
            let out_json: serde_json::Value =
                provider_common::component_v0_6::decode_cbor(&out)
                    .expect("decode apply output");
            assert_eq!(
                out_json.get("ok"),
                Some(&serde_json::Value::Bool(false)),
            );
            let error = out_json
                .get("error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            assert!(
                error.contains($val_field),
                "expected error to mention '{}', got: {}",
                $val_field,
                error,
            );
        }
    };
}
