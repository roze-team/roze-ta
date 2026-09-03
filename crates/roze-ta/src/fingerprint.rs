//! Versioned canonical content hashes. See docs/contracts/engine-v2.md.
use crate::error::{ErrorCode, TaError};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const HASH_PROTOCOL: &str = "roze-ta-canonical-v1";

pub(crate) fn digest(value: &impl Serialize) -> Result<String, TaError> {
    let value = serde_json::to_value(value)
        .map_err(|_| TaError::new(ErrorCode::EncodingFailed, "content cannot be encoded"))?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(HASH_PROTOCOL.as_bytes());
    bytes.push(0);
    encode(&value, &mut bytes);
    Ok(bytes_digest(&bytes))
}
pub(crate) fn bytes_digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}
pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(DIGITS[(byte >> 4) as usize] as char);
        text.push(DIGITS[(byte & 15) as usize] as char);
    }
    text
}
pub(crate) fn unhex(text: &str) -> Option<Vec<u8>> {
    fn digit(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            _ => None,
        }
    }
    if !text.len().is_multiple_of(2) {
        return None;
    }
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| Some(digit(pair[0])? * 16 + digit(pair[1])?))
        .collect()
}
fn sized(tag: u8, bytes: &[u8], out: &mut Vec<u8>) {
    out.push(tag);
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}
fn encode(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Null => out.push(b'n'),
        Value::Bool(v) => out.push(if *v { b't' } else { b'f' }),
        Value::String(v) => sized(b's', v.as_bytes(), out),
        Value::Number(v) if v.is_f64() => {
            let v = v.as_f64().expect("JSON floating number");
            out.push(b'd');
            out.extend_from_slice(&(if v == 0.0 { 0 } else { v.to_bits() }).to_be_bytes());
        }
        Value::Number(v) => sized(b'i', v.to_string().as_bytes(), out),
        Value::Array(v) => {
            out.push(b'a');
            out.extend_from_slice(&(v.len() as u64).to_be_bytes());
            for item in v {
                encode(item, out);
            }
        }
        Value::Object(v) => {
            out.push(b'o');
            out.extend_from_slice(&(v.len() as u64).to_be_bytes());
            let mut keys: Vec<_> = v.keys().collect();
            keys.sort();
            for key in keys {
                sized(b's', key.as_bytes(), out);
                encode(&v[key], out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_vectors_match_independently_encoded_bytes() {
        // Independently assembled protocol bytes hashed with .NET SHA-256.
        assert_eq!(
            digest(&Value::Null).unwrap(),
            "7210b59b67acd9f3597e415599517dc54241d58480022abeb804fbfd45542641"
        );
        assert_eq!(
            digest(&0.0_f64).unwrap(),
            "fd9cf5a6d78e9b96fe781e0cd832d144fd4c8a1f52a8fe3d2b92a988b22cfdac"
        );
        assert_eq!(digest(&-0.0_f64).unwrap(), digest(&0.0_f64).unwrap());
        assert_eq!(
            digest(&"TA").unwrap(),
            "1b79f2038e3d3cdf3128f6684ffba9dfd8f9fab7801a1e1792c0c64eed388652"
        );
        let a: Value = serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"a":1,"b":2}"#).unwrap();
        assert_eq!(digest(&a).unwrap(), digest(&b).unwrap());
    }
}
