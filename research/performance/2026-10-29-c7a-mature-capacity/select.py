"""Select cited ladder aggregates; keep raw samples and cloud receipts private."""
from pathlib import Path
import argparse
import bisect
import hashlib
import json
import sys
import tarfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raw", type=Path)
    parser.add_argument("harness", type=Path, help="External staged, linked harness")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    sys.path.insert(0, str(args.harness.resolve()))
    from metrics import prior
    from window import reduce_window

    raw = args.raw.resolve()
    summary = json.loads((raw / "run-summary.json").read_text())
    pins = json.loads((raw / "pins.json").read_text())
    process = json.loads((raw / "host-run/process.json").read_text())
    assert process["binarySha256"] == pins["binaries"]["host"]
    config = json.loads((raw / "host-run/config.json").read_text())
    assert config["bots"]["count"] == 0 and config["players"]["max"] == summary["maxBotSlots"] + 1
    host = [json.loads(x) for x in (raw / "host-run/telemetry.jsonl").read_text().splitlines()]
    clients = [json.loads(x) for x in (raw / "load-run/telemetry.jsonl").read_text().splitlines()]
    client_rows = [json.loads(x[7:]) for x in (raw / "load-run/client.log").read_text().splitlines() if x.startswith("CLIENT ")]
    hz = 100  # Verified with getconf CLK_TCK on both instances before collection.
    sockets = []
    censuses = []
    for line in (raw / "host-run/host.log").read_text().splitlines():
        if line.startswith("SOCKET "):
            sockets.append(json.loads(line[7:]))
        elif line.startswith("CENSUS "):
            value = json.loads(line[7:])
            censuses.append((value["unix"], value["tick"]))
    stats = [json.loads(x) for x in (raw / "load-run/stats.jsonl").read_text().splitlines()]
    human_periods = []
    active = []
    for row in [*sockets, {"externalPlayers": 0}]:
        if row.get("externalPlayers", 0):
            active.append(row)
        elif active:
            start, end = active[0]["unix"], active[-1]["unix"]
            seat_stats = [x["players"] for x in stats if start <= x["unix"] <= end]
            times = [x[0] for x in censuses]
            ticks = [censuses[max(0, bisect.bisect_right(times, at) - 1)][1] for at in [start, end]]
            human_periods.append({"sampleTickRange": ticks, "sampleSpanSeconds": end - start, "externalPlayingRange": [min(x["externalPlayers"] for x in active), max(x["externalPlayers"] for x in active)], "statsPlayersRange": [min(seat_stats), max(seat_stats)] if seat_stats else None})
            active = []

    def cpu(samples, member, start, end):
        first, _ = prior.cpu_at(samples, start, member)
        last, _ = prior.cpu_at(samples, end, member)
        return prior.cpu_delta(first, last)

    def core_at(at):
        samples = [x for x in host if "core" in x]
        index = bisect.bisect_left([x["server"]["unix"] for x in samples], at)
        assert 0 < index < len(samples), "core edge must be bracketed"
        first, last = samples[index - 1:index + 1]
        span = last["server"]["unix"] - first["server"]["unix"]
        assert 0 < span < 2.5
        fraction = (at - first["server"]["unix"]) / span
        a = [int(x) / hz for x in first["core"].split()[1:]]
        b = [int(x) / hz for x in last["core"].split()[1:]]
        return [x + (y - x) * fraction for x, y in zip(a, b)]

    windows = []
    for window in summary["windows"]:
        result = reduce_window(raw / "load-run", window)
        assert result == window["result"], "offline selection must reproduce online gates"
        start, end = result.pop("windowUnix")
        n = result["ticks"]
        phase = "confirm-counting" if window["label"].startswith("down-") else "counting"
        assert all((raw / "host-run" / f"{window['label']}-{edge}-processes.txt").is_file() for edge in ["start", "end"])
        assert all((raw / "load-run" / f"generator-{phase}-{window['bots']}-{edge}.txt").is_file() for edge in ["start", "end"])
        server_cpu = cpu(host, "server", start, end)
        collector_cpu = cpu(host, "collector", start, end)
        client_cpu = cpu(clients, "client", start, end)
        world_cpu = sum(t["userSeconds"] + t["kernelSeconds"] for t in server_cpu["threads"] if t["role"].startswith(("slither-simulat", "slither-sockets")))
        task_mean = world_cpu / n * 1000
        clock_mean = result["worldCpuMsPerTick"]["mean"]
        reconciles = abs(task_mean - clock_mean) <= max(.03, .02 * clock_mean)
        result["accounting"] = {
            "reconciles": reconciles,
            "worldTaskCpuMeanMsPerTick": task_mean,
            "clockMinusTaskMeanMsPerTick": clock_mean - task_mean,
            "hostProcessCpuMeanMsPerTick": (server_cpu["userSeconds"] + server_cpu["kernelSeconds"]) / n * 1000,
            "collectorCpuMeanMsPerTick": (collector_cpu["userSeconds"] + collector_cpu["kernelSeconds"]) / n * 1000,
            "threadCpuMsPerTick": [{"role": t["role"], "user": t["userSeconds"] / n * 1000, "kernel": t["kernelSeconds"] / n * 1000, "affinity": t["affinity"]} for t in server_cpu["threads"]],
        }
        assert reconciles, "CPU-clock and task-counter disagreement"
        reports = [x for x in client_rows if start - 1.1 <= x["unix"] <= end + 1.1]
        deltas = {key: sum(max(x[key] for x in reports if x["worker"] == w) - min(x[key] for x in reports if x["worker"] == w) for w in range(1, 7)) for key in ["frames", "bytes", "decisions", "joins", "deaths", "forced", "admissionRefusals"]}
        assert all(max(x["frames"] for x in reports if x["worker"] == w) > min(x["frames"] for x in reports if x["worker"] == w) for w in range(1, 7))
        assert deltas["forced"] == 0 and deltas["admissionRefusals"] == 0
        assert not any(x["thin"] for x in reports)
        result["client"].update({
            "processCpuCoresMean": (client_cpu["userSeconds"] + client_cpu["kernelSeconds"]) / (end - start),
            "counterDeltas": deltas,
            "maxReceiptAgeSeconds": max(x["maxReceiptAge"] for x in reports),
        })
        a, b = core_at(start), core_at(end)
        result["hostVcpuContextMsPerTick"] = {key: (b[i] - a[i]) / n * 1000 for i, key in enumerate(["user", "nice", "system", "idle", "iowait", "irq", "softirq", "steal"])}
        # These are distinct accounting views, not additive CPU components.
        result["clientValid"] = result["clientValid"] and reconciles
        result["passes"] = result["passes"] and reconciles
        windows.append(result)

    cpu_info = json.loads((raw / "host-run/cpu-private.json").read_text())["lscpu"]
    model = next(x["data"] for x in cpu_info if x["field"] == "Model name:")
    with tarfile.open(args.harness / "main.tar") as archive:
        original = {x.name: hashlib.sha256(archive.extractfile(x).read()).hexdigest() for x in archive if x.isfile() and x.name.startswith("src/") and x.name.endswith(".rs")}
    overlays = {k: v for k, v in pins["sourceOverlays"].items() if original.get(k) != v}
    selection = {
        "runtime": summary["runtime"],
        "server": {"instanceType": "c7a.medium", "vcpus": 1, "cpuModel": model},
        "generator": {"instanceType": "c8a.2xlarge", "vcpus": 8, "workers": 6},
        "status": summary["status"],
        "initialBots": summary["initialBots"],
        "maxBotSlots": summary["maxBotSlots"],
        "stopReason": summary.get("stopReason"),
        "humanPlayingPeriods": human_periods,
        "matureScreen": [{k: v for k, v in x.items() if k != "unix"} for x in summary.get("matureScreen", [])],
        "windows": windows,
        "pins": {k: pins[k] for k in ["runtime", "archiveSha256", "priorHarnessSha256", "clientSourceSha256", "cpuTarget", "toolchain", "binaries"]},
        "rawAccess": "Private; raw logs, command inventories, source copies, cloud receipts and identifiers excluded.",
    }
    selection["pins"]["changedSourceSha256"] = overlays
    selection["pins"]["newHarnessSha256"] = {name: hashlib.sha256((args.harness / name).read_bytes()).hexdigest() for name in ["prepare.py", "build.sh", "host_agent.py", "metrics.py", "window.py", "run_ladder.py", "generator_boundaries.py"]}
    if "failure" in summary:
        # Failure prose needs manual privacy review; only its class is public.
        selection["failureType"] = summary["failureType"]
    args.output.write_text(json.dumps(selection, indent=2) + "\n", encoding="utf-8")
    print("Selected", len(windows), "windows; passing", [x["bots"] for x in windows if x["passes"]])


if __name__ == "__main__":
    main()
