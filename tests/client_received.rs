use slither_sim::{
    client::{Minimap, ReceivedField, ReceivedTracker},
    physics, *,
};

fn snake() -> Snake {
    Snake {
        id: "snake".into(),
        head: Point::default(),
        body: (0..8)
            .map(|n| Point {
                x: -42. * n as f64,
                y: 0.,
            })
            .collect(),
        heading: 0.,
        speed: 120.,
        radius: 14.5,
        length: 336.,
        segments: 8,
        fam: 0.,
        score: physics::score(8, 0.),
        kills: 0,
        alive: true,
    }
}

#[test]
fn independent_receipt_ages_and_history_do_not_refresh_on_poll() {
    let s = snake();
    let mut tracker = ReceivedTracker::new(1);
    tracker.appearance(0, &s, 0., 0.).unwrap();
    tracker
        .motion(0, None, Some(1.), Some(140.), 0.084)
        .unwrap();
    tracker.position(0, Point { x: 12., y: 0. }, 0.126).unwrap();
    let metadata = tracker.metadata(0.252);
    let row = metadata.snake("snake").unwrap();
    assert!((row.position.age_seconds - 0.126).abs() < 1e-10);
    assert!((row.heading.age_seconds - 0.252).abs() < 1e-10);
    assert!((row.wanted_heading.age_seconds - 0.168).abs() < 1e-10);
    assert!((row.body.age_seconds - 0.252).abs() < 1e-10);
    assert_eq!(row.wanted_history[0].value, 1.);
    assert_eq!(row.speed_history[0].value, 120.);
    assert_eq!(tracker.metadata(0.252), metadata);
    let encoded = serde_json::to_value(&metadata).unwrap();
    assert_eq!(encoded["connectionGeneration"], 0);
    assert_eq!(encoded["snakes"][0]["snakeId"], "snake");
    assert!(encoded["snakes"][0]["position"]["ageSeconds"].is_number());
    metadata.validate().unwrap();
}

#[test]
fn appearance_generation_and_reconnect_reset_history() {
    let s = snake();
    let mut tracker = ReceivedTracker::new(1);
    tracker.appearance(0, &s, 0., 0.).unwrap();
    tracker.motion(0, None, Some(1.), None, 0.1).unwrap();
    let old = tracker.metadata(0.1).snakes[0].incarnation;
    tracker.removal(0, 0.2).unwrap();
    assert!(tracker.metadata(0.2).snakes.is_empty());
    assert!(tracker.anchor(0, Point::default(), false, 0.2).is_err());
    tracker.appearance(0, &s, 0., 0.3).unwrap();
    let metadata = tracker.metadata(0.3);
    assert!(metadata.snakes[0].incarnation > old);
    assert!(metadata.snakes[0].wanted_history.is_empty());
    tracker.reconnect();
    assert_eq!(tracker.metadata(0.3).connection_generation, 1);
    assert!(tracker.metadata(0.3).snakes.is_empty());
    // A backward local clock also resets connection state.
    tracker.appearance(0, &s, 0., 0.4).unwrap();
    assert!(tracker.position(0, Point::default(), 0.1).is_err());
    assert_eq!(tracker.metadata(0.1).connection_generation, 2);
}

#[test]
fn delayed_polling_expires_histories_on_the_local_receipt_clock() {
    let mut tracker = ReceivedTracker::new(1);
    tracker.appearance(0, &snake(), 0., 0.).unwrap();
    tracker
        .motion(0, None, Some(1.), Some(140.), 0.084)
        .unwrap();
    let mut metadata = tracker.metadata(0.252);
    metadata.age_by(0.3);
    assert_eq!(metadata.snakes[0].wanted_history.len(), 1);
    assert!(
        metadata.snakes[0]
            .speed_history
            .iter()
            .all(|entry| !entry.valid)
    );
    metadata.validate().unwrap();
    metadata.age_by(0.04);
    assert!(metadata.snakes[0].wanted_history.is_empty());
    metadata.validate().unwrap();
}

