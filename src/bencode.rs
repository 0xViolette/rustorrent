use std::fmt;

#[derive(Debug)]
pub enum ParseError {
    Fail(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Fail(msg) => write!(f, "unable to parse from/into Bencoded value: {}", msg),
        }
    }
}

impl std::error::Error for ParseError {}

type ByteString = Vec<u8>;

#[derive(Clone, Debug)]
pub enum BencodeValue {
    Integer(i64),
    String(ByteString),
    List(Vec<BencodeValue>),
    Dict(std::collections::BTreeMap<ByteString, BencodeValue>),
}

impl BencodeValue {
    pub fn convert(&self) -> serde_json::Value {
        match self {
            Self::Integer(x) => (*x).into(),

            Self::String(s) => (*s).clone().into(),

            Self::List(v) => serde_json::Value::Array(v.into_iter().map(Self::convert).collect()),

            Self::Dict(kv) => {
                let mut result = serde_json::Map::new();

                for (k, v) in kv {
                    let key = String::from_utf8(k.clone())
                        .unwrap_or_else(|_| panic!("malformed encoded value"))
                        .to_string();

                    result.insert(key, Self::convert(v));
                }

                serde_json::Value::Object(result)
            }
        }
    }
    pub fn from_bytes(input: &[u8]) -> Result<Self, ParseError> {
        let (value, remainder) = parse_value(input)?;

        if !remainder.is_empty() {
            return Err(ParseError::Fail("from_bytes failed".into()));
        }

        Ok(value)
    }

    pub fn as_integer(&self) -> Result<i64, ParseError> {
        if let BencodeValue::Integer(x) = self {
            Ok(x.clone())
        } else {
            Err(ParseError::Fail("as_integer failed".into()))
        }
    }

    pub fn as_byte_string(&self) -> Result<ByteString, ParseError> {
        if let BencodeValue::String(s) = self {
            Ok(s.clone())
        } else {
            Err(ParseError::Fail("as_byte_string failed".into()))
        }
    }

    pub fn as_vec(&self) -> Result<Vec<Self>, ParseError> {
        if let BencodeValue::List(v) = self {
            Ok(v.clone())
        } else {
            Err(ParseError::Fail("as_vec failed".into()))
        }
    }

    pub fn as_dict(&self) -> Result<std::collections::BTreeMap<ByteString, Self>, ParseError> {
        if let BencodeValue::Dict(dict) = self {
            Ok(dict.clone())
        } else {
            Err(ParseError::Fail("as_dict failed".into()))
        }
    }
}

fn parse_integer(input: &[u8]) -> Option<i64> {
    match input {
        b"0" => Some(0),
        s => {
            let (is_negative, digits) = s
                .strip_prefix(&[b'-'])
                .and_then(|s| Some((true, s)))
                .unwrap_or_else(|| (false, s));

            if digits.is_empty() || digits.starts_with(&[b'0']) {
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

pub fn parse_value(input: &[u8]) -> Result<(BencodeValue, &[u8]), ParseError> {
    match input
        .iter()
        .next()
        .ok_or(ParseError::Fail("input is empty".into()))?
    {
        b'0'..=b'9' => {
            let colon_idx = input
                .iter()
                .position(|&c| c == b':')
                .ok_or(ParseError::Fail("colon not found in byte string".into()))?;

            let len = std::str::from_utf8(&input[..colon_idx])
                .map_err(|_| ParseError::Fail("byte string length not valid utf8".into()))?
                .parse::<usize>()
                .map_err(|_| ParseError::Fail("byte string length not a number".into()))?;

            println!("{}", len);

            let (byte_string, rest) = (
                input
                    .get(colon_idx + 1..colon_idx + 1 + len)
                    .ok_or(ParseError::Fail("byte string too short".into()))?,
                input.get(colon_idx + 1 + len..).ok_or(ParseError::Fail(
                    "missing remainder after byte string".into(),
                ))?,
            );

            Ok((BencodeValue::String(byte_string.to_owned()), rest))
        }

        b'i' => {
            let end_idx = input
                .iter()
                .position(|&c| c == b'e')
                .ok_or(ParseError::Fail("integer missing 'e' terminator".into()))?;
            let body = &input[1..end_idx];

            let rem = input
                .get(end_idx + 1..)
                .ok_or(ParseError::Fail("missing remainder after integer".into()))?;

            parse_integer(body)
                .map(|x| (BencodeValue::Integer(x), rem))
                .ok_or(ParseError::Fail("invalid integer body".into()))
        }

        b'l' => {
            let mut v: Vec<BencodeValue> = vec![];
            let mut input = input
                .strip_prefix(&[b'l'])
                .ok_or(ParseError::Fail("missing 'l' prefix for list".into()))?;

            if input.starts_with(&[b'e']) {
                input = input
                    .strip_prefix(&[b'e'])
                    .ok_or(ParseError::Fail("missing 'e' to close empty list".into()))?;
                return Ok((BencodeValue::List(v), input));
            }

            let rest;

            loop {
                if let Ok((val, rem)) = parse_value(input) {
                    input = rem;
                    v.push(val);

                    if input.starts_with(&[b'e']) {
                        rest = input
                            .strip_prefix(&[b'e'])
                            .ok_or(ParseError::Fail("missing 'e' to close list".into()))?;
                        break;
                    }
                } else {
                    return Err(ParseError::Fail("failed to parse list element".into()));
                }
            }

            Ok((BencodeValue::List(v), rest))
        }

        b'd' => {
            let mut keys = vec![];
            let mut kv = std::collections::BTreeMap::new();

            let mut input = input
                .strip_prefix(&[b'd'])
                .ok_or(ParseError::Fail("missing 'd' prefix for dict".into()))?;

            if input.starts_with(&[b'e']) {
                input = input
                    .strip_prefix(&[b'e'])
                    .ok_or(ParseError::Fail("missing 'e' to close empty dict".into()))?;
                return Ok((BencodeValue::Dict(kv), input));
            }

            let rest;

            loop {
                if let Ok((BencodeValue::String(k), rem)) = parse_value(input) {
                    input = rem;

                    if let Ok((v, rem)) = parse_value(input) {
                        input = rem;

                        keys.push(k.clone());
                        kv.insert(k.clone(), v);

                        if input.starts_with(&[b'e']) {
                            rest = input
                                .strip_prefix(&[b'e'])
                                .ok_or(ParseError::Fail("missing 'e' to close dict".into()))?;
                            break;
                        }
                    } else {
                        return Err(ParseError::Fail("failed to parse dict value".into()));
                    }
                } else {
                    println!("{}", String::from_utf8_lossy(input));
                    return Err(ParseError::Fail("dict key is not a byte string".into()));
                }
            }

            if !keys.is_sorted() {
                Err(ParseError::Fail("dict keys not sorted".into()))
            } else {
                Ok((BencodeValue::Dict(kv), rest))
            }
        }

        _ => Err(ParseError::Fail("unknown bencode type".into())),
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

            for k in d.keys() {
                result.extend_from_slice(&encode(&BencodeValue::String(k.clone())));

                result.extend_from_slice(&encode(d.get(k).unwrap()));
            }
            result.push(b'e');
            result
        }
    }
}
