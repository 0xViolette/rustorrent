use std::env;

enum ParseError {
    Fail,
}

mod bencode {
    use super::ParseError;

    type ByteString = Vec<u8>;

    #[derive(Clone)]
    pub enum BencodeValue {
        Integer(i64),
        String(ByteString),
        List(Vec<BencodeValue>),
        Dict(std::collections::BTreeMap<ByteString, BencodeValue>),
    }

    impl BencodeValue {
        pub fn from_bytes(input: &[u8]) -> Result<Self, ParseError> {
            let (value, remainder) = parse_value(input)?;

            if !remainder.is_empty() {
                return Err(ParseError::Fail);
            }

            Ok(value)
        }

        pub fn as_integer(&self) -> Result<i64, ParseError> {
            if let BencodeValue::Integer(x) = self {
                Ok(x.clone())
            } else {
                Err(ParseError::Fail)
            }
        }

        pub fn as_byte_string(&self) -> Result<ByteString, ParseError> {
            if let BencodeValue::String(s) = self {
                Ok(s.clone())
            } else {
                Err(ParseError::Fail)
            }
        }

        pub fn as_vec(&self) -> Result<Vec<Self>, ParseError> {
            if let BencodeValue::List(v) = self {
                Ok(v.clone())
            } else {
                Err(ParseError::Fail)
            }
        }

        pub fn as_dict(&self) -> Result<std::collections::BTreeMap<ByteString, Self>, ParseError> {
            if let BencodeValue::Dict(dict) = self {
                Ok(dict.clone())
            } else {
                Err(ParseError::Fail)
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
        match input.iter().next().ok_or(ParseError::Fail)? {
            b'0'..=b'9' => {
                let colon_idx = input
                    .iter()
                    .position(|&c| c == b':')
                    .ok_or(ParseError::Fail)?;

                let len = std::str::from_utf8(&input[..colon_idx])
                    .map_err(|_| ParseError::Fail)?
                    .parse::<usize>()
                    .map_err(|_| ParseError::Fail)?;

                let (byte_string, rest) = (
                    input
                        .get(colon_idx + 1..colon_idx + 1 + len)
                        .ok_or(ParseError::Fail)?,
                    input.get(colon_idx + 1 + len..).ok_or(ParseError::Fail)?,
                );

                Ok((BencodeValue::String(byte_string.to_owned()), rest))
            }

            b'i' => {
                let end_idx = input
                    .iter()
                    .position(|&c| c == b'e')
                    .ok_or(ParseError::Fail)?;
                let body = &input[1..end_idx];

                let rem = input.get(end_idx + 1..).ok_or(ParseError::Fail)?;

                parse_integer(body)
                    .map(|x| (BencodeValue::Integer(x), rem))
                    .ok_or(ParseError::Fail)
            }

            b'l' => {
                let mut v: Vec<BencodeValue> = vec![];
                let mut input = input.strip_prefix(&[b'l']).ok_or(ParseError::Fail)?;

                if input.starts_with(&[b'e']) {
                    input = input.strip_prefix(&[b'e']).ok_or(ParseError::Fail)?;
                    return Ok((BencodeValue::List(v), input));
                }

                let rest;

                loop {
                    if let Ok((val, rem)) = parse_value(input) {
                        input = rem;
                        v.push(val);

                        if input.starts_with(&[b'e']) {
                            rest = input.strip_prefix(&[b'e']).ok_or(ParseError::Fail)?;
                            break;
                        }
                    } else {
                        return Err(ParseError::Fail);
                    }
                }

                Ok((BencodeValue::List(v), rest))
            }

            b'd' => {
                let mut keys = vec![];
                let mut kv = std::collections::BTreeMap::new();

                let mut input = input.strip_prefix(&[b'd']).ok_or(ParseError::Fail)?;

                if input.starts_with(&[b'e']) {
                    input = input.strip_prefix(&[b'e']).ok_or(ParseError::Fail)?;
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
                                rest = input.strip_prefix(&[b'e']).ok_or(ParseError::Fail)?;
                                break;
                            }
                        } else {
                            return Err(ParseError::Fail);
                        }
                    } else {
                        return Err(ParseError::Fail);
                    }
                }

                if !keys.is_sorted() {
                    Err(ParseError::Fail)
                } else {
                    Ok((BencodeValue::Dict(kv), rest))
                }
            }

