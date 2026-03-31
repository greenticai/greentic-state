use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Capability tiers that describe the quality of the rendered plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderTier {
    TierA,
    TierB,
    TierC,
    TierD,
}

/// Warning emitted while constructing a render plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderWarning {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Items produced by the renderer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RenderItem {
    Text(String),
    AdaptiveCard(Value),
}

/// A render plan produced from a channel message envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderPlan {
    pub tier: RenderTier,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_text: Option<String>,
    #[serde(default)]
    pub items: Vec<RenderItem>,
    #[serde(default)]
    pub warnings: Vec<RenderWarning>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<Value>,
}

impl Default for RenderPlan {
    fn default() -> Self {
        Self {
            tier: RenderTier::TierD,
            summary_text: None,
            items: Vec::new(),
            warnings: Vec::new(),
            debug: None,
        }
    }
}
