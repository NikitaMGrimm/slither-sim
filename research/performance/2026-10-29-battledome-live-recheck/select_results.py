"""Reuse #262's canonical selector; bind this comparison's arm and lean output.

Usage: select_results.py PRIVATE_RAW STAGED_HARNESS OUTPUT ARM BUILD_PINS
STAGED_HARNESS contains the linked reducer/helpers and this arm's main.tar.
BUILD_PINS is the private native-build receipt, not a public per-file map.
"""
from pathlib import Path
import hashlib
import json
import sys
import tarfile
import tempfile
import types

HERE = Path(__file__).resolve().parent
RUNTIMES = {
    "base": "cc1bb1e9935094a00cf513413d7922d858dd5c1e",
    "candidate": "539b37f3be969827b5eac67c2978e604f9e814e8",
}


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def bind_archive(raw):
    """Bind the exact reducer inputs to the retrieved archive, before selection."""
    receipt = json.loads((raw.parent / "retrieval-verification.json").read_text())
    archive = raw.parent / "results.tar.gz"
    assert archive.stat().st_size == receipt["archiveBytes"]
    assert sha(archive) == receipt["archiveSha256"]
    seen = set()
    with tarfile.open(archive, "r|gz") as stream:
        for member in stream:
            if member.isdir():
                continue
            assert member.isfile(), "Only regular raw files may supply selection"
            path = Path(member.name)
            assert not path.is_absolute() and ".." not in path.parts
            assert member.name not in seen, "Duplicate archive input"
            with stream.extractfile(member) as source:
                assert hashlib.file_digest(source, "sha256").hexdigest() == sha(raw / path), member.name
            seen.add(member.name)
    assert seen == {p.relative_to(raw).as_posix() for p in raw.rglob("*") if p.is_file()}
    return receipt


