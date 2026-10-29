"""Recompute the report tables from bounded selected-results.json."""
from pathlib import Path
import json
import sys


def f(value):
    return "—" if value is None else f"{value:.3f}"


def triple(value, keys=("p50", "p99", "max")):
    return " / ".join(f(value[k]) for k in keys)


def main():
    data = json.loads(Path(sys.argv[1]).read_text())
    rows = sorted(data["rows"], key=lambda r: (r["primary"]["bots"], r["arm"] != "base"))
    print("| Seats | Arm (order) | CPU p50 / p99 / max ms | Deferred | Gap jitter p99 ms | Excess p99 / max ms | Stalls >20 / >100 / >250 ms |")
    print("| --- | --- | ---: | ---: | ---: | ---: | ---: |")
    for row in rows:
        w = row["primary"]
        c = w["client"]
        s = c["residualStalls"]
        print(f"| {w['bots']} | {row['arm']} ({row['order']}) | {triple(w['worldCpuMsPerTick'])} | {w['deferredTicks']} | {f(c['gapResidualMs']['p99'])} | {triple(c['frameExcessMs'], ('p99', 'max'))} | {s['20']} / {s['100']} / {s['250']} |")
    for seats in data.get("notMeasured", []):
        print(f"| {seats} | both arms | Not measured | — | — | — | — |")
    print("\n| Seats | Arm | Respawn p50 / p95 / max ms (n) | Pending last / max age ms | Alive mean / min | Boost % / births | Length first → last | Food first → last |")
    print("| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |")
    for row in rows:
        w = row["primary"]
        s, c, b = w["spawns"], w["census"], w["boost"]
        print(f"| {w['bots']} | {row['arm']} | {triple(s['respawns'], ('p50', 'p95', 'max'))} ({s['respawns']['n']}) | {s['pendingAtLastWorkerReports']} / {f(s['pendingMaxMs'])} | {w['alive']['mean']:.2f} / {w['alive']['min']} | {b['aliveTickShare']*100:.2f} / {b['foodBirths']} | {c['meanSegments']['first']:.2f} → {c['meanSegments']['last']:.2f} | {c['food']['first']} → {c['food']['last']} |")
    print("\n| Seats | Arm | Tail n / mean CPU ms | Spawn ms / % | Sockets+broadcast ms / % | Minimap ms / % | Other ms / % | Search / exhausted ticks |")
    print("| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |")
    for row in rows:
        w = row["primary"]
        t = w["slowestCpu1Percent"]
        cells = " | ".join(f"{f(t['exclusiveMs'][k])} / {t['exclusivePercent'][k]:.2f}" for k in ["spawnSearch", "socketsBroadcast", "minimap", "other"])
        e = t["eventTicks"]
        print(f"| {w['bots']} | {row['arm']} | {t['n']} / {f(t['worldMeanMs'])} | {cells} | {e['admissionSearch']} / {e['searchRefused']} |")
    print("\n| Order | Arm / seats | Canonical ticks / seconds | Full-run deferred | Calibration deferred | Outside all marked windows |")
    print("| ---: | --- | ---: | ---: | ---: | ---: |")
    for row in sorted(rows, key=lambda r: r["order"]):
        w = row["primary"]
        calibration = sum(c["deferredTicks"] for c in row["calibrations"])
        print(f"| {row['order']} | {row['arm']} / {w['bots']} | {w['ticks']} / {w['wallSeconds']:.3f} | {row['totalDeferredTicks']} | {calibration} | {row['deferredOutsideAllMarkedWindows']} |")
    supplementary = [("Interrupted pair", r) for r in data.get("interruptedAttempt", {}).get("rows", [])]
    supplementary += [("Before generator replacement", r) for r in data.get("generatorReplacementAttempt", {}).get("rows", [])]
    supplementary += [("Earlier repeated pair", r) for r in data.get("earlierRepeatAttempt", {}).get("rows", [])]
    if supplementary:
        print("\nSupplementary windows, excluded from the primary comparison:\n")
        print("| Attempt | Seats | Arm | CPU p99 / max ms | Deferred | Respawn p95 ms | Slow-tail exhausted ticks / n |")
        print("| --- | --- | --- | ---: | ---: | ---: | ---: |")
        for attempt, row in supplementary:
            w = row["primary"]
            if row["evidenceStatus"] != "verified":
                print(f"| {attempt} (unverified archive; excluded) | {w['bots']} | {row['arm']} | — | — | — | — |")
                continue
            t = w["slowestCpu1Percent"]
            print(f"| {attempt} | {w['bots']} | {row['arm']} | {triple(w['worldCpuMsPerTick'], ('p99', 'max'))} | {w['deferredTicks']} | {f(w['spawns']['respawns']['p95'])} | {t['eventTicks']['searchRefused']} / {t['n']} |")


if __name__ == "__main__":
    main()
