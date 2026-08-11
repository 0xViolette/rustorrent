#![allow(unused)]
#![allow(dead_code)]
#![allow(non_snake_case)]

use std::env;

use serde::{self, Serialize};
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;

mod bencode {
    type ByteString = Vec<u8>;

    pub enum BencodeValue {
        Integer(i64),
        String(ByteString),
        List(Vec<BencodeValue>),
        Dict(std::collections::BTreeMap<ByteString, BencodeValue>),
    }

    pub enum ParseError {
        Fail,
    }

    impl BencodeValue {
        pub fn from_bytes(input: &[u8]) -> Result<Self, ParseError> {
            let (value, remainder) = parse_value(input)?;

            if !remainder.is_empty() {
                return Err(ParseError::Fail);
            }

            Ok(value)
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
            c @ b'0'..=b'9' => {
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
}

mod torrent {
    pub struct Torrent {
        pub announce: String,
        pub info: Info,
    }

    pub struct Info {
        pub length: i64,
        pub name: String,
        pub piece_length: i64,
        pub pieces: Vec<u8>,
    }
}

fn torrentInfo_to_bencodevalue(info: &torrent::Info) -> bencode::BencodeValue {
    let mut result = BTreeMap::new();

    result.insert(
        "length".to_string().into_bytes(),
        bencode::BencodeValue::Integer(info.length),
    );
    result.insert(
        "name".to_string().into_bytes(),
        bencode::BencodeValue::String(info.name.clone().into_bytes()),
    );
    result.insert(
        "piece length".to_string().into_bytes(),
        bencode::BencodeValue::Integer(info.piece_length),
    );
    result.insert(
        "pieces".to_string().into_bytes(),
        bencode::BencodeValue::String(info.pieces.clone()),
    );

    bencode::BencodeValue::Dict(result)
}

fn bencode(val: &bencode::BencodeValue) -> Vec<u8> {
    match val {
        bencode::BencodeValue::Integer(x) => format!("i{x}e").into_bytes(),
        bencode::BencodeValue::String(s) => {
            let mut v = format!("{}:", s.len()).into_bytes();
            v.extend(s);
            v
        }
        bencode::BencodeValue::List(v) => {
            let mut result: Vec<u8> = vec![];
            result.push(b'l');
            for x in v.iter() {
                result.extend_from_slice(&bencode(x));
            }
            result.push(b'e');
            result
        }
        bencode::BencodeValue::Dict(d) => {
            let mut result: Vec<u8> = vec![];
            result.push(b'd');
            let mut keys: Vec<&Vec<u8>> = d.keys().collect();
            keys.sort();

            for k in keys {
                result.extend_from_slice(&bencode(&bencode::BencodeValue::String(k.clone())));

                result.extend_from_slice(&bencode(d.get(k).unwrap()));
            }
            result.push(b'e');
            result
        }
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

fn parse_torrent(content: &[u8]) -> Result<torrent::Torrent, String> {
    let (parsed, _) =
        bencode::parse_value(content).map_err(|_| "Failed to parse the torrent file")?;
    let bencode::BencodeValue::Dict(dict) = &parsed else {
        return Err("Torrent file should be a bencoded dictionary".into());
    };

    let bencode::BencodeValue::String(announce) = dict
        .get(&"announce".to_string().into_bytes())
        .ok_or("couldn't find 'announce' as a key")?
    else {
        return Err("'announce' was not a valid bencoded string".into());
    };
    let bencode::BencodeValue::Dict(info) = &dict
        .get(&"info".to_string().into_bytes())
        .ok_or("couldn't find 'info' as a key")?
    else {
        return Err("info' was not a valid bencoded dict".into());
    };
    let bencode::BencodeValue::Integer(length) = &info
        .get(&"length".to_string().into_bytes())
        .ok_or("couldn't find 'length' as a key")?
    else {
        return Err("length' was not a valid bencoded integer".into());
    };
    let bencode::BencodeValue::String(name) = &info
        .get(&"name".to_string().into_bytes())
        .ok_or("couldn't find 'name' as a key")?
    else {
        return Err("name' was not a valid bencoded string".into());
    };
    let bencode::BencodeValue::Integer(piece_length) = &info
        .get(&"piece length".to_string().into_bytes())
        .ok_or("couldn't find 'piece length' as a key")?
    else {
        return Err("piece length' was not a valid bencoded integer".into());
    };
    let bencode::BencodeValue::String(pieces) = &info
        .get(&"pieces".to_string().into_bytes())
        .ok_or("couldn't find 'pieces' as a key")?
    else {
        return Err("pieces' was not a valid bencoded string".into());
    };
    Ok(torrent::Torrent {
        announce: String::from_utf8(announce.clone()).map_err(|e| e.to_string())?,
        info: torrent::Info {
            length: length.clone(),
            name: String::from_utf8(name.clone()).map_err(|e| e.to_string())?,
            piece_length: piece_length.clone(),
            pieces: pieces.clone(),
        },
    })
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
        let parsed_torrent = parse_torrent(&torrent_file).unwrap_or_else(|e| panic!("{}", e));

        println!("Tracked URL: {}", parsed_torrent.announce);
        println!("Length: {}", parsed_torrent.info.length);
        println!(
            "Info Hash: {}",
            hex::encode(Sha1::digest(bencode(&torrentInfo_to_bencodevalue(
                &parsed_torrent.info
            ))))
        );
    } else {
        println!("unknown command: {}", args[1]);
    }
}
