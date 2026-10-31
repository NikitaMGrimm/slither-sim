//! Authored synthetic bytes and pings; no network.
use slither_client::server_directory::*;
use std::{collections::BTreeMap, net::SocketAddr};

fn encode(records: &[[u8; 28]]) -> Vec<u8> {
    let mut result = vec![b'a'];
    let mut offset = 0;
    for byte in records.iter().flatten() {
        for nibble in [byte >> 4, byte & 15] {
            result.push(b'a' + (nibble + offset) % 26);
            offset = (offset + 7) % 26;
        }
    }
    result
}
fn record(active: u8, port: u16, population: u16, cluster: u8, id: u16) -> [u8; 28] {
    let mut r = [0; 28];
    r[0] = active;
    r[1..5].copy_from_slice(&[192, 0, 2, 1]);
    r[21..23].copy_from_slice(&port.to_be_bytes());
    r[23..25].copy_from_slice(&population.to_be_bytes());
    r[25] = cluster;
    r[26..28].copy_from_slice(&id.to_be_bytes());
    r
}

#[test]
fn directory_rotates_across_records_and_splits_ipv6_clusters() {
    let a = record(26, 445, 400, 7, 1024);
    let mut b = record(27, 446, 65535, 8, 65534);
    b[5..21].copy_from_slice(
        &"2001:db8::1234"
            .parse::<std::net::Ipv6Addr>()
            .unwrap()
            .octets(),
    );
    let servers = decode_directory(&encode(&[a, b])).unwrap();
    assert_eq!(servers.len(), 3);
    assert_eq!(servers[0].address.to_string(), "192.0.2.1:445");
    assert_eq!(servers[0].population, 400);
    assert_eq!(servers[0].id, 1024);
    assert!(servers[0].active);
    assert!(!servers[1].active);
    assert_eq!(servers[2].cluster, 1008);
    assert_eq!(servers[2].endpoint(), "ws://[2001:db8::1234]:446/slither");
    for bad in [
        vec![],
        vec![b'a'],
        vec![b'b'; 57],
        vec![b'a'; 56],
        vec![0; 57],
        encode(&[record(0, 0, 20, 1, 1)]),
    ] {
        assert!(decode_directory(&bad).is_err());
    }
    let mut invalid_nibble = encode(&[a]);
    invalid_nibble[1] = b'z';
    assert!(decode_directory(&invalid_nibble).is_err());
    assert!(decode_directory(&vec![b'a'; 300_000]).is_err());
}

fn server(cluster: u16, population: u16, active: bool, port: u16) -> Server {
    Server {
        address: SocketAddr::from(([192, 0, 2, 5], port)),
        population,
        cluster,
        id: port,
        active,
    }
}
#[test]
fn fastest_cluster_then_population_plus_five_weights_and_original_fallback() {
    let servers = [
        server(1, 90, true, 448),
        server(2, 5, true, 447),
        server(2, 25, true, 446),
        server(2, 65535, false, 445),
    ];
    let pings = BTreeMap::from([(1, 50), (2, 10)]);
    assert_eq!(select_server(&servers, &pings, 0.0).unwrap(), 1);
    assert_eq!(select_server(&servers, &pings, 0.25).unwrap(), 1);
    assert_eq!(select_server(&servers, &pings, 0.25001).unwrap(), 2);
    assert_eq!(select_server(&servers, &pings, 0.9999).unwrap(), 2);
    assert_eq!(
        select_server(&servers, &BTreeMap::from([(1, 10), (2, 10)]), 0.5).unwrap(),
        2
    );
    assert_eq!(select_server(&servers, &BTreeMap::new(), 0.99).unwrap(), 2);
    assert!(select_server(&[], &pings, 0.0).is_err());
    assert!(select_server(&servers, &pings, 1.0).is_err());
    assert!(select_server(&servers, &pings, f64::NAN).is_err());
    // Even an inactive fallback seed can be replaced by an equally pinged populated record.
    assert_eq!(
        select_server(
            &[server(1, 21, false, 450), server(2, 0, false, 451)],
            &BTreeMap::new(),
            0.99
        )
        .unwrap(),
        0
    );
}
