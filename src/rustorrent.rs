use std::{
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;

use crate::{
    peer::{self, Peer, peer_id::PeerID, value::PeerError},
    torrent::{self, model::Torrent},
};

pub struct Client {
    torrent: Torrent,
    id: PeerID,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("couldn't read file: {path}")]
    Read { source: io::Error, path: PathBuf },
    #[error("couldn't write to file: {path}")]
    Write { source: io::Error, path: PathBuf },
    #[error("couldn't parse torrent file")]
    MetaInfoParse(#[from] torrent::MetaInfoParseError),
    #[error("download failed: {0}")]
    Download(#[from] PeerError),
}

impl Client {
    pub fn new(torrent_filepath: impl AsRef<Path>) -> Result<Self, ClientError> {
        let bytes = fs::read(&torrent_filepath).map_err(|source| ClientError::Read {
            source,
            path: torrent_filepath.as_ref().into(),
        })?;
        let torrent = Torrent::new(&bytes)?;
        Ok(Self {
            torrent,
            id: PeerID::generate(),
        })
    }

    pub fn download(&self, output_filepath: impl AsRef<Path>) -> Result<(), ClientError> {
        Ok(peer::download(&self.torrent.meta_info)?).and_then(|content| {
            std::fs::write(&output_filepath, content).map_err(|source| ClientError::Write {
                source,
                path: output_filepath.as_ref().into(),
            })
        })
    }
}
