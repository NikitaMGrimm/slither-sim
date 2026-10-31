//! Bounded directory decoding and selection. No acquisition or transport.
#![allow(missing_docs)]
use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};
type Result<T> = std::result::Result<T, String>;
pub const DIRECTORY_LIMIT: usize = 256 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Server {
    pub address: SocketAddr,
    pub population: u16,
    pub cluster: u16,
    pub id: u16,
    pub active: bool,
}
impl Server {
    pub fn endpoint(&self) -> String {
        format!("ws://{}/slither", self.address)
    }
}

pub fn decode_directory(encoded: &[u8]) -> Result<Vec<Server>> {
    let encoded = encoded.trim_ascii();
    if encoded.len() > DIRECTORY_LIMIT
        || encoded.first() != Some(&b'a')
        || encoded.len() <= 1
        || !(encoded.len() - 1).is_multiple_of(56)
    {
        return Err("Invalid directory length/version".into());
    }
    let mut servers = Vec::new();
    for (record_index, chars) in encoded[1..].as_chunks::<56>().0.iter().enumerate() {
        let mut record = [0u8; 28];
        for (index, pair) in chars.as_chunks::<2>().0.iter().enumerate() {
            let mut value = 0;
            for (half, byte) in pair.iter().enumerate() {
                if !byte.is_ascii_lowercase() {
                    return Err("Invalid directory character".into());
                }
                let shift = ((record_index * 56 + index * 2 + half) * 7) % 26;
                let nibble = (usize::from(byte - b'a') + 26 - shift) % 26;
                if nibble > 15 {
                    return Err("Invalid directory nibble".into());
                }
                value = value * 16 + nibble as u8;
            }
            record[index] = value;
        }
        let port = u16::from_be_bytes([record[21], record[22]]);
        if port == 0 {
            return Err("Invalid directory port".into());
        }
        let base = Server {
            address: SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(record[1], record[2], record[3], record[4])),
                port,
            ),
            population: u16::from_be_bytes([record[23], record[24]]),
            cluster: u16::from(record[25]),
            id: u16::from_be_bytes([record[26], record[27]]),
            active: record[0] <= 26,
        };
        let ipv6 = Ipv6Addr::from(<[u8; 16]>::try_from(&record[5..21]).expect("record span"));
        if !ipv6.is_unspecified() {
            let mut v6 = base.clone();
            v6.address = SocketAddr::new(IpAddr::V6(ipv6), port);
            v6.cluster += 1000;
            servers.push(base);
            servers.push(v6);
        } else {
            servers.push(base);
        }
        if servers.len() > 4096 {
            return Err("Directory server limit".into());
        }
    }
    Ok(servers)
}

/// Fresh selection has no historical exclusions. Tied cluster minima prefer the higher ID.
pub fn select_server(servers: &[Server], pings: &BTreeMap<u16, u32>, unit: f64) -> Result<usize> {
    if servers.is_empty() || !(0.0..1.0).contains(&unit) {
        return Err("Invalid server selection input".into());
    }
    let best = pings
        .iter()
        .min_by_key(|(id, ms)| (**ms, std::cmp::Reverse(**id)));
    if let Some((&cluster, _)) = best {
        let eligible: Vec<_> = servers
            .iter()
            .enumerate()
            .filter(|(_, s)| s.cluster == cluster && s.active)
            .collect();
        let total: u64 = eligible
            .iter()
            .map(|(_, s)| u64::from(s.population) + 5)
            .sum();
        let mut cumulative = 0.0;
        for (position, &(index, server)) in eligible.iter().enumerate() {
            cumulative += (f64::from(server.population) + 5.0) / total as f64;
            if cumulative >= unit || position + 1 == eligible.len() {
                return Ok(index);
            }
        }
    }
    // Original fallback: stable port order, random seed, then reverse scan with population >20.
    let mut order: Vec<_> = (0..servers.len()).collect();
    order.sort_by_key(|i| servers[*i].address.port());
    let mut chosen = order[(unit * order.len() as f64) as usize];
    let ping = |s: &Server| {
        pings
            .get(&s.cluster)
            .copied()
            .unwrap_or(if s.active { 9_999_998 } else { 9_999_999 })
    };
    for index in order.into_iter().rev() {
        if servers[index].population > 20 && ping(&servers[index]) <= ping(&servers[chosen]) {
            chosen = index;
        }
    }
    Ok(chosen)
}
