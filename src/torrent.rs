use anyhow::{Context, Result};
use reqwest::{self, Url};
use sha1::{Digest, Sha1};
use std::{fmt, net::SocketAddrV4};

const PEER_ID: &[u8; 20] = b"\x19\x01\xees\xbd?\xed\x81\x82Vw\xcb\x94\xdd\x87(\x05\xe9\xa2G";

use crate::bencode;
use bencode::BencodeValue;

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
    pub hash: [u8; 20],
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
    let parsed = bencode::decode(bytes).context("failed to parse bencode")?;
    let dict = parsed.as_dict().context("top-level value is not a dict")?;

    let announce = dict
        .get("announce".as_bytes())
        .context("missing 'announce' key")?
        .as_bytes()
        .context("'announce' is not a byte string")?;

    let info = dict
        .get("info".as_bytes())
        .context("missing 'info' key")?
        .as_dict()
        .context("'info' is not a dict")?;

    let length: usize = info
        .get("length".as_bytes())
        .context("missing 'length' key in info")?
        .as_integer()
        .context("'length' is not an integer")?
        .clone()
        .try_into()
        .context("'length' is negative")?;

    let name = info
        .get("name".as_bytes())
        .context("missing 'name' key in info")?
        .as_bytes()
        .context("'name' is not a byte string")?;

    let piece_length: usize = info
        .get("piece length".as_bytes())
        .context("missing 'piece length' key in info")?
        .as_integer()
        .context("'piece length' is not an integer")?
        .clone()
        .try_into()
        .context("'piece length' is negative")?;

    let pieces: Vec<String> = info
        .get("pieces".as_bytes())
        .context("missing 'pieces' key in info")?
        .as_bytes()
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
        announce: String::from_utf8_lossy(announce).into_owned(),
        info: Info {
            length,
            name: String::from_utf8_lossy(name).into_owned(),
            piece_length,
            pieces,
            hash: Sha1::digest(bencode::encode(&BencodeValue::Dict(info.clone()))).into(),
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
        let req_params = TrackerRequest {
            info_hash: &self.info.hash,
            peer_id: PEER_ID,
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

        if let BencodeValue::Dict(decoded_resp) = bencode::decode(&resp)
            .and_then(|x| {
                println!("{:?}", x);
                Ok(x)
            })
            .context("failed to parse tracker response")?
        {
            let interval: usize = decoded_resp
                .get(&b"interval"[..])
                .ok_or(anyhow::anyhow!("`interval` key not found in dictionary"))?
                .as_integer()?
                .clone()
                .try_into()?;

            let peers = decoded_resp
                .get("peers".as_bytes())
                .ok_or(anyhow::anyhow!("`peers` key not found in dictionary"))?
                .as_bytes()?
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

    pub fn get_peer_id(&self, peer_addr: &SocketAddrV4) -> Result<String> {
        Ok(hex::encode(peer::connect(self, peer_addr)?.id))
    }
}

mod peer {
    use crate::torrent::{PEER_ID, Torrent};
    use anyhow::{Context, Result};
    use std::io::{Read, Write};

    const PROTOCOL_LEN: u8 = 19;
    const PROTOCOL_NAME: &[u8] = b"BitTorrent protocol";
    const HANDSHAKE_SIZE: usize = 1 + 19 + 8 + 20 + 20; // = 68

    pub struct Peer {
        pub id: [u8; 20],
        pub addr: std::net::SocketAddrV4,
        pub conn: Option<std::net::TcpStream>,
    }

    struct PeerMessage<'a> {
        length: u8,
        kind: u8,
        msg: &'a [u8],
    }

    pub fn connect(torrent: &Torrent, peer_addr: &std::net::SocketAddrV4) -> Result<Peer> {
        let msg = build_handshake(&torrent.info.hash, PEER_ID);

        let mut stream = std::net::TcpStream::connect(peer_addr)
            .context("failed to establish connection with peer")?;

        stream
            .write_all(&msg)
            .context("failed to send BitTorrent handshake with peer")?;

        let mut response_msg = [0u8; 68];
        stream
            .read_exact(&mut response_msg)
            .context("failed to recieve BitTorrent handshake with peer")?;

        Ok(Peer {
            id: response_msg[48..68].try_into().unwrap(),
            addr: *peer_addr,
            conn: Some(stream),
        })
    }

    fn build_handshake(info_hash: &[u8; 20], peer_id: &[u8; 20]) -> [u8; 68] {
        let mut msg = [0u8; HANDSHAKE_SIZE];
        msg[0] = PROTOCOL_LEN;
        msg[1..20].copy_from_slice(PROTOCOL_NAME);

        msg[28..48].copy_from_slice(info_hash);
        msg[48..68].copy_from_slice(peer_id);

        msg
    }
}
