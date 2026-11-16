//! Borrowed structural preflight, before constructing an owned reply tree.
//! Byte/node/key/depth limits are decode-work bounds, not allocator/RSS claims.
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::value::RawValue;
use std::{collections::BTreeSet, fmt};

const MAX_DEPTH: usize = 32;
const MAX_OBJECT_FIELDS: usize = 128;

pub(super) fn check(bytes: &[u8], limit: usize) -> Result<(), String> {
    if bytes.len() > limit {
        return Err("response exceeds operation frame allowance".into());
    }
    let mut quoted = false;
    let mut escaped = false;
    for &byte in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else if byte == b'"' {
            quoted = true;
        } else if byte.is_ascii_whitespace() {
            return Err("prepared frame must be compact JSON".into());
        }
    }
    let raw: &RawValue = serde_json::from_slice(bytes).map_err(|_| "invalid JSON")?;
    // Each JSON node consumes at least one input byte. This is explicit and
    // conservative; no reserve from an untrusted sequence length/size hint.
    let mut nodes = bytes.len();
    inspect(raw, 0, &mut nodes)
}

fn inspect(raw: &RawValue, depth: usize, nodes: &mut usize) -> Result<(), String> {
    if depth > MAX_DEPTH || *nodes == 0 {
        return Err("prepared frame structure exceeds decode allowance".into());
    }
    *nodes -= 1;
    let text = raw.get();
    match text.as_bytes()[0] {
        b'{' | b'[' => {
            let mut decoder = serde_json::Deserializer::from_str(text);
            Nested {
                depth: depth + 1,
                nodes,
            }
            .deserialize(&mut decoder)
            .map_err(|_| "invalid or excessive prepared structure".into())
        }
        b'"' | b't' | b'f' | b'n' => Ok(()), // RawValue already validated JSON syntax.
        _ => {
            let number: serde_json::Number =
                serde_json::from_str(text).map_err(|_| "invalid finite JSON number")?;
            if number.to_string() != text {
                return Err("noncanonical prepared number spelling".into());
            }
            Ok(())
        }
    }
}

struct Nested<'a> {
    depth: usize,
    nodes: &'a mut usize,
}
impl<'de> DeserializeSeed<'de> for Nested<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Nested<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded prepared object or array")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        while let Some(raw) = a.next_element::<&RawValue>()? {
            inspect(raw, self.depth, self.nodes).map_err(serde::de::Error::custom)?;
        }
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = a.next_key::<String>()? {
            if keys.len() == MAX_OBJECT_FIELDS || !keys.insert(key) {
                return Err(serde::de::Error::custom(
                    "duplicate or excessive prepared object fields",
                ));
            }
            let raw = a.next_value::<&RawValue>()?;
            inspect(raw, self.depth, self.nodes).map_err(serde::de::Error::custom)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prepared_raw_admission_precedes_owned_values_and_preserves_encoding_bound() {
        for raw in [
            r#"{"float":0.020643016,"text":"\\\"\n\u0000"}"#,
            r#"{"integer":18446744073709551615,"negative":-0.0}"#,
        ] {
            check(raw.as_bytes(), raw.len()).unwrap();
            let value: serde_json::Value = serde_json::from_str(raw).unwrap();
            let compact = serde_json::to_vec(&value).unwrap();
            assert!(compact.len() <= raw.len());
            let escaped = serde_json::to_vec(std::str::from_utf8(&compact).unwrap()).unwrap();
            assert!(escaped.len() <= 2 * raw.len() + 2);
            assert!(check(raw.as_bytes(), raw.len() - 1).is_err());
        }
        for raw in [
            r#"{"x":1,"x":2}"#,
            r#"{"x":1e1}"#,
            r#"{"x":1e999}"#,
            "{\"x\": 1}",
        ] {
            assert!(check(raw.as_bytes(), 1000).is_err(), "{raw}");
        }
        let nested = format!(
            "{}0{}",
            "[".repeat(MAX_DEPTH + 1),
            "]".repeat(MAX_DEPTH + 1)
        );
        assert!(check(nested.as_bytes(), 1000).is_err());
    }
}
