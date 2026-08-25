use super::value::*;

pub fn encode(val: &BencodeValue) -> Vec<u8> {
    match val {
        BencodeValue::Integer(x) => format!("i{x}e").into_bytes(),
        BencodeValue::ByteString(s) => {
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
                result.extend_from_slice(&encode(&BencodeValue::ByteString(k.clone())));
                result.extend_from_slice(&encode(v));
            }
            result.push(b'e');
            result
        }
    }
}
