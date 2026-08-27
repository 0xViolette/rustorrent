use crate::torrent::MetaInfo;
use crate::{peer::value::*, tracker};
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

pub fn download_piece(_torrent: &MetaInfo, peer: &mut Peer) -> Result<(), PeerError> {
    // recieve bitfield message
    let bitfield = peer.recieve()?;
    println!("recieved msg, kind = {}(5)", bitfield.kind);

    // send interested
    peer.send(&PeerMessage::new(2, &[]))?;

    // recieve unchoke msg
    let unchoke_msg = peer.recieve()?;
    println!("recieved msg, kind = {}(1)", unchoke_msg.kind);

    // send request
    let index = 0u32.to_be_bytes();
    let begin = (0 * 2u32.pow(14)).to_be_bytes();
    let length = 2u32.pow(14).to_be_bytes();

    let mut payload = vec![];
    payload.extend_from_slice(&index);
    payload.extend_from_slice(&begin);
    payload.extend_from_slice(&length);

    peer.send(&PeerMessage::new(6, &payload))?;

    // recieve piece
    let piece_msg = peer.recieve()?;
    println!("recieved msg, kind = {}(7)", piece_msg.kind);

    Ok(())
}
