//! Deterministic render planner for messaging providers.
//!
//! Ported from greentic-messaging `libs/core/src/render_planner.rs` with adaptations
//! for the providers workspace. Produces a `RenderPlan` based on `PlannerCapabilities`.

use crate::plan::{RenderItem, RenderPlan, RenderTier, RenderWarning};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Describes what a target channel supports.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlannerCapabilities {
    pub supports_adaptive_cards: bool,
    pub supports_markdown: bool,
    pub supports_html: bool,
    pub supports_images: bool,
    pub supports_buttons: bool,
    pub max_text_len: Option<u32>,
    pub max_payload_bytes: Option<u32>,
}

/// A button / link action extracted from an Adaptive Card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannerAction {
    pub title: String,
    pub url: Option<String>,
}

/// Intermediate card representation for the planner.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlannerCard {
    pub title: Option<String>,
    pub text: Option<String>,
    pub actions: Vec<PlannerAction>,
    pub images: Vec<String>,
}

/// Entry point: produce a `RenderPlan` from a card and capabilities.
pub fn plan_render(
    card: &PlannerCard,
    caps: &PlannerCapabilities,
    ac_json: Option<&Value>,
) -> RenderPlan {
    let tier = select_tier(caps, ac_json.is_some());
    let mut warnings: Vec<RenderWarning> = Vec::new();

    let mut items: Vec<RenderItem> = Vec::new();

    match tier {
        RenderTier::TierA | RenderTier::TierB => {
            // Pass AC through; also include text summary
            if let Some(text) = build_summary_text(card, caps, &mut warnings) {
                items.push(RenderItem::Text(text.clone()));
            }
            if let Some(ac) = ac_json {
                items.push(RenderItem::AdaptiveCard(ac.clone()));
            }
            if tier == RenderTier::TierB && has_unsupported_elements(card, caps) {
                warnings.push(RenderWarning {
                    code: "unsupported_elements_removed".into(),
                    message: Some("Some card elements were removed for this channel".into()),
                    path: None,
                });
            }
        }
        RenderTier::TierC | RenderTier::TierD => {
            // Text-only fallback
            if let Some(text) = build_summary_text(card, caps, &mut warnings) {
                items.push(RenderItem::Text(text.clone()));
            }
            if ac_json.is_some() {
                warnings.push(RenderWarning {
                    code: "adaptive_card_downsampled".into(),
                    message: Some("Adaptive Card was converted to text for this channel".into()),
                    path: None,
                });
            }
        }
    }

    let summary_text = items.iter().find_map(|item| match item {
        RenderItem::Text(t) => Some(t.clone()),
        _ => None,
    });

    RenderPlan {
        tier,
        summary_text,
        items,
        warnings,
        debug: None,
    }
}

fn select_tier(caps: &PlannerCapabilities, has_ac: bool) -> RenderTier {
    if !has_ac {
        return RenderTier::TierD;
    }
    if caps.supports_adaptive_cards {
        if caps.supports_buttons && caps.supports_images {
            RenderTier::TierA
        } else {
            RenderTier::TierB
        }
    } else {
        RenderTier::TierD
    }
}

