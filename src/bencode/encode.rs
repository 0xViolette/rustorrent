use super::model::*;

pub fn encode(val: &BencodeValue) -> Vec<u8> {
    match val {
        BencodeValue::Integer(x) => format!("i{x}e").into_bytes(),
        BencodeValue::ByteString(s) => {
            let mut v = format!("{}:", s.len()).into_bytes();
            v.extend(s);
            v
        }
        BencodeValue::List(v) => std::iter::once(b'l')
            .chain(v.iter().flat_map(|x| encode(x)))
            .chain(std::iter::once(b'e'))
            .collect(),
        BencodeValue::Dict(d) => std::iter::once(b'd')
            .chain(d.iter().flat_map(|(k, v)| {
                encode(&BencodeValue::ByteString(k.clone()))
                    .into_iter()
                    .chain(encode(v))
            }))
            .chain(std::iter::once(b'e'))
            .collect(),
    }
}
