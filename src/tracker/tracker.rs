use super::value::*;
use std::net::{Ipv4Addr, SocketAddrV4};
use url::Url;

use crate::{bencode, torrent::MetaInfo};

pub fn build_request(torrent: &MetaInfo) -> TrackerRequest<'_> {
    TrackerRequest {
        url: &torrent.announce,
        info_hash: &torrent.info.hash,
        peer_id: PEER_ID,
        uploaded: 0,
        downloaded: 0,
        left: torrent.info.length,
        compact: 1,
        port: 6881,
    }
}

pub fn announce(params: &TrackerRequest) -> Result<TrackerResponse, TrackerError> {
    let mut url = Url::try_from(params.url).map_err(|e| TrackerError::InvalidUrl(e.to_string()))?;
    if !url.scheme().starts_with("http") {
        return Err(TrackerError::InvalidUrl(url.into()));
    }

    url.set_query(Some(&params.to_query()));

    let resp = reqwest::blocking::get(url)?.bytes()?;
    let decoded_resp = bencode::decode(&resp)?;

    let dict = decoded_resp.as_dict()?;

    let interval = dict
        .get("interval".as_bytes())
        .ok_or(TrackerError::InvalidResponse("missing 'interval'".into()))?
        .as_integer()? as usize;

    let peers = dict
        .get("peers".as_bytes())
        .ok_or(TrackerError::InvalidResponse("missing 'peers'".into()))?
        .as_bytes()?
        .chunks_exact(6)
        .filter(|x| x.len() == 6)
        .map(|x| {
            SocketAddrV4::new(
                Ipv4Addr::new(x[0], x[1], x[2], x[3]),
                u16::from_be_bytes([x[4], x[5]]),
            )
        })
        .collect::<Vec<SocketAddrV4>>();

    Ok(TrackerResponse { interval, peers })
}
