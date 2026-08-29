use thiserror::Error;

pub type ByteString = Vec<u8>;

#[derive(Clone)]
pub enum BencodeValue {
    Integer(i64),
    ByteString(ByteString),
    List(Vec<BencodeValue>),
    Dict(std::collections::BTreeMap<ByteString, BencodeValue>),
}

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
