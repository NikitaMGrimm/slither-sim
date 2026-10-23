"""Prepare an offline observation phase diagnostic; Python standard library only.

This program NEVER builds or runs Rust. Example (use a NEW absolute output path):
  python -B tools/perf_observation_phases.py --commit FULL_SHA --output ABS_PATH

The output contains baseline/ and instrumented/ source snapshots, a manifest,
the retained recipe/resource, a unified instrumentation patch, and COMMANDS.txt.
Only Cargo files, src/*.rs, README and the frozen observation harness are exported.
The sole build-scaffolding edit in BOTH snapshots removes the explicit portable_bot
example entry whose source is intentionally not exported. Dependencies, features,
release profile, runtime source and baseline fixture bytes otherwise stay pinned.

The generated diagnostic harness retains the original fixture and trace routines.
Use --trace separately and compare COMPLETE JSONL bytes with compare-traces below.
No clocks are enabled for traces. Ordinary finite fixtures are not a proof for all
numeric boundaries, mutation paths, invalid masks or nonfinite payloads. Profiles
include instrumentation overhead and must never be promoted to comparative timing.
Requires Python 3.11+ and local Git objects; no fetch, dependency installation,
environment changes, recursive deletion or cleanup of failed attempts is performed.
"""

import argparse
import difflib
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shlex
import stat
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
RESOURCE = Path(__file__).with_suffix(".rs")
OBSERVATION = "src/observation/mod.rs"
HARNESS = "examples/perf_observation.rs"
CASES = ("observe-dense", "observe-compact", "observe-wide", "observe-strip",
         "observe-one", "observe-empty")
# Git blob guards, not claims about scientific provenance. Changing these requires
# reviewing the new source and every insertion, not merely refreshing a checksum.
SHAPE = {
    OBSERVATION: "938cd16e812e70e91cf0444c666ba064f5f7904b",
    HARNESS: "f7576e8336e74e23a9a6b47f1edd5819e4a4450a",
    "Cargo.toml": "8a7d5a2e4978d62a46fcc81f4456bd0c5e085cb5",
    "src/model.rs": "7c4fd0c08aaac94e9e546a5b4deb48d212fc8a92",
    "src/physics.rs": "968c584850d9e3f337e346328af454085ddce310",
}
MAX_FILES = 512
MAX_FILE_BYTES = 2 * 1024 * 1024
MAX_SOURCE_BYTES = 16 * 1024 * 1024
MAX_TRACE_BYTES = 256 * 1024 * 1024
PORTABLE_TARGET = ('[[example]]\nname = "portable_bot"\n'
                   'required-features = ["networking"]\n\n')
GENERATED = {"src/observation/phase_diagnostic.rs", "examples/perf_observation_phases.rs"}


def safe_component(name):
    # Apply Windows restrictions on every platform so snapshots stay portable.
    reserved = {"CON", "PRN", "AUX", "NUL"} | {
        f"{prefix}{i}" for prefix in ("COM", "LPT") for i in range(1, 10)}
    return (re.fullmatch(r"[A-Za-z0-9_.-]+", name) is not None
            and name not in (".", "..") and not name.endswith(".")
            and name.split(".")[0].upper() not in reserved)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def git(root, *args):
    return subprocess.run(["git", "-C", str(root), *args], check=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          timeout=30).stdout


def replace(text, old, new, count=1):
    if text.count(old) != count:
        raise ValueError(f"unknown source shape: expected {count} occurrences of {old!r}")
    return text.replace(old, new)


