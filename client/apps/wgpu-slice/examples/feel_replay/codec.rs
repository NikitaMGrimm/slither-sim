//! A1's codec, applied to raw Rust numbers before any JSON text serialization.
//!
//! Native subset: object keys must be ASCII and must not be canonical array
//! indices (decimal 0..=4294967294 without leading zeros). This avoids JS integer
//! key enumeration and UTF-16 versus Rust string ordering differences. Reading
//! and writing reject unsupported keys recursively rather than reorder them.
//! String values may contain any Unicode scalar values. Lone UTF-16 surrogates
//! are rejected by serde_json on read and cannot inhabit Rust String on write.
use serde_json::{Value, json};

fn check_key(key: &str) -> Result<(), String> {
    if !key.is_ascii() {
        return Err("Native feel codec requires ASCII object keys".into());
    }
    if key
        .parse::<u32>()
        .is_ok_and(|index| index != u32::MAX && index.to_string() == key)
    {
        return Err("Native feel codec rejects canonical array-index object keys".into());
    }
    Ok(())
}

pub fn read(text: &str) -> Result<Value, String> {
    // A1's JS reader also admits lone UTF-16 surrogates; this native subset does not.
    let wrapper: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if wrapper.as_object().is_none_or(|o| o.len() != 2)
        || wrapper["encoding"] != "feel-f64-v1"
        || wrapper.get("value").is_none()
    {
        return Err("Expected feel-f64-v1 wrapper".into());
    }
    decode(wrapper["value"].clone())
}

fn decode(value: Value) -> Result<Value, String> {
    match value {
        Value::Number(_) => Err("Bare JSON number".into()),
        Value::Array(a) => a.into_iter().map(decode).collect(),
        Value::Object(mut o) => {
            for key in o.keys() {
                check_key(key)?;
            }
            if let Some(bits) = o.remove("$f64") {
                let s = bits.as_str().ok_or("Invalid f64 hex")?;
                if !o.is_empty()
                    || s.len() != 16
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err("Invalid f64 hex".into());
                }
                let n = f64::from_bits(u64::from_str_radix(s, 16).map_err(|e| e.to_string())?);
                if !n.is_finite() {
                    return Err("Non-finite f64".into());
                }
                Ok(json!(n))
            } else {
                o.into_iter().map(|(k, v)| Ok((k, decode(v)?))).collect()
            }
        }
        other => Ok(other),
    }
}

pub fn write(value: Value) -> Result<String, String> {
    fn encode(v: Value) -> Result<Value, String> {
        match v {
            Value::Number(n) => {
                let n = n
                    .as_f64()
                    .filter(|n| n.is_finite())
                    .ok_or("Finite f64 required")?;
                Ok(json!({"$f64": format!("{:016x}", n.to_bits())}))
            }
            Value::Array(a) => a.into_iter().map(encode).collect(),
            Value::Object(o) => {
                for key in o.keys() {
                    check_key(key)?;
                }
                if o.contains_key("$f64") {
                    return Err("Reserved $f64 key".into());
                }
                o.into_iter().map(|(k, v)| Ok((k, encode(v)?))).collect()
            }
            other => Ok(other),
        }
    }
    // A1 fixes wrapper order separately from the sorted payload keys.
    let payload = serde_json::to_string_pretty(&encode(value)?).map_err(|e| e.to_string())?;
    let payload = payload.replace('\n', "\n  ");
    Ok(format!(
        "{{\n  \"encoding\": \"feel-f64-v1\",\n  \"value\": {payload}\n}}\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_bits_and_rejects_invalid_documents() {
        let values = [0., -0., f64::from_bits(1), f64::MAX, 1.0000000000000002];
        let parsed = read(&write(json!(values)).unwrap()).unwrap();
        for (a, b) in values.iter().zip(parsed.as_array().unwrap()) {
            assert_eq!(a.to_bits(), b.as_f64().unwrap().to_bits());
        }
        for text in [
            r#"{"encoding":"feel-f64-v1","value":1}"#,
            r#"{"encoding":"feel-f64-v1","value":{"$f64":"7ff0000000000000"}}"#,
            r#"{"encoding":"feel-f64-v1","value":{"$f64":"8000000000000000","extra":true}}"#,
        ] {
            assert!(read(text).is_err());
        }
    }

    fn rejects_key_on_read_and_write(key: &str, message: &str) {
        // Nest the key so neither direction can validate only the root object.
        let value = json!({"nested": [{(key): true}]});
        let document = json!({"encoding":"feel-f64-v1", "value":value}).to_string();
        assert!(read(&document).unwrap_err().contains(message));
        assert!(write(value).unwrap_err().contains(message));
    }

    #[test]
    fn rejects_canonical_array_index_keys_on_read_and_write() {
        for key in ["0", "1", "2", "10", "4294967294"] {
            rejects_key_on_read_and_write(key, "array-index");
        }
    }

    #[test]
    fn rejects_non_ascii_keys_on_read_and_write() {
        for key in ["é", "\u{e000}", "\u{10000}"] {
            rejects_key_on_read_and_write(key, "ASCII");
        }
    }

    #[test]
    fn rejects_unrepresentable_surrogates_on_read() {
        for value in [
            r#""\ud800""#,
            r#""\udc00""#,
            r#"["valid", "\ud800"]"#,
            r#"{"\ud800":true}"#,
        ] {
            let document = format!(r#"{{"encoding":"feel-f64-v1","value":{value}}}"#);
            assert!(read(&document).is_err());
        }
    }

    #[test]
    fn accepts_non_index_ascii_keys_and_unicode_scalar_values() {
        for key in [
            "",
            "00",
            "01",
            "-0",
            "+1",
            "1.0",
            "4294967295",
            "4294967296",
        ] {
            let value = json!({(key): "é\u{e000}\u{10000}"});
            assert_eq!(read(&write(value.clone()).unwrap()).unwrap(), value);
        }
        assert_eq!(
            read(r#"{"encoding":"feel-f64-v1","value":"\ud800\udc00"}"#).unwrap(),
            json!("\u{10000}")
        );
    }
}
