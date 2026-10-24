"""Rotating-order rounds of perf_observation executables; median per-batch materialization."""
import json, subprocess, sys, statistics as st, pathlib
labels = sys.argv[1].split(","); exes = sys.argv[2].split(","); reps = int(sys.argv[3]); out = pathlib.Path(sys.argv[4])
cases = [c.split(":") for c in sys.argv[5].split(",")]
rows = []
with out.open("a") as f:
    for rep in range(-1, reps):
        for case, it in cases:
            order = list(range(len(labels))); k = rep % len(order); order = order[k:] + order[:k]
            for j in order:
                r = json.loads(subprocess.run([exes[j], case, it], capture_output=True, text=True, check=True).stdout.splitlines()[-1])
                row = {"rep": rep, "label": labels[j], "case": case, "iterations": int(it), "us": r["timing"]["wallSeconds"] / int(it) * 1e6}
                f.write(json.dumps(row) + "\n"); rows.append(row)
for case, _ in cases:
    base = {r["rep"]: r["us"] for r in rows if r["case"] == case and r["label"] == labels[0] and r["rep"] >= 0}
    for l in labels:
        v = {r["rep"]: r["us"] for r in rows if r["case"] == case and r["label"] == l and r["rep"] >= 0}
        ratio = [v[k] / base[k] for k in base]
        print(f"{case:16} {l:8} {st.median(v.values()):8.2f} us [{min(v.values()):.2f}-{max(v.values()):.2f}] ratio {st.median(ratio):.3f} [{min(ratio):.3f}-{max(ratio):.3f}]")
