"""Offline frozen count/turn analysis; full derived traffic remains external."""
import argparse
import base64
import ctypes
import gzip
import hashlib
import json
import math
import os
from pathlib import Path
import statistics
import subprocess

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
A = 2 * math.asin(math.sqrt(2) / 39) + 2 * math.pi / 65536


def limit_cpu():
    if os.name == "nt":
        k = ctypes.WinDLL("kernel32", use_last_error=True)
        k.GetCurrentProcess.restype = ctypes.c_void_p
        k.GetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_size_t)]
        k.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
        current, system = ctypes.c_size_t(), ctypes.c_size_t()
        handle = k.GetCurrentProcess()
        if not k.GetProcessAffinityMask(handle, ctypes.byref(current), ctypes.byref(system)):
            raise ctypes.WinError(ctypes.get_last_error())
        if not k.SetProcessAffinityMask(handle, current.value & -current.value):
            raise ctypes.WinError(ctypes.get_last_error())
    else:
        os.sched_setaffinity(0, [min(os.sched_getaffinity(0))])
    os.environ.update(CARGO_BUILD_JOBS="1", RAYON_NUM_THREADS="1", RUST_TEST_THREADS="1")


def sha(path):
    with Path(path).open("rb") as f:
        return hashlib.file_digest(f, "sha256").hexdigest()


def dump(path, obj):
    Path(path).write_text(json.dumps(obj, indent=2, allow_nan=False) + "\n", encoding="utf-8", newline="\n")


def rate(n, denominator=106):
    sc = min(6., max(1., 1 + (n - 2) / denominator))
    return .033 / .008 * (.13 + .87 * ((7 - sc) / 6) ** 2)


def wrap(a):
    return (a + math.pi) % (2 * math.pi) - math.pi


def score(theta, seconds, prediction):
    residual = abs(abs(theta) - prediction * seconds)
    return dict(predicted_rate=prediction, absolute_rate_error=residual / seconds,
        angle_error=residual, admitted_at_jitter_ms={str(j): residual <= A + prediction * (.001 + 2 * j / 1000)
            for j in (0, 7, 14, 42)}, minimum_endpoint_jitter_ms=max(0., ((residual - A) / prediction - .001) * 500))


def summary(values):
    return dict(minimum=min(values), median=statistics.median(values), maximum=max(values))


def export(source, target):
    connection = 0
    with gzip.open(source, "rt", encoding="utf-8") as f, target.open("w", encoding="utf-8", newline="\n") as out:
        for line, text in enumerate(f, 1):
            r = json.loads(text)
            if isinstance(r, dict):
                if r.get("event") == "connect":
                    connection += 1
                continue
            direction, time, data = r
            if direction == "i":
                out.write(json.dumps(dict(line=line, connection=connection, t_ms=time,
                    bytes=list(base64.b64decode(data, validate=True))), separators=(",", ":")) + "\n")


