# 🧲 rustorrent

> A minimal BitTorrent client written in Rust (inspired by CodeCrafters challenge)

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)

## Features

- **Bencode** — hand-rolled `decode` / `encode` with `Dict` / `List` / `Bytes` / `Integer` (`src/bencode`)
- **Torrent Parsing** — single-file `length` / `name` / `piece length` / `pieces`, `info_hash = SHA1(bencode(info))` (`src/torrent/model.rs`)
- **Tracker** — HTTP `compact=1` announce, `peers` as 6-byte `IPv4:port` chunks (`src/tracker/tracker.rs`)
- **Peer Wire** — `19 + BitTorrent protocol` 68B handshake, Bitfield → Interested → Unchoke → Request / Piece (`16 KiB` blocks) (`src/peer/peer.rs`)
- **Download** — sequential piece fetch, per-piece SHA1 `verify_piece`, assemble to file (`src/rustorrent.rs`)

## Usage

```sh
cargo run -- decode "d3:foo3:bar5:helloi52ee"
cargo run -- info sample.torrent
cargo run -- peers sample.torrent
cargo run -- handshake <torrent> <ip:port>
cargo run -- download_piece <torrent> <ip:port> <piece-index>
cargo run -- download <torrent> -o <output>
```

Examples:

```sh
cargo run -- decode "l5:hello5:worldi42ee"
cargo run -- info sample.torrent
# Tracked URL: http://bittorrent-test-tracker.codecrafters.io/announce
# Length: 92063, Piece Length: 32768, Info Hash: d69f91e6b2ae4c542468d1073a71d4ea13879a7f

cargo run -- peers sample.torrent
# 165.232.38.164:51433
# ...

cargo run -- handshake sample.torrent 165.232.38.164:51433
# Peer ID: ...

cargo run -- download sample.torrent -o /tmp/sample.out
```

## Project Layout

```
src/
  bencode/  decode.rs  encode.rs  model.rs  value.rs
  torrent/  model.rs              # MetaInfo / Info / verify_piece()
  tracker/  tracker.rs value.rs   # PEER_ID, TrackerRequest::to_query(), announce()
  peer/     peer.rs value.rs peer_id.rs  # connect(), wait_for_bitfield/unchoke, fetch_blocks()
  rustorrent.rs                   # Client::new() / download()
  main.rs                         # clap CLI
```

## Limitations

- HTTP tracker only, `compact=1` required; `InvalidResponse("missing 'peers'")` on empty tracker.
- Single-file torrents only; no `announce-list`, DHT, magnet, UDP, or encryption.
- Sequential download, one peer per piece cycle; verbose `reading length/kind` logs.
- Panics on tracker failure (`peer.rs:240 expect("tracker announce failed")`).
