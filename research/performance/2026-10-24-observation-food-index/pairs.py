"""Adjacent alternating baseline/candidate runs of perf_baseline rows; raw rows to JSONL."""
import json, subprocess, sys, time, pathlib
binaries = {"base": sys.argv[1], "cand": sys.argv[2]}
out = pathlib.Path(sys.argv[3]); reps = int(sys.argv[4]); mode = sys.argv[5]  # timing|profile
cases = [c.split(":") for c in sys.argv[6].split(",")]  # name:iterations
with out.open("a") as f:
    for rep in range(-1, reps):  # rep -1 is an untallied warmup
        for name, it in cases:
            for par in (False, True):
                order = ["base", "cand"] if rep % 2 == 0 else ["cand", "base"]
                for label in order:
                    cmd = [binaries[label], name, it] + (["--profile"] if mode == "profile" else []) + (["--parallel"] if par else [])
                    started = time.time()
                    r = json.loads(subprocess.run(cmd, capture_output=True, text=True, check=True).stdout.splitlines()[-1])
                    d = r["details"]; n = int(it)
                    row = {"rep": rep, "label": label, "case": name, "parallel": par, "mode": mode, "iterations": n,
                           "op_ms": r["timing"]["wallSeconds"] / n * 1e3, "p50_ms": r["timing"]["latencySeconds"]["p50"] * 1e3,
                           "food": d["final"]["food"], "living": d["final"]["living"], "started": started}
                    if mode == "profile":
                        row["observe_ms"] = d["observeSeconds"] / n * 1e3
                        row["step_ms"] = row["op_ms"] - row["observe_ms"]
                    f.write(json.dumps(row) + "\n"); f.flush()
