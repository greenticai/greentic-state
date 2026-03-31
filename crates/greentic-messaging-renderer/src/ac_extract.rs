//! Adaptive Card JSON -> PlannerCard extraction.
//!
//! Walks an AC v1.x body recursively and extracts text, title, actions, and images
//! into a `PlannerCard` suitable for the render planner.

use crate::planner::{PlannerAction, PlannerCard};
use serde_json::Value;

/// Extract a `PlannerCard` from an Adaptive Card JSON value.
pub fn extract_planner_card(ac: &Value) -> PlannerCard {
    let body = ac.get("body").and_then(Value::as_array);
    let ac_actions = ac.get("actions").and_then(Value::as_array);

    let mut title: Option<String> = None;
    let mut text_parts: Vec<String> = Vec::new();
    let mut actions: Vec<PlannerAction> = Vec::new();
    let mut images: Vec<String> = Vec::new();

    if let Some(body) = body {
        extract_body_elements(body, &mut title, &mut text_parts, &mut actions, &mut images);
    }

    if let Some(ac_actions) = ac_actions {
        extract_actions(ac_actions, &mut actions);
    }

    let text = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join("\n"))
    };

    PlannerCard {
        title,
        text,
        actions,
        images,
    }
}

fn extract_body_elements(
    elements: &[Value],
    title: &mut Option<String>,
    text_parts: &mut Vec<String>,
    actions: &mut Vec<PlannerAction>,
    images: &mut Vec<String>,
) {
    for element in elements {
        let element_type = element
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();

        match element_type {
            "TextBlock" => {
                if let Some(text) = element.get("text").and_then(Value::as_str) {
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    // First bold/large TextBlock becomes title
                    if title.is_none() && is_title_textblock(element) {
                        *title = Some(trimmed.to_string());
                    } else {
                        text_parts.push(trimmed.to_string());
                    }
                }
            }
            "RichTextBlock" => {
                let mut parts = Vec::new();
                if let Some(inlines) = element.get("inlines").and_then(Value::as_array) {
                    for inline in inlines {
                        if let Some(text) = inline.get("text").and_then(Value::as_str) {
                            if !text.is_empty() {
                                parts.push(text.to_string());
                            }
                        } else if let Some(text) = inline.as_str()
                            && !text.is_empty()
                        {
                            parts.push(text.to_string());
                        }
                    }
                }
                let joined = parts.join("").trim().to_string();
                if !joined.is_empty() {
                    text_parts.push(joined);
                }
            }
            "Image" => {
                if let Some(url) = element.get("url").and_then(Value::as_str) {
                    images.push(url.to_string());
                }
            }
            "ImageSet" => {
                if let Some(imgs) = element.get("images").and_then(Value::as_array) {
                    for img in imgs {
                        if let Some(url) = img.get("url").and_then(Value::as_str) {
                            images.push(url.to_string());
                        }
                    }
                }
            }
            "ActionSet" => {
                if let Some(action_list) = element.get("actions").and_then(Value::as_array) {
                    extract_actions(action_list, actions);
                }
            }
            "Container" => {
                if let Some(items) = element.get("items").and_then(Value::as_array) {
                    extract_body_elements(items, title, text_parts, actions, images);
                }
            }
            "ColumnSet" => {
                if let Some(columns) = element.get("columns").and_then(Value::as_array) {
                    for col in columns {
                        if let Some(items) = col.get("items").and_then(Value::as_array) {
                            extract_body_elements(items, title, text_parts, actions, images);
                        }
                    }
                }
            }
            "FactSet" => {
                if let Some(facts) = element.get("facts").and_then(Value::as_array) {
                    for fact in facts {
                        let fact_title = fact.get("title").and_then(Value::as_str).unwrap_or("");
                        let fact_value = fact.get("value").and_then(Value::as_str).unwrap_or("");
                        if !fact_title.is_empty() || !fact_value.is_empty() {
                            text_parts.push(format!("{}: {}", fact_title, fact_value));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_actions(action_list: &[Value], actions: &mut Vec<PlannerAction>) {
    for action in action_list {
        let action_type = action
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();

        let title = action
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        if title.is_empty() {
            continue;
        }

        let url = match action_type {
            "Action.OpenUrl" => action.get("url").and_then(Value::as_str).map(String::from),
            _ => None,
        };

        actions.push(PlannerAction { title, url });
    }
}

fn is_title_textblock(element: &Value) -> bool {
    // Check weight
    if let Some(weight) = element.get("weight").and_then(Value::as_str)
        && weight.eq_ignore_ascii_case("bolder")
    {
        return true;
    }
    // Check size
    if let Some(size) = element.get("size").and_then(Value::as_str) {
        match size.to_ascii_lowercase().as_str() {
            "large" | "extralarge" | "medium" => return true,
            _ => {}
        }
    }
    // Check style
    if let Some(style) = element.get("style").and_then(Value::as_str)
        && style.eq_ignore_ascii_case("heading")
    {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_simple_textblock() {
        let ac = json!({
            "type": "AdaptiveCard",
            "body": [
                {"type": "TextBlock", "text": "Hello World"}
            ]
        });
        let card = extract_planner_card(&ac);
        assert_eq!(card.text, Some("Hello World".to_string()));
        assert!(card.title.is_none());
    }

    #[test]
    fn extracts_title_from_bold_textblock() {
        let ac = json!({
            "type": "AdaptiveCard",
            "body": [
                {"type": "TextBlock", "text": "My Title", "weight": "Bolder"},
                {"type": "TextBlock", "text": "Body text"}
            ]
        });
        let card = extract_planner_card(&ac);
        assert_eq!(card.title, Some("My Title".to_string()));
        assert_eq!(card.text, Some("Body text".to_string()));
    }

    #[test]
    fn extracts_title_from_large_textblock() {
        let ac = json!({
            "type": "AdaptiveCard",
            "body": [
                {"type": "TextBlock", "text": "Big Title", "size": "Large"},
                {"type": "TextBlock", "text": "Normal text"}
            ]
        });
        let card = extract_planner_card(&ac);
        assert_eq!(card.title, Some("Big Title".to_string()));
    }

    #[test]
    fn extracts_actions() {
        let ac = json!({
            "type": "AdaptiveCard",
            "body": [],
            "actions": [
                {"type": "Action.OpenUrl", "title": "Visit", "url": "https://example.com"},
                {"type": "Action.Submit", "title": "Submit"}
            ]
        });
        let card = extract_planner_card(&ac);
        assert_eq!(card.actions.len(), 2);
        assert_eq!(card.actions[0].title, "Visit");
        assert_eq!(card.actions[0].url, Some("https://example.com".to_string()));
        assert_eq!(card.actions[1].title, "Submit");
        assert!(card.actions[1].url.is_none());
    }

    #[test]
    fn extracts_images() {
        let ac = json!({
            "type": "AdaptiveCard",
            "body": [
                {"type": "Image", "url": "https://example.com/img1.png"},
                {"type": "ImageSet", "images": [
                    {"type": "Image", "url": "https://example.com/img2.png"},
                    {"type": "Image", "url": "https://example.com/img3.png"}
                ]}
            ]
        });
        let card = extract_planner_card(&ac);
        assert_eq!(card.images.len(), 3);
    }

    #[test]
    fn handles_nested_container() {
        let ac = json!({
            "type": "AdaptiveCard",
            "body": [
                {
                    "type": "Container",
                    "items": [
                        {"type": "TextBlock", "text": "Inside container"}
                    ]
                }
            ]
        });
        let card = extract_planner_card(&ac);
        assert_eq!(card.text, Some("Inside container".to_string()));
    }

    #[test]
    fn handles_columnset() {
        let ac = json!({
            "type": "AdaptiveCard",
            "body": [
                {
                    "type": "ColumnSet",
                    "columns": [
                        {"items": [{"type": "TextBlock", "text": "Col 1"}]},
                        {"items": [{"type": "TextBlock", "text": "Col 2"}]}
                    ]
                }
            ]
        });
        let card = extract_planner_card(&ac);
        assert_eq!(card.text, Some("Col 1\nCol 2".to_string()));
    }

    #[test]
    fn handles_factset() {
        let ac = json!({
            "type": "AdaptiveCard",
            "body": [
                {
                    "type": "FactSet",
                    "facts": [
                        {"title": "Name", "value": "John"},
                        {"title": "Age", "value": "30"}
                    ]
                }
            ]
        });
        let card = extract_planner_card(&ac);
        assert!(card.text.as_ref().unwrap().contains("Name: John"));
        assert!(card.text.as_ref().unwrap().contains("Age: 30"));
    }

    #[test]
    fn handles_rich_text_block() {
        let ac = json!({
            "type": "AdaptiveCard",
            "body": [
                {
                    "type": "RichTextBlock",
                    "inlines": [
                        {"type": "TextRun", "text": "Rich "},
                        {"type": "TextRun", "text": "text"}
                    ]
                }
            ]
        });
        let card = extract_planner_card(&ac);
        assert_eq!(card.text, Some("Rich text".to_string()));
    }

    #[test]
    fn handles_actionset_in_body() {
        let ac = json!({
            "type": "AdaptiveCard",
            "body": [
                {
                    "type": "ActionSet",
                    "actions": [
                        {"type": "Action.OpenUrl", "title": "Click", "url": "https://example.com"}
                    ]
                }
            ]
        });
        let card = extract_planner_card(&ac);
        assert_eq!(card.actions.len(), 1);
        assert_eq!(card.actions[0].title, "Click");
    }

    #[test]
    fn handles_empty_card() {
        let ac = json!({"type": "AdaptiveCard"});
        let card = extract_planner_card(&ac);
        assert!(card.title.is_none());
        assert!(card.text.is_none());
        assert!(card.actions.is_empty());
        assert!(card.images.is_empty());
    }

    #[test]
    fn skips_empty_text_blocks() {
        let ac = json!({
            "type": "AdaptiveCard",
            "body": [
                {"type": "TextBlock", "text": ""},
                {"type": "TextBlock", "text": "   "},
                {"type": "TextBlock", "text": "Real text"}
            ]
        });
        let card = extract_planner_card(&ac);
        assert_eq!(card.text, Some("Real text".to_string()));
    }
}
