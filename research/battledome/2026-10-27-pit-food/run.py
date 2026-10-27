"""Archived-source host-path sanity check. Standard library; no sockets."""
import argparse
import ctypes
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent


def digest(data):
    return hashlib.sha256(data).hexdigest()


def two_cpus():
    if os.name == "nt":
        from ctypes import wintypes
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.GetCurrentProcess.restype = wintypes.HANDLE
        kernel.GetProcessAffinityMask.argtypes = [wintypes.HANDLE, ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_size_t)]
        kernel.SetProcessAffinityMask.argtypes = [wintypes.HANDLE, ctypes.c_size_t]
        process = kernel.GetCurrentProcess()
        allowed, system = ctypes.c_size_t(), ctypes.c_size_t()
        assert kernel.GetProcessAffinityMask(process, ctypes.byref(allowed), ctypes.byref(system))
        bits = [1 << i for i in range(allowed.value.bit_length()) if allowed.value & (1 << i)]
        assert kernel.SetProcessAffinityMask(process, sum(bits[:2]))
    else:
        os.sched_setaffinity(0, sorted(os.sched_getaffinity(0))[:2])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--main", required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    two_cpus()
    env = os.environ | {"CARGO_BUILD_JOBS": "1", "RUST_TEST_THREADS": "1", "RAYON_NUM_THREADS": "1", "CARGO_TARGET_DIR": str(out / "build")}
    pins = {"fixtureSha256": digest((HERE / "fixture.rs").read_bytes()), "planSha256": digest((HERE / "PLAN.md").read_bytes()), "runnerSha256": digest(Path(__file__).read_bytes()), "sources": {}}
    binaries = {}
    for arm, revision in [("main", args.main), ("pit-food", args.candidate)]:
        commit = subprocess.check_output(["git", "rev-parse", revision], cwd=ROOT, text=True).strip()
        source = out / (arm + "-source")
        source.mkdir()
        archive = subprocess.check_output(["git", "archive", commit], cwd=ROOT)
        with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
            tar.extractall(source, filter="data")
        fixture = source / "src/client/server/food_supply_study.rs"
        fixture.write_bytes((HERE / "fixture.rs").read_bytes())
        server = source / "src/client/server.rs"
        text = server.read_text(encoding="utf-8")
        assert text.count("mod tests {") == 1
        server.write_text(text.replace("mod tests {", 'mod tests {\n    include!("server/food_supply_study.rs");'), encoding="utf-8", newline="\n")
        remote = source / "src/client/simulation/remote.rs"
        text = remote.read_text(encoding="utf-8")
        assert text.count("impl RawSimulator {") == 1
        remote.write_text(text.replace("impl RawSimulator {", "impl RawSimulator {\n    #[cfg(test)]\n    pub(crate) fn study_world(&self) -> &crate::Simulator { self.backend.world() }"), encoding="utf-8", newline="\n")
        manifest = {str(p.relative_to(source)).replace("\\", "/"): digest(p.read_bytes()) for p in sorted((source / "src").rglob("*.rs"))}
        pins["sources"][arm] = {"commit": commit, "rustSourceManifestSha256": digest(json.dumps(manifest, sort_keys=True).encode())}
        (out / (arm + "-manifest.json")).write_text(json.dumps(manifest, indent=2), encoding="utf-8")
        command = ["cargo", "test", "--release", "--locked", "--lib", "--no-default-features", "--features", "networking", "--no-run", "--message-format=json"]
        print("building", arm, commit, flush=True)
        with (out / (arm + "-build.jsonl")).open("w", encoding="utf-8") as stdout, (out / (arm + "-build.stderr")).open("w", encoding="utf-8") as stderr:
            subprocess.run(command, cwd=source, env=env, stdout=stdout, stderr=stderr, check=True)
        artifacts = [json.loads(line) for line in (out / (arm + "-build.jsonl")).read_text(encoding="utf-8").splitlines()]
        executables = [a["executable"] for a in artifacts if a.get("reason") == "compiler-artifact" and a.get("executable") and a.get("target", {}).get("name") == "slither_sim"]
        assert len(executables) == 1, executables
        binary = out / (arm + (".exe" if os.name == "nt" else ".bin"))
        shutil.copyfile(executables[0], binary)
        binary.chmod(binary.stat().st_mode | 0o100)
        binaries[arm] = binary
        pins["sources"][arm]["binarySha256"] = digest(binary.read_bytes())
    (out / "source-pins.json").write_text(json.dumps(pins, indent=2) + "\n", encoding="utf-8")
    runs = [("pilot-main-32", "main", 32, 2), ("main-32", "main", 32, 300), ("pit-food-32", "pit-food", 32, 300), ("pit-food-64", "pit-food", 64, 300), ("main-64", "main", 64, 300)]
    for name, arm, seats, seconds in runs:
        result = out / (name + ".json")
        run_env = env | {"STUDY_ARM": arm, "STUDY_SEATS": str(seats), "STUDY_SEED": "7", "STUDY_SECONDS": str(seconds), "STUDY_OUTPUT": str(result)}
        row = {"name": name, "arm": arm, "seats": seats, "seed": 7, "seconds": seconds, "event": "reserved"}
        with (out / "attempts.jsonl").open("a", encoding="utf-8") as ledger:
            ledger.write(json.dumps(row) + "\n")
        print("running", name, flush=True)
        with (out / (name + ".stdout")).open("w", encoding="utf-8") as stdout, (out / (name + ".stderr")).open("w", encoding="utf-8") as stderr:
            run = subprocess.run([str(binaries[arm]), "pit_food_supply_study", "--ignored", "--nocapture", "--test-threads=1"], cwd=out, env=run_env, stdout=stdout, stderr=stderr)
        row |= {"event": "completed", "exit": run.returncode, "resultSha256": digest(result.read_bytes()) if result.exists() else None}
        with (out / "attempts.jsonl").open("a", encoding="utf-8") as ledger:
            ledger.write(json.dumps(row) + "\n")
        assert run.returncode == 0, row
        data = json.loads(result.read_text(encoding="utf-8"))
        print(name, "food min/max/end", data["minimumFood"], data["maximumFood"], data["samples"][-1]["food"], flush=True)


if __name__ == "__main__":
    main()
