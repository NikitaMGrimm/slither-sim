//! ORIGINAL SYNTHETIC directory, script and selection fixtures. No external inputs/network.
#![cfg(not(target_arch = "wasm32"))]
use slither_wgpu_slice::{game::server_endpoint, official::*};
use url::Url;

#[test]
fn page_selects_current_external_game_script_without_comment_or_ad_scripts() {
    let base = Url::parse("http://slither.io/?a").unwrap();
    let page = "<!-- <script src='/s/game111.js'></script> --><script src='https://ads.invalid/a.js'></script><script type='text/javascript' src='https://slither.io/s/game999.js'></script>";
    assert_eq!(
        client_script(page, &base).unwrap().as_str(),
        "https://slither.io/s/game999.js"
    );
    assert!(
        client_script(
            "<script src='https://other.invalid/s/game999.js'></script>",
            &base
        )
        .is_err()
    );
    assert!(
        client_script(
            "<script src='/s/game1.js'></script><script src='/s/game2.js'></script>",
            &base
        )
        .is_err()
    );
    assert!(client_script("<script>let x=1;</script>", &base).is_err());
}

#[test]
fn closed_profile_literals_and_dynamic_directory_url_never_evaluate_script() {
    let script = "throw Error('do not execute'); var client_version = 4242; var cpw = [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19]; getData('https://slither.io/i999.txt', handler);";
    let p = profile(script.as_bytes()).unwrap();
    assert_eq!(p.client_version, 4242);
    assert_eq!(p.page_echo, std::array::from_fn(|i| i as u8));
    assert_eq!(
        directory_url(script).unwrap().as_str(),
        "https://slither.io/i999.txt"
    );
    for bad in [
        script.replace("4242;", "f();"),
        script.replace("18,19]", "18]"),
        script.replace("18,19]", "18,256]"),
        format!("{script} var client_version=4;"),
        script.replace("var cpw", "var cpw_suffix"),
    ] {
        assert!(profile(bad.as_bytes()).is_err());
    }
    assert!(directory_url("https://other.invalid/i999.txt").is_err());
    assert!(directory_url("https://slither.io/i1.txt https://slither.io/i2.txt").is_err());
    assert!(profile(&vec![b' '; 1024 * 1024 + 1]).is_err());
}

#[test]
fn explicit_hosts_and_shared_urls_allow_remote_ipv4_ipv6_and_dns() {
    for host in [
        "192.0.2.1:445",
        "game.example:80",
        "[2001:db8::1]:446",
        "127.0.0.1:19491",
    ] {
        let endpoint = explicit_endpoint(host).unwrap();
        assert!(server_endpoint(&endpoint).is_ok());
    }
    for bad in [
        "game.example",
        "x:0",
        "x:65536",
        "user@host:445",
        "x:445/other",
        "x:445?q=a",
        "x:445#f",
        "x:445\n",
        "ws://x:445",
    ] {
        assert!(explicit_endpoint(bad).is_err(), "{bad:?}");
    }
    for bad in [
        "wss://x:445/slither",
        "ws://x:445/",
        "ws://x/slither?q=a",
        "ws://x/slither#f",
        "ws://u@x/slither",
        "ws://x:0/slither",
    ] {
        assert!(server_endpoint(bad).is_err());
    }
}
