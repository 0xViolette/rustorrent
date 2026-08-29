use crate::torrent::model::MetaInfo;
use crate::{peer::value::*, tracker};
use sha1::{Digest, Sha1};
use std::{
    io::{Read, Write},
    net::SocketAddrV4,
};

const PROTOCOL_LEN: u8 = 19;
const PROTOCOL_NAME: &[u8] = b"BitTorrent protocol";
const HANDSHAKE_SIZE: usize = 1 + 19 + 8 + 20 + 20; // = 68

pub fn connect(torrent: &MetaInfo, peer_addr: &std::net::SocketAddrV4) -> Result<Peer, PeerError> {
    let msg = build_handshake(&torrent.info.hash, tracker::PEER_ID);

    let mut stream = std::net::TcpStream::connect(peer_addr)?;

    stream.write_all(&msg)?;

    let mut response_msg = [0u8; 68];
    stream.read_exact(&mut response_msg)?;

    Ok(Peer {
        id: response_msg[48..68].try_into().unwrap(),
        addr: *peer_addr,
        conn: stream,
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

impl Peer {
    pub fn send(&mut self, msg: &PeerMessage) -> Result<(), PeerError> {
        self.conn.write_all(&msg.as_bytes())?;

        Ok(())
    }

    pub fn recieve(&mut self) -> Result<PeerMessage, PeerError> {
        let mut length_bytes = [0u8; 4];
        self.conn.read_exact(&mut length_bytes)?;
        let length = u32::from_be_bytes(length_bytes);

        let mut kind_byte = [0u8; 1];
        self.conn.read_exact(&mut kind_byte)?;
        let kind = u8::from_be_bytes(kind_byte);

        if length == 0 {
            panic!("peer message prefix length is 0");
        }

        if length == 1 {
            Ok(PeerMessage::new(kind, &[]))
        } else {
            let mut msg_bytes = vec![0; length as usize - 1];
            self.conn.read_exact(&mut msg_bytes)?;
            Ok(PeerMessage::new(kind, &msg_bytes))
        }
    }
}

pub fn get_peer_id(torrent: &MetaInfo, peer_addr: &SocketAddrV4) -> Result<String, PeerError> {
    Ok(hex::encode(connect(torrent, peer_addr)?.id))
}

pub fn download_piece(
    torrent: &MetaInfo,
    peer: &mut Peer,
    piece_index: u32,
) -> Result<Vec<u8>, PeerError> {
    // recieve bitfield message
    let _bitfield = peer.recieve()?;

    // send interested
    peer.send(&PeerMessage::new(2, &[]))?;

    // recieve unchoke msg
    let _unchoke_msg = peer.recieve()?;

    // send request

    let piece_length = if piece_index == (torrent.info.pieces.len() as u32 - 1) {
        (torrent.info.length - torrent.info.piece_length * (torrent.info.pieces.len() - 1)) as u32
    } else {
        torrent.info.piece_length as u32
    };

    let ideal_block_length = 2u32.pow(14);
    let num_blocks = if piece_length % ideal_block_length != 0 {
        piece_length / ideal_block_length + 1
    } else {
        piece_length / ideal_block_length
    };

    let index = piece_index.to_be_bytes();
    let mut piece = vec![];
    for i in 0..num_blocks {
        let begin = (ideal_block_length * i).to_be_bytes();

        let length = if i < num_blocks - 1 {
            ideal_block_length
        } else {
            piece_length - ideal_block_length * (num_blocks - 1)
        }
        .to_be_bytes();

        let mut payload = vec![];
        payload.extend_from_slice(&index);
        payload.extend_from_slice(&begin);
        payload.extend_from_slice(&length);

        peer.send(&PeerMessage::new(6, &payload))?;

        // recieve piece
        let piece_msg = peer.recieve()?;
        piece.extend(&piece_msg.payload[8..]);
    }
    let actual_hash: [u8; 20] = Sha1::digest(&piece).into();
    let expected_hash = torrent.info.pieces[piece_index as usize];

    if actual_hash == expected_hash {
        Ok(piece)
    } else {
        Err(PeerError::InvalidPiece)
    }
}

pub fn download(torrent: &MetaInfo) -> Result<Vec<u8>, PeerError> {
    let peer_addrs = tracker::announce(&tracker::build_request(torrent))
        .unwrap()
        .peers;

    let mut full = vec![];
    let mut peer1 = connect(torrent, &peer_addrs[0])?;
    let mut peer2 = connect(torrent, &peer_addrs[1])?;
    let mut peer3 = connect(torrent, &peer_addrs[2])?;
    let mut peers = [&mut peer1, &mut peer2, &mut peer3];
    for piece_index in 0..torrent.info.pieces.len() {
        full.extend(&download_piece(
            torrent,
            &mut peers[piece_index],
            piece_index as u32,
        )?);
    }

    Ok(full)
}
