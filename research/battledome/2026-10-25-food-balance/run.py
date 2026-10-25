"""Build and sample this study against a checkout; no game-server connection."""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--seconds", type=float, default=600)
    parser.add_argument("--agents", type=int, nargs="+", default=[32, 64])
    parser.add_argument("--seeds", type=int, nargs="+", default=[1, 2, 3])
    args = parser.parse_args()
    # The affinity is inherited by Cargo, rustc and the measurement children.
    if os.name == "nt":
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.GetCurrentProcess.restype = ctypes.c_void_p
        kernel.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
        if not kernel.SetProcessAffinityMask(kernel.GetCurrentProcess(), 3):
            raise ctypes.WinError(ctypes.get_last_error())
    else:
        os.sched_setaffinity(0, sorted(os.sched_getaffinity(0))[:2])
    repo = args.repo.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    build = output / "build"
    (build / "src").mkdir(parents=True)
    source = Path(__file__).with_name("measure.rs")
    shutil.copyfile(source, build / "src/main.rs")
    policy = (repo / "examples/cautious_bot.rs").read_text(encoding="utf-8")
    (build / "src/cautious.rs").write_text(
        policy.replace("//!", "//") + "\npub fn action(view: &Observation) -> Action { decide(view) }\n",
        encoding="utf-8", newline="\n")
    (build / "Cargo.toml").write_text(
        '[package]\nname="battledome-balance"\nversion="0.0.0"\nedition="2024"\n'
        '[workspace]\n[dependencies]\nserde_json="1"\n'
        f'slither-sim={{path={json.dumps(repo.as_posix())},default-features=false}}\n'
        '[profile.release]\nlto="thin"\ncodegen-units=1\n', encoding="utf-8", newline="\n")
    shutil.copyfile(repo / "Cargo.lock", build / "Cargo.lock")
    target = args.target_dir.resolve() if args.target_dir else build / "target"
    env = dict(os.environ, CARGO_BUILD_JOBS="2", RAYON_NUM_THREADS="2", RUST_TEST_THREADS="2",
               CARGO_TARGET_DIR=str(target))
    manifest = {"source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip(),
                "source_diff": subprocess.check_output(["git", "diff", "--", "src"], cwd=repo, text=True),
                "harness_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                "policy_sha256": hashlib.sha256(policy.encode()).hexdigest(),
                "seconds": args.seconds, "seeds": args.seeds, "agents": args.agents,
                "cpu_affinity": "two logical CPUs", "runs": []}
    manifest_path = output / "manifest.json"
    def save():
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8", newline="\n")
    save()
    # The wrapper adds one local package to the copied lock; resolve offline,
    # retaining the resolved wrapper lock with the attempt for reproduction.
    with (output / "build.log").open("w", encoding="utf-8") as log:
        subprocess.run(["cargo", "build", "--offline", "--release"], cwd=build, env=env,
                       stdout=log, stderr=log, timeout=570, check=True)
    name = "battledome-balance.exe" if os.name == "nt" else "battledome-balance"
    binary = output / name
    shutil.copyfile(target / "release" / name, binary)
    manifest["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
    manifest["cargo_lock_sha256"] = hashlib.sha256((build / "Cargo.lock").read_bytes()).hexdigest()
    for agents in args.agents:
        for seed in args.seeds:
            name = f"n{agents}-s{seed}"
            started = time.monotonic()
            row = {"name": name, "status": "running"}
            manifest["runs"].append(row)
            save()
            try:
                with (output / f"{name}.csv").open("w", encoding="utf-8", newline="\n") as data, \
                     (output / f"{name}.log").open("w", encoding="utf-8") as log:
                    result = subprocess.run([str(binary), str(seed), str(agents), str(args.seconds)],
                                            env=env, stdout=data, stderr=log, timeout=570, check=False)
                row["exit_code"] = result.returncode
                row["status"] = "complete" if result.returncode == 0 else "failed"
            except BaseException as error:
                row["status"] = "failed"
                row["error"] = f"{type(error).__name__}: {error}"
                raise
            finally:
                row["wall_seconds"] = time.monotonic() - started
                save()
                print(json.dumps(row), flush=True)
            if row["status"] != "complete":
                raise RuntimeError(f"{name} failed; partial evidence retained")


if __name__ == "__main__":
    main()
