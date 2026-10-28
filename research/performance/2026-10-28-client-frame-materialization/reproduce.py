"""Offline, one-CPU instruction/parity study; reuse #242's bot-split harness.

Supply git archives base.tar, candidate.tar and donor.tar (the population-scaling
study at ccc87eddd9439d375eedab62e66e642313f0bb5a). Raw output stays outside Git.
Run build once per arm, then trace, matrix, profile, pairs and cleanup. No timing claims.
"""
import argparse
import filecmp
import gzip
import hashlib
import importlib.util
import inspect
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile

if not __debug__:
    raise SystemExit("run without -O: evidence assertions are required")


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def swap(path, before, after):
    text = path.read_text()
    if text.count(before) != 1:
        raise RuntimeError(f"overlay anchor: {path.name}: {before!r}")
    path.write_text(text.replace(before, after), newline="\n")


def run(command, log, cwd=None, extra=None):
    with log.open("x") as stream:
        subprocess.run(command, cwd=cwd, env=dict(os.environ, **(extra or {})),
                       stdout=stream, stderr=subprocess.STDOUT, check=True)


def build(args):
    driver_sha = sha(Path(__file__))
    (args.output / (args.arm + "-driver.py")).write_bytes(Path(__file__).read_bytes())
    arm = args.arm
    source = args.output / arm / "source"
    source.mkdir(parents=True, exist_ok=False)
    archive = args.archives / (arm.removeprefix("trace-").split("-")[0] + ".tar")
    with tarfile.open(archive) as stream:
        commit = stream.pax_headers.get("comment")
        stream.extractall(source, filter="data")
    donor = args.output / arm / "donor"
    with tarfile.open(args.archives / "donor.tar") as stream:
        if stream.pax_headers.get("comment") != "ccc87eddd9439d375eedab62e66e642313f0bb5a":
            raise RuntimeError("unexpected donor revision")
        stream.extractall(donor, filter="data")
    donor /= "research/performance/2026-10-27-population-scaling"
    spec = importlib.util.spec_from_file_location("donor", donor / "reproduce.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    (source / "src/study_counter.rs").write_bytes((donor / "counter.rs").read_bytes())
    with (source / "src/lib.rs").open("a") as stream:
        stream.write("\npub mod study_counter;\n")
    with (source / "src/client/simulation.rs").open("a") as stream:
        stream.write("\npub mod study_bots;\n")
    text = (donor / "bots.rs").read_text().replace(
        "agents:500,world:crate::Config::official(500)",
        "agents:population,world:crate::Config::official(population)")
    text = text.replace("    let config=", '    let population=std::env::var("STUDY_POPULATION")'
                        '.unwrap_or("500".into()).parse().unwrap();\n    let config=')
    text = text.replace("    let mut rows=", '    use std::io::Write;\n'
                        '    let mut trace=std::env::var_os("STUDY_TRACE").map(|_| '
                        'std::io::BufWriter::new(std::io::stdout()));\n'
                        '    let mut rows=')
    text = text.replace("        if let Some(a)=a", '        if let Some(trace)=&mut trace {\n'
                        '            let actions=ACTIONS.with_borrow_mut(std::mem::take);\n'
                        '            writeln!(trace,"{}",json!({"decision":decision,"actions":actions,'
                        '"frame":frame,"world":(decision>=1000 || decision==0 || decision==999)'
                        '.then(||sim.world.checkpoint().unwrap())})).unwrap();\n'
                        '        }\n        if let Some(a)=a')
    text = text.replace('    pmu::reset(false);', '    if let Some(trace)=&mut trace {'
                        'writeln!(trace,"{}",json!({"end":true,"decisions":1200})).unwrap();}\n'
                        '    pmu::reset(false);')
    if arm.startswith("trace-"):
        # Flush before the donor's println can overtake the buffered final row.
        # Kept separate from the frozen counting binaries; never timed.
        text = text.replace('    pmu::reset(false);', '    if let Some(trace)=&mut trace {'
                            'trace.flush().unwrap();}\n    pmu::reset(false);')
    text += '\nthread_local! {static ACTIONS: std::cell::RefCell<Vec<(usize,Action)>>=const {std::cell::RefCell::new(Vec::new())};}\n'
    text += 'pub fn action(slot:usize, action:Action) {\n'
    text += '    static TRACE:std::sync::OnceLock<bool>=std::sync::OnceLock::new();\n'
    text += '    if *TRACE.get_or_init(||std::env::var_os("STUDY_TRACE").is_some()) {ACTIONS.with_borrow_mut(|v|v.push((slot,action)));}\n}\n'
    (source / "src/client/simulation/study_bots.rs").write_text(text, newline="\n")
    (source / "examples/study_bots.rs").write_text(
        "fn main(){slither_sim::client::simulation::study_bots::run();}\n")
    if arm.endswith("profile"):
        # Reuse the original bot brackets, without the unrelated old physics hooks.
        tail = inspect.getsource(module.instrument).split(
            '    path = source / "src/client/simulation/broadcast.rs"')[1]
        tail = tail.replace("self.broadcast_with(early, hand_over)?;",
                            "self.broadcast_with(early, hand_over, false)?;")
        exec('def instrument_bots(source):\n    path = source / '
             '"src/client/simulation/broadcast.rs"' + tail, module.__dict__)
        module.instrument_bots(source)
        path = source / "src/client/protocol.rs"
        for stage, name, result in [(20, "metadata", "ReceivedMetadata"), (21, "hud", "ClientHud")]:
            params = "now: f64, server: f64" if name == "metadata" else "now: f64"
            call = "now, server" if name == "metadata" else "now"
            anchor = f"    fn {name}(&self, {params}) -> {result} {{"
            swap(path, anchor, anchor + f"\n        let start=crate::study_counter::start();\n"
                 f"        let result=self.{name}_inner({call});\n"
                 f"        crate::study_counter::add({stage},start);\n        result\n    }}\n"
                 f"    fn {name}_inner(&self, {params}) -> {result} {{")
    swap(source / "src/client/simulation.rs", "                        self.send_to(slot, action)?;",
         "                        self.send_to(slot, action)?;\n                        study_bots::action(slot, action);")
    target = args.output / arm / "target"
    examples = ["study_bots"] if arm.endswith("profile") or arm.startswith("trace-") else ["study_bots", "perf_baseline"]
    run(["cargo", "build", "--offline", "--release", "--locked", "-j", "1", "--config",
         "profile.release.debug=1", *[arg for name in examples for arg in ["--example", name]]],
        args.output / (arm + "-build.log"), source, {"CARGO_TARGET_DIR": str(target)})
    pins = {"runtimeCommit": commit, "archiveSha256": sha(archive),
            "donorArchiveSha256": sha(args.archives / "donor.tar"), "driverSha256": driver_sha,
            "toolchain": subprocess.check_output(["rustc", "-V"], text=True).strip(),
            "sources": {str(p.relative_to(source)): sha(p) for p in sorted(source.rglob("*.rs"))}, "binaries": {}}
    for name in examples:
        binary = args.output / arm / name
        shutil.copy2(target / "release/examples" / name, binary)
        pins["binaries"][name] = sha(binary)
    (args.output / arm / "pins.json").write_text(json.dumps(pins, indent=2) + "\n")
    # Copies and source hashes are retained; do not accumulate four Cargo targets.
    shutil.rmtree(target)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["build", "trace", "matrix", "profile", "pairs", "cleanup"])
    parser.add_argument("archives", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--cpu", type=int, default=1)
    parser.add_argument("--arm", choices=["base", "base-profile", "candidate", "candidate-profile", "trace-base", "trace-candidate"])
    parser.add_argument("--population", type=int, choices=[500, 2000], default=500)
    args = parser.parse_args()
    args.output = args.output.resolve()
    args.archives = args.archives.resolve()
    os.sched_setaffinity(0, {args.cpu})
    os.environ.update(CARGO_BUILD_JOBS="1", RAYON_NUM_THREADS="1", RUST_TEST_THREADS="1", RUSTUP_TOOLCHAIN="1.99.0")
    if args.mode == "build":
        if not args.arm:
            parser.error("build requires --arm")
        build(args)
        return
    for arm in ["base", "base-profile", "candidate", "candidate-profile", "trace-base", "trace-candidate"]:
        pins = json.loads((args.output / arm / "pins.json").read_text())
        for name, digest in pins["binaries"].items():
            if sha(args.output / arm / name) != digest:
                raise RuntimeError("binary drift")
    if args.mode == "pairs":
        for repeat in range(-1, 5):
            for arm in (["base", "candidate"] if repeat % 2 else ["candidate", "base"]):
                folder = args.output / f"bots-{args.population}-{repeat}-{arm}"
                run([str(args.output / arm / "study_bots"), str(folder)],
                    folder.with_suffix(".log"), extra={"STUDY_POPULATION": str(args.population)})
                if sha(folder / "world.json") != sha(args.output / f"verified-trace-{args.population}-{arm}/world.json"):
                    raise RuntimeError("profile/plain endpoint differs")
    elif args.mode == "profile":
        for arm in ["base", "candidate"]:
            folder = args.output / f"profile-{args.population}-{arm}"
            run([str(args.output / (arm + "-profile") / "study_bots"), str(folder)],
                folder.with_suffix(".log"), extra={"STUDY_POPULATION": str(args.population)})
            if sha(folder / "world.json") != sha(args.output / f"verified-trace-{args.population}-{arm}/world.json"):
                raise RuntimeError("profile/plain endpoint differs")
    elif args.mode == "matrix":
        catalog = json.loads(subprocess.check_output([str(args.output / "base/perf_baseline"), "--list"]))
        (args.output / "catalog.json").write_text(json.dumps(catalog, indent=2) + "\n")
        rows = []
        for case in catalog["workloads"]:
            if not case["available"] or case["name"].startswith("host-"):
                continue
            row = {"case": case["name"], "iterations": case["traceIterations"]}
            for arm in ["base", "candidate"]:
                dest = args.output / f"matrix-{case['name']}-{arm}.jsonl.gz"
                with gzip.open(dest, "xb") as stream:
                    process = subprocess.Popen([str(args.output / arm / "perf_baseline"), case["name"],
                                                str(case["traceIterations"]), "--trace"], stdout=subprocess.PIPE)
                    digest = hashlib.sha256()
                    for chunk in iter(lambda: process.stdout.read(1024 * 1024), b""):
                        digest.update(chunk)
                        stream.write(chunk)
                    if process.wait() != 0:
                        raise RuntimeError("matrix trace failed")
                row[arm] = digest.hexdigest()
            rows.append(row)
            (args.output / "matrix.json").write_text(json.dumps(rows, indent=2) + "\n")
            if row["base"] != row["candidate"]:
                raise RuntimeError(f"matrix trace differs: {case['name']}")
            with gzip.open(args.output / f"matrix-{case['name']}-base.jsonl.gz", "rb") as left, gzip.open(args.output / f"matrix-{case['name']}-candidate.jsonl.gz", "rb") as right:
                while True:
                    chunk = left.read(1024 * 1024)
                    if chunk != right.read(1024 * 1024):
                        raise RuntimeError("matrix decompressed bytes differ")
                    if not chunk:
                        break
    elif args.mode == "trace":
        hashes = {}
        if args.arm and args.arm not in ["base", "candidate"]:
            parser.error("trace --arm must select base or candidate")
        for arm in ([args.arm] if args.arm else ["base", "candidate"]):
            folder = args.output / f"verified-trace-{args.population}-{arm}"
            with gzip.open(folder.with_suffix(".jsonl.gz"), "xb", compresslevel=1) as stream, folder.with_suffix(".log").open("x") as log:
                process = subprocess.Popen([str(args.output / ("trace-" + arm) / "study_bots"), str(folder)],
                                           stdout=subprocess.PIPE, stderr=log,
                                           env=dict(os.environ, STUDY_POPULATION=str(args.population), STUDY_TRACE="1"))
                digest = hashlib.sha256()
                for chunk in iter(lambda: process.stdout.read(1024 * 1024), b""):
                    digest.update(chunk)
                    stream.write(chunk)
                if process.wait() != 0:
                    raise RuntimeError("bot trace failed")
                hashes[arm] = digest.hexdigest()
        base = args.output / f"verified-trace-{args.population}-base"
        candidate = args.output / f"verified-trace-{args.population}-candidate"
        if not filecmp.cmp(base / "world.json", candidate / "world.json", shallow=False):
            raise RuntimeError("bot world differs")
        with gzip.open(base.with_suffix(".jsonl.gz"), "rb") as left, gzip.open(candidate.with_suffix(".jsonl.gz"), "rb") as right:
            digest = hashlib.sha256()
            while True:
                chunk = left.read(1024 * 1024)
                if chunk != right.read(1024 * 1024):
                    raise RuntimeError("bot trace differs")
                if not chunk:
                    break
                digest.update(chunk)
        hashes["base"] = hashes["candidate"] = digest.hexdigest()
        hashes["world.json"] = sha(base / "world.json")
        (args.output / f"verified-trace-{args.population}.json").write_text(json.dumps(hashes, indent=2) + "\n")
    elif args.mode == "cleanup":
        for path in args.output.glob("*/target"):
            if path.resolve().parent.parent != args.output:
                raise RuntimeError("target outside output")
            shutil.rmtree(path)


if __name__ == "__main__":
    main()