def instrument(source):
    """Insert measurements into the guarded ACTUAL code; never substitute a model."""
    s = source.decode("utf-8")
    s = replace(s, "use crate::{", "use phase_diagnostic::{Clock, Count, Phase};\nuse crate::{")
    s = replace(s, "        let index = if batch", "        let _clock = Clock::new(Phase::Index);\n"
                "        let mut index_anchors = 0;\n        let index = if batch")
    s = replace(s, "                        b.include(p);", "                        index_anchors += 1;\n"
                "                        b.include(p);")
    s = replace(s, "        Self { world, index }", "        if index.is_some() {\n"
                "            phase_diagnostic::add(&[(Count::IndexSnakes, world.snakes.len()),\n"
                "                (Count::IndexAnchors, index_anchors), (Count::IndexFood, world.food.len())]);\n"
                "        }\n        Self { world, index }")
    s = replace(s, "        let food = if let Some(search) = &self.index {",
                "        let mut food_candidates = 0;\n"
                "        let food: Vec<Food> = if let Some(search) = &self.index {\n"
                "            let filter_clock = Clock::new(Phase::FoodFilter);")
    s = replace(s, ".filter(|&i| visible(w.food[i].point()))",
                ".filter(|&i| { food_candidates += 1; visible(w.food[i].point()) })", count=2)
    s = replace(s, "            candidates.sort_unstable();\n"
                "            candidates.into_iter().map(|i| w.food[i].clone()).collect()",
                "            drop(filter_clock);\n"
                "            let order_clock = Clock::new(Phase::FoodOrder);\n"
                "            candidates.sort_unstable();\n            drop(order_clock);\n"
                "            let _copy_clock = Clock::new(Phase::FoodCopy);\n"
                "            candidates.into_iter().map(|i| w.food[i].clone()).collect()")
    s = replace(s, "        } else {\n            w.food", "        } else {\n"
                "            let _clock = Clock::new(Phase::FoodLinearFilterCopy);\n            w.food")
    s = replace(s, ".filter(|f| visible(f.point()))",
                ".filter(|f| { food_candidates += 1; visible(f.point()) })")
    s = replace(s, "        Ok(Observation {", "        phase_diagnostic::add(&[\n"
                "            (Count::ObservedSlots, 1),\n"
                "            (if self.index.is_some() { Count::IndexedObservations } else { Count::LinearObservations }, 1),\n"
                "            (Count::FoodCandidates, food_candidates),\n"
                "            (Count::FoodAccepted, food.len()), (Count::FoodCopies, food.len()),\n"
                "        ]);\n        Ok(Observation {")
    s = replace(s, "            me: me.clone(),", "            me: {\n"
                "                let clock = Clock::new(Phase::OwnCopy);\n"
                "                let copy = me.clone();\n                drop(clock);\n"
                "                phase_diagnostic::add(&[(Count::OwnCopiedAnchors, copy.body.len())]);\n"
                "                copy\n            },")
    s = replace(s, "            prey: w\n", "            prey: {\n"
                "                let clock = Clock::new(Phase::PreyFilterCopy);\n"
                "                let mut candidates = 0;\n                let copies: Vec<Prey> = w\n")
    s = replace(s, ".filter(|p| visible(p.position))",
                ".filter(|p| { candidates += 1; visible(p.position) })")
    s = replace(s, "            opponents: w\n", "            opponents: {\n"
                "                let clock = Clock::new(Phase::OpponentFilterCopy);\n"
                "                let mut candidates = 0;\n                let mut head_tests = 0;\n"
                "                let mut anchor_tests = 0;\n                let mut copied_anchors = 0;\n"
                "                let copies: Vec<Snake> = w\n")
    s = replace(s, "                .filter(|(i, s)| {\n", "                .filter(|(i, s)| {\n"
                "                    candidates += 1;\n")
    s = replace(s, "&& (visible(s.head) || s.body.iter().any(|&p| visible(p)))",
                "&& ({ head_tests += 1; visible(s.head) }\n"
                "                            || s.body.iter().any(|&p| { anchor_tests += 1; visible(p) }))")
    s = replace(s, ".map(|(_, s)| s.clone())", ".map(|(_, s)| {\n"
                "                    let copy = s.clone();\n"
                "                    copied_anchors += copy.body.len();\n                    copy\n                })")
    s = replace(s, "                .collect(),\n            opponents:", "                .collect();\n"
                "                drop(clock);\n                phase_diagnostic::add(&[\n"
                "                    (Count::PreyCandidates, candidates),\n"
                "                    (Count::PreyAccepted, copies.len()), (Count::PreyCopies, copies.len()),\n"
                "                ]);\n                copies\n            },\n            opponents:")
    s = replace(s, "                .collect(),\n            arena:", "                .collect();\n"
                "                drop(clock);\n                phase_diagnostic::add(&[\n"
                "                    (Count::OpponentCandidates, candidates),\n"
                "                    (Count::OpponentHeadTests, head_tests),\n"
                "                    (Count::OpponentAnchorTests, anchor_tests),\n"
                "                    (Count::OpponentAccepted, copies.len()),\n"
                "                    (Count::OpponentCopiedAnchors, copied_anchors),\n"
                "                ]);\n                copies\n            },\n            arena:")
    s = replace(s, "        if selected.is_some_and", "        let _batch_clock = Clock::new(Phase::Batch);\n"
                "        if selected.is_some_and")
    s = replace(s, "            return Ok((0..self.snakes.len()).map(|_| None).collect());",
                "            phase_diagnostic::add(&[(Count::EmptyBatches, 1)]);\n"
                "            return Ok((0..self.snakes.len()).map(|_| None).collect());")
    return (s + '\n#[path = "phase_diagnostic.rs"]\npub mod phase_diagnostic;\n').encode("utf-8")


