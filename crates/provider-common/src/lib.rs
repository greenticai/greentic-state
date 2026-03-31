pub mod component_v0_6;
pub mod helpers;
pub mod http_compat;
pub mod lifecycle_keys;
pub mod qa_helpers;
pub mod qa_invoke_bridge;
pub mod test_macros;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Common error type providers can reuse to surface failures.
#[derive(Debug, Error, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderError {
    #[error("validation error: {0}")]
    Validation(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("missing secret: {name} (scope: {scope})")]
    MissingSecret {
        name: String,
        scope: String,
        remediation: String,
    },
    #[error("unknown provider error: {0}")]
    Other(String),
}

impl ProviderError {
    pub fn validation(msg: impl Into<String>) -> Self {
        ProviderError::Validation(msg.into())
    }

    pub fn transport(msg: impl Into<String>) -> Self {
        ProviderError::Transport(msg.into())
    }

    pub fn other(msg: impl Into<String>) -> Self {
        ProviderError::Other(msg.into())
    }

    pub fn missing_secret(name: impl Into<String>) -> Self {
        let name = name.into();
        ProviderError::MissingSecret {
            name: name.clone(),
            scope: "tenant".into(),
            remediation: format!(
                "Provide the `{name}` secret via greentic:secrets-store for this tenant."
            ),
        }
    }
}

pub const PROVIDER_CAPABILITIES_VERSION: &str = "v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ProviderCapabilitiesV1 {
    pub supports_threads: bool,
    pub supports_buttons: bool,
    pub supports_webhook_validation: bool,
    pub supports_formatting_options: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ProviderLimitsV1 {
    pub max_text_len: u32,
    pub callback_data_max_bytes: u32,
    pub max_buttons_per_row: u32,
    pub max_button_rows: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ProviderMetadataV1 {
    pub provider_id: String,
    pub display_name: String,
    pub version: String,
    pub rate_limit_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CapabilitiesResponseV1 {
    pub version: String,
    pub metadata: ProviderMetadataV1,
    pub capabilities: ProviderCapabilitiesV1,
    pub limits: ProviderLimitsV1,
}

impl CapabilitiesResponseV1 {
    pub fn new(
        metadata: ProviderMetadataV1,
        capabilities: ProviderCapabilitiesV1,
        limits: ProviderLimitsV1,
    ) -> Self {
        Self {
            version: PROVIDER_CAPABILITIES_VERSION.to_string(),
            metadata,
            capabilities,
            limits,
        }
    }
}

/// Backwards-friendly aliases for V1.
pub type ProviderCapabilities = ProviderCapabilitiesV1;
pub type ProviderLimits = ProviderLimitsV1;
pub type ProviderMetadata = ProviderMetadataV1;
pub type CapabilitiesResponse = CapabilitiesResponseV1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RenderTier {
    TierA,
    TierB,
    TierC,
    TierD,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RenderWarning {
    pub code: String,
    pub message: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RenderPlan {
    pub tier: RenderTier,
    pub summary_text: Option<String>,
    pub actions: Vec<String>,
    pub attachments: Vec<String>,
    pub warnings: Vec<RenderWarning>,
    pub debug: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ProviderPayload {
    /// MIME type for the body (e.g., application/json, text/plain).
    pub content_type: String,
    /// Transport-ready bytes for the provider edge.
    pub body: Vec<u8>,
    /// Optional metadata to drive edge handling (ids, headers, etc.).
    pub metadata: Option<serde_json::Map<String, Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct EncodeResult {
    pub payload: ProviderPayload,
    pub warnings: Vec<RenderWarning>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_validation_error() {
        let err = ProviderError::validation("missing token");
        assert_eq!(err, ProviderError::Validation("missing token".into()));
        assert_eq!(err.to_string(), "validation error: missing token");
    }

    #[test]
    fn builds_missing_secret_error() {
        let err = ProviderError::missing_secret("API_KEY");
        assert!(matches!(err, ProviderError::MissingSecret { .. }));
        assert_eq!(err.to_string(), "missing secret: API_KEY (scope: tenant)");
    }

    #[test]
    fn capabilities_round_trip() {
        let caps = CapabilitiesResponseV1::new(
            ProviderMetadataV1 {
                provider_id: "slack".into(),
                display_name: "Slack".into(),
                version: "1.0.0".into(),
                rate_limit_hint: None,
            },
            ProviderCapabilitiesV1 {
                supports_threads: false,
                supports_buttons: false,
                supports_webhook_validation: true,
                supports_formatting_options: false,
            },
            ProviderLimitsV1 {
                max_text_len: 40_000,
                callback_data_max_bytes: 0,
                max_buttons_per_row: 0,
                max_button_rows: 0,
            },
        );

        let json = serde_json::to_string(&caps).expect("serialize");
        let decoded: CapabilitiesResponseV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.version, PROVIDER_CAPABILITIES_VERSION);
        assert_eq!(decoded.metadata.provider_id, "slack");
        assert!(decoded.capabilities.supports_webhook_validation);
        assert_eq!(decoded.limits.max_text_len, 40_000);
    }

    #[test]
    fn render_plan_round_trip_and_tier_serialization() {
        let plan = RenderPlan {
            tier: RenderTier::TierB,
            summary_text: Some("card summary".into()),
            actions: vec!["accept".into(), "decline".into()],
            attachments: vec!["https://example.invalid/a.png".into()],
            warnings: vec![RenderWarning {
                code: "text_truncated".into(),
                message: Some("trimmed to fit".into()),
                path: Some("/body/text".into()),
            }],
            debug: Some(serde_json::json!({"info":"debug"})),
        };

        let json = serde_json::to_string(&plan).expect("serialize");
        assert!(json.contains("\"tier\":\"tier_b\""));
        let decoded: RenderPlan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.tier, RenderTier::TierB);
        assert_eq!(decoded.warnings[0].code, "text_truncated");
    }

    #[test]
    fn provider_payload_round_trip() {
        let payload = ProviderPayload {
            content_type: "application/json".into(),
            body: br#"{"hello":"world"}"#.to_vec(),
            metadata: Some(serde_json::Map::from_iter([(
                "trace_id".into(),
                serde_json::json!("abc123"),
            )])),
        };
        let encoded = serde_json::to_string(&payload).expect("serialize");
        let decoded: ProviderPayload = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded.content_type, "application/json");
        assert_eq!(decoded.body, br#"{"hello":"world"}"#);
        assert_eq!(decoded.metadata.unwrap()["trace_id"], "abc123");
    }
}
