mod bencode;
mod torrent;

use std::env;

fn decode_bencoded_value(encoded_value: &[u8]) -> serde_json::Value {
    if let Ok((val, _)) = bencode::parse_value(encoded_value) {
        val.convert()
    } else {
        panic!("malformed encoded value");
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        eprintln!("Logs:");

        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(&encoded_value.clone().into_bytes());

        println!("{}", decoded_value.to_string());
    } else if command == "info" {
        eprintln!("Logs:");
        let filename = &args[2];
        let torrent_file = std::fs::read(filename).expect("File not found");
        let parsed_torrent =
            torrent::from_bytes(&torrent_file).unwrap_or_else(|_| panic!("Couldn't parse torrent"));

        println!("{:#?}", parsed_torrent);
    } else if command == "peers" {
        eprintln!("Logs:");
        let filename = &args[2];
        let torrent_file = std::fs::read(filename).expect("File not found");
        let parsed_torrent =
            torrent::from_bytes(&torrent_file).unwrap_or_else(|_| panic!("Couldn't parse torrent"));
        parsed_torrent.get_peers_from_tracker().unwrap();
    } else {
        println!("unknown command: {}", args[1]);
    }
}
