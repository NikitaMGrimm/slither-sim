"""Bounded same-binary socket metric pilot, before freezing PR inference inputs.

Six balanced fresh-process A/A pairs; one discarded warmup pair per workload.
No CI decision is made from instruction counts without a working counter.
"""

import json
import math
import shutil
import statistics
import subprocess
import time

from perf_baseline import digest, native
from perf_compare import child_error
from perf_stats import ORDER


def residual_sd(pairs, key):
    values = []
    for pair in pairs:
        a, b = (pair[label]["workload"]["timing"].get(key) for label in ("baseline", "candidate"))
        if any(type(v) not in (float, int) or not math.isfinite(v) or v <= 0 for v in (a, b)):
            return None
        values.append(math.log(b) - math.log(a))
    if len(values) != 6:
        return None
    mu = statistics.mean(values)
    gamma = statistics.mean(o * v for o, v in zip(ORDER, values))
    return math.sqrt(sum((v - mu - gamma * o) ** 2 for o, v in zip(ORDER, values)) / 4) * 100


def counter_probe():
    result = {}
    for name, args in {"perf": ["stat", "-e", "instructions", "--", "true"],
                       "valgrind": ["--tool=cachegrind", "--cachegrind-out-file=/dev/null", "true"]}.items():
        command = shutil.which(name)
        if command is None:
            result[name] = {"available": False, "reason": "not installed"}
            continue
        try:
            run = subprocess.run([command, *args], capture_output=True, text=True, timeout=10)
            result[name] = {"available": run.returncode == 0, "exit_code": run.returncode,
                            "diagnostic": (run.stdout + run.stderr)[-4000:]}
        except (OSError, subprocess.TimeoutExpired) as error:
            result[name] = {"available": False, "reason": str(error)}
    return result


def probe(binary, workloads, output, *, deadline):
    sockets = list(workloads) if any(w.mode == "socket-host" for w in workloads) else []
    if not sockets:
        return {}
    output.mkdir()
    result = {"schema": 1, "executable_sha256": digest(binary), "pairs": 6,
              "selection": "CPU if finite and no noisier than busy wall; otherwise busy wall; no inference from this pilot",
              "counters": counter_probe(), "workloads": {}}
    # Probe time consumes the same job reserve, with an additional 240-second
    # bound. Incomplete probes retain wall; there are no retries or exclusions.
    deadline = min(deadline, time.perf_counter() + 240)
    with (output / "results.jsonl").open("w", encoding="utf-8", newline="\n") as log:
        for workload in sockets:
            pairs, failure = [], None
            for index in range(7):
                pair = {}
                labels = ("baseline", "candidate") if index == 0 or ORDER[index - 1] == 1 else ("candidate", "baseline")
                try:
                    for label in labels:
                        row = native(binary, workload.name, workload.iterations,
                                     output / f"{workload.name}-{index}-{label}-stderr.txt", deadline=deadline)
                        log.write(json.dumps({"case": workload.name, "pair": index, "label": label, "record": row}) + "\n")
                        log.flush()
                        error = child_error(row, workload)
                        if error:
                            raise ValueError(error)
                        pair[label] = row
                except (OSError, RuntimeError, TimeoutError, EOFError, ValueError) as error:
                    failure = str(error)
                    break
                if index:
                    pairs.append(pair)
            cpu_key = "cpuSeconds" if workload.mode == "socket-host" else "processCpuSeconds"
            cpu, wall = residual_sd(pairs, cpu_key), residual_sd(pairs, "wallSeconds")
            result["workloads"][workload.name] = {
                "selected": "cpuSeconds" if workload.mode == "socket-host" and cpu is not None and wall is not None and cpu <= wall else "wallSeconds",
                "cpu_scope": "host window" if workload.mode == "socket-host" else "entire case including setup/warmup; diagnostic only",
                "cpu_sd_percent": cpu, "wall_sd_percent": wall, "complete_pairs": len(pairs), "failure": failure}
    if digest(binary) != result["executable_sha256"]:
        raise ValueError("benchmark executable changed during A/A probe")
    (output / "summary.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8", newline="\n")
    return result
