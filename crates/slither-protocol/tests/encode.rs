use slither_protocol::{ErrorKind, v19::client::*};
#[test]
fn direct_server_login_matches_merged_client_pin() {
    // client/apps/wgpu-slice/tests/endpoint_login.rs at 8b36a91c (#391).
    // These are caller data for that profile, never write_login defaults.
    let echo = [
        54, 206, 204, 169, 97, 178, 74, 136, 124, 117, 14, 210, 106, 236, 8, 208, 136, 213, 140,
        111,
    ];
    let expected = [
        115, 30, 1, 35, 54, 206, 204, 169, 97, 178, 74, 136, 124, 117, 14, 210, 106, 236, 8, 208,
        136, 213, 140, 111, 9, 5, b'F', b'i', b'x', b'e', b'd', 0, 255,
    ];
    let mut out = [0; 400];
    let len = write_login(
        ModernLogin {
            client_version: 291,
            page_echo: &echo,
            nickname: b"Fixed",
            skin: 9,
            custom_skin: &[],
        },
        &mut out,
    )
    .unwrap();
    assert_eq!(&out[..len], expected);
    // The merged e2e tool deliberately supplies its separate zero profile.
    let len = write_login(
        ModernLogin {
            client_version: 0,
            page_echo: &[0; 20],
            nickname: b"Local E2E",
            skin: 0,
            custom_skin: &[],
        },
        &mut out,
    )
    .unwrap();
    assert_eq!(
        &out[..len],
        &[
            b's', 30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, b'L',
            b'o', b'c', b'a', b'l', b' ', b'E', b'2', b'E', 0, 255
        ]
    );
}
#[test]
fn builders_are_atomic_and_accept_caller_admitted_latin1() {
    let login = ModernLogin {
        client_version: 0x1234,
        page_echo: &[9; 20],
        nickname: &[0, 255],
        skin: 8,
        custom_skin: &[0, 128, 255],
    };
    let mut out = [0xaa; 34];
    assert_eq!(write_login(login, &mut out).unwrap(), 33);
    assert_eq!(&out[24..], &[8, 2, 0, 255, 0, 255, 0, 128, 255, 0xaa]);
    for bad in [
        ModernLogin {
            nickname: &[0; 25],
            ..login
        },
        ModernLogin {
            custom_skin: &[0; 256],
            ..login
        },
    ] {
        let before = out;
        assert!(write_login(bad, &mut out).is_err());
        assert_eq!(out, before);
    }
    let before = out;
    assert_eq!(
        write_login(login, &mut out[..32]).unwrap_err().kind,
        ErrorKind::OutputCapacity
    );
    assert_eq!(out, before);
    assert_eq!(write_champion(b"Ready", &mut out).unwrap(), 7);
    assert_eq!(&out[..7], b"\xffvReady");
    let before = out;
    assert!(write_champion(&[0; 141], &mut out).is_err());
    assert_eq!(out, before);
    assert!(write_champion(b"Ready", &mut out[..6]).is_err());
    assert_eq!(out, before);
}
#[test]
fn input_negotiation_and_ping_bytes_are_fixed() {
    assert_eq!(heading_byte(0).unwrap(), [0]);
    assert_eq!(heading_byte(250).unwrap(), [250]);
    assert!(heading_byte(251).is_err());
    assert_eq!(boost(true), [253]);
    assert_eq!(boost(false), [254]);
    assert_eq!(turn(false, 1).unwrap(), [252, 1]);
    assert_eq!(turn(true, 127).unwrap(), [252, 255]);
    assert!(turn(false, 0).is_err());
    assert!(turn(true, 128).is_err());
    assert_eq!(
        (PING, REQUEST_BARE, REQUEST_SEQUENCE, REQUEST_SESSION),
        ([251], [1], [2], [4])
    );
    assert_eq!(
        (STOCK_HELLO, SHORT_HELLO, LEGACY_HELLO, NTL_HELLO),
        ([99, 0], [99], [99, 3], [99, 4])
    );
}

#[test]
fn simulator_heading_retains_its_normalization_and_quantization() {
    use std::f64::consts::TAU;
    for heading in [
        -1e200,
        -TAU,
        -1.,
        -f64::MIN_POSITIVE,
        -0.,
        0.,
        f64::MIN_POSITIVE,
        1.,
        TAU,
        1e200,
    ] {
        let old = (heading.rem_euclid(TAU) * 251. / TAU).floor().min(250.) as u8;
        assert_eq!(sim_heading(heading).unwrap(), [old]);
    }
    for heading in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(sim_heading(heading).is_err());
    }
}
