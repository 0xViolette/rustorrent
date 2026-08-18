use anyhow::{Context, Result};
use reqwest::{self, Url};
use sha1::{Digest, Sha1};
use std::fmt;

use crate::bencode::{self, BencodeValue};

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

pub fn from_bytes(bytes: &[u8]) -> Result<Torrent> {
    let parsed = bencode::BencodeValue::from_bytes(bytes).context("failed to parse bencode")?;
    let dict = parsed.as_dict().context("top-level value is not a dict")?;

    let announce = dict
        .get(&b"announce"[..])
        .context("missing 'announce' key")?
        .as_byte_string()
        .context("'announce' is not a byte string")?;

    let info = dict
        .get(&b"info"[..])
        .context("missing 'info' key")?
        .as_dict()
        .context("'info' is not a dict")?;

    let length: u64 = info
        .get(&b"length"[..])
        .context("missing 'length' key in info")?
        .as_integer()
        .context("'length' is not an integer")?
        .try_into()
        .context("'length' is negative")?;

    let name = info
        .get(&b"name"[..])
        .context("missing 'name' key in info")?
        .as_byte_string()
        .context("'name' is not a byte string")?;

    let piece_length = info
        .get(&b"piece length"[..])
        .context("missing 'piece length' key in info")?
        .as_integer()
        .context("'piece length' is not an integer")?;

    let pieces: Vec<String> = info
        .get(&b"pieces"[..])
        .context("missing 'pieces' key in info")?
        .as_byte_string()
        .context("'pieces' is not a byte string")?
        .chunks(20)
        .map(|x| {
            x.try_into()
                .map_err(|_| anyhow::anyhow!("piece hash is not 20 bytes"))
        })
        .collect::<Result<Vec<[u8; 20]>>>()?
        .iter()
        .map(hex::encode)
        .collect();

    Ok(Torrent {
        announce: String::from_utf8(announce)
            .context("'announce' is not valid UTF-8")?,
        info: Info {
            length,
            name: String::from_utf8(name).context("'name' is not valid UTF-8")?,
            piece_length,
            pieces,
            hash: Sha1::digest(bencode::encode(&BencodeValue::Dict(info))).to_vec(),
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
    pub fn get_peers_from_tracker(&self) -> Result<()> {
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

        let mut url = Url::parse(&self.announce).context("invalid tracker URL")?;
        url.set_query(Some(&req_params.to_query()));

        let resp = reqwest::blocking::get(url)
            .context("failed to contact tracker")?
            .bytes()
            .context("failed to read tracker response")?;

        let (decoded_resp, _) =
            bencode::parse_value(&resp).context("failed to parse tracker response")?;

        println!("{:#?}", decoded_resp.convert().to_string());
        Ok(())
    }
}
