#![allow(unused)]
#![allow(dead_code)]

use std::collections::HashMap;
use std::env;

use serde::Serialize;

// Available if you need it!
// use serde_bencode

#[derive(Serialize)]
#[serde(untagged)]
enum BencodeValue {
    Integer(i64),
    String(Vec<u8>),
    List(Vec<BencodeValue>),
    Dict(HashMap<Vec<u8>, BencodeValue>),
}

struct Torrent {
    announce: String,
    info: TorrentInfo,
}

struct TorrentInfo {
    length: i64,
    name: String,
    piece_length: i64,
    pieces: Vec<u8>,
}

fn parse_integer(body: &[u8]) -> Option<i64> {
    match body {
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

fn parse(input: &[u8]) -> Option<(BencodeValue, &[u8])> {
    match input.iter().next() {
        Some(c) if c.is_ascii_digit() => {
            let colon_idx = input.iter().position(|&c| c == b':')?;

            let len = std::str::from_utf8(&input[..colon_idx])
                .ok()?
                .parse::<usize>()
                .ok()?;

            let (byte_string, rest) = (
                input.get(colon_idx + 1..colon_idx + 1 + len)?,
                input.get(colon_idx + 1 + len..)?,
            );

            Some((BencodeValue::String(byte_string.to_owned()), rest))
        }

        Some(b'i') => {
            let end_idx = input.iter().position(|&c| c == b'e')?;
            let body = &input[1..end_idx];

            let rem = input.get(end_idx + 1..)?;

            parse_integer(body).map(|x| (BencodeValue::Integer(x), rem))
        }

        Some(b'l') => {
            let mut v: Vec<BencodeValue> = vec![];
            let mut input = input.strip_prefix(&[b'l'])?;

            if input.starts_with(&[b'e']) {
                input = input.strip_prefix(&[b'e'])?;
                return Some((BencodeValue::List(v), input));
            }

            let rest;

            loop {
                if let Some((val, rem)) = parse(input) {
                    input = rem;
                    v.push(val);

                    if input.starts_with(&[b'e']) {
                        rest = input.strip_prefix(&[b'e'])?;
                        break;
                    }
                } else {
                    return None;
                }
            }

            Some((BencodeValue::List(v), rest))
        }

        Some(b'd') => {
            let mut keys = vec![];
            let mut kv = HashMap::new();

            let mut input = input.strip_prefix(&[b'd'])?;

            if input.starts_with(&[b'e']) {
                input = input.strip_prefix(&[b'e'])?;
                return Some((BencodeValue::Dict(kv), input));
            }

            let rest;

            loop {
                if let Some((BencodeValue::String(k), rem)) = parse(input) {
                    input = rem;

                    if let Some((v, rem)) = parse(input) {
                        input = rem;

                        keys.push(k.clone());
                        kv.insert(k.clone(), v);

                        if input.starts_with(&[b'e']) {
                            rest = input.strip_prefix(&[b'e'])?;
                            break;
                        }
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }

            if !keys.is_sorted() {
                None
            } else {
                Some((BencodeValue::Dict(kv), rest))
            }
        }

        _ => None,
    }
}

fn decode_bencoded_value(encoded_value: &[u8]) -> serde_json::Value {
    fn convert(value: BencodeValue) -> serde_json::Value {
        match value {
            BencodeValue::Integer(x) => x.into(),

            // IMPORTANT: preserve Vec<u8> -> JSON array
            BencodeValue::String(s) => s.into(),

            BencodeValue::List(v) => serde_json::Value::Array(v.into_iter().map(convert).collect()),

            BencodeValue::Dict(kv) => {
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

    if let Some((val, _)) = parse(encoded_value) {
        convert(val)
    } else {
        panic!("malformed encoded value");
    }
}

fn parse_torrent(content: &[u8]) -> Result<Torrent, String> {
    let (parsed, _) = parse(content).ok_or("Failed to parse the torrent file")?;
    let BencodeValue::Dict(dict) = &parsed else {
        return Err("Torrent file should be a bencoded dictionary".into());
    };

    let BencodeValue::String(announce) = dict
        .get(&"announce".to_string().into_bytes())
        .ok_or("couldn't find 'announce' as a key")?
    else {
        return Err("'announce' was not a valid bencoded string".into());
    };
    let BencodeValue::Dict(info) = &dict
        .get(&"info".to_string().into_bytes())
        .ok_or("couldn't find 'info' as a key")?
    else {
        return Err("info' was not a valid bencoded dict".into());
    };
    let BencodeValue::Integer(length) = &info
        .get(&"length".to_string().into_bytes())
        .ok_or("couldn't find 'length' as a key")?
    else {
        return Err("length' was not a valid bencoded integer".into());
    };
    let BencodeValue::String(name) = &info
        .get(&"name".to_string().into_bytes())
        .ok_or("couldn't find 'name' as a key")?
    else {
        return Err("name' was not a valid bencoded string".into());
    };
    let BencodeValue::Integer(piece_length) = &info
        .get(&"piece length".to_string().into_bytes())
        .ok_or("couldn't find 'piece length' as a key")?
    else {
        return Err("piece length' was not a valid bencoded integer".into());
    };
    let BencodeValue::String(pieces) = &info
        .get(&"pieces".to_string().into_bytes())
        .ok_or("couldn't find 'pieces' as a key")?
    else {
        return Err("pieces' was not a valid bencoded string".into());
    };
    Ok(Torrent {
        announce: String::from_utf8(announce.clone()).map_err(|e| e.to_string())?,
        info: TorrentInfo {
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
    } else {
        println!("unknown command: {}", args[1]);
    }
}