            _ => Err(ParseError::Fail),
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
}

mod torrent {
    use sha1::{Digest, Sha1};
    use std::fmt;

    use super::{ParseError, bencode};

    pub struct Torrent {
        pub announce: String,
        pub info: Info,
    }

    pub struct Info {
        pub length: i64,
        pub name: String,
        pub piece_length: i64,
        pub pieces: Vec<String>,
        pub hash: Vec<u8>,
    }

    impl fmt::Debug for Torrent {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("Torrent")
                .field("Tracked URL", &self.announce)
                .field("Length", &self.info.length)
                .field("Info Hash", &hex::encode(&self.info.hash))
                .field("Piece Length", &self.info.piece_length)
                .field("Pieces", &self.info.pieces)
                .finish()
        }
    }

    pub fn from_bytes(content: &[u8]) -> Result<Torrent, ParseError> {
        let parsed = bencode::BencodeValue::from_bytes(content)?;
        let dict = parsed.as_dict()?;
        let announce = dict
            .get(&"announce".to_string().into_bytes())
            .ok_or(ParseError::Fail)?
            .as_byte_string()?;
        let info = dict
            .get(&"info".to_string().into_bytes())
            .ok_or(ParseError::Fail)?
            .as_dict()?;

        let length = info
            .get(&"length".to_string().into_bytes())
            .ok_or(ParseError::Fail)?
            .as_integer()?;

        let name = info
            .get(&"name".to_string().into_bytes())
            .ok_or(ParseError::Fail)?
            .as_byte_string()?;

        let piece_length = info
            .get(&"piece length".to_string().into_bytes())
            .ok_or(ParseError::Fail)?
            .as_integer()?;

        let pieces: Vec<String> = info
            .get(&"pieces".to_string().into_bytes())
            .ok_or(ParseError::Fail)?
            .as_byte_string()?
            .chunks(20)
            .map(|x| x.try_into().map_err(|_| ParseError::Fail))
            .collect::<Result<Vec<[u8; 20]>, ParseError>>()?
            .iter()
            .map(|x| hex::encode(x))
            .collect();

        Ok(Torrent {
            announce: String::from_utf8(announce.clone()).map_err(|_| ParseError::Fail)?,
            info: Info {
                length: length,
                name: String::from_utf8(name).map_err(|_| ParseError::Fail)?,
                piece_length: piece_length,
                pieces: pieces,
                hash: Sha1::digest(bencode::encode(&bencode::BencodeValue::Dict(info))).to_vec(),
            },
        })
    }
}

fn decode_bencoded_value(encoded_value: &[u8]) -> serde_json::Value {
    fn convert(value: bencode::BencodeValue) -> serde_json::Value {
        match value {
            bencode::BencodeValue::Integer(x) => x.into(),

            bencode::BencodeValue::String(s) => s.into(),

            bencode::BencodeValue::List(v) => {
                serde_json::Value::Array(v.into_iter().map(convert).collect())
            }

            bencode::BencodeValue::Dict(kv) => {
                let mut result = serde_json::Map::new();

                for (k, v) in kv {
                    let key = String::from_utf8(k)
                        .unwrap_or_else(|_| panic!("malformed encoded value"))
                        .to_string();

                    result.insert(key, convert(v));
                }

                serde_json::Value::Object(result)
            }
        }
    }

    if let Ok((val, _)) = bencode::parse_value(encoded_value) {
        convert(val)
    } else {
        panic!("malformed encoded value");
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        eprintln!("Logs:");

        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(&encoded_value.clone().into_bytes());

        println!("{}", decoded_value.to_string());
    } else if command == "info" {
        eprintln!("Logs:");
        let filename = &args[2];
        let torrent_file = std::fs::read(filename).expect("File not found");
        let parsed_torrent =
            torrent::from_bytes(&torrent_file).unwrap_or_else(|_| panic!("Couldn't parse torrent"));

        println!("{:#?}", parsed_torrent);
    } else {
        println!("unknown command: {}", args[1]);
    }
}