fn build_summary_text(
    card: &PlannerCard,
    caps: &PlannerCapabilities,
    warnings: &mut Vec<RenderWarning>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    if let Some(title) = &card.title {
        let t = title.trim();
        if !t.is_empty() {
            parts.push(t.to_string());
        }
    }

    if let Some(text) = &card.text {
        let t = text.trim();
        if !t.is_empty() {
            parts.push(t.to_string());
        }
    }

    // Include action labels for text-only tiers
    if !caps.supports_buttons && !card.actions.is_empty() {
        let action_labels: Vec<String> = card
            .actions
            .iter()
            .map(|a| {
                if let Some(url) = &a.url {
                    format!("[{}]({})", a.title, url)
                } else {
                    a.title.clone()
                }
            })
            .collect();
        parts.push(action_labels.join(" | "));
    }

    if parts.is_empty() {
        return None;
    }

    let mut text = parts.join("\n\n");

    let (sanitized, did_sanitize) = sanitize_text(&text, caps);
    text = sanitized;
    if did_sanitize {
        warnings.push(RenderWarning {
            code: "text_sanitized".into(),
            message: Some("Text was sanitized for this channel".into()),
            path: None,
        });
    }

    // Truncate by chars
    if let Some(max_len) = caps.max_text_len {
        let (truncated, did_truncate) = truncate_chars(&text, max_len as usize);
        text = truncated;
        if did_truncate {
            warnings.push(RenderWarning {
                code: "text_truncated".into(),
                message: Some(format!("Text truncated to {} chars", max_len)),
                path: None,
            });
        }
    }

    // Truncate by bytes
    if let Some(max_bytes) = caps.max_payload_bytes {
        let (truncated, did_truncate) = truncate_bytes(&text, max_bytes as usize);
        text = truncated;
        if did_truncate {
            warnings.push(RenderWarning {
                code: "payload_truncated".into(),
                message: Some(format!("Payload truncated to {} bytes", max_bytes)),
                path: None,
            });
        }
    }

    Some(text)
}

/// Strip HTML tags if !supports_html, strip markdown markers if !supports_markdown.
fn sanitize_text(text: &str, caps: &PlannerCapabilities) -> (String, bool) {
    let mut result = text.to_string();
    let mut changed = false;

    if !caps.supports_html {
        let stripped = strip_html_tags(&result);
        if stripped != result {
            changed = true;
            result = stripped;
        }
    }

    if !caps.supports_markdown {
        let stripped = strip_markdown_markers(&result);
        if stripped != result {
            changed = true;
            result = stripped;
        }
    }

    (result, changed)
}

fn strip_html_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

fn strip_markdown_markers(text: &str) -> String {
    text.replace("**", "")
        .replace("__", "")
        .replace("~~", "")
        .replace('`', "")
}

/// Truncate to at most `max` characters, appending ellipsis if truncated.
pub fn truncate_chars(text: &str, max: usize) -> (String, bool) {
    if max == 0 {
        return (String::new(), !text.is_empty());
    }
    let char_count = text.chars().count();
    if char_count <= max {
        return (text.to_string(), false);
    }
    let truncated: String = text.chars().take(max.saturating_sub(1)).collect();
    (format!("{truncated}\u{2026}"), true)
}

/// Truncate to at most `max` bytes on a char boundary, appending ellipsis.
pub fn truncate_bytes(text: &str, max: usize) -> (String, bool) {
    if text.len() <= max {
        return (text.to_string(), false);
    }
    if max < 4 {
        return (String::new(), true);
    }
    let boundary = max - 3; // room for ellipsis
    let end = text
        .char_indices()
        .take_while(|(i, _)| *i <= boundary)
        .last()
        .map(|(i, ch)| i + ch.len_utf8())
        .unwrap_or(0);
    let truncated = &text[..end];
    (format!("{truncated}\u{2026}"), true)
}

