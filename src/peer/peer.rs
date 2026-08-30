use crate::torrent::model::MetaInfo;
use crate::{peer::value::*, tracker};
use std::{
    io::{Read, Write},
    net::SocketAddrV4,
};

const PROTOCOL_LEN: u8 = 19;
const PROTOCOL_NAME: &[u8] = b"BitTorrent protocol";
const HANDSHAKE_SIZE: usize = 1 + 19 + 8 + 20 + 20; // = 68

const BLOCK_SIZE: u32 = 1 << 14;

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
        active: false,
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

    pub fn receive(&mut self) -> Result<PeerMessage, PeerError> {
        println!("reading length");

        let mut length_bytes = [0u8; 4];
        self.conn.read_exact(&mut length_bytes)?;

        let length = u32::from_be_bytes(length_bytes);

        println!("got length: {length}");

        if length == 0 {
            return Ok(PeerMessage::KeepAlive);
        }

        println!("reading kind");

        let mut kind_byte = [0u8; 1];
        self.conn.read_exact(&mut kind_byte)?;

        let kind = kind_byte[0].try_into()?;

        println!("got kind: {:?}", kind);

        println!("reading {} payload bytes", length - 1);

        let mut payload = vec![0; length as usize - 1];
        self.conn.read_exact(&mut payload)?;

        Ok(PeerMessage::Message {
            length,
            kind,
            payload,
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
    // recieve bitfield message
    // if !peer.active {
    //     let bitfield = peer.recieve()?;
    //     assert_eq!(bitfield.kind, PeerMessageType::Bitfield);
    //     println!("bitfield recieved");
    //     peer.active = true;
    // }

    // send interested
    println!("sending interested");
    peer.send(&PeerMessage::new(PeerMessageType::Interested, &[]))?;

    // loop until unchoke
    println!("waiting for unchoke");
    loop {
        match peer.receive()? {
            PeerMessage::Message {
                kind: PeerMessageType::Have,
                payload,
                ..
            } => {
                let index = u32::from_be_bytes(payload.as_slice().try_into().unwrap());

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
                println!("got message: {:?}", kind);
            }

            PeerMessage::KeepAlive => {}
        }
    }

    println!("starting piece requests");

    // send request

    let piece_length = std::cmp::min(
        torrent.info.piece_length,
        torrent.info.length - torrent.info.piece_length * piece_index as usize,
    ) as u32;

    let num_blocks = piece_length.div_ceil(BLOCK_SIZE);

    let mut piece = vec![];
    for i in 0..num_blocks {
        let begin = BLOCK_SIZE * i;

        let length = (piece_length - begin).min(BLOCK_SIZE);

        let payload = [
            piece_index.to_be_bytes(),
            begin.to_be_bytes(),
            length.to_be_bytes(),
        ]
        .concat();

        let request = PeerMessage::new(PeerMessageType::Request, &payload);

        println!("request: {:02x?}", request.as_bytes());

        peer.send(&request)?;

        // recieve piece
        let piece_msg = loop {
            match peer.receive()? {
                PeerMessage::Message {
                    kind: PeerMessageType::Piece,
                    ..
                } => break payload,

                PeerMessage::KeepAlive => continue,

                _ => continue,
            }
        };

        piece.extend_from_slice(&piece_msg);
    }

    if !torrent.verify_piece(&piece, piece_index as usize) {
        return Err(PeerError::InvalidPiece);
    }

    Ok(piece)
}

pub fn download(torrent: &MetaInfo) -> Result<Vec<u8>, PeerError> {
    let peer_addrs = tracker::announce(&tracker::build_request(torrent))
        .unwrap()
        .peers;

    let mut full = vec![];
    let mut peers: Vec<Peer> = peer_addrs
        .iter()
        .map(|x| connect(torrent, x))
        .collect::<Result<Vec<_>, _>>()?;
    let num_peers = peers.len();
    println!("{num_peers}");
    println!("{}", torrent.info.pieces.len());
    for piece_index in 0..torrent.info.pieces.len() {
        let piece = download_piece(
            torrent,
            &mut peers[piece_index.min(num_peers - 1)],
            piece_index as u32,
        )?;
        println!("downloaded {} piece", piece_index);
        full.extend_from_slice(&piece);
    }

    Ok(full)
}