def reconstruct(fixture, row_paths):
    tracks = {}
    audits = []
    for label, path in row_paths.items():
        for line in path.open(encoding="utf-8"):
            r = json.loads(line)
            if r.get("kind") == "audit":
                audits.append(dict(source=label, **r))
                continue
            key = (label, r["connection"], r["id"], r["generation"])
            tracks.setdefault(key, []).append(r)
    output = []
    for run_index, run in enumerate(fixture["runs"]):
        key = (run["source"], run["connection"], run["snake_id"], run["generation"])
        track = tracks[key]
        first, last = run["first_center_anchor_index"], run["last_center_anchor_index"]
        assert all(r["index"] == i for i, r in enumerate(track))
        selected = track[first:last + 1]
        assert selected[0]["legacy_server_ms"] == run["first_center_server_ms"]
        assert selected[-1]["legacy_server_ms"] == run["last_center_server_ms"]
        # Audit the fixed selection and its neighboring triplets; never reselect.
        margins, chords, gaps = [], [], []
        sign = 1 if run["direction"] == "left" else -1
        for i in range(first - 1, last + 2):
            a, b, c = track[i - 1:i + 2]
            u = [b["anchor"][k] - a["anchor"][k] for k in (0, 1)]
            v = [c["anchor"][k] - b["anchor"][k] for k in (0, 1)]
            edges = [math.hypot(*u), math.hypot(*v)]
            spans = [b["server_ms"] - a["server_ms"], c["server_ms"] - b["server_ms"]]
            delta = wrap(math.atan2(v[1], v[0]) - math.atan2(u[1], u[0]))
            margin = wrap(b["wanted"] - math.atan2(u[1] + v[1], u[0] + v[0])) * sign
            assert all(39 <= e <= 45 for e in edges)
            assert all(0 < t <= 700 for t in spans)
            assert delta * sign / (sum(spans) / 2000) > .15 and margin > .55
            if first <= i <= last:
                speed = b["speed_per_32ms"]
                assert (speed >= 13.5 if run["mode"] == "boost" else speed <= 7)
            margins.append(margin); chords.extend(edges); gaps.extend(spans)
        angle, duration = 0., 0.
        substitutions = {name: 0. for name in ("logical", "physical", "physical_minus_dying", "logical_plus_fam", "displayed_length")}
        phases = {f"D{d}_{phase}": 0. for d in (106, 111) for phase in ("before", "after", "previous_anchor", "next_anchor")}
        for i in range(first, last + 1):
            a, b, c = track[i - 1:i + 2]
            v1 = [b["anchor"][k] - a["anchor"][k] for k in (0, 1)]
            v2 = [c["anchor"][k] - b["anchor"][k] for k in (0, 1)]
            dt = (c["server_ms"] - a["server_ms"]) / 2000
            angle += wrap(math.atan2(v2[1], v2[0]) - math.atan2(v1[1], v1[0]))
            duration += dt
            n = b["after"]
            for name in substitutions:
                value = n["logical"] + n["fam"] if name == "logical_plus_fam" else n[name]
                substitutions[name] += rate(value) * dt
            for d in (106, 111):
                for phase, count in (("before", b["before"]["logical"]), ("after", n["logical"]),
                    ("previous_anchor", a["after"]["logical"]), ("next_anchor", c["after"]["logical"])):
                    phases[f"D{d}_{phase}"] += rate(count, d) * dt
        delta_angle = angle - run["angle_delta_radians"]
        delta_time = duration - run["duration_seconds"]
        if abs(delta_angle) > 1e-8 or abs(delta_time) > 1e-10:
            raise ValueError(f"fixture reproduction mismatch for run {run_index}: angle={delta_angle} time={delta_time}")
        counters = {name: summary([r["after"][name] for r in selected]) for name in
            ("logical", "physical", "dying", "physical_minus_dying", "fam", "displayed_length")}
        assert all(r["wire_count"] == r["after"]["logical"] for r in selected)
        assert all(r["after"]["physical_minus_dying"] == r["after"]["logical"] for r in selected)
        output.append(dict(run=run_index, source=run["source"], connection=run["connection"],
            snake_id=run["snake_id"], generation=run["generation"], first_anchor=first, last_anchor=last,
            count=counters, legacy_epoch_offset_ms=selected[0]["legacy_server_ms"]-selected[0]["server_ms"],
            wire_logical_mismatches=0, angle_reproduction_error=delta_angle,
            fixed_selection_gate_audit=dict(minimum_wanted_margin=min(margins),
                chord_minimum=min(chords), chord_maximum=max(chords),
                minimum_server_gap_ms=min(gaps), maximum_server_gap_ms=max(gaps)),
            duration_reproduction_error=delta_time, before_after_count_changes=sum(
                r["before"]["logical"] != r["after"]["logical"] for r in selected),
            substitutions={k: score(angle, duration, v / duration) for k, v in substitutions.items()},
            phases={k: score(angle, duration, v / duration) for k, v in phases.items()}))
    return output, audits


