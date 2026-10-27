"""Select bounded CPU-target results after all build/trace/count/audit stages.

python3 select.py REPOSITORY RAW_OUTPUT MERGED_RUNTIME > results.json
Full raw samples and disassembly remain outside Git.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

if not __debug__:
    raise SystemExit("Study checks require Python without optimization")
repo, root = (Path(v).resolve() for v in sys.argv[1:3])
merged = sys.argv[3]
targets = ["default", "x86-64-v2", "x86-64-v3", "native"]
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
result = {"mergedRuntime": merged, "toolchain": (root / "rustc.txt").read_text(),
          "flags": {}, "builds": {}, "traces": {}, "counters": {}, "math": {}, "codegen": {}}
scheduled = []
for target in targets:
    build = root / "builds" / f"native-{target}"
    pins = json.loads((build / "pins.json").read_text())
    subprocess.run(["git", "-C", str(repo), "diff", "--exit-code", pins["runtimeCommit"], merged,
                    "--", "src", "examples", "Cargo.toml", "Cargo.lock", "build.rs"], check=True)
    binaries = json.loads((build / "binaries.json").read_text())
    for name, digest in binaries.items():
        assert sha(build / "bin" / name) == digest
    assert binaries["study_protocols"] == pins["binarySha256"]
    result["builds"][target] = {**pins, "binaries": binaries}
    result["flags"][target] = "" if target == "default" else f"-C target-cpu={target}"
    bits = root / f"audit/math-{target}.bits"
    assert bits.read_bytes() == (root / "audit/math-default.bits").read_bytes()
    result["math"][target] = {"equal": True, "valuesCompared": len(bits.read_text().splitlines()), "sha256": sha(bits)}
    imports = (root / f"audit/{target}.symbols").read_text()
    names = sorted(re.findall(r"\b(asin|atan2|cos|exp|hypot|log|pow|sin|sincos)@", imports))
    assembly = (root / f"audit/{target}.asm").read_text()
    symbols = (root / f"audit/{target}.all-symbols").read_text()
    result["codegen"][target] = {"mathImports": names,
        "roundingHelpersPresent": {n: bool(re.search(r" [Tt] " + n + r"$", symbols, re.MULTILINE)) for n in ["floor", "round", "ceil"]},
        "roundingInstructionSites": len(re.findall(r"\b(?:v?round(?:sd|pd|ss|ps)|vrndscale(?:sd|pd|ss|ps))\b", assembly)),
        "targetFeatures": (root / f"audit/{target}.cfg").read_text().splitlines(),
        "disassemblySha256": sha(root / f"audit/{target}.asm")}
    assert names == result["codegen"]["default"]["mathImports"]
    if target == "default":
        continue
    pair = root / f"compare-{target}"
    assert (pair / "traces.complete").exists()
    raw_traces = json.loads((pair / "traces/selected.json").read_text())
    historical = []
    for folder in ["stock-traces", "dome-traces"]:
        for row in json.loads((pair / folder / "identity.json").read_text()):
            assert row["equal"] and row["base"]["sha256"] == row["candidate"]["sha256"]
            historical.append({"case": f"{folder}/{row['case']}", "sha256": row["base"]["sha256"], "bytes": row["base"]["bytes"]})
    assert len(raw_traces) == 3 and len(historical) == 9
    assert all(r["equal"] for r in raw_traces.values())
    result["traces"][target] = {"official": raw_traces, "historical": historical, "equalCount": 12}
raw_counts = json.loads((root / "counts/raw.json").read_text())
assert set(raw_counts) == set(targets[1:]), "Incomplete or unexpected target set"
for target in targets[1:]:
    assert set(raw_counts[target]) == {"official500", "official500-half", "official500-legacy"}, "Incomplete or unexpected protocol case set"
    result["counters"][target] = {}
    for case, entry in raw_counts[target].items():
        rows = entry["rows"]
        assert len(rows) == 6 and [r["pair"] for r in rows] == list(range(-1, 5))
        assert [r["warmup"] for r in rows] == [True] + [False] * 5
        for row in rows:
            for arm in ["default", target]:
                scheduled.extend(v["scheduledPct"] for v in row[arm].values())
        result["counters"][target][case] = {"warmupPairs": 1, "measuredPairs": 5, **entry["metrics"]}
assert min(scheduled) >= 99
result["minimumScheduledPct"] = min(scheduled)
result["rawCountsSha256"] = sha(root / "counts/raw.json")
print(json.dumps(result, indent=2))
