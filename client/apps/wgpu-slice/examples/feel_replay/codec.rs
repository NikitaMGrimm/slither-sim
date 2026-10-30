//! A1's codec, applied to raw Rust numbers before any JSON text serialization.
use serde_json::{Value, json};

pub fn read(text: &str) -> Result<Value, String> {
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
}
