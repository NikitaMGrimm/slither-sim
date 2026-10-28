"""Select path-free counters/hashes and print descriptive tables; keep all pairs."""
import argparse
import hashlib
import json
from pathlib import Path
import statistics as st

if not __debug__:
    raise SystemExit("Run without Python optimization")

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("root", type=Path)
p.add_argument("output", type=Path)
a = p.parse_args()
root = a.root
load = lambda name: json.loads((root / name).read_text())
results = {"format": "spawn-bounds-study-v1", "machine": "a 4-vCPU Zen 5 EPYC KVM guest",
           "source": load("final-source-pins.json"), "binaries": load("binary-pins.json"),
           "socketBinaries": load("socket-pins.json"), "mutation": load("mutation/result.json"),
           "matrix": load("matrix/selected.json"), "scaling": load("scaling/selected.json"),
           "sockets": load("socket-counts/selected.json")}
catalog = load("matrix/catalog.json")
results["workloads"] = {v["name"]: {k: v[k] for k in ["iterations", "traceIterations", "definition"]}
                        for v in catalog["workloads"]}
results["harnesses"] = {}
for arm in ["base", "candidate"]:
    results["harnesses"][arm] = {name: hashlib.sha256((root / arm / name).read_bytes()).hexdigest()
        for name in ["examples/perf_baseline.rs", "examples/study_matrix.rs", "src/study_counter.rs",
                     "src/client/simulation/study_scaling.rs"]}
assert len(results["matrix"]) == 23 and len(results["scaling"]) == len(results["sockets"]) == 2
for name, row in results["matrix"].items():
    coverage = []
    for pair in range(-1, 5):
        for arm in ["base", "candidate"]:
            log = root / "matrix" / f"{name}-{pair}-{arm}.log"
            report = json.loads(log.read_text().splitlines()[-1])
            details = report.get("details", {})
            coverage.append({k: details[k] for k in ["initial", "final", "population", "lifecycle",
                "validity", "physicsTicks", "warmup", "admissionCalls", "reconnectTimed", "epochs",
                "initialQueued"] if k in details})
    assert all(v == coverage[0] for v in coverage), (name, "measured lifecycle/statistics drift")
    row["coverage"] = coverage[0]
    pairs = [r for r in row["pairs"] if not r["warmup"]]
    assert len(pairs) == 5
    reductions = [(1 - r["candidate"][0] / r["base"][0]) * 100 for r in pairs]
    medians = [st.median(r[arm][0] for r in pairs) for arm in ["base", "candidate"]]
    row["summary"] = {"baseMedian": medians[0], "candidateMedian": medians[1],
                      "pairedReductionPercent": st.median(reductions), "range": [min(reductions), max(reductions)]}
    print(name, *medians, st.median(reductions), min(reductions), max(reductions))
    if name.startswith("client-"):
        n = results["workloads"][name]["iterations"] * 6
        print("CLIENT PHASES", name, {arm: [v / n for v in row[arm + "Diagnostic"]["instructions"][:2]]
                                     for arm in ["base", "candidate"]})
for name, row in results["scaling"].items():
    for pair in row["pairs"]:
        for arm in ["base", "candidate"]:
            path = root / "scaling" / f'{name}-{pair["pair"]}-{arm}' / "rows.json"
            ticks = json.loads(path.read_text())
            assert len(ticks) == (1000 if name == "2000-common" else 20)
            fractions = [r[phase][2] / r[phase][1] for r in ticks for phase in ["spawn", "step"]]
            assert min(fractions) >= .99
            for phase in ["spawn", "step"]:
                assert sum(r[phase][0] for r in ticks) / len(ticks) == pair[arm][phase]
            pair[arm]["rowsSha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
            pair[arm]["minimumPmuRunningFraction"] = min(fractions)
    pairs = [r for r in row["pairs"] if not r["warmup"]]
    assert len(pairs) == 5
    summary = {}
    for arm in ["base", "candidate"]:
        summary[arm] = {phase: st.median(r[arm][phase] for r in pairs) for phase in ["spawn", "step"]}
        summary[arm]["total"] = st.median(r[arm]["spawn"] + r[arm]["step"] for r in pairs)
    summary["totalReductionPercent"] = st.median((1 - (r["candidate"]["spawn"] + r["candidate"]["step"]) /
        (r["base"]["spawn"] + r["base"]["step"])) * 100 for r in pairs)
    summary["processReductionPercent"] = st.median((1 - r["candidate"]["process"][0] / r["base"]["process"][0]) * 100 for r in pairs)
    row["summary"] = summary
    print("SCALING", name, summary)
for name, pairs in results["sockets"].items():
    for pair in pairs:
        for arm in ["base", "candidate"]:
            log = root / "socket-counts" / f'{name}-{pair["pair"]}-{arm}.log'
            pair[arm]["logSha256"] = hashlib.sha256(log.read_bytes()).hexdigest()
    pairs = [r for r in pairs if not r["warmup"]]
    assert len(pairs) == 5
    print("SOCKET", name, {arm: {phase: st.median(r[arm][phase] for r in pairs)
          for phase in ["instructionsPerTick", "searchPerTick"]} for arm in ["base", "candidate"]})
a.output.write_text(json.dumps(results, indent=2) + "\n")
