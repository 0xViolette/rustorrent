use std::fmt;
use thiserror::Error;

pub type ByteString = Vec<u8>;

#[derive(Debug, Error)]
pub enum BencodeParseError {
    #[error("unexpected end of input")]
    UnexpectedEof,

    #[error("missing delimiter '{delimiter}'")]
    MissingDelimiter { delimiter: char },

    #[error("invalid integer [{reason}]")]
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

#[derive(Clone)]
pub enum BencodeValue {
    Integer(i64),
    ByteString(ByteString),
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
            Self::ByteString(s) => {
                serde_json::Value::String(String::from_utf8_lossy(s).into_owned())
            }
            Self::List(v) => {
                serde_json::Value::Array(v.iter().map(Self::convert_to_serde).collect())
            }
            Self::Dict(kv) => {
                let mut result = serde_json::Map::new();
                for (k, v) in kv {
                    result.insert(
                        String::from_utf8_lossy(k).into_owned(),
                        Self::convert_to_serde(v),
                    );
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

    pub fn as_bytes(&self) -> Result<&[u8], BencodeParseError> {
        if let BencodeValue::ByteString(s) = self {
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
            BencodeValue::ByteString(_) => "byte string",
            BencodeValue::List(_) => "list",
            BencodeValue::Dict(_) => "dict",
        }
    }
}
