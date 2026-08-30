use std::{
    io::{Read, Write},
    net::SocketAddrV4,
};

use crate::{
    peer::value::{Peer, PeerError, PeerMessage, PeerMessageType},
    torrent::model::MetaInfo,
    tracker,
};

const PROTOCOL_LEN: u8 = 19;
const PROTOCOL_NAME: &[u8] = b"BitTorrent protocol";
const HANDSHAKE_SIZE: usize = 1 + 19 + 8 + 20 + 20; // = 68
const INFO_HASH_RANGE: std::ops::Range<usize> = 28..48;
const PEER_ID_RANGE: std::ops::Range<usize> = 48..68;

const BLOCK_SIZE: u32 = 16_384; // 1 << 14

pub fn connect(torrent: &MetaInfo, peer_addr: &SocketAddrV4) -> Result<Peer, PeerError> {
    let handshake_req = build_handshake_message(&torrent.info.hash, tracker::PEER_ID);

    let mut tcp_stream = std::net::TcpStream::connect(peer_addr)?;

    tcp_stream.write_all(&handshake_req)?;

    let mut handshake_resp = [0u8; HANDSHAKE_SIZE];
    tcp_stream.read_exact(&mut handshake_resp)?;

    Ok(Peer {
        id: handshake_resp[PEER_ID_RANGE]
            .try_into()
            .expect("peer id must be 20 bytes"),
        addr: *peer_addr,
        conn: tcp_stream,
        active: false,
    })
}

fn build_handshake_message(info_hash: &[u8; 20], peer_id: &[u8; 20]) -> [u8; HANDSHAKE_SIZE] {
    let mut buf = [0u8; HANDSHAKE_SIZE];
    buf[0] = PROTOCOL_LEN;
    buf[1..20].copy_from_slice(PROTOCOL_NAME);
    // 20..28 reserved bytes stay zero
    buf[INFO_HASH_RANGE].copy_from_slice(info_hash);
    buf[PEER_ID_RANGE].copy_from_slice(peer_id);

    buf
}

impl Peer {
    pub fn send(&mut self, msg: &PeerMessage) -> Result<(), PeerError> {
        self.conn.write_all(&msg.as_bytes())?;

        Ok(())
    }

    pub fn receive(&mut self) -> Result<PeerMessage, PeerError> {
        println!("reading length");

        let mut len_buf = [0u8; 4];
        self.conn.read_exact(&mut len_buf)?;

        let length = u32::from_be_bytes(len_buf);

        println!("got length: {length}");

        if length == 0 {
            return Ok(PeerMessage::KeepAlive);
        }

        println!("reading kind");

        let mut type_buf = [0u8; 1];
        self.conn.read_exact(&mut type_buf)?;

        let kind = type_buf[0].try_into()?;

        println!("got kind: {kind:?}");

        println!("reading {} payload bytes", length - 1);

        let mut payload_buf = vec![0; length as usize - 1];
        self.conn.read_exact(&mut payload_buf)?;

        Ok(PeerMessage::Message {
            length,
            kind,
            payload: payload_buf,
        })
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
    if !peer.active {
        wait_for_bitfield(peer)?;
    }

    wait_for_unchoke(peer)?;

    println!("starting piece requests");

    let piece_len = piece_length(torrent, piece_index);
    let piece_data = fetch_blocks(peer, piece_index, piece_len)?;

    println!(
        "verify_piece: {}",
        torrent.verify_piece(&piece_data, piece_index as usize)
    );

    if !torrent.verify_piece(&piece_data, piece_index as usize) {
        return Err(PeerError::InvalidPiece);
    }

    Ok(piece_data)
}

fn wait_for_bitfield(peer: &mut Peer) -> Result<(), PeerError> {
    loop {
        let msg = peer.receive()?;
        match msg {
            PeerMessage::KeepAlive => continue,
            PeerMessage::Message {
                kind: PeerMessageType::Bitfield,
                payload,
                ..
            } => {
                peer.active = true;
                println!("recieved bitfield message");
                println!("{:02x?}", payload.as_slice());
                break;
            }
            PeerMessage::Message { kind, .. } => {
                println!("expected bitfield, recieved: {:?}", kind);
                return Err(PeerError::InvalidPiece);
            }
        }
    }
    Ok(())
}

fn wait_for_unchoke(peer: &mut Peer) -> Result<(), PeerError> {
    println!("sending interested");
    peer.send(&PeerMessage::new(PeerMessageType::Interested, &[]))?;

    println!("waiting for unchoke");
    loop {
        match peer.receive()? {
            PeerMessage::Message {
                kind: PeerMessageType::Have,
                payload,
                ..
            } => {
                let index = u32::from_be_bytes(
                    payload
                        .as_slice()
                        .try_into()
                        .expect("Have payload must be 4 bytes"),
                );

                println!("peer has piece {index}");
            }

            PeerMessage::Message {
                kind: PeerMessageType::Unchoke,
                ..
            } => {
                println!("got unchoke");
                break;
            }

            PeerMessage::Message { kind, .. } => {
                println!("got message: {kind:?}");
            }

            PeerMessage::KeepAlive => {}
        }
    }
    Ok(())
}

fn piece_length(torrent: &MetaInfo, piece_index: u32) -> u32 {
    let piece_idx = piece_index as usize;
    std::cmp::min(
        torrent.info.piece_length,
        torrent.info.length - torrent.info.piece_length * piece_idx,
    ) as u32
}

fn fetch_blocks(peer: &mut Peer, piece_index: u32, piece_len: u32) -> Result<Vec<u8>, PeerError> {
    let block_count = piece_len.div_ceil(BLOCK_SIZE);

    let mut piece_data = Vec::with_capacity(piece_len as usize);
    for block_idx in 0..block_count {
        let begin = BLOCK_SIZE * block_idx;
        let length = (piece_len - begin).min(BLOCK_SIZE);

        let payload = [
            piece_index.to_be_bytes(),
            begin.to_be_bytes(),
            length.to_be_bytes(),
        ]
        .concat();

        let request = PeerMessage::new(PeerMessageType::Request, &payload);

        println!("request: {:02x?}", request.as_bytes());

        peer.send(&request)?;

        let piece_payload = loop {
            match peer.receive()? {
                PeerMessage::Message {
                    kind: PeerMessageType::Piece,
                    payload,
                    ..
                } => break payload,

                PeerMessage::KeepAlive => continue,

                _ => continue,
            }
        };

        piece_data.extend_from_slice(&piece_payload[8..]);
    }
    Ok(piece_data)
}

pub fn download(torrent: &MetaInfo) -> Result<Vec<u8>, PeerError> {
    let tracker_peers = tracker::announce(&tracker::build_request(torrent))
        .expect("tracker announce failed")
        .peers;

    let mut torrent_data = Vec::new();
    let mut connected_peers: Vec<Peer> = tracker_peers
        .iter()
        .map(|addr| connect(torrent, addr))
        .collect::<Result<Vec<_>, _>>()?;
    let num_peers = connected_peers.len();
    for piece_index in 0..torrent.info.pieces.len() {
        let piece = download_piece(
            torrent,
            &mut connected_peers[piece_index.min(num_peers - 1)],
            piece_index as u32,
        )?;
        println!("downloaded {} piece", piece_index);
        torrent_data.extend_from_slice(&piece);
    }

    Ok(torrent_data)
}
