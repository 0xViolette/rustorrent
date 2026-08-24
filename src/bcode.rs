use std::{borrow::Borrow, fmt};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BencodeParseError {
    #[error("unexpected end of input")]
    UnexpectedEof,

    #[error("missing delimiter '{delimiter}'")]
    MissingDelimiter { delimiter: char },

    #[error("invalid integer: {reason}")]
    InvalidInteger { reason: String },

    #[error("invalid UTF-8: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),

    #[error("byte string data too short")]
    ShortInput,

    #[error("unconsumed data after parsing")]
    TrailingData,

    #[error("expected {expected}, got {actual}")]
    TypeMismatch {
        expected: &'static str,
        actual: &'static str,
    },

    #[error("dict key is not a byte string")]
    InvalidDictKey,

    #[error("dict keys are not sorted")]
    UnsortedKeys,

    #[error("unknown bencode type: '{0}'")]
    UnknownType(char),

    #[error("failed to parse list element")]
    InvalidListElement,

    #[error("failed to parse dict value")]
    InvalidDictValue,
}

#[derive(Clone, PartialEq, Hash, Eq, PartialOrd, Ord)]
pub struct ByteString(Vec<u8>);

impl ByteString {
    pub fn from_u8(input: &[u8]) -> Self {
        ByteString(input.to_vec())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn from_string(input: &str) -> Self {
        ByteString(input.as_bytes().to_vec())
    }

    pub fn to_string(&self) -> String {
        String::from_utf8_lossy(&self.0).to_string()
    }
}

impl std::ops::Deref for ByteString {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for ByteString {
    fn borrow(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap()
    }
}

impl Borrow<[u8]> for ByteString {
    fn borrow(&self) -> &[u8] {
        &self.0
    }
}

impl IntoIterator for ByteString {
    type Item = u8;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a ByteString {
    type Item = &'a u8;
    type IntoIter = std::slice::Iter<'a, u8>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[derive(Clone)]
pub enum BencodeValue {
    Integer(i64),
    String(ByteString),
    List(Vec<BencodeValue>),
    Dict(std::collections::BTreeMap<ByteString, BencodeValue>),
}

impl fmt::Debug for BencodeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.convert_to_serde())
    }
}

impl BencodeValue {
    fn convert_to_serde(&self) -> serde_json::Value {
        match self {
            Self::Integer(x) => (*x).into(),
            Self::String(s) => s.to_string().into(),
            Self::List(v) => {
                serde_json::Value::Array(v.iter().map(Self::convert_to_serde).collect())
            }
            Self::Dict(kv) => {
                let mut result = serde_json::Map::new();
                for (k, v) in kv {
                    let key = k.to_string();
                    result.insert(key, Self::convert_to_serde(v));
                }
                serde_json::Value::Object(result)
            }
        }
    }

    pub fn as_integer(&self) -> Result<&i64, BencodeParseError> {
        if let BencodeValue::Integer(x) = self {
            Ok(x)
        } else {
            Err(BencodeParseError::TypeMismatch {
                expected: "integer",
                actual: self.type_name(),
            })
        }
    }

    pub fn as_byte_string(&self) -> Result<&ByteString, BencodeParseError> {
        if let BencodeValue::String(s) = self {
            Ok(s)
        } else {
            Err(BencodeParseError::TypeMismatch {
                expected: "byte string",
                actual: self.type_name(),
            })
        }
    }

    #[allow(dead_code)]
    pub fn as_vec(&self) -> Result<&Vec<Self>, BencodeParseError> {
        if let BencodeValue::List(v) = self {
            Ok(v)
        } else {
            Err(BencodeParseError::TypeMismatch {
                expected: "list",
                actual: self.type_name(),
            })
        }
    }

