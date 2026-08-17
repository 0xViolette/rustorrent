use reqwest::{self, Url};
use sha1::{Digest, Sha1};
use std::fmt;
use std::net::SocketAddr;
use urlencoding;

use crate::bencode::ParseError;
use crate::{bencode, decode_bencoded_value};

pub struct Torrent {
    pub announce: String,
    pub info: Info,
}

pub struct Info {
    pub length: u64,
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

pub fn from_bytes(bytes: &[u8]) -> Result<Torrent, ParseError> {
    let parsed = bencode::BencodeValue::from_bytes(bytes)?;
    let dict = parsed.as_dict()?;
    let announce = dict
        .get(&"announce".to_string().into_bytes())
        .ok_or(ParseError::Fail("announce key not found".into()))?
        .as_byte_string()?;
    let info = dict
        .get(&"info".to_string().into_bytes())
        .ok_or(ParseError::Fail("info key not found".into()))?
        .as_dict()?;

    let length: u64 = info
        .get(&"length".to_string().into_bytes())
        .ok_or(ParseError::Fail("length key not found".into()))?
        .as_integer()?
        .cast_unsigned();

    let name = info
        .get(&"name".to_string().into_bytes())
        .ok_or(ParseError::Fail("name key not found".into()))?
        .as_byte_string()?;

    let piece_length = info
        .get(&"piece length".to_string().into_bytes())
        .ok_or(ParseError::Fail("piece length key not found".into()))?
        .as_integer()?;

    let pieces: Vec<String> = info
        .get(&"pieces".to_string().into_bytes())
        .ok_or(ParseError::Fail("pieces key not found".into()))?
        .as_byte_string()?
        .chunks(20)
        .map(|x| {
            x.try_into()
                .map_err(|_| ParseError::Fail("piece not 20 bytes".into()))
        })
        .collect::<Result<Vec<[u8; 20]>, ParseError>>()?
        .iter()
        .map(|x| hex::encode(x))
        .collect();

    Ok(Torrent {
        announce: String::from_utf8(announce.clone())
            .map_err(|_| ParseError::Fail("announce not valid utf8".into()))?,
        info: Info {
            length: length.into(),
            name: String::from_utf8(name)
                .map_err(|_| ParseError::Fail("name not valid utf8".into()))?,
            piece_length: piece_length,
            pieces: pieces,
            hash: Sha1::digest(bencode::encode(&bencode::BencodeValue::Dict(info))).to_vec(),
        },
    })
}

struct Peer {
    ip: String,
    port: u16,
}

struct TrackerResponse {
    interval: i64,
    peers: Vec<String>,
}

struct TrackerRequest<'a> {
    info_hash: &'a [u8],
    peer_id: &'a [u8],
    port: u16,
    uploaded: u64,
    downloaded: u64,
    left: u64,
    compact: u8,
}

impl<'a> TrackerRequest<'a> {
    fn to_query(&self) -> String {
        format!(
            "info_hash={}&peer_id={}&port={}&uploaded={}&downloaded={}&left={}&compact={}",
            urlencoding::encode_binary(self.info_hash),
            urlencoding::encode_binary(self.peer_id),
            self.port,
            self.uploaded,
            self.downloaded,
            self.left,
            self.compact,
        )
    }
}

impl Torrent {
    pub fn get_peers_from_tracker(&self) -> anyhow::Result<()> {
        let peer_id = b"\x19\x01\xees\xbd?\xed\x81\x82Vw\xcb\x94\xdd\x87(\x05\xe9\xa2G";
        let req_params = TrackerRequest {
            info_hash: &self.info.hash,
            peer_id,
            uploaded: 0,
            downloaded: 0,
            left: self.info.length,
            compact: 1,
            port: 6881,
        };

        let mut url = Url::parse(&self.announce)?;
        url.set_query(Some(&req_params.to_query()));

        let resp = reqwest::blocking::get(url)?.bytes()?;
        let (decoded_resp, _) = bencode::parse_value(&resp)?;
        println!("{:#?}", decoded_resp.convert().to_string());
        Ok(())
    }
}
