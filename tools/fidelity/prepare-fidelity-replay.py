"""Export a bounded, explicitly partial replay from a complete headless archive.

Controls are recorded local sends, mapped to the server clock with a single
affine clock fit plus median measured ping RTT. This does not reveal the exact
input-processing tick. Observations compare transmitted anchors, not a guessed
head position. No future pose is imposed on the replay.
"""

import argparse
import hashlib
import json
import math
import statistics
from pathlib import Path

import slither_protocol as sp


def clock_fit(frames):
    xs = [f.t for f in frames]
    ys = [f.server for f in frames]
    mx, my = statistics.mean(xs), statistics.mean(ys)
    den = math.fsum((x - mx) ** 2 for x in xs)
    slope = math.fsum((x - mx) * (y - my) for x, y in zip(xs, ys)) / den if den else 1.0
    intercept = statistics.median(y - slope * x for x, y in zip(xs, ys))
    residuals = sorted(abs(y - slope * x - intercept) for x, y in zip(xs, ys))
    return slope, intercept, residuals[int(0.95 * (len(residuals) - 1))]


def tangent(anchors, fallback, dt=0.007, history=3):
    """Estimate endpoint heading using only anchors at or before the start."""
    if history < 3:
        raise ValueError("heading history must be at least 3 anchors")
    if len(anchors) < 3:
        return fallback, "packet-heading (insufficient preceding anchors)"
    if history >= 4 and len(anchors) >= history:
        points = anchors[-history:]
        midpoints = [(a[0] + b[0]) / 2000 for a, b in zip(points, points[1:])]
        chords = [math.atan2(b[2] - a[2], b[1] - a[1]) for a, b in zip(points, points[1:])]
        unwrapped = [chords[0]]
        for angle in chords[1:]:
            unwrapped.append(
                unwrapped[-1]
                + math.atan2(math.sin(angle - unwrapped[-1]), math.cos(angle - unwrapped[-1]))
            )
        mean_time, mean_angle = statistics.mean(midpoints), statistics.mean(unwrapped)
        spread = math.fsum((time - mean_time) ** 2 for time in midpoints)
        if spread > 0:
            omega = (
                math.fsum(
                    (time - mean_time) * (angle - mean_angle)
                    for time, angle in zip(midpoints, unwrapped)
                )
                / spread
            )
            end_time = points[-1][0] / 1000
            return (
                mean_angle + omega * (end_time - mean_time - dt / 2),
                f"preceding-{history}-anchor least-squares tangent",
            )
    a, b, c = anchors[-3:]
    old = math.atan2(b[2] - a[2], b[1] - a[1])
    new = math.atan2(c[2] - b[2], c[1] - b[1])
    delta = math.atan2(math.sin(new - old), math.cos(new - old))
    span = (c[0] - a[0]) / 2000
    omega = delta / span if span > 0 else 0
    # Chords point along the midpoint tangent. The last Euler step's heading is
    # about half a physics tick behind the smooth tangent at the endpoint.
    return new + delta / 2 - omega * dt / 2, "preceding-three-anchor tangent"


def reported_heading(actual, wanted, segments, age_ms):
    """Causal heading estimate from the last reported pose and wanted direction."""
    if age_ms < 0 or not all(math.isfinite(x) for x in (actual, wanted, age_ms)):
        raise ValueError("invalid reported-heading state")
    turn_scale = min(6.0, 1.0 + (segments - 2) / 111.0)
    scang = 0.13 + 0.87 * ((7.0 - turn_scale) / 6.0) ** 2
    limit = (0.033 / 0.008 * scang) * age_ms / 1000
    delta = math.atan2(math.sin(wanted - actual), math.cos(wanted - actual))
    return actual + max(-limit, min(limit, delta))


def reports_actual_heading(packet):
    """Whether a rotation packet refreshes `ang`, not just `wang` or speed."""
    if not packet or chr(packet[0]) not in "eE345d7":
        return False
    kind, length = chr(packet[0]), len(packet)
    if length == 6:
        return True
    if length in (5, 3):
        return kind not in "E4"
    if length in (4, 2):
        return kind not in "E43"
    return False


