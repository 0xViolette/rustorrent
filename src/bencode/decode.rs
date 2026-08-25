use super::value::*;

pub fn decode(input: &[u8]) -> Result<BencodeValue, BencodeParseError> {
    parse_value(input).and_then(|(value, remainder)| {
        remainder
            .is_empty()
            .then_some(value)
            .ok_or(BencodeParseError::TrailingData)
    })
}

fn parse_integer(input: &[u8]) -> Result<i64, BencodeParseError> {
    match input {
        _ if input.is_empty() => Err(BencodeParseError::InvalidInteger {
            reason: "empty string".into(),
        }),
        _ if input.starts_with(b"-0") => Err(BencodeParseError::InvalidInteger {
            reason: "no leading 0s".into(),
        }),
        _ if input.starts_with(b"+") => Err(BencodeParseError::InvalidInteger {
            reason: "plus sign (+) not allowed".into(),
        }),
        _ if input.starts_with(b"0") && input.len() > 1 => Err(BencodeParseError::InvalidInteger {
            reason: "no leading 0s".into(),
        }),
        s => str::from_utf8(s)?
            .parse::<i64>()
            .map_err(|e| BencodeParseError::InvalidInteger {
                reason: e.to_string(),
            }),
    }
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

            Ok((BencodeValue::ByteString(byte_string.to_vec()), rest))
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

            Ok((BencodeValue::Integer(parse_integer(body)?), rem))
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
                if let Ok((BencodeValue::ByteString(k), rem)) = parse_value(input) {
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
