"""Post-hoc wall/CPU context from the same archived canonical tick interval.

Capture layout is unchanged from #262's reduce.py. This does not select windows,
change qualification, or infer a scheduler/kernel cause from a clock gap.
"""
import json
import math
import statistics


def analyze(lines, tick_range):
    records, census = {}, []
    first, last = tick_range
    for line in lines:
        if not line.endswith(b"\n"):
            continue
        if line.startswith(b"CENSUS "):
            value = json.loads(line[7:])
            if first <= value["tick"] <= last:
                census.append(value)
        elif line.startswith(b"CAPACITY "):
            value = json.loads(line[9:])
            if first < value["tick"] <= last:
                assert len(value["rows"]) == 100
                for offset, row in enumerate(value["rows"]):
                    records[value["tick"] - 99 + offset] = row
    assert sorted(records) == list(range(first + 1, last + 1))
    assert census[0]["tick"] == first and census[-1]["tick"] == last

    def summary(values):
        values = sorted(values)
        return {"n": len(values), "mean": statistics.mean(values),
                **{name: values[math.ceil((len(values) - 1) * q)]
                   for name, q in [("p50", .5), ("p99", .99), ("max", 1)]}}

    def describe(tick, row):
        return {"tick": tick, "busyWallMs": row[2] / 1e6,
                "simulationCpuMs": row[0] / 1e6, "socketCpuMs": row[1] / 1e6,
                "admissionSearch": row[4][3] > 0, "foodEpoch": row[4][0] > 0}

    slow = sorted(records.items(), key=lambda item: item[1][2], reverse=True)
    tail = slow[:math.ceil(len(slow) * .01)]
    batches = []
    for before, after in zip(census, census[1:]):
        delta = after["deferred"] - before["deferred"]
        assert delta >= 0
        if delta:
            rows = [records[t] for t in range(before["tick"] + 1, after["tick"] + 1)]
            batches.append({"tickRange": [before["tick"], after["tick"]],
                            "deferredTicks": delta,
                            "maxBusyWallMs": max(r[2] for r in rows) / 1e6,
                            "maxAlignedCpuMs": max(r[0] + r[1] for r in rows) / 1e6})
    return {
        "tickRange": tick_range,
        "busyWallMs": summary([r[2] / 1e6 for r in records.values()]),
        "wallMinusSimulationCpuMs": summary([(r[2] - r[0]) / 1e6 for r in records.values()]),
        "largestWallTick": describe(*slow[0]),
        "slowestWall1Percent": {"n": len(tail),
            "meanBusyWallMs": statistics.mean(r[2] for _, r in tail) / 1e6,
            "meanSimulationCpuMs": statistics.mean(r[0] for _, r in tail) / 1e6,
            "meanSocketCpuMs": statistics.mean(r[1] for _, r in tail) / 1e6,
            "admissionTicks": sum(r[4][3] > 0 for _, r in tail),
            "foodEpochTicks": sum(r[4][0] > 0 for _, r in tail)},
        "deferralBatches": {"n": len(batches),
            "totalDeferredTicks": sum(v["deferredTicks"] for v in batches),
            "largestThree": sorted(batches, key=lambda v: v["deferredTicks"], reverse=True)[:3]},
    }