    pub fn as_dict(
        &self,
    ) -> Result<&std::collections::BTreeMap<ByteString, Self>, BencodeParseError> {
        if let BencodeValue::Dict(dict) = self {
            Ok(dict)
        } else {
            Err(BencodeParseError::TypeMismatch {
                expected: "dict",
                actual: self.type_name(),
            })
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            BencodeValue::Integer(_) => "integer",
            BencodeValue::String(_) => "byte string",
            BencodeValue::List(_) => "list",
            BencodeValue::Dict(_) => "dict",
        }
    }
}

fn parse_integer(input: &[u8]) -> Option<i64> {
    match input {
        b"0" => Some(0),
        s => {
            let (is_negative, digits) = s
                .strip_prefix(b"-")
                .map(|s| (true, s))
                .unwrap_or((false, s));

            if digits.is_empty() || digits.starts_with(b"0") {
                None
            } else {
                str::from_utf8(digits)
                    .ok()?
                    .parse::<i64>()
                    .map(|n| if is_negative { -n } else { n })
                    .ok()
            }
        }
    }
}

pub fn parse(input: &[u8]) -> Result<BencodeValue, BencodeParseError> {
    parse_value(input).and_then(|(value, remainder)| {
        remainder
            .is_empty()
            .then_some(value)
            .ok_or(BencodeParseError::TrailingData)
    })
}

fn parse_value(input: &[u8]) -> Result<(BencodeValue, &[u8]), BencodeParseError> {
    match input
        .iter()
        .next()
        .ok_or(BencodeParseError::UnexpectedEof)?
    {
        b'0'..=b'9' => {
            let colon_idx = input
                .iter()
                .position(|&c| c == b':')
                .ok_or(BencodeParseError::MissingDelimiter { delimiter: ':' })?;

            let len = std::str::from_utf8(&input[..colon_idx])?
                .parse::<usize>()
                .map_err(|e| BencodeParseError::InvalidInteger {
                    reason: e.to_string(),
                })?;

            let (byte_string, rest) = (
                input
                    .get(colon_idx + 1..colon_idx + 1 + len)
                    .ok_or(BencodeParseError::ShortInput)?,
                input
                    .get(colon_idx + 1 + len..)
                    .ok_or(BencodeParseError::UnexpectedEof)?,
            );

            Ok((BencodeValue::String(ByteString::from_u8(byte_string)), rest))
        }

        b'i' => {
            let end_idx = input
                .iter()
                .position(|&c| c == b'e')
                .ok_or(BencodeParseError::MissingDelimiter { delimiter: 'e' })?;
            let body = &input[1..end_idx];

            let rem = input
                .get(end_idx + 1..)
                .ok_or(BencodeParseError::UnexpectedEof)?;

            parse_integer(body)
                .map(|x| (BencodeValue::Integer(x), rem))
                .ok_or_else(|| BencodeParseError::InvalidInteger {
                    reason: format!("invalid integer body: {}", String::from_utf8_lossy(body)),
                })
        }

        b'l' => {
            let mut v: Vec<BencodeValue> = vec![];
            let mut input = input.strip_prefix(b"l").unwrap();

            if input.starts_with(b"e") {
                input = input.strip_prefix(b"e").unwrap();
                return Ok((BencodeValue::List(v), input));
            }

            let rest;

            loop {
                if let Ok((val, rem)) = parse_value(input) {
                    input = rem;
                    v.push(val);

                    if input.starts_with(b"e") {
                        rest = input.strip_prefix(b"e").unwrap();
                        break;
                    }
                } else {
                    return Err(BencodeParseError::InvalidListElement);
                }
            }

            Ok((BencodeValue::List(v), rest))
        }

        b'd' => {
            let mut dict = std::collections::BTreeMap::new();

            let mut input = input.strip_prefix(b"d").unwrap();

            if input.starts_with(b"e") {
                input = input.strip_prefix(b"e").unwrap();
                return Ok((BencodeValue::Dict(dict), input));
            }

            let rest;

            loop {
                if let Ok((BencodeValue::String(k), rem)) = parse_value(input) {
                    input = rem;

                    if let Ok((v, rem)) = parse_value(input) {
                        input = rem;

                        dict.insert(k, v);

                        if input.starts_with(b"e") {
                            rest = input.strip_prefix(b"e").unwrap();
                            break;
                        }
                    } else {
                        return Err(BencodeParseError::InvalidDictValue);
                    }
                } else {
                    return Err(BencodeParseError::InvalidDictKey);
                }
            }

            Ok((BencodeValue::Dict(dict), rest))
        }

        other => Err(BencodeParseError::UnknownType(*other as char)),
    }
}

pub fn encode(val: &BencodeValue) -> Vec<u8> {
    match val {
        BencodeValue::Integer(x) => format!("i{x}e").into_bytes(),
        BencodeValue::String(s) => {
            let mut v = format!("{}:", s.len()).into_bytes();
            v.extend(s);
            v
        }
        BencodeValue::List(v) => {
            let mut result: Vec<u8> = vec![];
            result.push(b'l');
            for x in v.iter() {
                result.extend_from_slice(&encode(x));
            }
            result.push(b'e');
            result
        }
        BencodeValue::Dict(d) => {
            let mut result: Vec<u8> = vec![];
            result.push(b'd');
            for (k, v) in d {
                result.extend_from_slice(&encode(&BencodeValue::String(k.clone())));
                result.extend_from_slice(&encode(v));
            }
            result.push(b'e');
            result
        }
    }
}
