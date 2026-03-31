//! Generic QA result types shared across all messaging providers.
//!
//! Each provider has a different `ProviderConfigOut` type, so
//! [`ApplyAnswersResult`] is generic over the config `C`.

use serde::Serialize;

/// Standard cleanup steps used by most providers on remove.
pub const DEFAULT_REMOVE_CLEANUP: &[&str] = &[
    "delete_config_key",
    "delete_provenance_key",
    "delete_provider_state_namespace",
    "best_effort_revoke_webhooks",
    "best_effort_revoke_tokens",
    "best_effort_delete_provider_owned_secrets",
];

/// Result of `apply_answers`, generic over the provider config type.
#[derive(Debug, Clone, Serialize)]
pub struct ApplyAnswersResult<C: Serialize> {
    pub ok: bool,
    pub config: Option<C>,
    pub remove: Option<RemovePlan>,
    pub diagnostics: Vec<String>,
    pub error: Option<String>,
}

/// Describes what to clean up when removing a provider.
#[derive(Debug, Clone, Serialize)]
pub struct RemovePlan {
    pub remove_all: bool,
    pub cleanup: Vec<String>,
}

impl<C: Serialize> ApplyAnswersResult<C> {
    /// A successful result with the merged config.
    pub fn success(config: C) -> Self {
        Self {
            ok: true,
            config: Some(config),
            remove: None,
            diagnostics: Vec::new(),
            error: None,
        }
    }

    /// A remove result with the given cleanup steps.
    pub fn remove(cleanup: Vec<String>) -> Self {
        Self {
            ok: true,
            config: None,
            remove: Some(RemovePlan {
                remove_all: true,
                cleanup,
            }),
            diagnostics: Vec::new(),
            error: None,
        }
    }

    /// A remove result using the [`DEFAULT_REMOVE_CLEANUP`] steps.
    pub fn remove_default() -> Self {
        Self::remove(
            DEFAULT_REMOVE_CLEANUP
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        )
    }

    /// A validation error (config was invalid).
    pub fn validation_error(error: String) -> Self {
        Self {
            ok: false,
            config: None,
            remove: None,
            diagnostics: Vec::new(),
            error: Some(error),
        }
    }

    /// A decode error (input CBOR was invalid).
    pub fn decode_error(err: String) -> Self {
        Self {
            ok: false,
            config: None,
            remove: None,
            diagnostics: Vec::new(),
            error: Some(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[derive(Debug, Clone, Serialize)]
    struct TestConfig {
        name: String,
    }

    #[test]
    fn success_serializes_with_config() {
        let result = ApplyAnswersResult::success(TestConfig {
            name: "test".into(),
        });
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["ok"], true);
        assert_eq!(json["config"]["name"], "test");
        assert_eq!(json["remove"], Value::Null);
        assert_eq!(json["error"], Value::Null);
    }

    #[test]
    fn remove_default_has_six_steps() {
        let result: ApplyAnswersResult<TestConfig> = ApplyAnswersResult::remove_default();
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["ok"], true);
        assert_eq!(json["config"], Value::Null);
        let cleanup = json["remove"]["cleanup"].as_array().unwrap();
        assert_eq!(cleanup.len(), 6);
        assert!(cleanup[0].as_str().unwrap().contains("delete_config_key"));
    }

    #[test]
    fn validation_error_sets_error() {
        let result: ApplyAnswersResult<TestConfig> =
            ApplyAnswersResult::validation_error("bad url".into());
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["ok"], false);
        assert_eq!(json["error"], "bad url");
    }

    #[test]
    fn decode_error_sets_error() {
        let result: ApplyAnswersResult<TestConfig> =
            ApplyAnswersResult::decode_error("invalid cbor".into());
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["ok"], false);
        assert!(json["error"].as_str().unwrap().contains("invalid cbor"));
    }
}
