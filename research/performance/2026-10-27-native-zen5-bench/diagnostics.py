"""Additive CPU/clock telemetry and retained-data anomaly reduction; stdlib only."""
import hashlib
import inspect
import json
import math
from pathlib import Path


def require(ok, message):
    if not ok:
        raise RuntimeError(message)


def stats(values):
    values = sorted(values)
    if not values:
        return {"count": 0}
    return {"count": len(values), "min": values[0],
            **{name: values[math.ceil(len(values) * rank) - 1]
               for name, rank in [("p50", .5), ("p99", .99), ("max", 1)]}}


def clock_sample():
    mhz = None
    for block in Path("/proc/cpuinfo").read_text().split("\n\n"):
        fields = dict(line.split(":", 1) for line in block.splitlines() if ":" in line)
        fields = {k.strip(): v.strip() for k, v in fields.items()}
        if fields.get("processor") == "0" and "cpu MHz" in fields:
            mhz = float(fields["cpu MHz"])
            break
    cpu0 = next((line for line in Path("/proc/stat").read_text().splitlines()
                 if line.startswith("cpu0 ")), None)
    return {"cpu0Mhz": mhz, "cpu0Counters": cpu0}


def worker_overlay(original, destination):
    text = Path(original).read_text(encoding="utf-8")
    anchor = "'cpuSeconds': (int(stat[11]) + int(stat[12])) / HZ,"
    require(text.count(anchor) == 1, "CPU sampler anchor mismatch")
    text = text.replace(anchor, anchor + "\n                'userCpuSeconds': int(stat[11]) / HZ,"
                        "\n                'systemCpuSeconds': int(stat[12]) / HZ,")
    anchor = "def snapshot(pids):"
    require(text.count(anchor) == 1, "snapshot anchor mismatch")
    text = text.replace(anchor, inspect.getsource(clock_sample) + "\n\n" + anchor)
    anchor = "'cpu': Path('/proc/stat').read_text().splitlines()[0], 'nic': net,"
    require(text.count(anchor) == 1, "snapshot CPU anchor mismatch")
    text = text.replace(anchor, anchor + "\n            'diagnostic': clock_sample(),")
    Path(destination).write_text(text, encoding="utf-8", newline="\n")


def boundaries(telemetry):
    rows = telemetry["rows"]
    return (next(r for r in rows if r.get("boundary") == "start"),
            next(r for r in rows if r.get("boundary") == "end"))


def steal(a, b):
    x = [int(v) for v in a.split()[1:]]
    y = [int(v) for v in b.split()[1:]]
    delta = [v - u for u, v in zip(x, y)]
    total = sum(delta[:8])
    return 100 * delta[7] / total if total else None


def cpu_clock(out):
    out = Path(out)
    telemetry = json.loads((out / "host/telemetry.json").read_text())
    counter = json.loads((out / "host/host.json").read_text())
    ca, cb = boundaries(counter)
    ticks = cb["ticks"] - ca["ticks"]
    a, b = boundaries(telemetry)
    require(ticks > 0 and len(a["processes"]) == 1, "invalid host boundary")
    pid = next(iter(a["processes"]))
    pa, pb = a["processes"][pid], b["processes"][pid]
    require(pa is not None and pb is not None, "missing host process at boundary")
    if "userCpuSeconds" not in pa or "userCpuSeconds" not in pb:
        return {"status": "unavailable", "reason": "original sampler retained combined CPU only",
                "userMsPerExecutedTick": None, "systemMsPerExecutedTick": None,
                "observedCpu0Mhz": {"count": 0}, "cpu0StealPercent": None}
    user = pb["userCpuSeconds"] - pa["userCpuSeconds"]
    system = pb["systemCpuSeconds"] - pa["systemCpuSeconds"]
    total = pb["cpuSeconds"] - pa["cpuSeconds"]
    require(abs(user + system - total) < 1e-6, "user/system counters do not sum to total")
    samples = [r["diagnostic"]["cpu0Mhz"] for r in telemetry["rows"]
               if a["unix"] <= r["unix"] <= b["unix"] and r["diagnostic"]["cpu0Mhz"] is not None]
    return {"status": "measured", "executedTicks": ticks,
            "userMsPerExecutedTick": user * 1000 / ticks,
            "systemMsPerExecutedTick": system * 1000 / ticks,
            "totalMsPerExecutedTick": total * 1000 / ticks,
            "resourceWindowSeconds": b["unix"] - a["unix"],
            "observedCpu0Mhz": stats(samples),
            "cpu0StealPercent": steal(a["diagnostic"]["cpu0Counters"], b["diagnostic"]["cpu0Counters"])}


