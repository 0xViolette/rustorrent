use ::rustorrent::{bencode, peer, torrent, tracker};
use clap::{Parser, Subcommand};
use rustorrent::rustorrent;
use std::net::SocketAddrV4;
use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Parser)]
#[command(name = "rustorrent")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Decode {
        encoded_value: String,
    },

    Info {
        filename: PathBuf,
    },

    Peers {
        filename: PathBuf,
    },

    Handshake {
        filename: PathBuf,
        peer_addr: SocketAddrV4,
    },

    #[command(name = "download_piece")]
    DownloadPiece {
        filename: PathBuf,
        peer_addr: SocketAddrV4,
        piece_index: u32,
    },

    Download {
        filename: PathBuf,

        #[arg(short, long)]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Decode { encoded_value } => {
            let decoded_value = bencode::decode(encoded_value.as_bytes())?;
            println!("{:?}", decoded_value);
        }

        Command::Info { filename } => {
            let torrent_file = std::fs::read(&filename)
                .context(format!("failed to read {}", filename.display()))?;
            let parsed_torrent = torrent::MetaInfo::from_bytes(&torrent_file)
                .context("failed to parse torrent file")?;
            println!("{:#?}", parsed_torrent);
        }
        Command::Peers { filename } => {
            let torrent_file = std::fs::read(&filename)
                .context(format!("failed to read {}", filename.display()))?;
            let parsed_torrent = torrent::MetaInfo::from_bytes(&torrent_file)
                .context("failed to parse torrent file")?;

            let request = tracker::build_request(&parsed_torrent);
            tracker::announce(&request)
                .context("failed to get peers from tracker")?
                .peers
                .iter()
                .for_each(|x| println!("{}", x));
        }
        Command::Handshake {
            filename,
            peer_addr,
        } => {
            let torrent_file = std::fs::read(&filename)
                .context(format!("failed to read {}", filename.display()))?;
            let parsed_torrent = torrent::MetaInfo::from_bytes(&torrent_file)
                .context("failed to parse torrent file")?;

            println!(
                "Peer ID: {}",
                peer::get_peer_id(&parsed_torrent, &peer_addr)?
            );
        }

        Command::DownloadPiece {
            filename,
            peer_addr,
            piece_index,
        } => {
            let torrent_file = std::fs::read(&filename)
                .context(format!("failed to read {}", filename.display()))?;
            let parsed_torrent = torrent::MetaInfo::from_bytes(&torrent_file)
                .context("failed to parse torrent file")?;

            let mut peer = peer::connect(&parsed_torrent, &peer_addr)?;
            peer::download_piece(&parsed_torrent, &mut peer, piece_index)?;
        }

        Command::Download { filename, output } => {
            let client = rustorrent::Client::new(filename)?;
            client.download(output).context("Failed to download")?;
            println!("downloaded successfully");
        }
    }

    Ok(())
}
