"""Offline Linux x86-64 admission study; one CPU, one Cargo/Rayon/test worker.

prepare OUTPUT --repository REPOSITORY --base REF --candidate REF: archive sources.
build OUTPUT: build/copy plain matrix, diagnostic matrix and reused #242 harness.
matrix OUTPUT: literal catalog trace parity, then five alternating process pairs.
scaling OUTPUT --fixtures DIRECTORY: reuse #242 warmed 2000-common/5000-capped.
Run focused/full Rust checks and mutation control separately; see REPORT.md.
cleanup OUTPUT: remove only the task-owned Rust targets after rehashing copies.
"""
import argparse
import ctypes
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tarfile

if not __debug__:
    raise SystemExit("Run without Python optimization: study checks must execute")

FIXTURE_REF = "cae1d7a5a9c9e44f2f9565164a410020fbb7fd0b"
FIXTURE_PATH = "research/performance/2026-10-27-population-scaling"
ARMS = ["base", "candidate"]


def sha(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def swap(path, old, new):
    text = path.read_text()
    assert text.count(old) == 1, (path, old)
    path.write_text(text.replace(old, new), newline="\n")


def run(command, log, cwd=None):
    with log.open("x") as stream:
        subprocess.run(list(map(str, command)), cwd=cwd, env=ENV,
                       stdout=stream, stderr=subprocess.STDOUT, check=True)


def literal(left, right):
    with left.open("rb") as a, right.open("rb") as b:
        while True:
            x, y = a.read(1 << 20), b.read(1 << 20)
            assert x == y, (left, right)
            if not x:
                return


def process_count(command, log):
    """Attach instructions:u to a stopped child before exec, never the driver.

    Includes the small resumed Python exec trampoline and native startup/exit;
    excludes the Python parent. Equivalent arms use the same launcher. No perf
    executable, privileged changes, multiplexing, or socket workload required.
    """
    with log.open("x") as stream:
        pid = os.fork()
        if pid == 0:
            os.dup2(stream.fileno(), 1)
            os.dup2(stream.fileno(), 2)
            os.kill(os.getpid(), 19)  # SIGSTOP
            os.execve(str(command[0]), list(map(str, command)), ENV)
        try:
            assert os.waitpid(pid, os.WUNTRACED)[1] == 4991
            attr = (ctypes.c_uint64 * 16)()
            attr[0] = 128 << 32  # PERF_TYPE_HARDWARE, sizeof(perf_event_attr)
            attr[1] = 1  # PERF_COUNT_HW_INSTRUCTIONS
            attr[4] = 3  # total_time_enabled/running
            attr[5] = (1 << 1) | (1 << 5) | (1 << 6)  # inherit, user only
            libc = ctypes.CDLL(None, use_errno=True)
            fd = libc.syscall(298, ctypes.byref(attr), pid, -1, -1, 0)
            assert fd >= 0, os.strerror(ctypes.get_errno())
            os.kill(pid, 18)  # SIGCONT
            assert os.waitpid(pid, 0)[1] == 0, log
            values = struct.unpack("QQQ", os.read(fd, 24))
            os.close(fd)
            assert values[1] > 0 and values[2] / values[1] >= .99, values
            return list(values)
        finally:
            try:
                os.kill(pid, 9)
                os.waitpid(pid, 0)
            except ProcessLookupError:
                pass


def prepare():
    ROOT.mkdir()
    fixture = ROOT / "fixture"
    fixture.mkdir()
    for name in ["reproduce.py", "physics.rs", "warm_bounds.rs", "counter.rs", "checkpoint_shape.rs"]:
        (fixture / name).write_bytes(subprocess.check_output(
            ["git", "-C", str(A.repository), "show", f"{FIXTURE_REF}:{FIXTURE_PATH}/{name}"]))
    pins = {"base": A.base, "candidate": A.candidate, "fixtureRef": FIXTURE_REF}
    for arm, ref in zip(ARMS, [A.base, A.candidate]):
        archive = ROOT / f"{arm}.tar"
        subprocess.run(["git", "-C", str(A.repository), "archive", ref, "-o", str(archive)], check=True)
        source = ROOT / arm
        source.mkdir()
        with tarfile.open(archive) as stream:
            stream.extractall(source, filter="data")
        pins[f"{arm}ArchiveSha256"] = sha(archive)
    (ROOT / "source-pins.json").write_text(json.dumps(pins, indent=2) + "\n")


def overlay(source):
    fixture = ROOT / "fixture"
    text = (source / "src/client/simulation.rs").read_text()
    pose = text.split("    fn spawn_pose_waited(", 1)[1].split("\n    /// Drive non-controlled", 1)[0]
    pose = "fn host_pose(world: &Simulator, random: &mut Rng, slot: usize, waited:" + pose.split("waited:", 1)[1]
    pose = pose.replace("self.world", "world").replace("self.spawn_random", "random")
    pose = pose.replace("self\n                .world", "world").replace("self\n                    .world", "world")
    assert "self" not in pose
    fast = pose.replace("fn host_pose(", "fn host_pose_fast(").replace(
        "waited: f64)", "waited: f64, bounds: &[Option<crate::physics::Bounds>])")
    fast = fast.replace("world\n                    .body_is_clear_excluding(&body, spawn::CLEARANCE, Some(slot))",
                        "warm_clear(world, bounds, &body, spawn::CLEARANCE, Some(slot))")
    fast = fast.replace("world\n                .spawn_margin_bounded_excluding(&body, best, Some(slot))",
                        "warm_margin(world, bounds, &body, best, Some(slot))")
    assert "warm_clear(" in fast and "warm_margin(" in fast
    (source / "src/client/simulation/study_scaling.rs").write_text(
        (fixture / "physics.rs").read_text() + "\n" + (fixture / "warm_bounds.rs").read_text() + "\n" + pose + "\n" + fast)
    swap(source / "src/client/simulation/study_scaling.rs", "for _ in 0..1000 { rows.push",
         'for _ in 0..std::env::var("SPAWN_STUDY_TICKS").map_or(1000, |v|v.parse().unwrap()) { rows.push')
    shutil.copyfile(fixture / "counter.rs", source / "src/study_counter.rs")
    with (source / "src/lib.rs").open("a") as stream:
        stream.write("\npub mod study_counter;\n")
    with (source / "src/client/simulation.rs").open("a") as stream:
        stream.write("\npub mod study_scaling;\n")
    (source / "examples/study_scaling.rs").write_text(
        "fn main(){slither_sim::client::simulation::study_scaling::run();}\n")
    # Reuse #242's checked memory-bounded importer for large checkpoint inputs.
    spec = importlib.util.spec_from_file_location("population", fixture / "reproduce.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.checkpoint_preparation(source)
    # Diagnostic copy only; the ordinary perf_baseline binary stays untouched.
    path = source / "examples/study_matrix.rs"
    shutil.copyfile(source / "examples/perf_baseline.rs", path)
    swap(path, "fn main() {", "fn main() {\n    slither_sim::study_counter::reset(true);")
    swap(path, "        let result = f();", "        let pmu = slither_sim::study_counter::start();\n        let result = f();\n        slither_sim::study_counter::add(0, pmu);")
    swap(path, "            let admission_start =", "            let admission_pmu = (measured && !args.trace).then(slither_sim::study_counter::start).flatten();\n            let admission_start =")
    swap(path, "            let admission = admission_start", "            slither_sim::study_counter::add(1, admission_pmu);\n            let admission = admission_start")
    # Print counters even when baseline main returns early after writing report.
    swap(path, "fn main() {", "fn main() {\n    struct Report;\n    impl Drop for Report {fn drop(&mut self) {eprintln!(\"STUDY_PMU {}\", slither_sim::study_counter::result());}}\n    let _report = Report;")


def build():
    pins = {}
    for arm in ARMS:
        source = ROOT / arm
        if not (source / "examples/study_matrix.rs").exists():
            overlay(source)
        target = ROOT / f"target-{arm}"
        # Separate targets are required: Cargo's mtime fingerprints can reuse
        # the wrong root package when switching archived paths in one target.
        # A dependency-only seed is safe only after removing the root package.
        if not target.exists() and (ROOT / "target").exists():
            shutil.copytree(ROOT / "target", target)
        clean_attempt = len(list(ROOT.glob(f"clean-{arm}*.log")))
        run(["cargo", "clean", "--release", "-p", "slither-sim", "--target-dir", target],
            ROOT / f"clean-{arm}-{clean_attempt}.log", source)
        attempt = len(list(ROOT.glob(f"build-{arm}*.log")))
        run(["cargo", "build", "--offline", "--locked", "--release", "-j", "1",
             "--target-dir", target, "--example", "perf_baseline", "--example", "study_matrix",
             "--example", "study_scaling"], ROOT / f"build-{arm}-{attempt}.log", source)
        out = ROOT / "bin" / arm
        out.mkdir(parents=True, exist_ok=True)
        for name in ["perf_baseline", "study_matrix", "study_scaling"]:
            shutil.copyfile(target / "release/examples" / name, out / name)
            (out / name).chmod(0o755)
        pins[arm] = {p.name: sha(p) for p in out.iterdir()}
    assert pins["base"]["perf_baseline"] != pins["candidate"]["perf_baseline"], "unexpected A/A binaries"
    (ROOT / "binary-pins.json").write_text(json.dumps(pins, indent=2) + "\n")


def rehash():
    pins = json.loads((ROOT / "binary-pins.json").read_text())
    for arm, files in pins.items():
        for name, digest in files.items():
            assert sha(ROOT / "bin" / arm / name) == digest


def binary(arm, name="perf_baseline"):
    return ROOT / "bin" / arm / name


def cleanup():
    rehash()
    for name in ["target-base", "target-candidate"]:
        path = (ROOT / name).resolve()
        assert path.parent == ROOT, "target outside task-owned output"
        if path.exists():
            shutil.rmtree(path)


def matrix():
    rehash()
    out = ROOT / "matrix"
    out.mkdir()
    catalogs = [json.loads(subprocess.check_output([binary(arm), "--list"], env=ENV)) for arm in ARMS]
    assert catalogs[0] == catalogs[1]
    (out / "catalog.json").write_text(json.dumps(catalogs[0], indent=2) + "\n")
    selected = {}
    for case in catalogs[0]["workloads"]:
        if case["name"].startswith("host-"):
            continue  # socket rows have no deterministic trace or instruction bracket
        name = case["name"]
        traces = []
        for arm in ARMS:
            path = out / f"{name}-{arm}.trace"
            run([binary(arm), name, case["traceIterations"], "--trace"], path)
            assert b'"kind":"report"' in path.read_bytes().splitlines()[-1], path
            traces.append(path)
        literal(*traces)
        row = {"traceSha256": sha(traces[0]), "traceBytes": traces[0].stat().st_size, "pairs": []}
        for path in traces:
            with path.open("rb") as src, gzip.open(str(path) + ".gz", "wb", compresslevel=1) as dst:
                shutil.copyfileobj(src, dst)
            path.unlink()
        for pair in range(-1, 5):
            counts = {"pair": pair, "warmup": pair < 0}
            for arm in (ARMS if pair % 2 == 0 or pair < 0 else ARMS[::-1]):
                counts[arm] = process_count([binary(arm), name, case["iterations"]], out / f"{name}-{pair}-{arm}.log")
            row["pairs"].append(counts)
        # Disjoint operation/admission counts, outside the process comparison.
        for arm in ARMS:
            log = out / f"{name}-{arm}-diagnostic.log"
            run([binary(arm, "study_matrix"), name, case["iterations"]], log)
            row[arm + "Diagnostic"] = json.loads(log.read_text().split("STUDY_PMU ")[-1])
        selected[name] = row
        (out / "selected.json").write_text(json.dumps(selected, indent=2) + "\n")
        print("MATRIX", name, flush=True)
    rehash()


def scaling():
    rehash()
    out = ROOT / "scaling"
    out.mkdir()
    selected = {}
    for case in ["2000-common", "5000-capped"]:
        fixture = A.fixtures / case
        ENV["SPAWN_STUDY_TICKS"] = "1000" if case == "2000-common" else "20"
        row = {"inputs": {name: sha(fixture / name) for name in ["world.json", "driver.json"]}, "pairs": []}
        published = Path(__file__).with_name("results.json")
        if published.exists():
            assert row["inputs"] == json.loads(published.read_text())["scaling"][case]["inputs"], "frozen fixture drift"
        for pair in range(-1, 5):
            reports = {}
            process = {}
            for arm in (ARMS if pair % 2 == 0 or pair < 0 else ARMS[::-1]):
                dest = out / f"{case}-{pair}-{arm}"
                log = dest.with_suffix(".log")
                process[arm] = process_count([binary(arm, "study_scaling"), "count", fixture, dest], log)
                reports[arm] = json.loads(log.read_text().splitlines()[-1])
            left, right = [out / f"{case}-{pair}-{arm}" for arm in ARMS]
            for name in ["world.json", "driver.json"]:
                literal(left / name, right / name)
            rows = [json.loads((p / "rows.json").read_text()) for p in [left, right]]
            # Every tick's lifecycle/population must agree, not just endpoints.
            assert [{k: v for k, v in r.items() if k not in {"spawn", "step"}} for r in rows[0]] == [
                {k: v for k, v in r.items() if k not in {"spawn", "step"}} for r in rows[1]]
            item = {"pair": pair, "warmup": pair < 0, "worldSha256": sha(left / "world.json"),
                    "driverSha256": sha(left / "driver.json")}
            if row["pairs"]:
                assert all(item[key] == row["pairs"][0][key] for key in ["worldSha256", "driverSha256"])
            for arm, ticks in zip(ARMS, rows):
                item[arm] = {phase: sum(r[phase][0] for r in ticks) / len(ticks) for phase in ["spawn", "step"]}
                item[arm]["attempts"] = sum(r["attempts"] for r in ticks)
                item[arm]["refusals"] = sum(r["refusals"] for r in ticks)
                item[arm]["births"] = sum(r["births"] for r in ticks)
                item[arm]["population"] = reports[arm]["end"]
                item[arm]["process"] = process[arm]
            row["pairs"].append(item)
            # Preserve hashes/counters and one complete endpoint per arm/case;
            # repetitions are byte-identical and disk is bounded.
            if pair != -1:
                for p in [left, right]:
                    for name in ["world.json", "driver.json"]:
                        assert sha(p / name) == item[name.replace(".json", "Sha256")]
                        (p / name).unlink()
            (out / "selected.partial.json").write_text(json.dumps(dict(selected, **{case: row}), indent=2) + "\n")
            print("SCALING", case, pair, item["base"]["spawn"], item["candidate"]["spawn"], flush=True)
        selected[case] = row
    (out / "selected.json").write_text(json.dumps(selected, indent=2) + "\n")
    rehash()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["prepare", "build", "matrix", "scaling", "cleanup"])
    parser.add_argument("output", type=Path)
    parser.add_argument("--repository", type=Path)
    parser.add_argument("--base")
    parser.add_argument("--candidate")
    parser.add_argument("--fixtures", type=Path)
    parser.add_argument("--cpu", type=int, default=0)
    A = parser.parse_args()
    ROOT = A.output.resolve()
    os.sched_setaffinity(0, {A.cpu})
    ENV = dict(os.environ, CARGO_BUILD_JOBS="1", RAYON_NUM_THREADS="1", RUST_TEST_THREADS="1",
               CARGO_PROFILE_RELEASE_DEBUG="1", RUSTFLAGS="", CARGO_ENCODED_RUSTFLAGS="")
    # Shell launchers can discard Cargo's hyphenated CARGO_BIN_EXE variables.
    for key, tool in [("RUSTC", "rustc"), ("RUSTDOC", "rustdoc")]:
        ENV[key] = subprocess.check_output(["rustup", "which", tool], text=True).strip()
    for key in ["SCALING_FAST_ADMISSION", "SCALING_ORIGINAL_WARM", "SLITHER_STUDY_ORIGINAL_IMPORT"]:
        ENV.pop(key, None)
    globals()[A.mode]()