def reports_wanted_heading(packet):
    """Whether a rotation packet refreshes `wang`."""
    if not packet or chr(packet[0]) not in "eE345d7":
        return False
    kind, length = chr(packet[0]), len(packet)
    if length == 6:
        return True
    if length in (5, 3):
        return kind != "e"
    if length in (4, 2):
        return kind not in "e3"
    return False


def advance_reported_state(state, time):
    """Advance a reported pose through one constant-target, constant-size interval."""
    state["heading"] = reported_heading(
        state["heading"], state["wanted"], state["segments"], time - state["time"]
    )
    state["time"] = time


def export(
    path, connection, start_seconds, duration, name, heading_history=3, heading_source="anchors"
):
    if heading_history < 3:
        raise ValueError("heading history must be at least 3 anchors")
    if heading_source not in ("anchors", "reported"):
        raise ValueError("heading source must be anchors or reported")
    records = list(sp.read(path))  # EOF also verifies the gzip footer.
    terminal_index = next(
        (
            i
            for i, r in enumerate(records)
            if isinstance(r, dict) and r.get("event") in ("capture-end", "capture-interrupted")
        ),
        None,
    )
    if terminal_index is None:
        raise ValueError("Archive has no capture-end record")
    if terminal_index != len(records) - 1:
        raise ValueError("Archive has records after terminal capture receipt")
    if records[terminal_index]["event"] != "capture-end":
        raise ValueError("Archive has no capture-end record")
    # Rounded receipt times may tie, but a single affine mapping cannot span
    # a local clock reset or move a later outgoing command into the initial state.
    previous_time = None
    for record in records:
        if not isinstance(record, (sp.Frame, sp.Out)) or record.conn != connection:
            continue
        if type(record.t) not in (int, float) or not math.isfinite(record.t):
            raise ValueError("Connection has invalid receipt clock")
        if previous_time is not None and record.t < previous_time:
            raise ValueError("Connection receipt clock regressed")
        previous_time = record.t
    frames = [r for r in records if isinstance(r, sp.Frame) and r.conn == connection]
    if sum(packet[:1] == b"a" for frame in frames for packet in frame.packets) > 1:
        raise ValueError(
            "Connection has multiple game setup packets; server clock epoch is ambiguous"
        )
    setup_index = next(
        (
            i
            for i, frame in enumerate(frames)
            if any(packet[:1] == b"a" for packet in frame.packets)
        ),
        None,
    )
    if setup_index is None:
        raise ValueError("Connection has no game setup packet")
    game_frames = frames[setup_index:]
    if len(game_frames) < 5:
        raise ValueError("Connection has insufficient frames")
    # Challenge and login frames precede the game's clock epoch. Their setup
    # header can be an arbitrary initial value, not an elapsed interval.
    slope, intercept, residual = clock_fit(game_frames)
    rtts, last_ping = [], None
    for r in records:
        if isinstance(r, sp.Out) and r.conn == connection and r.data == bytes([251]):
            last_ping = r.t
        elif isinstance(r, sp.Frame) and r.conn == connection:
            if last_ping is not None and any(p == b"p" for p in r.packets):
                if 0 <= r.t - last_ping < 2000:
                    rtts.append(r.t - last_ping)
                last_ping = None
    rtt = statistics.median(rtts) if rtts else 0.0
    controls = []
    action = {"heading": 0.0, "boost": False}
    for r in records:
        if not isinstance(r, sp.Out) or r.conn != connection or len(r.data) != 1:
            continue
        b = r.data[0]
        if b <= 250:
            action["heading"] = (b + 1e-8) / 251 * math.tau
        elif b in (253, 254):
            action["boost"] = b == 253
        else:
            continue
        controls.append((slope * (r.t + rtt) + intercept, dict(action)))

    world = sp.World()
    generation, food_ids = {}, {}
    anchors, observations, food_events, mass_events, pickups = [], [], [], [], []
    start = None
    origin = None
    initial = None
    init_time = None
    observed_anchor = 0
    last_mass = None
    last_observation = -math.inf
    ended = False
    unknown_removals = 0
    reported_state = None
    last_self_pose_report = None
    last_self_wanted_report = None
    reported_age_ms = None
    unmodeled_prey_pickups = []
    censored_mass_times = []

    def point(x, y):
        return {"x": x - origin[0], "y": y - origin[1]}

    def food(value):
        return {**point(value[0], value[1]), "size": value[2], "value": 0.0, "origin": "ambient"}

    for frame in frames:
        S = frame.server
        relative = None if start is None else (S - start) / 1000
        if relative is not None and relative > duration:
            ended = True
            break
        new_events = []
        frame_anchors = []
        for packet in frame.packets:
            actual_heading_report = reports_actual_heading(packet)
            wanted_heading_report = reports_wanted_heading(packet)
            old_self = world.snakes.get(world.self_id)
            old_segments = old_self.sct if old_self else None
            before = dict(world.foods)
            world.feed(packet, S)
            events = list(world.events)
            world.events.clear()
            if start is not None:
                for e in events:
                    if e[0] == "eat" and e[4] == world.self_id and e[2] not in food_ids:
                        raise ValueError(
                            "Self pickup of unobserved food; complete pickup targets unavailable"
                        )
                    if e[0] == "prey_eaten" and e[4] == world.self_id:
                        unmodeled_prey_pickups.append({"time": (S - start) / 1000, "id": e[2]})
            if any(e[0] == "init" for e in events):
                init_time = S
            for e in events:
                if (e[0] == "spawn" or (e[0] == "rot" and actual_heading_report)) and e[
                    2
                ] == world.self_id:
                    last_self_pose_report = S
                    me_report = world.snakes[world.self_id]
                    reported_state = {
                        "time": S,
                        "heading": me_report.ang,
                        "wanted": me_report.wang,
                        "segments": me_report.sct,
                    }
                elif e[0] == "rot" and wanted_heading_report and e[2] == world.self_id:
                    if reported_state is not None:
                        advance_reported_state(reported_state, S)
                        reported_state["wanted"] = world.snakes[world.self_id].wang
                if (
                    e[0] == "spawn"
                    and e[2] == world.self_id
                    or (e[0] == "rot" and wanted_heading_report and e[2] == world.self_id)
                ):
                    last_self_wanted_report = S
                if e[0] == "anchor" and e[2] == world.self_id:
                    anchors.append((S, e[3], e[4]))
                    frame_anchors.append((e[3], e[4]))
                if e[0] == "game_over" and start is not None:
                    raise ValueError("Requested clip includes death; choose a shorter clip")
                if e[0] == "remove" and e[2] == world.self_id and start is not None:
                    raise ValueError("Requested clip includes self removal; choose a shorter clip")
            me_after = world.snakes.get(world.self_id)
            if (
                reported_state is not None
                and old_segments is not None
                and me_after is not None
                and me_after.sct != old_segments
            ):
                advance_reported_state(reported_state, S)
                reported_state["segments"] = me_after.sct
            # Explicit rapid/ambient additions may reuse an existing coordinate.
            explicit = {e[2] for e in events if e[0] == "food"}
            # Bulk F snapshots do not emit individual food events, but can replace
            # a known coordinate with a differently sized pellet.
            explicit.update(
                fid for fid in before.keys() & world.foods.keys() if before[fid] != world.foods[fid]
            )
            added = (world.foods.keys() - before.keys()) | explicit
            eaten = {e[2]: e for e in events if e[0] == "eat"}
            removed = (before.keys() - world.foods.keys()) | (explicit & before.keys())
            for fid in removed:
                stable = food_ids.pop(fid, None)
                if stable is None:
                    continue
                e = eaten.get(fid)
                if e is not None and e[4] == world.self_id:
                    new_events.append(("pickup", stable, None, "self"))
                else:
                    reason = "replacement" if fid in explicit else "visibility"
                    if e is not None:
                        reason = "other-eater" if e[4] is not None else "unknown-eater"
                        unknown_removals += e[4] is None
                    new_events.append(("remove", stable, None, reason))
            for fid in sorted(added):
                if fid not in world.foods:
                    continue
                generation[fid] = generation.get(fid, 0) + 1
                stable = f"{connection}/{fid}/{generation[fid]}"
                food_ids[fid] = stable
                new_events.append(("add", stable, world.foods[fid], "observed-appearance"))
        me = world.snakes.get(world.self_id)
        if start is not None and (not me or not me.pts):
            raise ValueError("Requested clip lost self state; choose a shorter clip")
        if not me or not me.pts or init_time is None:
            continue
        if start is None:
            if S < init_time + start_seconds * 1000 or not frame_anchors:
                continue
            start = S
            origin = tuple(me.pts[-1])
            initial_action = {"heading": me.wang, "boost": False}
            for ct, ca in controls:
                if ct <= start:
                    initial_action = ca
            if me.fam >= 1.0:
                raise ValueError(
                    "Initial wire fam is saturated at one; hidden growth mass is unknown"
                )
            if initial_action["boost"] and me.sp < 13.5:
                raise ValueError(
                    "Initial held boost may be ramping or exhausted; start before boost or at full speed"
                )
            if heading_source == "reported":
                if reported_state is None or last_self_pose_report is None:
                    raise ValueError("No server pose report before replay start")
                reported_age_ms = S - last_self_pose_report
                differing = [
                    ct
                    for ct, ca in controls
                    if last_self_wanted_report < ct <= S
                    and abs(
                        math.atan2(
                            math.sin(ca["heading"] - me.wang), math.cos(ca["heading"] - me.wang)
                        )
                    )
                    > 0.05
                ]
                if differing:
                    raise ValueError("Reported heading has newer unacknowledged steering inputs")
                advance_reported_state(reported_state, S)
                heading = reported_state["heading"]
                heading_method = "server-reported heading advanced to start"
            else:
                heading, heading_method = tangent(anchors, me.ang, history=heading_history)
            initial = {
                "snake": {
                    "id": "recorded-self",
                    "head": point(*origin),
                    "body": [point(*p) for p in reversed(me.pts)],
                    "heading": heading,
                    "speed": me.sp / 0.032,
                    "segments": me.sct,
                    "fam": me.fam,
                    "radius": 14.5 * sp.scale(me.sct),
                    "length": me.sct * 42,
                    "score": 0.0,
                    "alive": True,
                },
                "action": initial_action,
                "food": [{"id": food_ids[fid], "food": food(f)} for fid, f in world.foods.items()],
            }
            last_mass = (me.sct, me.fam)
            mass_events.append({"time": 0.0, "segments": me.sct, "fam": me.fam})
            continue  # Initial state already includes this entire frame.
        t = (S - start) / 1000
        for kind, stable, value, reason in new_events:
            if kind == "pickup":
                pickups.append({"time": t, "id": stable})
            else:
                event = {"time": t, "kind": kind, "id": stable, "reason": reason}
                if kind == "add":
                    event["food"] = food(value)
                food_events.append(event)
        mass = (me.sct, me.fam)
        mass_changed = mass != last_mass
        # Wire fam saturates at one while queued growth may exceed a segment.
        # Do not mistake that lower bound for the server's actual total mass.
        if me.fam >= 1.0:
            censored_mass_times.append(t)
        elif mass_changed:
            mass_events.append({"time": t, "segments": me.sct, "fam": me.fam})
        last_mass = mass
        if frame_anchors or mass_changed or t - last_observation >= 0.1:
            row = {"time": t, "speed": me.sp / 0.032, "segments": me.sct}
            if me.fam < 1.0:
                row["fam"] = me.fam
            if frame_anchors:
                for anchor in frame_anchors:
                    observed_anchor += 1
                    observations.append(
                        {**row, "anchor": point(*anchor), "anchor_index": observed_anchor}
                    )
            else:
                observations.append(row)
            last_observation = t
    if initial is None or not ended:
        raise ValueError("Requested clip is not fully covered by live frames")
    return {
        "schema_version": 1,
        "name": name,
        "seed": 1,
        "duration_seconds": duration,
        "initial": initial,
        "controls": [
            {"time": (t - start) / 1000, "action": a}
            for t, a in controls
            if start < t <= start + duration * 1000
        ],
        "food_events": food_events,
        "mass_events": mass_events,
        "observations": observations,
        "expected_pickups": pickups,
        "options": {"boost_drops": "recorded", "sample_interval_seconds": 0.05},
        "provenance": {
            "source": str(path),
            "sha256": hashlib.sha256(Path(path).read_bytes()).hexdigest(),
            "connection": connection,
            "start_server_ms": start,
            "start_after_init_seconds": (start - init_time) / 1000,
            "clock_slope": slope,
            "clock_intercept_ms": intercept,
            "clock_residual_p95_ms": residual,
            "clock_fit_frames": len(game_frames),
            "clock_ignored_pregame_frames": setup_index,
            "ping_rtt_median_ms": rtt,
            "ping_samples": len(rtts),
            "initial_heading_method": heading_method,
            "initial_heading_source": heading_source,
            "initial_reported_heading_age_ms": reported_age_ms,
            "initial_heading_history": heading_history,
            "unknown_food_removals": unknown_removals,
            "unmodeled_self_prey_pickups": unmodeled_prey_pickups,
            "censored_mass_times": censored_mass_times,
            "limitations": [
                (
                    f"Initial heading estimated causally from up to {heading_history} preceding anchors; head within-tick overshoot unknown"
                    if heading_source == "anchors"
                    else "Initial heading advances an 8-bit server-reported angle through reported wanted headings and segment counts under the candidate turn law; intervening acceptance ticks and head overshoot remain unknown"
                ),
                "Send times use affine receipt/server clock mapping plus median ping RTT; exact acceptance tick unknown",
                "Unknown initial boost phase and RNG; food appearances are replayed, not generated by a recovered server rule",
                "Wire fam=1 is a lower bound on queued growth; saturated mass observations/interventions are omitted",
                "Own observed pickups are targets only; competitor and visibility removals are imposed",
                "Opponents, prey, hidden food, boundary and collision replay are not included",
            ],
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--connection", type=int, default=1)
    parser.add_argument(
        "--start",
        type=float,
        required=True,
        help="Server-clock seconds after initialization; snaps to next self anchor",
    )
    parser.add_argument("--duration", type=float, required=True)
    parser.add_argument(
        "--heading-history",
        type=int,
        default=3,
        help="Preceding anchors for initial heading; 3 keeps the original local estimate, 4+ uses a time-based least-squares fit",
    )
    parser.add_argument(
        "--heading-source",
        choices=("anchors", "reported"),
        default="anchors",
        help="Causal initial heading estimator; reported advances server angles through reported targets and sizes under the candidate turn law",
    )
    parser.add_argument("--name", default="recorded-clip")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if (
        not math.isfinite(args.start)
        or args.start < 0
        or not math.isfinite(args.duration)
        or args.duration <= 0
    ):
        parser.error("start must be nonnegative and duration positive")
    if args.heading_history < 3:
        parser.error("heading history must be at least 3")
    if args.output.exists():
        parser.error("output exists; choose a fresh path")
    clip = export(
        args.archive,
        args.connection,
        args.start,
        args.duration,
        args.name,
        heading_history=args.heading_history,
        heading_source=args.heading_source,
    )
    payload = json.dumps(clip, indent=2) + "\n"
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x", encoding="utf-8") as stream:
        stream.write(payload)
    print(
        json.dumps(
            {
                "name": clip["name"],
                "observations": len(clip["observations"]),
                "controls": len(clip["controls"]),
                "expected_pickups": len(clip["expected_pickups"]),
                "provenance": clip["provenance"],
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