def diagnostic_harness(source):
    s = source.decode("utf-8")
    s = replace(s, "    for iteration in 0..args.iterations {", "    let phase_session = if args.trace { None } else {\n"
                "        Some(slither_sim::observation::phase_diagnostic::Session::start())\n"
                "    };\n    for iteration in 0..args.iterations {")
    s = replace(s, "    let anchors: Vec<_>", "    if let Some(session) = phase_session {\n"
                "        line(&mut out, json!({\"kind\": \"phase-profile\", \"diagnostic\": session.finish()}))?;\n"
                "    }\n    let anchors: Vec<_>")
    s = replace(s, 'if args.trace { "trace" } else { "timing" }',
                'if args.trace { "trace" } else { "diagnostic-with-clock-and-counter-overhead" }')
    return s.encode("utf-8")


def source_files(root, commit):
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("--commit must be a full lowercase 40-hex commit ID, not a ref")
    if git(root, "rev-parse", "--verify", f"{commit}^{{commit}}").decode().strip() != commit:
        raise ValueError("commit ID does not identify that exact commit")
    tree = git(root, "rev-parse", f"{commit}^{{tree}}").decode().strip()
    entries = git(root, "ls-tree", "-r", "-l", "-z", "--full-tree", commit, "--",
                  "Cargo.toml", "Cargo.lock", "README.md", "src", HARNESS).split(b"\0")[:-1]
    if not 1 <= len(entries) <= MAX_FILES:
        raise ValueError("source file count exceeds preparation bound")
    files, total = {}, 0
    for entry in entries:
        metadata, raw_name = entry.split(b"\t", 1)
        mode, kind, oid, raw_size = metadata.split()
        name = raw_name.decode("ascii")
        path = PurePosixPath(name)
        if (mode != b"100644" or kind != b"blob" or path.is_absolute()
                or any(not safe_component(p) for p in path.parts)
                or not (name in ("Cargo.toml", "Cargo.lock", "README.md", HARNESS)
                        or name.startswith("src/") and name.endswith(".rs"))):
            raise ValueError(f"unsafe or unsupported source entry: {name}")
        size = int(raw_size)
        total += size
        if size > MAX_FILE_BYTES or total > MAX_SOURCE_BYTES:
            raise ValueError("source bytes exceed preparation bound")
        if name in SHAPE and oid.decode() != SHAPE[name]:
            raise ValueError(f"unknown source shape: {name}; review recipe before extending guards")
        data = git(root, "cat-file", "blob", oid.decode())
        if len(data) != size or b"\r" in data:
            raise ValueError(f"source must be exact LF bytes: {name}")
        files[name] = data
    if not set(SHAPE).union({"Cargo.lock", "README.md", "src/lib.rs"}) <= files.keys():
        raise ValueError("required source is missing")
    folded = {name.casefold() for name in files}
    if (len(folded) != len(files) or GENERATED & files.keys()
            or any(str(parent).casefold() in folded
                   for name in files for parent in PurePosixPath(name).parents)):
        raise ValueError("colliding source/generated paths")
    return files, tree


def safe_output(root, output):
    output = Path(output)
    if (not output.is_absolute() or output.drive.startswith("\\\\")
            or ".." in output.parts or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,63}", output.name)
            or not safe_component(output.name)
            or any(":" in part for part in output.parts[1:])
            or os.path.lexists(output)):
        raise ValueError("output must be a NEW absolute local task-named directory (no traversal/ADS)")
    # Reject symlinks, Windows junctions and all other reparse points in ancestors.
    for parent in output.parents:
        info = parent.lstat()
        if (not stat.S_ISDIR(info.st_mode) or stat.S_ISLNK(info.st_mode)
                or getattr(info, "st_file_attributes", 0) & 0x400):
            raise ValueError(f"unsafe output ancestor: {parent}")
    output = output.parent.resolve(strict=True) / output.name
    protected = [Path(git(root, "rev-parse", "--path-format=absolute", "--git-common-dir").decode().strip())]
    for line in git(root, "worktree", "list", "--porcelain", "-z").split(b"\0"):
        if line.startswith(b"worktree "):
            protected.append(Path(os.fsdecode(line[len(b"worktree "):])).resolve())
    if any(output == p or output.is_relative_to(p) for p in protected):
        raise ValueError("output must be outside every checkout and Git storage")
    return output


