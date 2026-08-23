use anyhow::{Context, Result};
use reqwest::{self, Url};
use sha1::{Digest, Sha1};
use std::{fmt, net::SocketAddrV4};

use crate::bencode::{self, BencodeValue};

pub struct Torrent {
    pub announce: String,
    pub info: Info,
}

pub struct Info {
    pub length: usize,
    #[allow(unused)]
    pub name: String,
    pub piece_length: usize,
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
    let parsed = bencode::parse(bytes).context("failed to parse bencode")?;
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

    let length: usize = info
        .get(&b"length"[..])
        .context("missing 'length' key in info")?
        .as_integer()
        .context("'length' is not an integer")?
        .clone()
        .try_into()
        .context("'length' is negative")?;

    let name = info
        .get(&b"name"[..])
        .context("missing 'name' key in info")?
        .as_byte_string()
        .context("'name' is not a byte string")?;

    let piece_length: usize = info
        .get(&b"piece length"[..])
        .context("missing 'piece length' key in info")?
        .as_integer()
        .context("'piece length' is not an integer")?
        .clone()
        .try_into()
        .context("'piece length' is negative")?;

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
        announce: String::from_utf8(announce.clone()).context("'announce' is not valid UTF-8")?,
        info: Info {
            length,
            name: String::from_utf8(name.clone()).context("'name' is not valid UTF-8")?,
            piece_length,
            pieces,
            hash: Sha1::digest(bencode::encode(&BencodeValue::Dict(info.clone()))).to_vec(),
        },
    })
}

pub struct TrackerResponse {
    #[allow(unused)]
    pub interval: usize,
    pub peers: Vec<std::net::SocketAddrV4>,
}

struct TrackerRequest<'a> {
    info_hash: &'a [u8],
    peer_id: &'a [u8],
    port: u16,
    uploaded: usize,
    downloaded: usize,
    left: usize,
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
    pub fn discover_peers(&self) -> Result<TrackerResponse> {
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

        if let BencodeValue::Dict(decoded_resp) =
            bencode::parse(&resp).context("failed to parse tracker response")?
        {
            let interval: usize = decoded_resp
                .get(&b"interval"[..])
                .ok_or(anyhow::anyhow!("`interval` key not found in dictionary"))?
                .as_integer()?
                .clone()
                .try_into()?;

            let peers = decoded_resp
                .get(&b"peers"[..])
                .ok_or(anyhow::anyhow!("`peers` key not found in dictionary"))?
                .as_byte_string()?
                .chunks_exact(6)
                .filter(|x| x.len() == 6)
                .map(|x| {
                    std::net::SocketAddrV4::new(
                        std::net::Ipv4Addr::new(x[0], x[1], x[2], x[3]),
                        u16::from_be_bytes([x[4], x[5]]),
                    )
                })
                .collect::<Vec<SocketAddrV4>>();

            Ok(TrackerResponse { interval, peers })
        } else {
            anyhow::bail!("Expected response to be a bencoded dictionary");
        }
    }
}