def main():
    raw, harness, destination = map(Path, sys.argv[1:4])
    receipt = bind_archive(raw)
    assert receipt["status"] == "complete" and receipt["requiredSourcesVerified"] is True
    arm = sys.argv[4]
    build = json.loads(Path(sys.argv[5]).read_text())
    summary = json.loads((raw / "run-summary.json").read_text())
    assert (receipt["label"], receipt["arm"], receipt["target"]) == (summary["runLabel"], arm, summary["primaryTarget"])
    pins = json.loads((raw / "pins.json").read_text())
    master = build["arms"][arm]
    assert summary["runtime"] == pins["runtime"] == master["runtime"] == RUNTIMES[arm]
    assert summary["arm"] == arm and summary["status"] == "complete"
    for key in ["archiveSha256", "instrumentedManifestSha256", "clientSourceSha256"]:
        assert pins[key] == master[key], key
    assert sha(harness / "main.tar") == pins["archiveSha256"]
    assert pins["binaries"]["host"] == master["binaries"]["host"]
    assert pins["binaries"]["clients"] == build["sharedClientSha256"]
    assert master["sharedPolicyUnchanged"]
    assert build["arms"]["base"]["sharedPolicySha256"] == build["arms"]["candidate"]["sharedPolicySha256"]

    # Windows libm pow(x, 2) differed by one ULP from the original Linux
    # reduction. A binary64 multiplication preserves that exact square and
    # reproduces the Linux online slope without weakening any replay/gate.
    reducer = (harness / "reduce.py").read_text()
    square = "sum((xx-xm)**2 for xx in x)"
    assert reducer.count(square) == 1
    portable = reducer.replace(square, "sum((xx-xm)*(xx-xm) for xx in x)")
    module = types.ModuleType("reduce")
    module.__file__ = str(harness / "reduce.py")
    exec(compile(portable, module.__file__, "exec"), module.__dict__)
    sys.modules["reduce"] = module

    selector = HERE.parent / "2026-10-29-battledome-experience/select_results.py"
    text = selector.read_text()
    old = "assert pins['runtime']==summary['runtime']=='165a5bb764aba6cbf39f42503e090f9b5e3c4d22'"
    assert text.count(old) == 1
    text = text.replace(old, f"assert pins['runtime']==summary['runtime']=={RUNTIMES[arm]!r}")
    old = "changed={k:v for k,v in pins['sourceOverlays'].items() if original.get(k)!=v}"
    assert text.count(old) == 1
    text = text.replace(old, "changed={k.replace(chr(92),'/'):v for k,v in pins['sourceOverlays'].items() if original.get(k.replace(chr(92),'/'))!=v}")
    argv = sys.argv[:]
    with tempfile.TemporaryDirectory() as temporary:
        full = Path(temporary) / "selected.json"
        sys.argv = [str(selector), str(raw), str(harness), str(full)]
        try:
            exec(compile(text, str(selector), "exec"), {"__file__": str(selector), "__name__": "__main__"})
        finally:
            sys.argv = argv
        selected = json.loads(full.read_text())

    windows = selected["windows"]
    assert all(w["clientValid"] and w["inventoryComplete"] and w["accounting"]["reconciles"] for w in windows)
    assert all(w["boost"]["aliveTickShare"] >= .99 for w in windows)
    assert not selected["humanPlayingPeriods"]
    primary = next(w for w in windows if w["label"] == f"up-{summary['primaryTarget']}")

    # The four categories are means from the same selected ticks. Minimap is
    # nested inside broadcast and must be subtracted before summing categories.
    tail = primary["slowestCpu1Percent"]
    phases = tail["phasesMs"]
    exclusive = {
        "spawnSearch": phases["admission"],
        "socketsBroadcast": phases["socket"] + phases["broadcast"] - tail["nestedMs"]["minimap"],
        "minimap": tail["nestedMs"]["minimap"],
        "other": tail["worldMeanMs"] - phases["admission"] - phases["socket"] - phases["broadcast"],
    }
    assert all(v >= 0 for v in exclusive.values()), exclusive
    primary["slowestCpu1Percent"]["exclusiveMs"] = exclusive
    primary["slowestCpu1Percent"]["exclusivePercent"] = {k: 100 * v / tail["worldMeanMs"] for k, v in exclusive.items()}

    keep = ["bots", "ticks", "tickRange", "wallSeconds", "worldCpuMsPerTick", "deferredTicks", "clientValid", "client", "socket", "alive", "occupied", "census", "boost", "spawns", "allTicks", "slowestCpu1Percent", "accounting", "hostCoreContextMsPerTick"]
    output = {
        "label": summary["runLabel"], "arm": arm,
        "sourceArchiveSha256": receipt["archiveSha256"],
        "primary": {k: primary[k] for k in keep},
        "calibrations": [{k: w[k] for k in ["bots", "ticks", "worldCpuMsPerTick", "deferredTicks", "clientValid", "boost"]} for w in windows if w is not primary],
        "totalDeferredTicks": selected["totalDeferredTicks"],
        "deferredOutsideAllMarkedWindows": selected["deferredOutsideCountedWindows"],
        "wholeRunSpawnLifecycle": selected["wholeRunSpawnLifecycle"],
        "verification": {
            "onlineRangesReproduced": True,
            "allCanonicalTaskClocksReconcile": True,
            "maximumClockTaskDifferenceMs": max(abs(w["accounting"]["clockMinusTaskMs"]) for w in windows),
            "allBoundaryInventoriesPresent": True,
            "allGeneratorAndQueueGatesPass": True,
            "minimumBoostShareIncludingCalibration": min(w["boost"]["aliveTickShare"] for w in windows),
            "humanSeats": 0,
        },
        "analysisSha256": {"priorSelector": sha(selector), "selector": sha(Path(__file__)), "reducer": sha(harness / "reduce.py"), "portableQueueReducer": hashlib.sha256(portable.encode()).hexdigest()},
    }
    destination.write_text(json.dumps(output, indent=2) + "\n", newline="\n")


if __name__ == "__main__":
    main()
