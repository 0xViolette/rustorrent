mod bencode;
mod torrent;

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
        let decoded_value = bencode::parse(encoded_value.as_bytes())?;
        println!("{:?}", decoded_value);
    } else if command == "info" {
        eprintln!("Logs:");
        let filename = args.get(2).context("usage: rustorrent info <filename>")?;
        let torrent_file = std::fs::read(filename).context(format!("failed to read {filename}"))?;
        let parsed_torrent =
            torrent::from_bytes(&torrent_file).context("failed to parse torrent file")?;
        println!("{:#?}", parsed_torrent);
    } else if command == "peers" {
        eprintln!("Logs:");
        let filename = args.get(2).context("usage: rustorrent peers <filename>")?;
        let torrent_file = std::fs::read(filename).context(format!("failed to read {filename}"))?;
        let parsed_torrent =
            torrent::from_bytes(&torrent_file).context("failed to parse torrent file")?;
        parsed_torrent
            .discover_peers()
            .context("failed to get peers from tracker")?
            .peers
            .iter()
            .for_each(|x| println!("{}", x));
    } else {
        anyhow::bail!("unknown command: {command}\nusage: rustorrent <decode|info|peers> <input>");
    }

    Ok(())
}
