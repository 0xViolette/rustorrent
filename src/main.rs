use rustorrent::{bencode, peer, torrent, tracker};
use std::env;

use anyhow::{Context, Result};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let command = args
        .get(1)
        .context("usage: rustorrent <decode|info|peers> <input>")?;

    if command == "decode" {
        eprintln!("Logs:");
        let encoded_value = args
            .get(2)
            .context("usage: rustorrent decode <encoded_value>")?;
        let decoded_value = bencode::decode(encoded_value.as_bytes())?;
        println!("{:?}", decoded_value);
    } else if command == "info" {
        eprintln!("Logs:");
        let filename = args.get(2).context("usage: rustorrent info <filename>")?;
        let torrent_file = std::fs::read(filename).context(format!("failed to read {filename}"))?;
        let parsed_torrent =
            torrent::MetaInfo::from_bytes(&torrent_file).context("failed to parse torrent file")?;
        println!("{:#?}", parsed_torrent);
    } else if command == "peers" {
        eprintln!("Logs:");
        let filename = args.get(2).context("usage: rustorrent peers <filename>")?;
        let torrent_file = std::fs::read(filename).context(format!("failed to read {filename}"))?;
        let parsed_torrent =
            torrent::MetaInfo::from_bytes(&torrent_file).context("failed to parse torrent file")?;

        let request = tracker::build_request(&parsed_torrent);
        tracker::announce(&request)
            .context("failed to get peers from tracker")?
            .peers
            .iter()
            .for_each(|x| println!("{}", x));
    } else if command == "handshake" {
        eprintln!("Logs:");
        let filename = args
            .get(2)
            .context("usage: rustorrent handshake <filename> <peer_ip>:<peer_port>")?;

        let peer_addr = args
            .get(3)
            .context("usage: rustorrent handshake <filename> <peer_ip>:<peer_port>")?
            .parse::<std::net::SocketAddrV4>()?;

        let torrent_file = std::fs::read(filename).context(format!("failed to read {filename}"))?;
        let parsed_torrent =
            torrent::MetaInfo::from_bytes(&torrent_file).context("failed to parse torrent file")?;

        println!(
            "Peer ID: {}",
            peer::get_peer_id(&parsed_torrent, &peer_addr)?
        );
    } else if command == "download_piece" {
        eprintln!("Logs:");
        let filename = args.get(2).context(
            "usage: rustorrent download_piece <filename> <peer_ip>:<peer_port> <piece_index>",
        )?;

        let peer_addr = args
            .get(3)
            .context(
                "usage: rustorrent download_piece <filename> <peer_ip>:<peer_port> <piece_index>",
            )?
            .parse::<std::net::SocketAddrV4>()?;

        let piece_index = args
            .get(4)
            .context(
                "usage: rustorrent download_piece <filename> <peer_ip>:<peer_port> <piece_index>",
            )?
            .parse::<u32>()?;

        let torrent_file = std::fs::read(filename).context(format!("failed to read {filename}"))?;
        let parsed_torrent =
            torrent::MetaInfo::from_bytes(&torrent_file).context("failed to parse torrent file")?;

        let mut peer = peer::connect(&parsed_torrent, &peer_addr)?;
        peer::download_piece(&parsed_torrent, &mut peer, piece_index)?;
    } else if command == "download" {
        eprintln!("Logs:");
        let filename = args
            .get(2)
            .context("usage: rustorrent download <filename> <output>")?;

        let output_filename = args
            .get(3)
            .context("usage: rustorrent download <filename> <output>")?;

        let torrent_file = std::fs::read(filename).context(format!("failed to read {filename}"))?;
        let parsed_torrent =
            torrent::MetaInfo::from_bytes(&torrent_file).context("failed to parse torrent file")?;

        let content = peer::download(&parsed_torrent)?;
        println!("downoaded successfully");

        std::fs::write(output_filename, content)?;
    } else {
        anyhow::bail!("unknown command: {command}\nusage: rustorrent <decode|info|peers> <input>");
    }

    Ok(())
}
