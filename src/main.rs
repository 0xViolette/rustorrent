use std::collections::HashMap;
use std::env;

use serde::Serialize;

// Available if you need it!
// use serde_bencode

#[derive(Serialize)]
#[serde(untagged)]
enum BencodeValue {
    Integer(i64),
    String(String),
    List(Vec<BencodeValue>),
    Dict(HashMap<String, BencodeValue>),
}

fn parse_integer(body: &str) -> Option<i64> {
    match body {
        "0" => Some(0),
        s => {
            let (is_negative, digits) = s
                .strip_prefix('-')
                .and_then(|s| Some((true, s)))
                .unwrap_or_else(|| (false, s));
            if digits.is_empty() || digits.starts_with('0') {
                None
            } else {
                digits
                    .parse::<i64>()
                    .map(|n| if is_negative { -n } else { n })
                    .ok()
            }
        }
    }
}

fn parse(input: &str) -> Option<(BencodeValue, &str)> {
    match input.chars().next() {
        Some(c) if c.is_ascii_digit() => {
            let (len, input) = input.split_once(':')?;

            let len = len.parse::<usize>().ok()?;

            let (string, rest) = input.split_at_checked(len)?;
            Some((BencodeValue::String(string.to_owned()), rest))
        }
        Some('i') => {
            let end_idx = input.find('e').unwrap();
            let body = &input[1..end_idx];
            let rem = if end_idx + 1 < input.len() {
                &input[end_idx + 1..]
            } else {
                ""
            };
            parse_integer(body).map(|x| (BencodeValue::Integer(x), rem))
        }
        Some('l') => {
            let mut v: Vec<BencodeValue> = vec![];
            let mut input = input.strip_prefix('l')?;
            if input.starts_with('e') {
                input = input.strip_prefix('e')?;
                return Some((BencodeValue::List(v), input));
            }
            let rest;
            loop {
                if let Some((val, rem)) = parse(input) {
                    input = rem;
                    v.push(val);
                    if input.starts_with('e') {
                        rest = input.strip_prefix('e')?;
                        break;
                    }
                } else {
                    return None;
                }
            }

            Some((BencodeValue::List(v), rest))
        }
        Some('d') => {
            let mut keys = vec![];
            let mut kv = HashMap::new();
            let mut input = input.strip_prefix('d')?;
            if input.starts_with('e') {
                input = input.strip_prefix('e')?;
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
                        if input.starts_with('e') {
                            rest = input.strip_prefix('e')?;
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

#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    if let Some((val, _)) = parse(encoded_value) {
        match val {
            BencodeValue::Integer(x) => x.into(),
            BencodeValue::String(s) => s.into(),
            BencodeValue::List(v) => serde_json::to_value(&v).unwrap(),
            BencodeValue::Dict(kv) => serde_json::to_value(&kv).unwrap(),
        }
    } else {
        panic!("Unhandled encoded value: {}", encoded_value)
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        eprintln!("Logs:");

        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value.to_string());
    } else {
        println!("unknown command: {}", args[1])
    }
}
