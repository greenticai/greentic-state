//! Operator HTTP format compatibility helpers.
//!
//! The greentic-operator sends `HttpInV1` with query/headers as `Vec<(String,String)>`
//! tuples, but `greentic-types` expects query as `Option<String>` and headers as
//! `Vec<Header>`. These helpers bridge the format mismatch so every provider doesn't
//! need its own copy.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use greentic_types::messaging::universal_dto::{Header, HttpInV1, HttpOutV1};
use serde_json::{Value, json};

/// Parse the operator's `HttpInV1` format (query as `Vec<[k,v]>`, headers as tuples)
/// into the `greentic-types` `HttpInV1` format.
///
/// Falls back gracefully: if fields are already in the expected format they pass through.
pub fn parse_operator_http_in(input_json: &[u8]) -> Result<HttpInV1, String> {
    parse_operator_http_in_inner(input_json, false)
}

/// Same as [`parse_operator_http_in`] but also extracts the `config` field from the
/// operator payload. Used by providers that need config for ingress (e.g. Email).
pub fn parse_operator_http_in_with_config(input_json: &[u8]) -> Result<HttpInV1, String> {
    parse_operator_http_in_inner(input_json, true)
}

fn parse_operator_http_in_inner(
    input_json: &[u8],
    extract_config: bool,
) -> Result<HttpInV1, String> {
    let val: Value = serde_json::from_slice(input_json).map_err(|e| e.to_string())?;
    let method = val
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("POST")
        .to_string();
    let path = val
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("/")
        .to_string();
    let body_b64 = val
        .get("body_b64")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            // Operator sends raw bytes as `body: [u8]` (JSON array of numbers);
            // convert to base64 so downstream code can decode uniformly.
            if let Some(Value::Array(arr)) = val.get("body") {
                let bytes: Vec<u8> = arr
                    .iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                    .collect();
                if !bytes.is_empty() {
                    return STANDARD.encode(&bytes);
                }
            }
            String::new()
        });
    let query = match val.get("query") {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Array(arr)) => {
            let pairs: Vec<String> = arr
                .iter()
                .filter_map(|pair| {
                    if let Value::Array(kv) = pair {
                        let k = kv.first().and_then(|v| v.as_str())?;
                        let v = kv.get(1).and_then(|v| v.as_str()).unwrap_or("");
                        Some(format!("{k}={v}"))
                    } else {
                        None
                    }
                })
                .collect();
            if pairs.is_empty() {
                None
            } else {
                Some(pairs.join("&"))
            }
        }
        _ => None,
    };
    let headers = parse_headers(&val);
    let route_hint = val
        .get("route")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let binding_id = val
        .get("binding_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let config = if extract_config {
        val.get("config").cloned()
    } else {
        None
    };
    Ok(HttpInV1 {
        method,
        path,
        query,
        headers,
        body_b64,
        route_hint,
        binding_id,
        config,
    })
}