def resource_window(telemetry, start, end):
    rows = telemetry["rows"]
    a, b = [min(rows, key=lambda row: abs(row["unix"] - at)) for at in [start, end]]
    elapsed = b["unix"] - a["unix"]
    require(elapsed > 0, "empty resource window")
    cpu_seconds = sum(b["processes"][pid]["cpuSeconds"] - value["cpuSeconds"]
                      for pid, value in a["processes"].items() if value and b["processes"].get(pid))
    selected = [r for r in rows if a["unix"] <= r["unix"] <= b["unix"]]
    rss = [sum(v["rssBytes"] for v in row["processes"].values() if v) for row in selected]
    per_thread = {}
    per_core = {}
    for pid, value in a["processes"].items():
        other = b["processes"].get(pid)
        cpus = {cpu for thread in a["threads"].get(pid, {}).values() for cpu in thread["cpus"]}
        if value and other and len(cpus) == 1:
            cpu = str(next(iter(cpus)))
            per_core[cpu] = per_core.get(cpu, 0) + (
                other["cpuSeconds"] - value["cpuSeconds"]) / elapsed * 100
    for pid, threads in a["threads"].items():
        for tid, value in threads.items():
            other = b["threads"].get(pid, {}).get(tid)
            if other:
                name = value["name"]
                per_thread[name] = per_thread.get(name, 0) + (
                    other["cpuSeconds"] - value["cpuSeconds"]) / elapsed * 100
    result = {"sampleStartOffsetSeconds": a["unix"] - start,
            "sampleEndOffsetSeconds": b["unix"] - end,
            "windowSeconds": elapsed, "cpuPercentOneCore": cpu_seconds / elapsed * 100,
            "aggregateStealPercent": steal(a["cpu"], b["cpu"]),
            "rssMiBMin": min(rss) / 2**20, "rssMiBMax": max(rss) / 2**20,
            "threadCpuPercentOneCore": per_thread,
            "processCpuPercentByPinnedCore": per_core,
            "userSystemSplit": "unavailable in original retained telemetry"}
    if all(v and "userCpuSeconds" in v and b["processes"].get(pid)
           for pid, v in a["processes"].items()):
        for source, target in [("userCpuSeconds", "userCpuPercentOneCore"),
                               ("systemCpuSeconds", "systemCpuPercentOneCore")]:
            result[target] = sum(b["processes"][pid][source] - v[source]
                                 for pid, v in a["processes"].items()) / elapsed * 100
        result["userSystemSplit"] = "measured"
    if "diagnostic" in a:
        result["cpu0StealPercent"] = steal(a["diagnostic"]["cpu0Counters"], b["diagnostic"]["cpu0Counters"])
        result["observedCpu0Mhz"] = stats(r["diagnostic"]["cpu0Mhz"] for r in selected
                                           if r["diagnostic"]["cpu0Mhz"] is not None)
    return result