def commands(output):
    windows = os.name == "nt"
    quote = (lambda s: "'" + str(s).replace("'", "''") + "'") if windows else shlex.quote
    exe = "perf_observation.exe" if windows else "perf_observation"
    diagnostic = "perf_observation_phases.exe" if windows else "perf_observation_phases"

    def bounded(argv, seconds, capture=None):
        # These are printed commands only. Python supplies portable deadlines and
        # byte-preserving exclusive captures without shell-dependent redirection.
        script = f"import subprocess; subprocess.run({[str(a) for a in argv]!r}, check=True, timeout={seconds}"
        if capture is not None:
            script += f", stdout=open({str(capture)!r}, 'xb'), stderr=open({str(capture) + '.stderr'!r}, 'xb')"
        return "python -B -c " + quote(script + ")")

    lines = ["# PREPARED ONLY. Preparation executes no Rust, tests or workloads.",
             "# Commands below are explicit parent actions; offline Cargo requires cached dependencies.",
             "# Builds: jobs=2, deadline=600s; workloads: deadline=120s, no stepping/network.",
             "# Captures use exclusive creation. Retain failed attempts; do not overwrite them.",
             "# Run traces before profiles; profiles contain overhead and are NOT comparative timings."]
    for variant, example in (("baseline", "perf_observation"), ("instrumented", "perf_observation_phases")):
        lines.append(bounded(["cargo", "build", "--offline", "--locked", "--release", "--no-default-features",
                              "-j", "2", "--manifest-path", output / variant / "Cargo.toml",
                              "--target-dir", output / (variant + "-target"), "--example", example], 600))
    baseline = output / "baseline-target/release/examples" / exe
    instrumented = output / "instrumented-target/release/examples" / diagnostic
    for case in CASES:
        traces = []
        for label, executable in (("baseline", baseline), ("instrumented", instrumented)):
            trace = output / f"{case}-{label}.jsonl"
            traces.append(trace)
            lines.append(bounded([executable, case, "2", "--trace"], 120, trace))
        lines.append(f"python -B {quote(str(output / 'recipe/perf_observation_phases.py'))} compare-traces "
                     + " ".join(quote(str(t)) for t in traces))
        lines.append(bounded([instrumented, case, "100"], 120, output / (case + "-phases.jsonl")))
    lines += ["# Retain binary SHA-256/toolchain/platform and trace exit codes before interpreting output.",
              "# Deadlines are direct-child timeouts, not OS CPU/memory/process-tree caps.",
              "# Never feed diagnostic profiles to perf_compare.py or infer speedup from phase sums."]
    return "\n".join(lines) + "\n"


