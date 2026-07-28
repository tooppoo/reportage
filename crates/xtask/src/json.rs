//! Order-preserving JSON document representation.
//!
//! Generated public schemas must keep their internal source schema's object member order at
//! every level, so the two files stay diffable against each other (see
//! docs/adr/20260727T151234Z_json-schema-artifact-generation.md). `serde_json::Value` cannot
//! carry that order: its `Map` is a `BTreeMap` unless the `preserve_order` feature is enabled,
//! and enabling that feature here would unify onto every workspace crate that links
//! `serde_json` and silently change the reportage CLI's own JSON output ordering.
//!
//! [`JsonValue`] therefore stores object members in a `Vec` and drives serde directly. serde's
//! `MapAccess` yields members in document order regardless of how `serde_json::Map` is
//! configured, so parsing through this type preserves order without touching the feature.

use std::fmt;

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

/// A parsed JSON document that remembers the order its object members were written in.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<JsonValue>),
    /// Members in source order. Duplicate keys are kept as written rather than collapsed,
    /// because the generator's contract is to preserve everything it does not explicitly strip.
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    /// Looks up a direct object member by key.
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(members) => members
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// The member keys of an object, in source order; empty for every other kind of value.
    pub fn keys(&self) -> Vec<&str> {
        match self {
            JsonValue::Object(members) => members.iter().map(|(name, _)| name.as_str()).collect(),
            _ => Vec::new(),
        }
    }
}

/// Parses a JSON document, preserving object member order.
pub fn parse(text: &str) -> Result<JsonValue, serde_json::Error> {
    serde_json::from_str(text)
}

/// Renders a JSON document as the repository's deterministic schema artifact text: two-space
/// indentation, LF line endings, and exactly one trailing newline.
///
/// The renderer never sorts or otherwise reorders members; ordering is entirely inherited from
/// the [`JsonValue`] it is given.
pub fn render(value: &JsonValue) -> String {
    let mut text = serde_json::to_string_pretty(value)
        .expect("JsonValue serialization is infallible for parsed documents");
    text.push('\n');
    text
}

impl Serialize for JsonValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            JsonValue::Null => serializer.serialize_unit(),
            JsonValue::Bool(value) => serializer.serialize_bool(*value),
            JsonValue::Number(value) => value.serialize(serializer),
            JsonValue::String(value) => serializer.serialize_str(value),
            JsonValue::Array(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            JsonValue::Object(members) => {
                let mut map = serializer.serialize_map(Some(members.len()))?;
                for (key, value) in members {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for JsonValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(JsonValueVisitor)
    }
}

struct JsonValueVisitor;

impl<'de> Visitor<'de> for JsonValueVisitor {
    type Value = JsonValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("any JSON value")
    }

    fn visit_unit<E>(self) -> Result<JsonValue, E> {
        Ok(JsonValue::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<JsonValue, E> {
        Ok(JsonValue::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<JsonValue, E> {
        Ok(JsonValue::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<JsonValue, E> {
        Ok(JsonValue::Number(value.into()))
    }

    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<JsonValue, E> {
        serde_json::Number::from_f64(value)
            .map(JsonValue::Number)
            .ok_or_else(|| E::custom("JSON numbers must be finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<JsonValue, E> {
        Ok(JsonValue::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<JsonValue, E> {
        Ok(JsonValue::String(value))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<JsonValue, A::Error> {
        let mut items = Vec::with_capacity(access.size_hint().unwrap_or(0));
        while let Some(item) = access.next_element()? {
            items.push(item);
        }
        Ok(JsonValue::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<JsonValue, A::Error> {
        let mut members = Vec::with_capacity(access.size_hint().unwrap_or(0));
        while let Some((key, value)) = access.next_entry()? {
            members.push((key, value));
        }
        Ok(JsonValue::Object(members))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_object_member_order() {
        let value = parse(r#"{"z": 1, "a": 2, "m": {"y": 3, "b": 4}}"#).expect("valid JSON");

        assert_eq!(value.keys(), vec!["z", "a", "m"]);
        assert_eq!(
            value.get("m").expect("nested object").keys(),
            vec!["y", "b"]
        );
    }

    #[test]
    fn renders_two_space_indentation_with_one_trailing_newline() {
        let value = parse(r#"{"b":[1,2],"a":{}}"#).expect("valid JSON");

        assert_eq!(
            render(&value),
            "{\n  \"b\": [\n    1,\n    2\n  ],\n  \"a\": {}\n}\n"
        );
    }

    #[test]
    fn round_trips_every_scalar_kind() {
        let source = "{\n  \"null\": null,\n  \"true\": true,\n  \"false\": false,\n  \"int\": -3,\n  \"big\": 9007199254740993,\n  \"float\": 1.5,\n  \"text\": \"é\\n\",\n  \"empty\": [],\n  \"nested\": [\n    {\n      \"a\": 1\n    }\n  ]\n}\n";

        let value = parse(source).expect("valid JSON");

        assert_eq!(render(&value), source);
    }

    #[test]
    fn keeps_duplicate_object_members() {
        let value = parse(r#"{"a": 1, "a": 2}"#).expect("valid JSON");

        assert_eq!(value.keys(), vec!["a", "a"]);
    }

    #[test]
    fn get_and_keys_are_empty_for_non_objects() {
        let value = parse("[1]").expect("valid JSON");

        assert_eq!(value.get("a"), None);
        assert!(value.keys().is_empty());
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(parse("{\"a\": }").is_err());
    }
}
