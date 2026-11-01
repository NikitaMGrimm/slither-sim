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
        checked: None,
        panel: 2,
        generation: p.generation,
        control: id,
        activation: Activation::Enter,
    }
}
#[test]
fn startup_discovers_once_and_custom_selection_stays_direct() {
    let mut p = ServerPicker::default();
    p.start_directory();
    assert!(p.can_play() && p.official_context());
    assert!(p.take_directory_request());
    p.open();
    assert!(!p.take_directory_request());
    p.replace(&[server(42, 1, true)], &BTreeMap::from([(1, 12)]))
        .unwrap();
    p.add_custom("192.0.2.9:445".into()).unwrap();
    assert!(p.activate(intent(&p, ControlId(16))));
    assert_eq!(p.selection.custom_address(), Some("192.0.2.9:445"));
    assert!(!p.official_context());
    assert!(p.can_play());
    p.open();
    assert!(!p.take_directory_request());
    let panel = p.panel([960., 720.]);
    assert!(
        panel
            .controls
            .iter()
            .any(|c| matches!(&c.kind, ControlKind::ListRow { .. }) && c.id == ControlId(17))
    );
    assert!(p.activate(intent(&p, AUTOMATIC)));
    assert!(p.official_context() && p.can_play());
    assert_eq!(p.selection, ServerPreference::Automatic);
    assert!(!p.take_directory_request()); // Reuse completed directory.
    p.open();
    assert!(p.search("42"));
    assert_eq!(p.selection, ServerPreference::Official(42));
}

