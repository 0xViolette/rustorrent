use std::net::{SocketAddrV4, TcpStream};
use thiserror::Error;

pub struct Peer {
    pub id: [u8; 20],
    pub addr: SocketAddrV4,
    pub conn: TcpStream,
    pub active: bool,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PeerMessageType {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
}

impl TryFrom<u8> for PeerMessageType {
    type Error = PeerError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Choke),
            1 => Ok(Self::Unchoke),
            2 => Ok(Self::Interested),
            3 => Ok(Self::NotInterested),
            4 => Ok(Self::Have),
            5 => Ok(Self::Bitfield),
            6 => Ok(Self::Request),
            7 => Ok(Self::Piece),
            8 => Ok(Self::Cancel),
            _ => Err(PeerError::InvalidMessageType(value)),
        }
    }
}

pub enum PeerMessage {
    KeepAlive,
    Message {
        length: u32,
        kind: PeerMessageType,
        payload: Vec<u8>,
    },
}

impl PeerMessage {
    pub fn new(kind: PeerMessageType, payload: &[u8]) -> Self {
        Self::Message {
            length: (1 + payload.len()) as u32,
            kind,
            payload: payload.into(),
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            Self::KeepAlive => vec![0, 0, 0, 0],
            Self::Message {
                length,
                kind,
                payload,
            } => length
                .to_be_bytes()
                .into_iter()
                .chain([*kind as u8])
                .chain(payload.iter().copied())
                .collect(),
        }
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
    #[error("Invalid peer message type: {0}")]
    InvalidMessageType(u8),
}