def anomaly(out):
    out = Path(out)
    counter = json.loads((out / "host/host.json").read_text())
    host = json.loads((out / "host/telemetry.json").read_text())
    load = json.loads((out / "load/telemetry.json").read_text())
    origin, ticks = json.loads((out / "host/ticks.json").read_text())
    start = counter["holdStartUnix"]
    end = counter["holdEndUnix"]
    records = [r for r in counter["rows"] if start <= r["unix"] <= end + .1]
    increments = [(a, b) for a, b in zip(records, records[1:]) if b["deferred"] > a["deferred"]]
    require(bool(increments), "no hold deferrals to analyze")
    first, last = increments[0][0]["unix"], increments[-1][1]["unix"]
    windows = [("before", max(start, first - 15), first), ("deferral episode", first, last),
               ("after", last, min(end, last + 15))]
    def summarize(label, a, b):
        chosen = [r for r in ticks if a <= origin + r[0] * 1e-9 < b]
        return {"label": label, "startSeconds": a - start, "endSeconds": b - start,
                "tickBusyMs": stats(r[1] * 1e-6 for r in chosen),
                "tickStartGapMs": stats(r[2] * 1e-6 for r in chosen if r[2]),
                "host": resource_window(host, a, b), "generator": resource_window(load, a, b)}
    events = []
    for a, b in increments:
        item = summarize("deferral increment", a["unix"], b["unix"])
        item["deferredTicks"] = b["deferred"] - a["deferred"]
        events.append(item)
    missing = ["host user/system CPU split", "CPU-0-specific steal", "clock MHz"] if "diagnostic" not in host["rows"][0] else []
    return {"attempt": out.name, "deferredTicks": sum(e["deferredTicks"] for e in events),
            "sampledDeferralIncrements": events,
            "windows": [summarize(*window) for window in windows],
            "samplingLimit": "counters and resources sampled about once per second; tick timings buffered per tick",
            "missingObservations": missing}


def cpu_split_fields(value):
    fields = set()
    if isinstance(value, dict):
        for key, child in value.items():
            name = key.lower()
            if name in {"utime", "stime", "userseconds", "systemseconds"} or any(
                    token in name for token in ["usercpu", "systemcpu", "cpuuser", "cpusystem", "userms", "systemms"]):
                fields.add(key)
            fields.update(cpu_split_fields(child))
    elif isinstance(value, list):
        for child in value:
            fields.update(cpu_split_fields(child))
    return sorted(fields)


def export(root):
    root = Path(root)
    summaries = []
    for p in sorted((root / "raw").glob("*/summary.json")):
        d = json.loads(p.read_text())
        if d["label"] not in ["qualification", "rerun"]:
            continue
        summaries.append({"attempt": p.parent.name, "commit": d["commit"], "preset": d["preset"],
                          "requested": d["requested"], "label": d["label"],
                          "combinedCpuMsPerExecutedTick": d["cpuMsPerExecutedTick"],
                          "cpuClock": cpu_clock(p.parent)})
    original = next((root / "raw").glob("*qualification-65ff713-official-500"))
    result = {"classification": "diagnostic; qualification rows unchanged", "cpuClockByAttempt": summaries,
              "original211Anomaly": anomaly(original),
              "sourceSha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    result["rerunDeferralDiagnostics"] = [anomaly(root / "raw" / row["attempt"])
        for row in summaries if row["label"] == "rerun"
        and json.loads((root / "raw" / row["attempt"] / "summary.json").read_text())["deferredTicks"] > 0]
    result["awsUserSystemSplit"] = []
    for name in ["2026-10-27-epyc-host-recheck", "2026-10-27-epyc-index-recheck"]:
        source = root / "harness/research/performance" / name / "selected-results.json"
        data = json.loads(source.read_text())
        fields = cpu_split_fields(data)
        result["awsUserSystemSplit"].append({"source": "research/performance/" + name + "/selected-results.json",
            "sourceSha256": hashlib.sha256(source.read_bytes()).hexdigest(), "splitFields": fields,
            "status": "unavailable" if not fields else "inspect available fields before comparing"})
    (root / "diagnostic-results.json").write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    import os
    os.sched_setaffinity(0, {1, 2, 3})
    export(Path(__file__).resolve().parent)
