use crate::bencode::{self, BencodeValue};
use sha1::{Digest, Sha1};
use std::fmt;
use thiserror::Error;

pub struct MetaInfo {
    pub announce: String,
    pub info: Info,
}

impl MetaInfo {
    pub fn from_bytes(bytes: &[u8]) -> Result<MetaInfo, TorrentError> {
        let parsed = bencode::decode(bytes)?;
        let dict = parsed.as_dict()?;

        let announce = dict
            .get("announce".as_bytes())
            .ok_or(TorrentError::MissingKey("announce".into()))?
            .as_bytes()?;

        let info = dict
            .get("info".as_bytes())
            .ok_or(TorrentError::MissingKey("info".into()))?
            .as_dict()?;

        let length: usize = info
            .get("length".as_bytes())
            .ok_or(TorrentError::MissingKey("length".into()))?
            .as_integer()?
            .try_into()
            .map_err(|_| TorrentError::InvalidValue("'length' is negative".into()))?;

        let name = info
            .get("name".as_bytes())
            .ok_or(TorrentError::MissingKey("name".into()))?
            .as_bytes()?;

        let piece_length: usize = info
            .get("piece length".as_bytes())
            .ok_or(TorrentError::MissingKey("piece length".into()))?
            .as_integer()?
            .try_into()
            .map_err(|_| TorrentError::InvalidValue("'piece length' is negative".into()))?;

        let pieces: Vec<[u8; 20]> = info
            .get("pieces".as_bytes())
            .ok_or(TorrentError::MissingKey("pieces".into()))?
            .as_bytes()?
            .chunks(20)
            .map(|x| {
                x.try_into()
                    .map_err(|_| TorrentError::InvalidValue("pieces is not multiple of 20".into()))
            })
            .collect::<Result<Vec<[u8; 20]>, TorrentError>>()?;

        Ok(MetaInfo {
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
}

impl fmt::Debug for MetaInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Torrent")
            .field("Tracked URL", &self.announce)
            .field("Length", &self.info.length)
            .field("Info Hash", &hex::encode(self.info.hash))
            .field("Piece Length", &self.info.piece_length)
            .field(
                "Pieces",
                &self
                    .info
                    .pieces
                    .iter()
                    .map(hex::encode)
                    .collect::<Vec<String>>(),
            )
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum TorrentError {
    #[error("bencode error: {0}")]
    Bencode(#[from] crate::bencode::value::BencodeParseError),
    #[error("missing key: {0}")]
    MissingKey(String),
    #[error("invalid value: {0}")]
    InvalidValue(String),
}

pub struct Info {
    pub length: usize,
    #[allow(unused)]
    pub name: String,
    pub piece_length: usize,
    pub pieces: Vec<[u8; 20]>,
    pub hash: [u8; 20],
}
