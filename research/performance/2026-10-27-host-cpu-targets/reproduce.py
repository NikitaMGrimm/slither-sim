"""Linux x86-64 CPU-target study, using the unchanged recipient-work fixtures.

python3 reproduce.py {build,trace,count,audit} REPOSITORY NEW_OUTPUT
    --runtime COMMIT --perf PERF --cpu 3
Builds one target at a time, one Cargo job. No network game connections.
Raw binaries, traces, disassembly and samples remain in OUTPUT, outside Git.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys

if not __debug__:
    raise SystemExit("Study checks require Python without -O/PYTHONOPTIMIZE")

FIXTURE = "8014ad0da6ad8d67f3d305ca3c43ed53841d66bd"
FIXTURE_PATH = "research/performance/2026-10-27-host-snake-candidates"
TARGETS = ["default", "x86-64-v2", "x86-64-v3", "native"]
CASES = ["official500", "official500-half", "official500-legacy"]
GUARD = 'if not __debug__: raise SystemExit("Run without Python optimization")\n'

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("mode", choices=["build", "trace", "count", "audit"])
p.add_argument("repository", type=Path)
p.add_argument("output", type=Path)
p.add_argument("--runtime", required=True)
p.add_argument("--perf")
p.add_argument("--cpu", type=int, default=3)
a = p.parse_args()
repo, root = a.repository.resolve(), a.output.resolve()
os.sched_setaffinity(0, {a.cpu})
env = dict(os.environ, CARGO_BUILD_JOBS="1", RAYON_NUM_THREADS="1",
           RUST_TEST_THREADS="1", RUSTUP_TOOLCHAIN="1.99.0",
           CARGO_PROFILE_RELEASE_DEBUG="1")
for key in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "STUDY_TICK_COUNTS"]:
    env.pop(key, None)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(cmd, log, cwd=root, extra=None):
    with log.open("w") as f:
        subprocess.run(cmd, cwd=cwd, env=dict(env, **(extra or {})),
                       stdout=f, stderr=subprocess.STDOUT, check=True)


def fixture_files():
    folder = root / "fixture"
    folder.mkdir()
    for name in ["reproduce.py", "protocol_fixture.rs", "tick_counter.rs"]:
        data = subprocess.check_output(
            ["git", "-C", str(repo), "show", f"{FIXTURE}:{FIXTURE_PATH}/{name}"])
        if name.endswith(".py"):
            data = GUARD.encode() + data
        (folder / name).write_bytes(data)
    return folder


def binary(target):
    return root / "builds" / f"native-{target}" / "bin/study_protocols"


def rehash():
    for target in TARGETS:
        out = root / "builds" / f"native-{target}"
        pins = json.loads((out / "pins.json").read_text())
        assert pins["runtimeCommit"] == a.runtime
        assert sha(binary(target)) == pins["binarySha256"]
        for name, digest in json.loads((out / "binaries.json").read_text()).items():
            assert sha(out / "bin" / name) == digest


if a.mode == "build":
    root.mkdir(parents=True, exist_ok=False)
    fixture = fixture_files()
    driver = (fixture / "reproduce.py").read_text()
    driver = driver.replace("arms=['base','candidate']", f"arms={TARGETS!r}")
    driver = driver.replace("zip(arms,[a.base,a.candidate])", "zip(arms,[a.base]*len(arms))")
    driver = driver.replace("target=out/'target';", "env['RUSTFLAGS']='' if arm=='default' else '-C target-cpu='+arm\n        target=out/'target';")
    (fixture / "build.py").write_text(driver)
    run([sys.executable, str(fixture / "build.py"), "build", str(repo),
         str(root / "builds"), "--base", a.runtime, "--cpu", str(a.cpu)], root / "build.log")
    for target in TARGETS:
        out = root / "builds" / f"native-{target}"
        (out / "binaries.json").write_text(json.dumps({f.name: sha(f) for f in (out / "bin").iterdir()}, indent=2) + "\n")
        helper = out / "harness/trace_compare.py"
        helper.write_text(GUARD + helper.read_text())
    run(["rustc", "-vV"], root / "rustc.txt")
    run(["lscpu"], root / "cpu-local-only.txt")

if a.mode == "trace":
    rehash()
    for target in TARGETS[1:]:
        pair = root / f"compare-{target}"
        pair.mkdir()
        for arm, actual in [("base", "default"), ("candidate", target)]:
            (pair / f"native-{arm}").symlink_to(root / "builds" / f"native-{actual}", target_is_directory=True)
        run([sys.executable, str(root / "fixture/reproduce.py"), "trace", str(repo),
             str(pair), "--cpu", str(a.cpu)], root / f"trace-{target}.log")
        print("TWELVE TRACES EQUAL", target, flush=True)
    rehash()

if a.mode == "count":
    assert a.perf
    rehash()
    out = root / "counts"
    out.mkdir()
    result = {}
    for target in TARGETS[1:]:
        assert (root / f"compare-{target}/traces.complete").exists()
        result[target] = {}
        for case in CASES:
            expected = json.loads((root / f"compare-{target}/traces/selected.json").read_text())[case]["summary"]
            rows = []
            for i in range(-1, 5):
                row = {"pair": i, "warmup": i < 0}
                arms = ["default", target]
                if i >= 0 and i % 2:
                    arms.reverse()
                for arm in arms:
                    prefix = out / f"{target}-{case}-{i}-{arm}"
                    run([a.perf, "stat", "-x;", "-e", "instructions:u,cycles:u",
                         "-o", str(prefix.with_suffix(".stat")), "--", str(binary(arm)), case], prefix.with_suffix(".log"))
                    counters = {}
                    for line in prefix.with_suffix(".stat").read_text().splitlines():
                        fields = line.split(";")
                        if len(fields) > 4 and fields[2] in ["instructions:u", "cycles:u"]:
                            assert float(fields[4]) >= 99, fields
                            counters[fields[2]] = {"value": int(fields[0]), "scheduledPct": float(fields[4])}
                    assert len(counters) == 2
                    assert json.loads(prefix.with_suffix(".log").read_text().splitlines()[-1]) == expected
                    row[arm] = counters
                rows.append(row)
            metrics = {}
            for event in ["instructions:u", "cycles:u"]:
                ratios = [r[target][event]["value"] / r["default"][event]["value"] for r in rows[1:]]
                metrics[event] = {"defaultMedian": statistics.median(r["default"][event]["value"] for r in rows[1:]),
                                  "targetMedian": statistics.median(r[target][event]["value"] for r in rows[1:]),
                                  "pairedMedian": statistics.median(ratios), "pairedRange": [min(ratios), max(ratios)]}
            result[target][case] = {"metrics": metrics, "rows": rows}
            (out / "raw.json").write_text(json.dumps(result, indent=2) + "\n")
            print("COUNT", target, case, metrics, flush=True)
    rehash()

if a.mode == "audit":
    rehash()
    out = root / "audit"
    out.mkdir()
    for target in TARGETS:
        run(["objdump", "-d", "-C", str(binary(target))], out / f"{target}.asm")
        run(["nm", "-D", str(binary(target))], out / f"{target}.symbols")
        run(["nm", "-C", str(binary(target))], out / f"{target}.all-symbols")
        run(["rustc", "--print", "cfg", "-O", "-C", "debug-assertions=no", *([] if target == "default" else ["-C", f"target-cpu={target}"])], out / f"{target}.cfg")
        probe = Path(__file__).with_name("math_probe.rs")
        executable = out / f"math-{target}"
        run(["rustc", "--edition=2024", "-O", *([] if target == "default" else ["-C", f"target-cpu={target}"]),
             str(probe), "-o", str(executable)], out / f"math-{target}-build.log")
        run([str(executable)], out / f"math-{target}.bits")
        assert (out / f"math-{target}.bits").read_bytes() == (out / "math-default.bits").read_bytes(), target
        print("MATH BITS EQUAL", target, flush=True)
