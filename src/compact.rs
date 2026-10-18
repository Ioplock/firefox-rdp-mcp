//! Token trimming for LLM-facing responses.
//!
//! Real-world pages (e.g. Avito) ship hashed CSS-module class names,
//! session-long `context=`/`utm_*` URL blobs and multi-KB excerpts on every
//! action. All of that is noise for an agent but costs context on every
//! call. This module strips it MCP-side from the already-structured
//! `ff-rdp` JSON — never by regexing text output.

use serde_json::Value;

/// Recursively strip DOM noise in place:
/// - delete `attrs.class` (hashed CSS modules, zero agent value)
/// - preserve complete URLs
/// - drop `attrs` objects left empty
pub fn strip_dom(v: &mut Value) {
    match v {
        Value::Object(map) => {
            if let Some(attrs) = map.get_mut("attrs") {
                if let Some(attrs_map) = attrs.as_object_mut() {
                    attrs_map.remove("class");
                    if attrs_map.is_empty() {
                        map.remove("attrs");
                    }
                }
            }
            for child in map.values_mut() {
                strip_dom(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_dom(item);
            }
        }
        _ => {}
    }
}

/// Browser context identities survive reconnects; actor strings do not.
pub fn tab_contexts(tabs: &Value) -> Vec<u64> {
    tabs["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t["browsingContextID"].as_u64())
        .collect()
}

/// Bound the serialized JSON without ever cutting a ref or a working URL.
/// Oversized scalar values are replaced with explicit omission markers.
pub fn bound_response(value: &mut Value, budget: usize) {
    fn size(v: &Value) -> usize {
        serde_json::to_vec(v).map(|s| s.len()).unwrap_or(usize::MAX)
    }
    fn shrink(v: &mut Value) -> bool {
        match v {
            Value::Array(a) if !a.is_empty() => {
                a.truncate(a.len() / 2);
                true
            }
            Value::Object(o) => {
                let key = o
                    .iter()
                    .filter(|(k, _)| {
                        !["ref", "url", "href", "tab", "requested_url"].contains(&k.as_str())
                    })
                    .filter(|(_, v)| {
                        matches!(v,Value::Array(a) if !a.is_empty())
                            || matches!(v, Value::Object(_))
                            || matches!(v,Value::String(s) if s.len()>100)
                    })
                    .max_by_key(|(_, v)| size(v))
                    .map(|(k, _)| k.clone());
                if let Some(k) = key {
                    if shrink(o.get_mut(&k).unwrap()) {
                        return true;
                    }
                    o.remove(&k);
                    return true;
                }
                false
            }
            Value::String(s) if s.len() > 100 => {
                *s = "[omitted: response budget; narrow query or use pagination]".into();
                true
            }
            _ => false,
        }
    }
    if size(value) <= budget {
        return;
    }
    let original = size(value);
    if let Some(o) = value.as_object_mut() {
        o.insert("response_note".into(),serde_json::json!({"truncated":true,"original_bytes":original,"hint":"narrow query or paginate"}));
    }
    while size(value) > budget {
        if !shrink(value) {
            *value = serde_json::json!({"response_note":{"truncated":true,"original_bytes":original,"hint":"response omitted; narrow query"}});
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn actors_change_but_contexts_do_not() {
        let a = json!({"results":[{"actor":"conn1.tab1","browsingContextID":3}]});
        let b = json!({"results":[{"actor":"conn2.tab1","browsingContextID":3}]});
        assert_eq!(tab_contexts(&a), tab_contexts(&b));
    }
    #[test]
    fn budget_is_valid_json_and_preserves_retained_urls_and_refs() {
        let link = format!("https://example.com/{}", "x".repeat(300));
        let mut data = json!({"results":{"interactive":(0..100).map(|i|json!({"ref":format!("e{i}"),"href":link,"name":"Привет"})).collect::<Vec<_>>()}});
        bound_response(&mut data, 2000);
        assert!(serde_json::to_vec(&data).unwrap().len() <= 2000);
        assert_eq!(data["response_note"]["truncated"], true);
        for row in data["results"]["interactive"].as_array().unwrap() {
            assert_eq!(row["href"], link);
            assert!(row["ref"].is_string());
        }
    }
}
