use thiserror::{self, Error};
use urlencoding;

#[derive(Debug, Error)]
pub enum TrackerError {
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("bencode error: {0}")]
    Bencode(#[from] crate::bencode::value::BencodeParseError),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}

pub struct TrackerRequest<'a> {
    pub url: &'a str,
    pub info_hash: &'a [u8],
    pub peer_id: &'a [u8],
    pub port: u16,
    pub uploaded: usize,
    pub downloaded: usize,
    pub left: usize,
    pub compact: u8,
}

impl<'a> TrackerRequest<'a> {
    pub fn to_query(&self) -> String {
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

pub struct TrackerResponse {
    pub interval: usize,
    pub peers: Vec<std::net::SocketAddrV4>,
}

pub const PEER_ID: &[u8; 20] = b"\x19\x01\xees\xbd?\xed\x81\x82Vw\xcb\x94\xdd\x87(\x05\xe9\xa2G";