#[test]
fn received_body_reconstructs_canonical_growth_and_tail_events() {
    let s = snake();
    let mut tracker = ReceivedTracker::new(1);
    tracker.appearance(0, &s, 0., 0.).unwrap();
    let mut expected = s.body;
    let anchor = Point { x: 42., y: 8. };
    physics::append_anchor(&mut expected, anchor, true);
    tracker.anchor(0, anchor, true, 0.1).unwrap();
    tracker.tail(0, 7, 0.2).unwrap();
    expected.truncate(7);
    let anchor = Point { x: 84., y: 20. };
    physics::append_anchor(&mut expected, anchor, false);
    tracker.anchor(0, anchor, false, 0.3).unwrap();
    assert_eq!(tracker.snake(0).unwrap().body, expected);
    assert_eq!(tracker.metadata(0.3).snakes[0].body.age_seconds, 0.);
}

#[test]
fn setup_contraction_applies_before_insertion_and_rejects_atomically() {
    let mut s = snake();
    s.body.truncate(5);
    let mut tracker = ReceivedTracker::new(1);
    tracker.appearance(0, &s, 0., 0.).unwrap();
    tracker
        .anchor_with(0, Point { x: 42., y: 0. }, true, 0.8, 0.1)
        .unwrap();
    // Weights are 0.8/4 and 0.8/2, each toward the already contracted neighbor.
    let expected = [42., 0., -42., -84., -117.6, -147.84];
    let body = &tracker.snake(0).unwrap().body;
    assert_eq!(body.len(), expected.len());
    for (point, x) in body.iter().zip(expected) {
        assert!((point.x - x).abs() < 1e-9 && point.y == 0., "{body:?}");
    }

    // Wire value zero is representable and disables contraction.
    let before = tracker.snake(0).unwrap().body.clone();
    tracker
        .anchor_with(0, Point { x: 84., y: 0. }, false, 0., 0.2)
        .unwrap();
    assert_eq!(tracker.snake(0).unwrap().body[1..], before[..5]);

    let before = tracker.clone();
    for invalid in [-0.001, 1.001, 65.535, f64::NAN, f64::INFINITY] {
        // A backward receipt would reconnect if the coefficient were accepted.
        assert!(
            tracker
                .anchor_with(0, Point { x: 126., y: 0. }, true, invalid, 0.)
                .is_err()
        );
        assert_eq!(tracker.metadata(0.3), before.metadata(0.3));
        assert_eq!(tracker.snake(0), before.snake(0));
    }
    tracker
        .anchor_with(0, Point { x: 126., y: 0. }, true, 1., 0.3)
        .unwrap();
}

#[test]
fn default_anchor_matches_physics_contraction_bit_for_bit() {
    let mut s = snake();
    s.body = (0..9)
        .map(|n| Point {
            x: -37. * n as f64,
            y: (n as f64 * 0.7).sin() * 19.,
        })
        .collect();
    let mut tracker = ReceivedTracker::new(1);
    tracker.appearance(0, &s, 0., 0.).unwrap();
    let mut expected = s.body.clone();
    for (n, grow) in [true, false, false, true].into_iter().enumerate() {
        let anchor = Point {
            x: 40. + n as f64 * 13.,
            y: n as f64 * 3.,
        };
        physics::append_anchor(&mut expected, anchor, grow);
        if n % 2 == 0 {
            tracker.anchor(0, anchor, grow, 0.1).unwrap();
        } else {
            tracker.anchor_with(0, anchor, grow, 0.43, 0.1).unwrap();
        }
        assert_eq!(tracker.snake(0).unwrap().body, expected);
    }
}

#[test]
fn ensure_slots_grows_without_resetting_existing_receipts() {
    let mut tracker = ReceivedTracker::new(1);
    tracker.appearance(0, &snake(), 0., 0.).unwrap();
    tracker
        .motion(0, None, Some(1.), Some(140.), 0.084)
        .unwrap();
    tracker.removal(0, 0.1).unwrap();
    tracker.appearance(0, &snake(), 0., 0.2).unwrap();
    tracker.motion(0, None, Some(2.), None, 0.25).unwrap();
    let before = tracker.metadata(0.3);
    assert_eq!(before.snakes[0].incarnation, 2);
    let mut other = snake();
    other.id = "other".into();
    assert!(tracker.appearance(2, &other, 0., 0.3).is_err());

    tracker.ensure_slots(3);
    assert_eq!(tracker.metadata(0.3), before);
    tracker.appearance(2, &other, 0., 0.3).unwrap();
    // Shrinking requests are ignored.
    tracker.ensure_slots(1);
    let after = tracker.metadata(0.3);
    assert_eq!(after.snakes[0], before.snakes[0]);
    assert_eq!(after.snakes[1].snake_id, "other");
    assert_eq!(after.snakes[1].incarnation, 1);
    assert_eq!(after.connection_generation, before.connection_generation);
}