def calculate(fixture):
    runs = []
    loss = {}
    for k in range(21):
        loss[k] = sum(r["duration_seconds"] * (abs(r["angle_delta_radians"]) / r["duration_seconds"]
            - rate(max(2, r["segments"] - k))) ** 2 for r in fixture["runs"])
    fitted_k = min(loss, key=lambda k: (loss[k], k))
    for i, r in enumerate(fixture["runs"]):
        models = {"D106": rate(r["segments"]), "D111": rate(r["segments"], 111),
                  "offset1": rate(max(2, r["segments"] - 1)), "offset5": rate(max(2, r["segments"] - 5)),
                  "offset_development_fit": rate(max(2, r["segments"] - fitted_k))}
        runs.append(dict(run=i, segments=r["segments"], mode=r["mode"], direction=r["direction"],
            seconds=r["duration_seconds"], observed_rate=abs(r["angle_delta_radians"]) / r["duration_seconds"],
            scores={name: score(r["angle_delta_radians"], r["duration_seconds"], q) for name, q in models.items()}))
    total = sum(r["seconds"] for r in runs)
    rmse = {name: math.sqrt(sum(r["seconds"] * r["scores"][name]["absolute_rate_error"] ** 2 for r in runs) / total)
            for name in runs[0]["scores"]}
    power = []
    for n in (9, 37, 100, 174, 244, 293):
        rates = {"D106": rate(n), "D111": rate(n, 111), "offset1": rate(max(2, n - 1)),
                 "offset5": rate(max(2, n - 5)), "offset_development_fit": rate(max(2, n - fitted_k))}
        q = rates["D111"]
        comparisons = {}
        for name, other in rates.items():
            if name == "D111":
                continue
            comparisons[name] = {str(j): ((2 * A + (q + other) * (.001 + 2 * j / 1000)) / abs(q - other))
                                 for j in (0, 7, 14, 42)}
        power.append(dict(segments=n, rates=rates, required_seconds_vs_D111=comparisons,
            one_segment_7ms_phase_angle=abs(rate(n + 1, 111) - q) * .007,
            equivalent_106_count_deficit=5 * (n - 2) / 111))
    admitted_offsets = {str(j): [k for k in range(21) if all(score(r["angle_delta_radians"],
        r["duration_seconds"], rate(max(2, r["segments"] - k)))["admitted_at_jitter_ms"][str(j)]
        for r in fixture["runs"])] for j in (0, 7, 14, 42)}
    return dict(angular_budget=A, fitted_global_offset=fitted_k,
                global_offsets_admitted_all_runs_by_jitter_ms=admitted_offsets, duration_weighted_rmse=rmse,
                runs=runs, power=power)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--archive-root", type=Path, required=True)
    p.add_argument("--binary", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    a = p.parse_args()
    limit_cpu()
    a.output.mkdir(parents=True, exist_ok=False)
    fixture_path = REPO / "tests/fixtures/movement-turn-runs.json"
    fixture = json.loads(fixture_path.read_text())
    rows = {}
    for source in fixture["sources"]:
        capture = a.archive_root / source["path"]
        if sha(capture) != source["sha256"]:
            raise ValueError("retained source hash mismatch")
        label = source["id"]
        incoming = a.output / (label + "-input.jsonl")
        export(capture, incoming)
        rows[label] = a.output / (label + "-rows.jsonl")
        with incoming.open("rb") as inp, rows[label].open("wb") as out, (a.output / (label + "-stderr.txt")).open("wb") as err:
            subprocess.run([str(a.binary.resolve())], stdin=inp, stdout=out, stderr=err, check=True)
        assert sha(capture) == source["sha256"]
    geometry = json.loads(subprocess.check_output([str(a.binary.resolve()), "geometry"], text=True))
    reconstructed, audits = reconstruct(fixture, rows)
    result = dict(schemaVersion=1, status="retrospective development; conditional uncertainty only",
        simulatorRevision="4dd66ab00ab8b0acb2b9ea1432a43f9fd31a8196",
        clientRevision="8d0772c015d6b9b73b80253db8c7349aa600b201",
        freezeCommit="b588af3c4d25286c226e4ed2f88bb4128ad32e03", planSha256=sha(HERE / "PLAN.md"),
        fixtureSha256=sha(fixture_path), binarySha256=sha(a.binary),
        sourceSha256={str(f.relative_to(HERE)).replace("\\", "/"): sha(f) for f in
            (HERE / "analyze.py", HERE / "harness/Cargo.toml", HERE / "harness/Cargo.lock", HERE / "harness/src/main.rs")},
        captures=[dict(id=s["id"], sha256=s["sha256"]) for s in fixture["sources"]],
        client_audits=audits, reconstructed=reconstructed, geometry=geometry, **calculate(fixture))
    dump(a.output / "selected-results.json", result)
    print(json.dumps(dict(runs=len(reconstructed), audits=audits, rmse=result["duration_weighted_rmse"])))


if __name__ == "__main__":
    main()