fn has_unsupported_elements(card: &PlannerCard, caps: &PlannerCapabilities) -> bool {
    if !caps.supports_buttons && !card.actions.is_empty() {
        return true;
    }
    if !caps.supports_images && !card.images.is_empty() {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_chars_no_op() {
        let (result, truncated) = truncate_chars("hello", 10);
        assert_eq!(result, "hello");
        assert!(!truncated);
    }

    #[test]
    fn truncate_chars_cuts() {
        let (result, truncated) = truncate_chars("hello world", 6);
        assert!(truncated);
        assert!(result.len() <= 10); // 5 chars + ellipsis
        assert!(result.ends_with('\u{2026}'));
    }

    #[test]
    fn truncate_bytes_no_op() {
        let (result, truncated) = truncate_bytes("hello", 100);
        assert_eq!(result, "hello");
        assert!(!truncated);
    }

    #[test]
    fn truncate_bytes_cuts() {
        let (result, truncated) = truncate_bytes("hello world this is long", 10);
        assert!(truncated);
        assert!(result.len() <= 13); // some room for ellipsis
    }

    #[test]
    fn sanitize_strips_html() {
        let caps = PlannerCapabilities {
            supports_html: false,
            ..Default::default()
        };
        let (result, changed) = sanitize_text("<b>bold</b> text", &caps);
        assert_eq!(result, "bold text");
        assert!(changed);
    }

    #[test]
    fn sanitize_strips_markdown() {
        let caps = PlannerCapabilities {
            supports_markdown: false,
            ..Default::default()
        };
        let (result, changed) = sanitize_text("**bold** and `code`", &caps);
        assert_eq!(result, "bold and code");
        assert!(changed);
    }

    #[test]
    fn sanitize_preserves_when_supported() {
        let caps = PlannerCapabilities {
            supports_html: true,
            supports_markdown: true,
            ..Default::default()
        };
        let input = "<b>bold</b> **md**";
        let (result, changed) = sanitize_text(input, &caps);
        assert_eq!(result, input);
        assert!(!changed);
    }

    #[test]
    fn select_tier_no_ac() {
        let caps = PlannerCapabilities::default();
        assert_eq!(select_tier(&caps, false), RenderTier::TierD);
    }

    #[test]
    fn select_tier_ac_supported_full() {
        let caps = PlannerCapabilities {
            supports_adaptive_cards: true,
            supports_buttons: true,
            supports_images: true,
            ..Default::default()
        };
        assert_eq!(select_tier(&caps, true), RenderTier::TierA);
    }

    #[test]
    fn select_tier_ac_supported_partial() {
        let caps = PlannerCapabilities {
            supports_adaptive_cards: true,
            supports_buttons: false,
            supports_images: true,
            ..Default::default()
        };
        assert_eq!(select_tier(&caps, true), RenderTier::TierB);
    }

    #[test]
    fn select_tier_ac_not_supported() {
        let caps = PlannerCapabilities {
            supports_adaptive_cards: false,
            ..Default::default()
        };
        assert_eq!(select_tier(&caps, true), RenderTier::TierD);
    }

    #[test]
    fn plan_render_text_only() {
        let card = PlannerCard {
            title: Some("Hello".into()),
            text: Some("World".into()),
            ..Default::default()
        };
        let caps = PlannerCapabilities::default();
        let plan = plan_render(&card, &caps, None);
        assert_eq!(plan.tier, RenderTier::TierD);
        assert!(plan.summary_text.is_some());
        assert!(plan.warnings.is_empty());
    }

    #[test]
    fn plan_render_with_ac_downsample() {
        let card = PlannerCard {
            title: Some("Title".into()),
            text: Some("Body".into()),
            ..Default::default()
        };
        let caps = PlannerCapabilities::default();
        let ac = serde_json::json!({"type": "AdaptiveCard"});
        let plan = plan_render(&card, &caps, Some(&ac));
        assert_eq!(plan.tier, RenderTier::TierD);
        assert!(
            plan.warnings
                .iter()
                .any(|w| w.code == "adaptive_card_downsampled")
        );
    }

    #[test]
    fn plan_render_with_ac_passthrough() {
        let card = PlannerCard {
            title: Some("Title".into()),
            text: Some("Body".into()),
            ..Default::default()
        };
        let caps = PlannerCapabilities {
            supports_adaptive_cards: true,
            supports_buttons: true,
            supports_images: true,
            ..Default::default()
        };
        let ac = serde_json::json!({"type": "AdaptiveCard"});
        let plan = plan_render(&card, &caps, Some(&ac));
        assert_eq!(plan.tier, RenderTier::TierA);
        assert!(
            plan.items
                .iter()
                .any(|i| matches!(i, RenderItem::AdaptiveCard(_)))
        );
    }
}
