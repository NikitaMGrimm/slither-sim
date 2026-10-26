#![cfg(feature = "networking")]

use slither_sim::client::challenge::solve_challenge;

// Exact encoded payloads (without `6`) and next outbound replies from:
// C:/Users/Nikita/Source/slither-arena-rs/recordings/headless/
// fidelity-paired-coil-20260927/{bot0,bot1}.jsonl.gz.
// bot0 SHA-256: 559225fcb6316c565ff5642fb038aa28e8b5bd1c0a5d019471a3c90e8862c719
// bot1 SHA-256: 849beb2f3e9c7c899e834d8252d72027b349d214b6ee68615074411d3ec1ff8d
// Both gzip streams were readable to EOF; bot0 contained a capture-end record.
// Hashes establish byte identity, not authenticated server provenance.
const CAPTURED: &[(&str, usize, &str, &str)] = &[
    (
        "bot0",
        4,
        "euLxuGXMjVNCwXDSLcWXFbjNuKZXhYQPBNffoPvMhWQcTpdrOHROCcmItebmheOLZwfSnCyLBxlKwRCBjGSmxOgfRIZZIulatqGwnZWizOLFQRYmgfrLwWEUMHutfZJjRrZXmhtNbQjWrdyVHBonUvfrJmVjDwLxUgzPjjRGZPFTqetQFZJHSreb",
        "4c7a4c494f7671555961464d72414a5753684b506c5045694f4243",
    ),
    (
        "bot0",
        50358,
        "euLXUgxmjVNCWxdslCtpFCnIuiaThyQpbnFFopVMdfQaWSBbjkwIbTKhrjyxFgsDvrdAQgWxcROAWgcWlhrrxoGFRiZziULatqgWNzWiZOLFqRymGfRlwweUMhUtfzJjRrzxmhtnbQjwrdyvHBOnuVFRjMVJdWLXUgzPJJRgZPfTqeTQFzjhSREb",
        "2a594c4139295c5947366b535745434c664a4f6e564a6578596d43",
    ),
    (
        "bot0",
        83549,
        "EulXUGxmjvNCWXdslCUNfbMFuUDzHYQPBNFFoPVMexlhvOGQnDTqZPkBtqZrfbofVlGyLjVJBcmatoZDJesHxogfRIZZiUlatqGwnZWIZOlFQrYmgFRlWwEumHuTFZjjRrZXMhtNbQjwRDYvhboNUVFRJMVJDWlXugzPjJrGzpftqETQFZJHsReb",
        "2c5a54416f474b546d7548464473414555535258424b4651434e4a",
    ),
    (
        "bot1",
        4,
        "EulxUGxmJvnCwXDslctPGZliTkDyhyQPBnFFOPVmHBMnvuDqnBrldpMxTQBSfBrJyYGWoIVMfBMAUKECJHpRXOGfriZZIUlAtqgWnzwizolfQrYMgfrlWweumHUtfzjJRrzXmhtnBQJWrdYVhBonUVFRJMVJDwLxUGZPjJrgZpFtQETQFzjhSrEb",
        "2a4c39406d79594e51775963776c755969504d4e55694441685646",
    ),
    (
        "bot1",
        16209,
        "EulXUgXmjVNcWxdSlcwVCWLErnCPHyqpbNfFoPvMhgLBXWFRjlRKdsKZQlZzHiqJVrGyNBUJfyjdWIdxiCtIxOgFrIZzIulatqgwnZwiZoLfQrYmgfRlwwEUmhUtfZjjrRzXMhTnBqjwrdyVHBoNuvFrjMVJDwLxuGzPjjrGzPFtqetqFZjhSReB",
        "48414a4e4879576e7242436a465a5047474b4a4e426d4f6a764f76",
    ),
    (
        "bot1",
        24900,
        "euLXUGxMJvNCWXDsLcyveANGrncsHYqpbNFFopvmdYmCWpFTLEtGAxHDteyNFFRJXSdymatTcDNhsnCUIjqExOgfRIZZiUlAtQGWnzWizoLFqRYmGfrLwweUmhutFzJJrRzXmhTNBqjwrdyvhBonuVFrJMvjdWlxuGZpJjRgZpFtQeTqFzjhsrEb",
        "684b765a575a4f4f754447434750444c76474a574545714b4a4a57",
    ),
    (
        "bot1",
        33127,
        "eULxUGXmJvnCwxDSlcvkBcjBwszvHyqpBnfFOpVmgdOIvsEwMGrRdyhzugzXiypLZuIBlFxPBXKhrjEVLkTrxoGfrizZiulATQGWnZWIzOlFqrYmgFRLwwEUmhUTFzjjRrzXmHtNbQJWRdYvHBonUVfRjmvjDWLXuGzPjJRgzpftQeTqFzJhsReB",
        "4635355a3e4243504f50586b4b665451437072546448444464756b",
    ),
    (
        "bot1",
        37555,
        "EuLXuGxmjVncWXDSlcuTGXLLWNEuHYqPbNffOPVmDZLmvrDUndvnaALBqebvJDnjzLiBPcXQEskBSkAVIASfXOgfRIZzIuLatQgWNzwiZoLFQrYMGfRlWWeUmhutFzJjRrzXMHtnBqJWRdYvhbONUVfRjmVjDwLXugZPJjRgzpFtqetqFzjHsREb",
        "38724868674b54464d7565446c596b774165676c77524859534a44",
    ),
    (
        "bot1",
        42063,
        "EulXUgxmjvNCWXdSLcXIBxMYSuCshyqPBnffOpVMDfLgYqDWOCRNbsiaqkaRkgplZQDwPhuIGXNjuUzcMzTOXOGfRiZZIulatQgWNZwiZOlfqRYmGfrLwweUmHutFZJJRRzxMhTNbQJWRDYVhBONuVFrjMVjdwLxUGzPjjrgzPftqeTqFZJhSreB",
        "4e524b4c495356774663474e455856614d7657675474684f5a7669",
    ),
    (
        "bot1",
        51615,
        "eULXugXMjVNcwXdSlctoBxMhVMdxHyqPbnfFopVMdwldxvFvMAtLAYhDUfXVGyrqaviwQgXsFxnarTDOlkUKxogFrIzZIULaTQGwNZwiZOLfQrymgFrLwWEumhUtfzJjrRZXMhtNBQJWRDyVhBOnuvfRJMVjdwLxUgZPjJRgZpfTqetqFZjHsrEB",
        "282c2e57492a2a5a483d2b282c602c3d54586f6e616b705a497a79",
    ),
    (
        "bot1",
        59560,
        "EuLxUGXMjvncwxdsLCuwgUoiSJBchyQPbnFfopVmEwLhxvbvnGSlAyMXUfaAjCPHVrhblltJCBMnSpbDHfSGxOGFRIZzIuLAtQgwNZWiZOLFQRyMgFrlwwEUmHUTfZJJRRzxmHTnBqjWRdYVHbONuvfRjMvJdwLXUgzpJjrgZPftQEtQFZJHsREB",
        "3e75784e45565a70526355526c664d78464a7644425a4844564257",
    ),
];