fn parse_headers(val: &Value) -> Vec<Header> {
    match val.get("headers") {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|item| {
                if let Value::Array(kv) = item {
                    let name = kv.first().and_then(|v| v.as_str())?.to_string();
                    let value = kv.get(1).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    Some(Header { name, value })
                } else if let Value::Object(map) = item {
                    let name = map.get("name").and_then(|v| v.as_str())?.to_string();
                    let value = map
                        .get("value")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Some(Header { name, value })
                } else {
                    None
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Serialize `HttpOutV1` with `"v":1` for operator v0.4.x compatibility.
///
/// Also transforms headers from `[{name, value}]` objects to `[[name, value]]` tuples
/// which the operator expects.
pub fn http_out_v1_bytes(out: &HttpOutV1) -> Vec<u8> {
    let mut val = serde_json::to_value(out).unwrap_or(Value::Null);
    if let Some(map) = val.as_object_mut() {
        map.insert("v".to_string(), json!(1));
        // Transform headers from [{name, value}] to [[name, value]] for operator compat
        if let Some(Value::Array(headers)) = map.get("headers") {
            let tuple_headers: Vec<Value> = headers
                .iter()
                .filter_map(|h| {
                    let name = h.get("name").and_then(|v| v.as_str())?;
                    let value = h.get("value").and_then(|v| v.as_str()).unwrap_or("");
                    Some(json!([name, value]))
                })
                .collect();
            map.insert("headers".to_string(), Value::Array(tuple_headers));
        }
    }
    serde_json::to_vec(&val).unwrap_or_default()
}

/// Build an error response in HttpOutV1 format.
pub fn http_out_error(status: u16, message: &str) -> Vec<u8> {
    let out = HttpOutV1 {
        status,
        headers: Vec::new(),
        body_b64: STANDARD.encode(message.as_bytes()),
        events: Vec::new(),
    };
    http_out_v1_bytes(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_operator_format_with_tuple_query() {
        let input = serde_json::to_vec(&json!({
            "method": "GET",
            "path": "/webhook",
            "body_b64": "",
            "query": [["hub.mode", "subscribe"], ["hub.challenge", "test123"]],
            "headers": [["content-type", "application/json"]],
        }))
        .unwrap();
        let req = parse_operator_http_in(&input).unwrap();
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/webhook");
        assert_eq!(
            req.query.as_deref(),
            Some("hub.mode=subscribe&hub.challenge=test123")
        );
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "content-type");
        assert!(req.config.is_none());
    }

    #[test]
    fn parse_operator_format_with_config() {
        let input = serde_json::to_vec(&json!({
            "method": "POST",
            "path": "/notifications",
            "body_b64": "",
            "query": [],
            "headers": [],
            "config": {"tenant_id": "abc"},
        }))
        .unwrap();
        let req = parse_operator_http_in_with_config(&input).unwrap();
        assert!(req.config.is_some());
        assert_eq!(req.config.unwrap()["tenant_id"], "abc");
    }

    #[test]
    fn parse_operator_format_without_config() {
        let input = serde_json::to_vec(&json!({
            "method": "POST",
            "path": "/notifications",
            "body_b64": "",
            "config": {"tenant_id": "abc"},
        }))
        .unwrap();
        let req = parse_operator_http_in(&input).unwrap();
        assert!(req.config.is_none());
    }

    #[test]
    fn parse_string_query_passthrough() {
        let input = serde_json::to_vec(&json!({
            "method": "GET",
            "path": "/webhook",
            "body_b64": "",
            "query": "foo=bar&baz=qux",
        }))
        .unwrap();
        let req = parse_operator_http_in(&input).unwrap();
        assert_eq!(req.query.as_deref(), Some("foo=bar&baz=qux"));
    }

    #[test]
    fn parse_object_headers() {
        let input = serde_json::to_vec(&json!({
            "method": "POST",
            "path": "/",
            "body_b64": "",
            "headers": [{"name": "Authorization", "value": "Bearer tok"}],
        }))
        .unwrap();
        let req = parse_operator_http_in(&input).unwrap();
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Authorization");
        assert_eq!(req.headers[0].value, "Bearer tok");
    }

    #[test]
    fn parse_operator_body_as_byte_array() {
        // The operator sends body as Vec<u8> (JSON array of numbers),
        // not as body_b64 string. Verify our parser handles this.
        let webhook = r#"{"update_id":123,"message":{"chat":{"id":999},"text":"hello"}}"#;
        let body_bytes: Vec<u8> = webhook.bytes().collect();
        let input = serde_json::to_vec(&json!({
            "method": "POST",
            "path": "/webhook",
            "body": body_bytes,
            "headers": [],
            "query": [],
        }))
        .unwrap();
        let req = parse_operator_http_in(&input).unwrap();
        assert!(
            !req.body_b64.is_empty(),
            "body_b64 should be populated from body array"
        );
        let decoded = STANDARD.decode(&req.body_b64).unwrap();
        let val: serde_json::Value = serde_json::from_slice(&decoded).unwrap();
        assert_eq!(val["update_id"], 123);
        assert_eq!(val["message"]["chat"]["id"], 999);
    }

    #[test]
    fn http_out_v1_bytes_injects_v_and_transforms_headers() {
        let out = HttpOutV1 {
            status: 200,
            headers: vec![Header {
                name: "Content-Type".into(),
                value: "application/json".into(),
            }],
            body_b64: STANDARD.encode(b"ok"),
            events: Vec::new(),
        };
        let bytes = http_out_v1_bytes(&out);
        let val: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["v"], 1);
        assert_eq!(val["headers"][0][0], "Content-Type");
        assert_eq!(val["headers"][0][1], "application/json");
    }

    #[test]
    fn http_out_error_returns_valid_json() {
        let bytes = http_out_error(400, "bad request");
        let val: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val["status"], 400);
        assert_eq!(val["v"], 1);
    }
}
