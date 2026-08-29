use std::net::{SocketAddrV4, TcpStream};
use thiserror::Error;

pub struct Peer {
    pub id: [u8; 20],
    pub addr: SocketAddrV4,
    pub conn: TcpStream,
}

pub struct PeerMessage {
    pub length: u32,
    pub kind: u8,
    pub payload: Vec<u8>,
}

impl PeerMessage {
    pub fn new(kind: u8, msg: &[u8]) -> Self {
        Self {
            length: (1 + msg.len()) as u32,
            kind,
            payload: msg.into(),
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut msg = Vec::with_capacity(4 + self.length as usize);
        msg.extend_from_slice(&self.length.to_be_bytes());
        msg.push(self.kind);
        msg.extend_from_slice(&self.payload);

        msg
    }
}

#[derive(Debug, Error)]
pub enum PeerError {
    #[error("IO: {0}")]
    Tcp(#[from] std::io::Error),
    #[error("No Connection: {0}")]
    NoConnection(String),
    #[error("Peer sent invalid piece")]
    InvalidPiece,
}