#[test]
fn appearance_cannot_expose_private_score_or_kills() {
    let mut s = snake();
    s.score = 999999.;
    s.kills = 100;
    let mut tracker = ReceivedTracker::new(1);
    tracker.appearance(0, &s, 0., 0.).unwrap();
    let received = tracker.snake(0).unwrap();
    assert_eq!(received.score, physics::score(s.segments, s.fam));
    assert_eq!(received.kills, 0);
    let mut metadata = tracker.metadata(0.);
    metadata.snakes[0].heading = ReceivedField {
        valid: false,
        predicted: true,
        age_seconds: 0.,
    };
    assert!(metadata.validate().is_err());
}

#[test]
fn head_position_and_body_anchors_are_independent_reports() {
    let s = snake();
    let mut tracker = ReceivedTracker::new(1);
    tracker.appearance(0, &s, 0., 0.).unwrap();
    tracker.position(0, Point { x: 30., y: 5. }, 0.1).unwrap();
    let received = tracker.snake(0).unwrap();
    assert_eq!(received.head, Point { x: 30., y: 5. });
    assert_eq!(received.body, s.body);
    let row = tracker.metadata(0.2).snakes[0].clone();
    assert!((row.position.age_seconds - 0.1).abs() < 1e-10);
    assert!((row.body.age_seconds - 0.2).abs() < 1e-10);

    tracker
        .anchor(0, Point { x: 42., y: 0. }, false, 0.3)
        .unwrap();
    assert_eq!(tracker.snake(0).unwrap().head, Point { x: 30., y: 5. });
    assert_eq!(tracker.snake(0).unwrap().body[0], Point { x: 42., y: 0. });
    assert!((tracker.metadata(0.3).snakes[0].position.age_seconds - 0.2).abs() < 1e-10);
}

#[test]
fn reported_own_score_does_not_refresh_mass_and_mass_reports_reset_it() {
    let s = snake();
    let mut tracker = ReceivedTracker::new(2);
    tracker.appearance(0, &s, 0., 0.).unwrap();
    tracker.own_score(0, 400.).unwrap();
    assert_eq!(tracker.snake(0).unwrap().score, 400.);
    assert!((tracker.metadata(0.1).snakes[0].mass.age_seconds - 0.1).abs() < 1e-10);
    for invalid in [-1., f64::NAN, f64::INFINITY] {
        assert!(tracker.own_score(0, invalid).is_err());
    }
    assert_eq!(tracker.snake(0).unwrap().score, 400.);
    assert!(tracker.own_score(1, 400.).is_err());

    let mut grown = s.clone();
    grown.segments = 9;
    tracker.mass(0, &grown, 0.2).unwrap();
    assert_eq!(tracker.snake(0).unwrap().score, physics::score(9, 0.));
    tracker.removal(0, 0.3).unwrap();
    assert!(tracker.own_score(0, 500.).is_err());
}

#[test]
fn alive_snakes_follow_appearance_and_removal() {
    let mut tracker = ReceivedTracker::new(3);
    let mut other = snake();
    other.id = "other".into();
    tracker.appearance(2, &snake(), 0., 0.).unwrap();
    tracker.appearance(0, &other, 0., 0.).unwrap();
    let ids = |tracker: &ReceivedTracker| -> Vec<(usize, String)> {
        tracker
            .alive_snakes()
            .map(|(slot, snake)| (slot, snake.id.clone()))
            .collect()
    };
    assert_eq!(
        ids(&tracker),
        [(0, "other".to_string()), (2, "snake".to_string())]
    );
    tracker.removal(0, 0.1).unwrap();
    assert_eq!(ids(&tracker), [(2, "snake".to_string())]);
    // A removed snake stays inspectable by slot, but is no longer received.
    assert!(!tracker.snake(0).unwrap().alive);
    assert_eq!(tracker.metadata(0.1).snakes.len(), 1);
    tracker.reconnect();
    assert_eq!(tracker.alive_snakes().count(), 0);
}

