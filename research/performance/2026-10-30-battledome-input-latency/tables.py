"""Render bounded selected results; no raw replay or external dependencies."""
from pathlib import Path
import json
import sys

data = json.loads(Path(sys.argv[1]).read_text())


def values(summary, keys=("p50", "p95", "p99", "max")):
    return " / ".join(f"{summary[key]:.3f}" for key in keys) if summary["n"] else "Not observed"


print("| Total seats | Confirmed / attempts | Input-to-effect p50 / p95 / p99 / max ms | Deferred ticks |")
print("| ---: | ---: | ---: | ---: |")
for row in data["rows"]:
    print(f"| {row['seats']} | {row['confirmed']} / {row['attempts']} | "
          f"{values(row['inputToEffectMs'])} | {row['observedDeferredTicks']} |")
print("\n| Seats | ICMP RTT p50 / p95 / p99 / max ms | Control RTT p50 / p95 / p99 / max ms | Game RTT p50 / p95 / p99 / max ms |")
print("| ---: | ---: | ---: | ---: |")
for row in data["rows"]:
    print(f"| {row['seats']} | {values(row['rttMs']['icmp'])} | {values(row['rttMs']['control'])} | {values(row['rttMs']['game'])} |")
print("\n| Seats | Latency minus ICMP RTT p50 / p95 / p99 / max ms | Latency minus control RTT p50 / p95 / p99 / max ms |")
print("| ---: | ---: | ---: |")
for row in data["rows"]:
    print(f"| {row['seats']} | {values(row['latencyMinusRttMs']['icmp'])} | {values(row['latencyMinusRttMs']['control'])} |")
print("\n| Seats | Stats bucket | Samples | Input-to-effect p50 / p95 / p99 / max ms |")
print("| ---: | --- | ---: | ---: |")
for row in data["rows"]:
    for name in ["normal-bucket", "deferred-bucket", "reporting-gap", "uncovered"]:
        summary = row["latencyByStatsBucketMs"].get(name, {"n": 0})
        print(f"| {row['seats']} | {name} | {summary['n']} | {values(summary)} |")
print("\n| Seats | Kernel capture effect p50 / p95 / p99 / max ms | Echo read age p50 / p95 / p99 / max ms | Send call p99 / max ms |")
print("| ---: | ---: | ---: | ---: |")
for row in data["rows"]:
    print(f"| {row['seats']} | {values(row['captureEffectMs']['outgoingToIncoming'])} | "
          f"{values(row['captureEffectMs']['echoReadAge'])} | {values(row['sendCallMs'], ('p99', 'max'))} |")
print("\n| Seats | Userspace gap p99 / max ms | Kernel gap p99 / max ms | Userspace excess p99 / max ms | Kernel excess p99 / max ms | Read age p99 / max ms |")
print("| ---: | ---: | ---: | ---: | ---: | ---: |")
for row in data["rows"]:
    metrics = row["frameMetricsMs"]
    print(f"| {row['seats']} | " + " | ".join(values(metrics[name], ("p99", "max"))
                                            for name in ["gap", "packetGap", "excess", "packetExcess", "readAge"]) + " |")
print("\n| Seats | Raw gaps >20 / >100 / >250 ms | Kernel gaps >20 / >100 / >250 ms | Residual stalls >20 / >100 / >250 ms | Kernel residual stalls >20 / >100 / >250 ms |")
print("| ---: | ---: | ---: | ---: | ---: |")
for row in data["rows"]:
    print(f"| {row['seats']} | " + " | ".join(" / ".join(str(row["stalls"][name][str(limit)])
                                                        for limit in [20, 100, 250])
                                            for name in ["rawGap", "packetGap", "gapResidual", "packetGapResidual"]) + " |")
print("\n| Seats | Load-worker excess p99 / max ms | Independent probe kernel excess p99 / max ms | Load-worker residual stalls >20 ms | Probe kernel residual stalls >20 ms | Worker pass p99 / max ms |")
print("| ---: | ---: | ---: | ---: | ---: | ---: |")
for row in data["rows"]:
    print(f"| {row['seats']} | {values(row['loadClientFrameMetricsMs']['excess'], ('p99','max'))} | "
          f"{values(row['frameMetricsMs']['packetExcess'], ('p99','max'))} | "
          f"{row['loadClientResidualStalls']['20']} | {row['stalls']['packetGapResidual']['20']} | "
          f"{values(row['loadWorkerMaxPassMs'], ('p99','max'))} |")
