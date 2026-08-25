//! Rendering details to protojson.

use super::encode::{base64, format_duration};
use super::{Detail, TYPE_PREFIX};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

impl Detail {
    /// The `@type` value this detail renders with.
    pub fn type_url(&self) -> String {
        let name = match self {
            Detail::ErrorInfo(_) => "google.rpc.ErrorInfo",
            Detail::BadRequest(_) => "google.rpc.BadRequest",
            Detail::RetryInfo(_) => "google.rpc.RetryInfo",
            Detail::QuotaFailure(_) => "google.rpc.QuotaFailure",
            Detail::PreconditionFailure(_) => "google.rpc.PreconditionFailure",
            Detail::ResourceInfo(_) => "google.rpc.ResourceInfo",
            Detail::Help(_) => "google.rpc.Help",
            Detail::LocalizedMessage(_) => "google.rpc.LocalizedMessage",
            Detail::DebugInfo(_) => "google.rpc.DebugInfo",
            Detail::RequestInfo(_) => "google.rpc.RequestInfo",
            Detail::Unknown { type_url, .. } => return type_url.clone(),
        };
        format!("{TYPE_PREFIX}{name}")
    }

    /// Renders the detail as protojson, `@type` first.
    ///
    /// Default-valued fields are omitted, matching protojson and keeping an
    /// error body free of noise like `"owner": ""` that a reader has to skip
    /// past to find what actually went wrong.
    pub fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("@type".into(), Value::String(self.type_url()));
        self.write_fields(&mut obj);
        Value::Object(obj)
    }

    /// Writes the detail's own fields into a prepared object.
    fn write_fields(&self, obj: &mut Map<String, Value>) {
        match self {
            Detail::ErrorInfo(d) => {
                put(obj, "reason", json!(d.reason));
                put(obj, "domain", json!(d.domain));
                // BTreeMap so metadata order is stable across runs, which
                // matters for golden tests and for diffing two error bodies.
                let sorted: BTreeMap<_, _> = d.metadata.iter().collect();
                put(obj, "metadata", json!(sorted));
            }
            Detail::BadRequest(d) => {
                let items: Vec<Value> = d
                    .field_violations
                    .iter()
                    .map(|v| {
                        let mut m = Map::new();
                        m.insert("field".into(), json!(v.field));
                        m.insert("description".into(), json!(v.description));
                        put(&mut m, "reason", json!(v.reason));
                        Value::Object(m)
                    })
                    .collect();
                put(obj, "fieldViolations", Value::Array(items));
            }
            Detail::RetryInfo(d) => {
                if let Some(delay) = &d.retry_delay {
                    put(obj, "retryDelay", json!(format_duration(delay)));
                }
            }
            Detail::QuotaFailure(d) => {
                let items: Vec<Value> = d
                    .violations
                    .iter()
                    .map(|v| json!({ "subject": v.subject, "description": v.description }))
                    .collect();
                put(obj, "violations", Value::Array(items));
            }
            Detail::PreconditionFailure(d) => {
                let items: Vec<Value> = d
                    .violations
                    .iter()
                    .map(|v| {
                        json!({
                            "type": v.r#type,
                            "subject": v.subject,
                            "description": v.description,
                        })
                    })
                    .collect();
                put(obj, "violations", Value::Array(items));
            }
            Detail::ResourceInfo(d) => {
                put(obj, "resourceType", json!(d.resource_type));
                put(obj, "resourceName", json!(d.resource_name));
                put(obj, "owner", json!(d.owner));
                put(obj, "description", json!(d.description));
            }
            Detail::Help(d) => {
                let items: Vec<Value> = d
                    .links
                    .iter()
                    .map(|l| json!({ "description": l.description, "url": l.url }))
                    .collect();
                put(obj, "links", Value::Array(items));
            }
            Detail::LocalizedMessage(d) => {
                put(obj, "locale", json!(d.locale));
                put(obj, "message", json!(d.message));
            }
            Detail::DebugInfo(d) => {
                put(obj, "stackEntries", json!(d.stack_entries));
                put(obj, "detail", json!(d.detail));
            }
            Detail::RequestInfo(d) => {
                put(obj, "requestId", json!(d.request_id));
                put(obj, "servingData", json!(d.serving_data));
            }
            Detail::Unknown { value, .. } => {
                // Nothing can be said about the fields, but the payload is
                // preserved so a caller who knows the type can still read it.
                put(obj, "value", json!(base64(value)));
            }
        }
    }
}

/// Inserts a value unless it is at its protojson default.
fn put(obj: &mut Map<String, Value>, key: &str, value: Value) {
    let empty = match &value {
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        Value::Null => true,
        _ => false,
    };
    if !empty {
        obj.insert(key.into(), value);
    }
}