#[test]
fn minimap_full_snapshot_fills_cells_from_the_end() {
    // 0b1000001 sets the last and the seventh-from-last cells; a run of 1 then
    // skips one cell before the final literal reaches cell 0.
    let map = Minimap::packet(None, &[b'M', 0, 3, 0b100_0001, 129, 0b100_0000]).unwrap();
    assert_eq!(
        map,
        Minimap {
            side: 3,
            cells: vec![1, 0, 1, 0, 0, 0, 0, 0, 1],
            age_seconds: 0.,
        }
    );
    map.validate().unwrap();
    // A long run skips 126 cells per count byte and may exhaust the grid.
    let empty = Minimap::packet(None, &[b'M', 0, 12, 255, 2, 0b111_1111]).unwrap();
    assert!(empty.cells.iter().all(|&cell| cell == 0));
}

#[test]
fn minimap_delta_toggles_cells_and_resets_receipt_age() {
    let mut previous = Minimap {
        side: 3,
        cells: vec![1, 0, 1, 0, 0, 0, 0, 0, 1],
        age_seconds: 2.5,
    };
    let map = Minimap::packet(Some(&previous), &[b'V', 134, 0b111_0000]).unwrap();
    assert_eq!(map.cells, [0, 1, 0, 0, 0, 0, 0, 0, 1]);
    assert_eq!(map.age_seconds, 0.);
    assert_eq!(previous.age_seconds, 2.5);

    assert!(Minimap::packet(None, &[b'V', 0]).is_err());
    assert!(Minimap::packet(Some(&previous), &[b'V', 255]).is_err());
    previous.cells[0] = 2;
    assert!(Minimap::packet(Some(&previous), &[b'V', 0]).is_err());
}

#[test]
fn minimap_rejects_invalid_packets_and_snapshots() {
    for payload in [
        &[][..],
        &[b'M', 0][..],
        &[b'M', 0, 0][..],
        &[b'M', 2, 1][..],
        &[b'M', 0, 3, 255][..],
        &[b'X', 0, 3][..],
    ] {
        assert!(Minimap::packet(None, payload).is_err(), "{payload:?}");
    }
    let valid = Minimap {
        side: 2,
        cells: vec![0, 1, 0, 0],
        age_seconds: 0.,
    };
    valid.validate().unwrap();
    for invalid in [
        Minimap {
            side: 0,
            cells: vec![],
            ..valid.clone()
        },
        Minimap {
            cells: vec![0; 3],
            ..valid.clone()
        },
        Minimap {
            cells: vec![0, 2, 0, 0],
            ..valid.clone()
        },
        Minimap {
            age_seconds: -1.,
            ..valid.clone()
        },
        Minimap {
            age_seconds: f64::NAN,
            ..valid.clone()
        },
    ] {
        assert!(invalid.validate().is_err(), "{invalid:?}");
    }
    let encoded =
        serde_json::json!({"side": 2, "cells": [0, 1, 0, 0], "age_seconds": 0., "extra": 1});
    assert_eq!(serde_json::from_value::<Minimap>(encoded).unwrap(), valid);
    let decoded: Minimap = serde_json::from_value(serde_json::to_value(&valid).unwrap()).unwrap();
    assert_eq!(decoded, valid);
}

#[test]
fn minimap_from_snakes_rasterizes_only_live_bodies() {
    let arena = Arena {
        center: Point::default(),
        radius: 100.,
    };
    let mut live = snake();
    live.body = vec![Point { x: -10., y: 0. }];
    let mut dead = snake();
    dead.head = Point { x: 90., y: 90. };
    dead.body = vec![Point { x: 95., y: 90. }];
    dead.alive = false;
    let map = Minimap::from_snakes(&[live, dead], &arena, 4);
    map.validate().unwrap();
    let marked: Vec<_> = (0..16).filter(|&i| map.cells[i] == 1).collect();
    assert_eq!(marked, [9, 10]);
}