def prepare(root, commit, output):
    output = safe_output(root, output)
    original, tree = source_files(root, commit)
    resource = RESOURCE.read_bytes()
    if not resource or len(resource) > MAX_FILE_BYTES or b"\r" in resource:
        raise ValueError("invalid instrumentation resource")
    baseline = dict(original)
    baseline["Cargo.toml"] = replace(original["Cargo.toml"].decode(), PORTABLE_TARGET, "").encode()
    transformed = dict(baseline)
    transformed[OBSERVATION] = instrument(original[OBSERVATION])
    transformed["src/observation/phase_diagnostic.rs"] = resource
    transformed["examples/perf_observation_phases.rs"] = diagnostic_harness(original[HARNESS])
    patch = "".join("".join(difflib.unified_diff(
        baseline.get(name, b"").decode().splitlines(keepends=True),
        transformed.get(name, b"").decode().splitlines(keepends=True),
        fromfile=f"a/{name}" if name in baseline else "/dev/null", tofile=f"b/{name}"))
        for name in sorted(transformed) if baseline.get(name) != transformed[name])
    recipe = Path(__file__).read_bytes()
    manifest = {
        "format": "slither-observation-phase-preparation-v1", "status": "prepared-only-unexecuted",
        "baseCommit": commit, "baseTree": tree,
        "originalSha256": {n: sha256(d) for n, d in sorted(original.items())},
        "baselineSha256": {n: sha256(d) for n, d in sorted(baseline.items())},
        "instrumentedSha256": {n: sha256(d) for n, d in sorted(transformed.items())},
        "recipeSha256": sha256(recipe), "resourceSha256": sha256(resource),
        "patchSha256": sha256(patch.encode()), "sourceShapeGitBlobs": SHAPE,
        "scaffoldingEdit": "remove only unexported portable_bot example registration in both Cargo.toml files",
        "fixture": "original examples/perf_observation.rs retained byte-for-byte in both snapshots; generated diagnostic variant only adds session/report hooks; trace bytes require exact equality",
        "limits": "diagnostic clock/counter overhead; no comparative speed claims; finite immutable fixtures do not cover every numerical/mutation boundary; no runtime validation performed by preparation",
    }
    output.mkdir()  # Exclusive claim, after all read/transform/shape checks; never overwrite.
    marker = output / "PREPARATION_INCOMPLETE"
    marker.write_text("Retain this failed attempt; do not build until manifest.json exists.\n", encoding="utf-8")
    for directory, files in (("baseline", baseline), ("instrumented", transformed)):
        for name, data in sorted(files.items()):
            target = output / directory / name
            target.parent.mkdir(parents=True, exist_ok=True)
            with target.open("xb") as stream:
                stream.write(data)
    retained = {"recipe/perf_observation_phases.py": recipe,
                "recipe/perf_observation_phases.rs": resource,
                "instrumentation.patch": patch.encode(), "COMMANDS.txt": commands(output).encode(),
                "manifest.json": (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()}
    for name, data in retained.items():
        target = output / name
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("xb") as stream:
            stream.write(data)
    marker.unlink()
    return output


def compare_traces(baseline, instrumented):
    """Exact bytes: no deletion of semantic/report fields or tolerance for ordering."""
    paths = [Path(baseline), Path(instrumented)]
    if any(not p.is_file() or not 0 < p.stat().st_size <= MAX_TRACE_BYTES for p in paths):
        raise ValueError("trace files must be nonempty regular files of at most 256 MiB each")
    # Validate trace envelopes too: two identical timing reports are not trace parity.
    for path in paths:
        initial = observations = reports = total = 0
        case = None
        with path.open("rb") as stream:
            while line := stream.readline(MAX_TRACE_BYTES + 1):
                total += len(line)
                if total > MAX_TRACE_BYTES:
                    raise ValueError("trace exceeds byte bound")
                record = json.loads(line)
                if not isinstance(record, dict):
                    raise ValueError("trace records must be objects")
                if record.get("kind") in ("initial", "observe"):
                    snapshot = record.get("snapshot")
                    if (not isinstance(snapshot, dict) or not {
                            "world", "pickupEvents", "foodSpawnEvents", "observations"} <= snapshot.keys()):
                        raise ValueError("trace is missing semantic snapshot fields")
                if record.get("kind") == "initial" and initial == observations == reports == 0:
                    case = record.get("case")
                    if case not in CASES or record.get("seed") != 7:
                        raise ValueError("unknown trace fixture")
                    initial += 1
                elif record.get("kind") == "observe" and initial == 1 and reports == 0:
                    if (record.get("iteration") != observations or observations >= 2
                            or not isinstance(record.get("result"), list)):
                        raise ValueError("expected one/two ordered observation trace records")
                    observations += 1
                elif record.get("kind") == "report" and initial == 1 and observations > 0 and reports == 0:
                    if (record.get("mode") != "trace" or record.get("iterations") != observations
                            or record.get("case") != case or record.get("timing") is not None
                            or record.get("outputDrop") is not None):
                        raise ValueError("expected a bounded one/two-iteration harness trace")
                    reports += 1
                else:
                    raise ValueError("unexpected trace record")
        if initial != 1 or reports != 1:
            raise ValueError("incomplete trace")
    with paths[0].open("rb") as left, paths[1].open("rb") as right:
        digest = hashlib.sha256()
        while True:
            a, b = left.read(1024 * 1024), right.read(1024 * 1024)
            if a != b:
                raise ValueError("trace bytes differ; retain both complete files for investigation")
            if not a:
                break
            digest.update(a)
    return digest.hexdigest()


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if argv[:1] == ["compare-traces"]:
        parser = argparse.ArgumentParser(description="Compare complete retained trace bytes, never execute a workload")
        parser.add_argument("baseline")
        parser.add_argument("instrumented")
        args = parser.parse_args(argv[1:])
        print("Exact complete trace parity SHA-256:", compare_traces(args.baseline, args.instrumented))
    else:
        parser = argparse.ArgumentParser(description=__doc__)
        parser.add_argument("--commit", required=True)
        parser.add_argument("--output", required=True, type=Path)
        args = parser.parse_args(argv)
        output = prepare(ROOT, args.commit, args.output)
        print(f"Prepared only, no Rust execution: {output}")
        print((output / "COMMANDS.txt").read_text(encoding="utf-8"), end="")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"Preparation/comparison failed: {error}", file=sys.stderr)
        sys.exit(1)
