"""Frozen offline boost-ramp analysis. Raw traffic and derived rows stay external."""
import argparse
from collections import Counter, defaultdict
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import statistics
import subprocess
import sys

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "tools/fidelity"))
import slither_protocol as sp

MODELS = ("P305", "V305", "P0", "P390", "P305_equal", "P305_offline", "empirical")
JITTER = (0, 7, 14, 42)
PINS = {
    "run3-large-radius-20260927": "0317f6bbfb7075e019295727e41071e816799d4adbb97d4f6a06ce51748c2a94",
    "run3-single-20260927": "6343bca18a9dbf9a4a105e5d6968087d007a3a3eb198c9a8eb53d095ddcdf06f",
    "visibility-covariates-20260928": "46490bb6a7be12d5450b054c040c9266d55f3e1e0cc53dbc8d8baeec24762101",
}


def limit_cpu():
    # Reuse the prior study's platform-specific affinity guard without executing it.
    path = HERE.parent / "2026-10-28-turn-count-phase/analyze.py"
    spec = importlib.util.spec_from_file_location("turn_study_affinity", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.limit_cpu()


def sha(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def dump(path, value):
    with Path(path).open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.write("\n")


def scale(n):
    return min(6., max(1., 1 + (n - 2) / 106))


def base(n):
    return 5.39 + .4 * scale(n)


def rate(n, up, model="P305"):
    if model not in MODELS:
        raise ValueError(model)
    x = scale(n) - 1
    beta = 0 if model == "P0" else .39 if model == "P390" else .305
    r = 15.45 / (1 + .39 * x) if model == "empirical" else (14 - base(n)) * .015 / .008 / (1 + beta * x)
    r *= 1.3 if model == "P305_offline" else 1
    return r if up else -r * (1 if model == "P305_equal" else 4 / 3)


def fit(points):
    if len(points) < 4 or any(not math.isfinite(x) for p in points for x in p[:2]):
        raise ValueError("need at least four finite speed/time points")
    if any(b[0] <= a[0] for a, b in zip(points, points[1:])):
        raise ValueError("non-increasing report times")
    times = [p[0] / 1000 for p in points]
    mt, mv = statistics.mean(times), statistics.mean(p[1] for p in points)
    den = sum((t - mt) ** 2 for t in times)
    weights = [(t - mt) / den for t in times]
    return dict(slope=sum(w * (p[1] - mv) for w, p in zip(weights, points)),
                weight_l1=sum(abs(w) for w in weights), duration=times[-1] - times[0])


def runs(points):
    """Historical same-sign selector, retaining point metadata and endpoints."""
    current, sign = [], 0
    for p in points:
        if not current:
            current = [p]
            continue
        dt, dv = p[0] - current[-1][0], p[1] - current[-1][1]
        direction = (dv > 0) - (dv < 0)
        if not 0 < dt <= 70 or not direction:
            if len(current) >= 6:
                yield sign > 0, current[1:-1]
            current, sign = [p], 0
        elif sign and direction != sign:
            if len(current) >= 6:
                yield sign > 0, current[1:-1]
            current, sign = [current[-1], p], direction
        else:
            current.append(p)
            sign = direction
    if len(current) >= 6:
        yield sign > 0, current[1:-1]


def extract(path, source):
    if sha(path) != PINS[source]:
        raise ValueError("archive hash mismatch: " + source)
    constant, crossing, series, generations = [], [], {}, Counter()
    counts = Counter()
    conn, world, matching, complete = None, None, False, False
    ordinal = 0

    def flush_constant(sid):
        item = series.get(sid)
        if item:
            for up, points in runs(item["fixed"]):
                constant.append(dict(conn=conn, sid=sid, generation=generations[sid],
                    segments=item["n"], up=up, points=[p[:2] for p in points], source=source))
            item["fixed"] = []

    def flush(sid):
        item = series.get(sid)
        if not item:
            return
        flush_constant(sid)
        for up, points in runs(item["all"]):
            # Packet order matters for events sharing an endpoint timestamp.
            # Retain these in prediction even though qualification requires a
            # strictly interior event. Excluding them would lose real size state.
            events = [e for e in item["events"] if points[0][3] < e[2] < points[-1][3]]
            if any(points[0][0] < e[0] < points[-1][0] for e in events):
                crossing.append(dict(source=source, conn=conn, sid=sid, generation=generations[sid],
                    up=up, points=points, events=events))
        del series[sid]

    for record in sp.read(path):
        if isinstance(record, dict):
            complete |= record.get("event") == "capture-end"
            continue
        if not isinstance(record, sp.Frame):
            continue
        if record.conn != conn:
            for sid in list(series):
                flush(sid)
            conn, world, matching = record.conn, sp.World(contract=False), False
            generations.clear()
        counts["incoming_frames"] += 1
        for packet in record.packets:
            ordinal += 1
            if packet[:1] == b"a":
                for sid in list(series):
                    flush(sid)
                matching = len(packet) >= 25 and packet[23] == 19 and tuple(sp.u16(packet, i) for i in (11, 13, 15)) == (539, 40, 1400)
                counts["matching_handshakes" if matching else "other_handshakes"] += 1
            world.feed(packet, record.server)
            for event in world.events:
                kind = event[0]
                if kind in ("spawn", "remove"):
                    flush(event[2])
                    if kind == "spawn":
                        generations[event[2]] += 1
                elif kind in ("anchor", "tail", "rot"):
                    sid = event[2]
                    snake = world.snakes.get(sid)
                    if snake is None:
                        continue
                    if sid in series and series[sid]["n"] != snake.sct:
                        flush_constant(sid)
                        series[sid]["n"] = snake.sct
                        series[sid]["events"].append([record.server, snake.sct, ordinal])
                        counts["size_splits"] += 1
                    if kind != "rot" or event[3][2] == event[4][2] or not matching:
                        continue
                    if sid not in series:
                        series[sid] = dict(n=snake.sct, fixed=[], all=[], events=[])
                    point = [record.server, snake.sp, snake.sct, ordinal]
                    series[sid]["fixed"].append(point)
                    series[sid]["all"].append(point)
                    counts["changed_speed_reports"] += 1
            world.events.clear()
    for sid in list(series):
        flush(sid)
    if not complete:
        raise ValueError("missing capture-end: " + source)
    counts["capture_end_present"] = 1
    return constant, crossing, dict(counts)


def fixed_summary(rows):
    if not rows:
        return dict(ramps=0)
    fits = [fit(r["points"]) for r in rows]
    output = dict(ramps=len(rows), tracks=len({(r["source"], r["conn"], r["sid"], r["generation"]) for r in rows}),
        segments=[min(r["segments"] for r in rows), max(r["segments"] for r in rows)],
        duration_s=sum(f["duration"] for f in fits), models={})
    for model in MODELS:
        predictions = [rate(r["segments"], r["up"], model) for r in rows]
        errors = [f["slope"] - p for f, p in zip(fits, predictions)]
        output["models"][model] = dict(rmse=math.sqrt(statistics.mean(e*e for e in errors)),
            mae=statistics.mean(abs(e) for e in errors), signed=statistics.mean(errors),
            duration_weighted_rmse=math.sqrt(sum(e*e*f["duration"] for e, f in zip(errors, fits)) / output["duration_s"]),
            outside_half_cell=sum(abs(e) > f["weight_l1"] / 36 for e, f in zip(errors, fits)),
            outside_full_cell_and_time={str(j): sum(abs(e) > f["weight_l1"] * (1/18 + abs(p)*(.0005+j/1000))
                for e, f, p in zip(errors, fits, predictions)) for j in JITTER})
    return output


def trajectory(row, model, event_shift_ms=0):
    points = row["points"]
    n, v, t = points[0][2], points[0][1], points[0][0]
    q = min(1., max(0., (v - base(n)) / (14 - base(n))))
    events = [(max(t, e[0]+event_shift_ms), e[2], e[1]) for e in row["events"]]
    ei, output = 0, [v]

    def advance(dt):
        nonlocal q, v
        if dt <= 0:
            return
        r = rate(n, row["up"], model)
        if model == "V305":
            target = 14 if row["up"] else base(n)
            v = min(target, v+r*dt) if row["up"] else max(target, v+r*dt)
        elif (row["up"] and q < 1) or (not row["up"] and q > 0):
            q = min(1., max(0., q + r/(14-base(n))*dt))
            v = base(n) + (14-base(n))*q

    for p in points[1:]:
        while ei < len(events) and events[ei][:2] <= (p[0], p[3]):
            et, _, nn = events[ei]
            advance((et-t)/1000)
            t, n = et, nn
            # PLAN candidates 1/3..5/7: immediate count effect recomposes
            # interior progress without advancing q. Idle q=0 retains speed;
            # V305 has no size-induced instantaneous refresh.
            if model != "V305" and 0 < q < 1:
                v = base(n) + (14-base(n))*q
            ei += 1
        advance((p[0]-t)/1000)
        t = p[0]
        output.append(v)
    return output


def cross_score(row):
    ns = [row["points"][0][2]] + [e[1] for e in row["events"]]
    out = dict(source=row["source"], conn=row["conn"], generation=row["generation"],
        first_packet=row["points"][0][3], last_packet=row["points"][-1][3], up=row["up"],
        duration_s=(row["points"][-1][0]-row["points"][0][0])/1000,
        segments=[min(ns), max(ns)],
        events=len(row["events"]), models={})
    differences = [b-a for a, b in zip(ns, ns[1:])]
    out["event_type"] = "mixed" if min(differences) < 0 < max(differences) else "growth" if max(differences) > 0 else "loss"
    for model in MODELS[:-1]:
        predicted = trajectory(row, model)
        errors = [a-p[1] for a, p in zip(predicted[1:], row["points"][1:])]
        rmax = max(abs(rate(n, row["up"], model)) for n in ns)
        gates = {}
        for j in JITTER:
            lo, hi = trajectory(row, model, -j-7), trajectory(row, model, j+7)
            indexes = [i for i, p in enumerate(row["points"]) if i and all(abs(p[0]-e[0]) > j+8 for e in row["events"])]
            budget = 2/18 + rmax*(.001+2*j/1000)
            excess = [max(min(predicted[i], lo[i], hi[i])-row["points"][i][1],
                          row["points"][i][1]-max(predicted[i], lo[i], hi[i]), 0) - budget for i in indexes]
            gates[str(j)] = dict(points=len(indexes), compatible=not any(e > 0 for e in excess) if indexes else None)
        out["models"][model] = dict(rmse=math.sqrt(statistics.mean(e*e for e in errors)),
            maximum_error=max(abs(e) for e in errors), gates=gates)
    p, v = trajectory(row, "P305"), trajectory(row, "V305")
    delay = trajectory(row, "P305", 7)
    direct_delay = trajectory(row, "V305", 7)
    out["progress_direct_max_separation"] = max(abs(a-b) for a, b in zip(p, v))
    out["immediate_delayed_max_separation"] = max(abs(a-b) for a, b in zip(p, delay))
    out["direct_immediate_delayed_max_separation"] = max(abs(a-b) for a, b in zip(v, direct_delay))
    out["delayed_models"] = {}
    for name, prediction in (("P305_delay7", delay), ("V305_delay7", direct_delay)):
        errors = [x-p[1] for x,p in zip(prediction[1:],row["points"][1:])]
        out["delayed_models"][name] = dict(rmse=math.sqrt(statistics.mean(e*e for e in errors)),
            maximum_error=max(abs(e) for e in errors))
    out["pairwise_separates_at_jitter"] = {str(j): max(abs(a-b) for a, b in zip(p, v)) >
        2*(2/18 + max(abs(rate(n, row["up"])) for n in ns)*(.001+2*j/1000)) for j in JITTER}
    return out


def cross_summary(rows, include_witnesses=True):
    scores = [cross_score(r) for r in rows]
    if not scores:
        return dict(intervals=0)
    out = dict(intervals=len(rows), tracks=len({(r["source"], r["conn"], r["sid"], r["generation"]) for r in rows}),
        types=dict(Counter(s["event_type"] for s in scores)), rise=sum(r["up"] for r in rows),
        events=sum(s["events"] for s in scores), duration_s=sum(s["duration_s"] for s in scores),
        progress_direct_max_separation=max(s["progress_direct_max_separation"] for s in scores),
        immediate_delayed_max_separation=max(s["immediate_delayed_max_separation"] for s in scores),
        direct_immediate_delayed_max_separation=max(s["direct_immediate_delayed_max_separation"] for s in scores),
        progress_direct_separating_intervals={str(j): sum(s["pairwise_separates_at_jitter"][str(j)] for s in scores) for j in JITTER}, models={})
    for m in MODELS[:-1]:
        out["models"][m] = dict(interval_mean_rmse=statistics.mean(s["models"][m]["rmse"] for s in scores),
            maximum_error=max(s["models"][m]["maximum_error"] for s in scores),
            gates={str(j): dict(exposed=sum(s["models"][m]["gates"][str(j)]["compatible"] is not None for s in scores),
                incompatible=sum(s["models"][m]["gates"][str(j)]["compatible"] is False for s in scores)) for j in JITTER})
    out["delayed_models"] = {name: dict(interval_mean_rmse=statistics.mean(s["delayed_models"][name]["rmse"] for s in scores),
        maximum_error=max(s["delayed_models"][name]["maximum_error"] for s in scores)) for name in ("P305_delay7", "V305_delay7")}
    # Bounded, deterministic witnesses: first emitted row per event type/direction.
    seen, witnesses = set(), []
    for s in scores:
        key = (s["event_type"], s["up"])
        if key not in seen:
            seen.add(key)
            witnesses.append(s)
    if include_witnesses:
        out["witnesses"] = witnesses
        out["j7_failures"] = [s for s in scores if s["models"]["P305"]["gates"]["7"]["compatible"] is False]
    return out


def power_cycle(n0, low, high, seconds=30., growth=True, generous_loss=False):
    """Authored 1-ms schedule: legal anchor-limited growth; observed-speed feedback absent."""
    n, q, v, distance = n0, low, base(n0)+(14-base(n0))*low, 0.
    up, switches, max_sep, event_count = True, 0, 0., 0
    input_budget = 0.
    rmax = 0.
    t, next_drop, mass_spent = 0., 0., 0.
    for _ in range(round(seconds/.001)):
        r = rate(n, up)
        rmax = max(rmax, abs(r))
        q = min(1., max(0., q+r/(14-base(n))*.001))
        target = 14 if up else base(n)
        v = min(target, v+r*.001) if up else max(target, v+r*.001)
        progress_speed = base(n)+(14-base(n))*q
        distance += progress_speed/.032*.001
        if distance >= 42:
            distance -= 42
            if growth and n < 532:
                n += 1
                event_count += 1
        if up and t >= next_drop:
            # Worst nutrition jitter from the current model; accumulated fam can
            # cross a segment boundary but no arbitrary multi-segment loss occurs.
            mass_spent += .0812*max(0., 1-n/430)**1.66*1.08
            if not growth and n > 2 and (generous_loss or mass_spent >= 1):
                n -= 1
                event_count += 1
                mass_spent = max(0., mass_spent-1)
            next_drop = t+.155
        max_sep = max(max_sep, abs(progress_speed-v))
        if (up and q >= high) or (not up and q <= low):
            # Uncertain input arrival can change each transition's trajectory;
            # its slope jump contributes up to (rise+fall)*j per transition.
            input_budget += abs(rate(n, True))+abs(rate(n, False))
            switches += 1
            up = not up
        t += .001
    return dict(start=n0, end=n, low=low, high=high, seconds=seconds, events=event_count,
        switches=switches, max_separation=max_sep, maximum_absolute_rate=rmax,
        full_cell_pair_budget=4/18, input_timing_budget={str(j): input_budget*j/1000 for j in JITTER},
        separates={str(j): max_sep > 4/18+2*rmax*(.001+2*j/1000)+input_budget*j/1000 for j in JITTER},
        maximum_admissible_transition_jitter_ms=max(0.,
            (max_sep-4/18-2*rmax*.001)*1000/(4*rmax+input_budget)))


def power():
    sizes = (2, 9, 37, 106, 214, 343, 430, 500)
    fixed = []
    for n in sizes:
        rise = (14-base(n))/rate(n, True)
        fall = -(14-base(n))/rate(n, False)
        # Growth can lengthen a ramp. Bound count growth by v<=437.5, hence
        # h(t)<=h(0)+(.305/106)*(437.5/42)*t until scale saturation.
        k = .305/106*(437.5/42)
        growing_rise_bound = min(.008/.015*(1+.305*5),
            (1+.305*(scale(n)-1))*math.expm1(k/(.015/.008))/k)
        fixed.append(dict(n=n, cruise=base(n), rise_s=rise, fall_s=fall,
            growing_rise_time_upper_s=growing_rise_bound,
            maximum_growing_anchors=math.ceil(437.5*growing_rise_bound/42),
            generous_loss_events=math.ceil(max(rise, fall)/.155),
            single_segment_refresh_bound=.4/106,
            monotone_growth_refresh_bound=math.ceil(437.5*growing_rise_bound/42)*.4/106,
            constant_rate={m: dict(rise=rate(n, True, m), fall=rate(n, False, m)) for m in MODELS}))
    cycles = [dict(kind=kind, **power_cycle(n, lo, hi, growth=kind=="growth", generous_loss=kind=="generous_loss"))
        for kind in ("growth", "loss", "generous_loss") for n in sizes for lo, hi in ((.2,.4),(.4,.6),(.6,.8))]
    # Phase: old/new count differs for one tick. A refresh plus slope perturbation
    # bounds any one-event speed effect; repetition with same initial state adds
    # no phase information once both candidates have applied the same count.
    slope_phase = max(abs(rate(n+1, up)-rate(n, up))*.007 for n in range(2,532) for up in (True,False))
    phase = .4/106 + slope_phase
    return dict(label="authored best-case bounds and schedules, not captured evidence", fixed=fixed,
        phase_one_event_bound=phase, phase_fifteen_event_bound=.4/106+15*slope_phase,
        universal_full_rise_time_upper_s=.008/.015*(1+.305*5),
        universal_growth_events_upper=math.ceil(437.5*(.008/.015*(1+.305*5))/42),
        minimum_growth_events_at_q_point2_quantization_only=math.ceil((4/18)/(.4/106*.8)),
        required_independent_intervals_for_monotone_latent_identification=None,
        interval_count_reason="No finite count removes the bounded quantizer/timing overlap; no IID noise model is assumed.",
        speed_byte=1/18, cycles=cycles)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--recordings", type=Path, required=True, help="external historical headless directory")
    parser.add_argument("--output", type=Path, required=True, help="fresh external directory")
    args = parser.parse_args()
    limit_cpu()
    args.output.mkdir(parents=True, exist_ok=False)
    fixture_path = REPO / "tests/fixtures/speed-ramp-validation.json"
    fixture = json.loads(fixture_path.read_text(encoding="utf-8"))
    constant, crossing, sources = [], [], []
    for source, digest in PINS.items():
        rows, cross, counts = extract(args.recordings / source / "bot0.jsonl.gz", source)
        expected = [r for r in fixture["ramps"] if r["source"] == source]
        # No timestamp-offset fitting: the original equation study already used
        # setup-relative time, unlike the older turn study's original clock.
        identity = lambda r: (r["conn"], r["sid"], r["generation"], r["points"][0][0])
        if sorted(rows, key=identity) != sorted(expected, key=identity):
            dump(args.output / "reproduction-failure.json", dict(source=source, raw_count=len(rows), fixture_count=len(expected), raw=rows))
            raise ValueError("exact fixture reproduction failed: " + source)
        constant.extend(rows)
        crossing.extend(cross)
        sources.append(dict(source=source, sha256=digest, counts=counts, exact_fixture_reproduction=True,
            reproduction_comparison="exact per-interval metadata/points, sorted by connection/life/start; output order immaterial",
            fixed=fixed_summary(rows), changing=cross_summary(cross, include_witnesses=False)))
    dump(args.output / "fixed-rows.json", constant)
    dump(args.output / "changing-rows.json", crossing)
    dump(args.output / "changing-scores.json", [cross_score(r) for r in crossing])
    power_result = power()
    cycles = power_result.pop("cycles")
    dump(args.output / "power-cycles.json", cycles)
    power_result["cycle_summary"] = []
    for kind in ("growth", "loss", "generous_loss"):
        cohort = [c for c in cycles if c["kind"] == kind]
        power_result["cycle_summary"].append(dict(kind=kind, schedules=len(cohort),
            separating_schedules={str(j): sum(c["separates"][str(j)] for c in cohort) for j in JITTER},
            maximum_separation_case=max(cohort, key=lambda c:c["max_separation"]),
            maximum_jitter_case=max(cohort, key=lambda c:c["maximum_admissible_transition_jitter_ms"])))
    groups = []
    for lo, hi in ((2,9),(10,29),(30,79),(80,159),(160,100000)):
        for up in (True, False):
            rows = [r for r in constant if lo <= r["segments"] <= hi and r["up"] == up]
            groups.append(dict(size_band=f"{lo}..{hi if hi != 100000 else 'max'}", up=up, **fixed_summary(rows)))
    result = dict(schema_version=1, data_origin="retained official protocol-19 recordings; retrospective development",
        plan_commit="3b4266150c7c4ab26ffc9583c957a391abb9820b",
        simulator_base=subprocess.check_output(["git", "rev-parse", "ffa6e032"], cwd=REPO, text=True).strip(),
        historical_introduction="91c52fa5396c5a0afba632e60ca8bb8c9a84aa28", historical_retained="c2fb7285febb4273034e30699dc880715a334423",
        hashes={"plan":sha(HERE/"PLAN.md"), "analyzer":sha(__file__), "decoder":sha(REPO/"tools/fidelity/slither_protocol.py"),
            "tests":sha(HERE/"test_analysis.py"),
            "affinity_helper":sha(HERE.parent/"2026-10-28-turn-count-phase/analyze.py"), "fixture":sha(fixture_path)},
        external_artifacts={p.name:sha(p) for p in sorted(args.output.glob("*.json"))},
        sources=sources, fixed=fixed_summary(constant), groups=groups, changing=cross_summary(crossing), power=power_result)
    dump(args.output / "selected-results.json", result)
    print(json.dumps(dict(fixed_ramps=len(constant), changing_intervals=len(crossing),
        fixed_rmse=result["fixed"]["models"]["P305"]["rmse"],
        changing_max_model_separation=result["changing"]["progress_direct_max_separation"])))


if __name__ == "__main__":
    main()
