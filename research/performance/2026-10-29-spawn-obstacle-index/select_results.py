"""Reduce bound, immutable native pairs remotely, including failed gate results."""
from pathlib import Path
import hashlib
import json
import math
import statistics as stats
import sys

root, destination = map(Path, sys.argv[1:3])


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def quantile(values, p):
    return sorted(values)[math.ceil((len(values) - 1) * p)]


def summarize(records):
    if not records:
        return {"n": 0}
    samples = [r["sample"] for r in records]
    assert all(s["enabledNs"] > 0 and s["runningNs"] / s["enabledNs"] >= .99 for s in samples)
    cpu = [s["cpuNs"] for s in samples]
    return {"n": len(cpu), "instructionsMean": stats.mean(s["instructions"] for s in samples),
            "cpuNs": {"mean": stats.mean(cpu), "p50": quantile(cpu, .5),
                      "p99": quantile(cpu, .99), "max": max(cpu)},
            "minimumPmuFraction": min(s["runningNs"] / s["enabledNs"] for s in samples)}


manifest_path = root / "measurement-pins.json"
manifest = json.loads(manifest_path.read_text())
builds = {}
for arm in ["base", "candidate", "mutation"]:
    path = root / arm / "pins.json"
    pin = json.loads(path.read_text())
    assert pin["releaseBuild"] and pin["rustflags"] == ""
    assert sha(root / arm / "source-manifest.json") == pin["instrumentedManifestSha256"]
    for name, digest in pin["binaries"].items():
        assert sha(root / arm / "bin" / name) == digest
    if arm != "mutation":
        assert sha(path) == manifest["arms"][arm]["pinsFileSha256"]
    builds[arm] = {key: pin[key] for key in ["commit", "archiveSha256", "instrumentedManifestSha256", "binaries"]}
rows = []
for row in ["live-refused", "live-successful", "spawn", "seats80", "seats160"]:
    for protocol in [19, 13]:
        binding = json.loads((root / f"{row}-{protocol}-measurement-binding.json").read_text())
        assert binding["status"] == "complete" and binding["measurementManifestSha256"] == sha(manifest_path)
        pairs = json.loads((root / f"pairs-{row}-{protocol}.json").read_text())
        assert len(pairs) == 6 and pairs[0]["warmup"] and [p["pair"] for p in pairs[1:]] == list(range(5))
        selected_pairs = []
        trace_digest = None
        for pair in pairs:
            result = {"pair": pair["pair"]}
            for arm in ["base", "candidate"]:
                value = json.loads((root / pair["paths"][arm]).read_text())
                records = value["records"] if row.startswith("live-") else value["ticks"]
                assert len(records) == (512 if row.startswith("live-") else 360)
                groups = {"all": records, "slowestCpu1Percent": sorted(records, key=lambda r:r["sample"]["cpuNs"], reverse=True)[:math.ceil(len(records) * .01)]}
                if row.startswith("live-"):
                    assert value["stateUnchanged"]
                    expected = [1, 4096, 1, 0] if row == "live-refused" else [1, 1638, 0, 0]
                    for record in records:
                        assert record["sample"]["events"] == expected
                        trace = json.dumps({k:v for k,v in record.items() if k != "sample"}, sort_keys=True, separators=(",", ":")).encode()
                        digest = hashlib.sha256(trace).hexdigest()
                        if trace_digest is None:
                            trace_digest = digest
                        assert digest == trace_digest
                else:
                    groups.update(search=[r for r in records if r["sample"]["events"][0]],
                                  exhausted=[r for r in records if r["sample"]["events"][2]],
                                  other=[r for r in records if not r["sample"]["events"][0]])
                result[arm] = {key:summarize(group) for key,group in groups.items()}
            if not pair["warmup"]:
                selected_pairs.append(result)
        ratios = [p["candidate"]["all"]["cpuNs"]["mean"] / p["base"]["all"]["cpuNs"]["mean"] for p in selected_pairs]
        instructions = [p["candidate"]["all"]["instructionsMean"] / p["base"]["all"]["instructionsMean"] for p in selected_pairs]
        delta = stats.median(p["candidate"]["all"]["cpuNs"]["p50"] for p in selected_pairs) - stats.median(p["base"]["all"]["cpuNs"]["p50"] for p in selected_pairs)
        gate = all(r < 1 for r in ratios) if row == "live-refused" else all(r <= 1 for r in ratios) if row == "live-successful" else delta <= 10_000
        def spread(values):
            return {"median":stats.median(values), "min":min(values), "max":max(values)}
        rows.append({"row":row, "protocolLabel":protocol,
                     "scope":"completeAdmission" if row.startswith("live-") else "completeHostTick",
                     "pairs":selected_pairs, "cpuRatio":spread(ratios), "instructionsRatio":spread(instructions),
                     "p50DeltaNs":delta, "gatePass":gate,
                     "resultAndRngCanonicalSha256":trace_digest,
                     "minimumPmuFraction":min(p[a]["all"]["minimumPmuFraction"] for p in selected_pairs for a in ["base", "candidate"])})
output = {"builds":builds, "measurementManifestSha256":sha(manifest_path),
          "runnerSha256":manifest["runnerSha256"], "fixtureSha256":manifest["fixtureSha256"],
          "selectorSha256":sha(Path(__file__)), "warmupWindowsDiscardedPerRow":1,
          "gate":{"passed":all(r["gatePass"] for r in rows), "failedRows":[[r["row"],r["protocolLabel"]] for r in rows if not r["gatePass"]]},
          "allRowsBoundToFrozenBuildPins":True, "rows":rows}
destination.write_text(json.dumps(output, indent=2) + "\n", newline="\n")
print(json.dumps({"gate":output["gate"], "rows":[{"row":r["row"],"protocol":r["protocolLabel"],"cpuRatio":r["cpuRatio"],"p50DeltaUs":r["p50DeltaNs"]/1000} for r in rows]}))