#[test]
fn automatic_reenters_discovery_from_direct_mode_without_a_load_button() {
    let mut p = ServerPicker::default();
    p.add_custom("192.0.2.9:445".into()).unwrap();
    p.open();
    assert!(p.activate(intent(&p, ControlId(16))));
    p.open();
    assert!(!p.take_directory_request());
    assert!(p.activate(intent(&p, AUTOMATIC)));
    assert!(p.take_directory_request());
    p.directory_failed("Injected failure");
    p.open();
    assert!(p.take_directory_request());
    p.close();
    p.open();
    assert!(!p.take_directory_request());
}
#[test]
fn custom_entries_are_bounded_deduplicated_windowed_and_removable() {
    let mut p = ServerPicker::default();
    p.open();
    for index in 1..=MAX_CUSTOM_SERVERS {
        p.add_custom(format!("192.0.2.{index}:445")).unwrap();
        let panel = p.panel([960., 720.]);
        assert!(
            panel
                .controls
                .iter()
                .filter(|c| matches!(c.kind, ControlKind::ListRow { .. }))
                .count()
                <= 6
        );
    }
    assert_eq!(p.take_custom_operations().len(), MAX_CUSTOM_SERVERS);
    assert!(p.add_custom("192.0.2.33:445".into()).is_err());
    p.add_custom("192.0.2.1:445".into()).unwrap();
    assert_eq!(
        p.take_custom_operations(),
        vec![CustomServerOperation::Add("192.0.2.1:445".into())]
    );
    for invalid in [
        "",
        "bad address:445",
        "host:445\n",
        &"x".repeat(MAX_CUSTOM_ADDRESS + 1),
    ] {
        assert!(p.add_custom(invalid.into()).is_err());
    }
    p.scroll(isize::MIN);
    assert!(p.activate(intent(&p, ControlId(16))));
    assert_eq!(p.selection.custom_address(), Some("192.0.2.1:445"));
    assert!(p.can_play());
    assert!(!p.official_context());
    assert!(!p.take_directory_request());
    let record = p.take_preference().unwrap();
    assert_eq!(ServerPreference::restore(Some(&record)), p.selection);
    p.open();
    assert!(p.activate(intent(&p, ControlId(8192))));
    assert_eq!(
        p.take_custom_operations(),
        vec![CustomServerOperation::Remove("192.0.2.1:445".into())]
    );
    assert_eq!(p.custom.len(), MAX_CUSTOM_SERVERS - 1);
    assert_eq!(p.selection, ServerPreference::Automatic);
    assert!(!p.can_play());
    assert!(p.take_preference().is_some());
    assert!(p.is_open());
    assert!(!p.take_directory_request());
    p.add_custom("192.0.2.33:445".into()).unwrap();
    assert_eq!(p.custom.len(), MAX_CUSTOM_SERVERS);
}
#[test]
fn arrows_from_remove_navigate_relative_to_its_server_row() {
    for count in [1, 2, MAX_CUSTOM_SERVERS] {
        let mut p = ServerPicker::default();
        p.open();
        for n in 1..=count {
            p.add_custom(format!("192.0.2.{n}:445")).unwrap();
        }
        for index in [0, count - 1] {
            for delta in [-1, 1] {
                let expected = index.saturating_add_signed(delta).min(count - 1);
                let target = p.navigate(Some(ControlId(8192 + index as u16)), delta);
                assert_eq!(target, ControlId(16 + expected as u16));
                assert!(
                    p.panel([960., 720.])
                        .controls
                        .iter()
                        .any(|c| c.id == target)
                );
            }
        }
    }
}
#[test]
fn custom_address_editing_is_bounded_and_preserves_hostname_text() {
    use slither_client::menu_input::TextRule;
    for address in ["192.0.2.1:445", "[2001:db8::1]:80", "bücher.example:443"] {
        assert_eq!(
            TextRule::ServerAddress.normalize_units(&address.encode_utf16().collect::<Vec<_>>()),
            address
        );
    }
    let text = TextRule::ServerAddress.normalize_units(&vec![b'a' as u16; 2048]);
    assert_eq!(text.len(), 512);
    assert!(
        !TextRule::ServerAddress
            .normalize_units(&[0, 10, 13])
            .chars()
            .any(char::is_control)
    );
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
fn startup_requests_once_and_endpoint_mode_stays_offline() {
    let mut p = ServerPicker::default();
    assert!(!p.take_directory_request());
    p.start_directory();
    assert!(p.take_directory_request());
    p.start_directory();
    p.open();
    assert!(!p.take_directory_request());
}
#[test]
fn incremental_pings_preserve_focus_capture_and_window_until_reopen() {
    let servers: Vec<_> = (1..=20).map(|id| server(id, id, true)).collect();
    let mut p = ServerPicker::default();
    p.start_directory();
    assert!(p.take_directory_request());
    p.open();
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    input.sync(&p.panel([960., 720.]), &mut edit);
    p.update_directory(&servers, &BTreeMap::new(), false)
        .unwrap();
    input.sync(&p.panel([960., 720.]), &mut edit);
    assert_eq!(input.focus(), Some(ADDRESS));
    let target = p.navigate(None, isize::MAX);
    let panel = p.panel([960., 720.]);
    input.sync(&panel, &mut edit);
    input.focus_control(&panel, target);
    let resolved = panel.resolve([960., 720.]).unwrap();
    let row = resolved.iter().find(|r| r.control.id == target).unwrap();
    let point = [row.rect.x + 10., row.rect.y + 10.];
    input.pointer(&panel, &resolved, point, Some(true));
    let generation = p.generation;
    let window = p.list.window(20);
    for complete in [false, true] {
        p.update_directory(&servers, &BTreeMap::from([(20, 10), (19, 40)]), complete)
            .unwrap();
        let panel = p.panel([960., 720.]);
        input.sync(&panel, &mut edit);
        assert_eq!(p.generation, generation);
        assert_eq!(p.list.window(20), window);
        assert_eq!(input.focus(), Some(target));
        assert_eq!(input.capture(), Some(target));
        assert_eq!(p.entries()[19].server.id, 20);
        assert_eq!(p.entries()[19].ping, Some(10));
    }
    let panel = p.panel([960., 720.]);
    let activation = input
        .pointer(
            &panel,
            &panel.resolve([960., 720.]).unwrap(),
            point,
            Some(false),
        )
        .unwrap();
    assert!(p.activate(activation));
    assert_eq!(p.selection.official_id(), Some(20));
    p.open();
    assert_eq!(p.rows[0].server.id, 20);
    assert_eq!(p.rows[1].server.id, 19);
    assert!(p.rows[2..].iter().all(|r| r.ping.is_none()));
    assert!(!p.take_directory_request());
}
#[test]
fn provisional_rows_do_not_suppress_retry_after_failure() {
    let mut p = ServerPicker::default();
    p.start_directory();
    assert!(p.take_directory_request());
    p.open();
    p.update_directory(&[server(1, 1, true)], &BTreeMap::new(), false)
        .unwrap();
    p.directory_failed("Cancelled sweep");
    assert!(p.rows.is_empty());
    p.open();
    assert!(!p.take_directory_request());
    p.close();
    p.open();
    assert!(p.take_directory_request());
    p.open();
    assert!(!p.take_directory_request());
}
#[test]
fn progress_defers_duplicate_membership_changes_and_invalidates_a_new_directory() {
    let servers = [
        server(7, 1, true),
        server(7, 1001, false),
        server(8, 2, true),
    ];
    let mut p = ServerPicker::default();
    p.start_directory();
    p.open();
    p.update_directory(&servers, &BTreeMap::new(), false)
        .unwrap();
    assert_eq!(
        p.entries().iter().map(|r| r.server.id).collect::<Vec<_>>(),
        vec![8]
    );
    let generation = p.generation;
    p.update_directory(&servers, &BTreeMap::from([(1, 10)]), true)
        .unwrap();
    assert_eq!(p.generation, generation);
    assert_eq!(
        p.entries().iter().map(|r| r.server.id).collect::<Vec<_>>(),
        vec![8]
    );
    p.close();
    p.open();
    assert_eq!(p.entries()[0].server.id, 7);
    assert!(p.entries()[0].server.active);
    let old = intent(&p, ControlId(16));
    p.update_directory(&[server(9, 1, true)], &BTreeMap::new(), false)
        .unwrap();
    assert!(!p.activate(old));
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
    assert_eq!(p.selection.official_id(), Some(1024));
    assert!(!p.is_open());
    assert_eq!(p.take_preference().as_deref(), Some("1:1024"));
    assert_eq!(p.take_preference(), None);
    p.open();
    assert_eq!(p.entries().len(), 2); // Selected inactive server stays reachable.
    assert!(p.search("0012"));
    assert_eq!(p.selection.official_id(), Some(12));
}
#[test]
fn open_is_only_directory_effect_automatic_is_default_and_preference_roundtrip() {
    let mut p = ServerPicker::default();
    p.start_directory();
    p.step(0.).unwrap();
    p.open();
    assert_eq!(p.selection, ServerPreference::Automatic);
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
        let pref = value.map_or(ServerPreference::Automatic, ServerPreference::Official);
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
        assert_eq!(
            ServerPreference::restore(Some(bad)),
            ServerPreference::Automatic
        );
    }
    p.selection = ServerPreference::Official(42);
    p.open();
    assert!(p.activate(intent(&p, AUTOMATIC)));
    assert_eq!(p.selection, ServerPreference::Automatic);
}
#[test]
fn opens_reuse_successful_empty_and_inflight_directory_results() {
    let mut p = ServerPicker::default();
    p.start_directory();
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
        p.start_directory();
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
    assert!(panel.controls.len() < 26);
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
    assert_eq!(input.focus(), Some(ADDRESS));
    input.tab(&panel, true);
    assert_eq!(input.focus(), Some(CLOSE));
    input.tab(&panel, false);
    assert_eq!(input.focus(), Some(ADDRESS));
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
