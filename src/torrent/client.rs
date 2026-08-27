use super::value::*;
use std::{fs, io, path::Path};
use thiserror::Error;

pub struct Torrent {
    pub meta_info: MetaInfo,
    pub uploaded: usize,
    pub downloaded: usize,
    pub left: usize,
}

pub struct Client {
    pub peer_id: [u8; 20],
    pub torrent: Torrent,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("failed to read torrent file `{path}`: {source}")]
    ReadTorrent {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("failed to parse torrent: {0}")]
    ParseTorrent(#[from] TorrentError),
}

#[allow(dead_code)]
impl Client {
    fn new(torrent_path: &Path) -> Result<Self, ClientError> {
        let torrent = Torrent {
            meta_info: MetaInfo::from_bytes(&fs::read(torrent_path).map_err(|e| {
                ClientError::ReadTorrent {
                    path: torrent_path.display().to_string(),
                    source: e,
                }
            })?)?,
            downloaded: 0,
            uploaded: 0,
            left: 0,
        };
        Ok(Client {
            peer_id: *b"\x19\x01\xees\xbd?\xed\x81\x82Vw\xcb\x94\xdd\x87(\x05\xe9\xa2G",
            torrent,
        })
    }
}