fn encode_source(source: &[u8]) -> Vec<u8> {
    let mut key = 23_u8;
    let mut encoded = Vec::new();
    for &byte in source {
        for nibble in [byte >> 4, byte & 15] {
            encoded.push(b'a' + (nibble + key) % 26);
            key = (key + 17) % 26;
        }
    }
    encoded
}

#[test]
fn all_eleven_recorded_challenges_match_the_exact_outbound_bytes() {
    for &(archive, line, encoded, expected) in CAPTURED {
        let reply = solve_challenge(encoded.as_bytes()).unwrap();
        let actual: String = reply.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(actual, expected, "{archive}, challenge record {line}");
    }
}

#[test]
fn recorded_legacy_request_received_the_existing_stock_challenge_format() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/client-legacy-login.json")).unwrap();
    let session = &fixture["joinedSession"];
    assert_eq!(session["request"]["hex"], "6304");
    let hex = |value: &serde_json::Value| {
        let text = value.as_str().unwrap();
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>()
    };
    let challenge = hex(&session["challenge"]["hex"]);
    assert_eq!(&challenge[..3], &[0, 0, b'6']);
    let reply = hex(&session["reply"]["hex"]);
    assert_eq!(reply.len(), 27);
    assert_eq!(solve_challenge(&challenge[3..]).unwrap(), reply);
}

#[test]
fn unknown_programs_are_rejected_including_host_constructor_access() {
    let accepted = b"var a = \"_k/T)\";a+=\"dr'Ay(^|``J9l@Bs'L{]JX\";for (var i=0;i<27;i++){idba[i]=a.charCodeAt(i);}";
    assert_eq!(
        solve_challenge(&encode_source(accepted)).unwrap(),
        *b"LzLIOvqUYaFMrAJWShKPlPEiOBC"
    );
    for source in [
        b"idba.constructor.constructor('return process')()".as_slice(),
        b"while(true){}",
        b"var a = \"short\";a+=\"also short\";for (var i=0;i<27;i++){idba[i]=a.charCodeAt(i);}",
    ] {
        assert!(solve_challenge(&encode_source(source)).is_err());
    }
    for suffix in [b";idba.fill(65)".as_slice(), b" ", b"//ignored"] {
        let mut injected = accepted.to_vec();
        injected.extend_from_slice(suffix);
        assert!(solve_challenge(&encode_source(&injected)).is_err());
    }
    for replacement in [b'"', b'\\', 0, 127, 255] {
        let mut injected = accepted.to_vec();
        injected[9] = replacement;
        assert!(solve_challenge(&encode_source(&injected)).is_err());
    }
}

#[test]
fn malformed_or_oversized_encoding_is_rejected_without_panicking() {
    for encoded in [
        b"".as_slice(),
        b"e",
        b"!!",
        b"zz",
        &[255, 255],
        &[b'a'; 514],
    ] {
        assert!(solve_challenge(encoded).is_err());
    }
    let valid = CAPTURED[0].2.as_bytes();
    for length in 0..valid.len() {
        assert!(solve_challenge(&valid[..length]).is_err());
    }
    for index in 0..valid.len() {
        let mut invalid = valid.to_vec();
        invalid[index] = b'!';
        assert!(solve_challenge(&invalid).is_err());
    }
}
