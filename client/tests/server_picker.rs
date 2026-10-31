//! Authored synthetic directories and injected ping results. No transport.
use slither_client::{
    menu_controls::*, menu_input::MenuInputState, server_directory::Server, server_picker::*,
};
use std::{collections::BTreeMap, net::SocketAddr};
fn server(id: u16, cluster: u16, active: bool) -> Server {
    Server {
        id,
        cluster,
        active,
        address: SocketAddr::from(([192, 0, 2, 1], id.max(1))),
        population: 40,
    }
}
fn picker() -> ServerPicker {
    let mut p = ServerPicker::default();
    p.replace(
        &(1..=2000)
            .map(|id| server(id, id % 3, true))
            .collect::<Vec<_>>(),
        &BTreeMap::from([(0, 20), (1, 50)]),
    )
    .unwrap();
    p
}
fn intent(p: &ServerPicker, id: ControlId) -> ControlIntent {
    ControlIntent {
        panel: 2,
        generation: p.generation,
        control: id,
        activation: Activation::Enter,
    }
}
#[test]
fn dedupe_fastest_address_ties_and_unknown_pings() {
    let a = server(7, 1, true);
    let mut v6 = a.clone();
    v6.cluster = 1001;
    v6.address = "[2001:db8::1]:445".parse().unwrap();
    let rows = ranked(
        &[
            a.clone(),
            v6.clone(),
            server(4, 2, true),
            server(9, 3, true),
        ],
        &BTreeMap::from([(1, 40), (1001, 10), (2, 10)]),
    );
    assert_eq!(
        rows.iter().map(|r| r.server.id).collect::<Vec<_>>(),
        vec![4, 7, 9]
    );
    assert_eq!(rows[1].server.address, v6.address);
    assert_eq!(rows[2].ping, None);
    let boundary = ranked(
        &[server(1, 2, true), server(9, 1, true)],
        &BTreeMap::from([(1, u32::MAX)]),
    );
    assert_eq!(boundary[0].server.id, 9);
    assert_eq!(boundary[1].ping, None);
    assert_eq!(
        ranked(&[a.clone(), v6.clone()], &BTreeMap::new())[0]
            .server
            .address,
        v6.address
    );
    assert_eq!(
        ranked(&[v6.clone(), a], &BTreeMap::from([(1, 10), (1001, 10)]))[0]
            .server
            .address,
        v6.address
    );
}
#[test]
fn numeric_rule_exact_selection_and_inactive_id() {
    let mut p = picker();
    p.replace(
        &[server(1024, 1, false), server(12, 2, true)],
        &BTreeMap::new(),
    )
    .unwrap();
    p.open();
    assert!(!p.search("a001"));
    assert_eq!(p.search, "001");
    assert!(p.search("x10a24more9"));
    assert_eq!(p.selection.0, Some(1024));
    assert!(!p.is_open());
    assert_eq!(p.take_preference().as_deref(), Some("1:1024"));
    assert_eq!(p.take_preference(), None);
    p.open();
    assert_eq!(p.entries().len(), 2); // Selected inactive server stays reachable.
    assert!(p.search("0012"));
    assert_eq!(p.selection.0, Some(12));
}
#[test]
fn open_is_only_directory_effect_automatic_is_default_and_preference_roundtrip() {
    let mut p = ServerPicker::default();
    p.step(0.).unwrap();
    p.open();
    assert_eq!(p.selection, ServerPreference(None));
    assert!(p.take_directory_request());
    assert!(!p.take_directory_request());
    assert_eq!(p.take_preference(), None);
    p.step(40.).unwrap();
    assert_eq!(p.alpha, 0.1);
    p.close();
    p.step(80.).unwrap();
    assert_eq!(p.alpha, 0.);
    assert!(p.step(79.).is_err());
    assert!(p.step(f64::NAN).is_err());
    for value in [None, Some(0), Some(65535)] {
        let pref = ServerPreference(value);
        assert_eq!(ServerPreference::restore(Some(&pref.serialize())), pref);
    }
    for bad in [
        "",
        "65535",
        "1:65536",
        "1:-1",
        "1:42/1",
        "2:42",
        "1:99999999999999",
    ] {
        assert_eq!(ServerPreference::restore(Some(bad)), ServerPreference(None));
    }
    p.selection = ServerPreference(Some(42));
    p.open();
    assert!(p.activate(intent(&p, AUTOMATIC)));
    assert_eq!(p.selection, ServerPreference(None));
}
#[test]
fn opens_reuse_successful_empty_and_inflight_directory_results() {
    let mut p = ServerPicker::default();
    p.open();
    assert!(p.take_directory_request());
    for _ in 0..3 {
        p.close();
        p.open();
        assert!(!p.take_directory_request());
    }
    p.replace(&[], &BTreeMap::new()).unwrap();
    for _ in 0..3 {
        p.close();
        p.open();
        assert!(!p.take_directory_request());
    }
}
#[test]
fn directory_failure_retries_only_on_next_open_and_preserves_completed_results() {
    for status in [
        "Directory worker stopped",
        "Thread start failed",
        "Discovery cancelled",
    ] {
        let mut p = ServerPicker::default();
        p.open();
        // Failure also clears any effect not yet consumed by the adapter.
        p.directory_failed(status);
        assert_eq!(p.status, status);
        for _ in 0..3 {
            p.open();
            assert!(!p.take_directory_request());
        }
        p.close();
        p.open();
        assert!(p.take_directory_request());
        for _ in 0..3 {
            p.close();
            p.open();
            assert!(!p.take_directory_request());
        }
        for servers in [vec![], vec![server(42, 1, true)]] {
            p.replace(&servers, &BTreeMap::new()).unwrap();
            let completed_status = p.status.clone();
            p.directory_failed(status);
            assert_eq!(p.status, completed_status);
            p.close();
            p.open();
            assert!(!p.take_directory_request());
            assert_eq!(p.rows.len(), servers.len());
        }
    }
}
#[test]
fn large_window_focus_scroll_and_refresh_cancel_capture() {
    let mut p = picker();
    p.open();
    p.alpha = 1.;
    let target = p.navigate(None, isize::MAX);
    let panel = p.panel([960., 720.]);
    assert!(panel.controls.iter().any(
        |c| matches!(&c.kind,ControlKind::Label {text} if text.starts_with("1995–2000 of 2000"))
    ));
    assert!(panel.controls.len() < 20);
    assert_eq!(
        panel
            .controls
            .iter()
            .filter(|c| matches!(c.kind, ControlKind::ListRow { .. }))
            .count(),
        6
    );
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    input.sync(&panel, &mut edit);
    input.focus_control(&panel, target);
    assert_eq!(input.focus(), Some(target));
    let resolved = panel.resolve([960., 720.]).unwrap();
    let r = resolved.iter().find(|r| r.control.id == target).unwrap();
    let point = [r.rect.x + 10., r.rect.y + 10.];
    input.pointer(&panel, &resolved, point, Some(true));
    assert_eq!(input.capture(), Some(target));
    let old = intent(&p, target);
    p.replace(&[server(8, 1, true)], &BTreeMap::new()).unwrap();
    let panel = p.panel([960., 720.]);
    input.sync(&panel, &mut edit);
    assert_eq!(input.capture(), None);
    assert!(!p.activate(old));
    p.scroll(isize::MIN);
    assert_eq!(p.list.window(1), 0..1);
    p.scroll(isize::MAX);
    assert_eq!(p.list.window(1), 0..1);
}
#[test]
fn backdrop_panel_shield_tab_trap_and_closing_selection() {
    let mut p = picker();
    p.open();
    p.alpha = 1.;
    let panel = p.panel([960., 720.]);
    let resolved = panel.resolve([960., 720.]).unwrap();
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    input.sync(&panel, &mut edit);
    assert_eq!(input.focus(), Some(SEARCH));
    input.tab(&panel, true);
    assert_eq!(input.focus(), Some(CLOSE));
    input.tab(&panel, false);
    assert_eq!(input.focus(), Some(SEARCH));
    let r = resolved
        .iter()
        .find(|r| r.control.id == ControlId(1))
        .unwrap();
    let point = [r.rect.x + 8., r.rect.y + 8.];
    input.pointer(&panel, &resolved, point, Some(true));
    let i = input
        .pointer(&panel, &resolved, point, Some(false))
        .unwrap();
    assert!(!p.activate(i));
    assert!(p.is_open());
    input.pointer(&panel, &resolved, [1., 1.], Some(true));
    let i = input
        .pointer(&panel, &resolved, [1., 1.], Some(false))
        .unwrap();
    assert!(p.activate(i));
    assert!(!p.is_open());
}
