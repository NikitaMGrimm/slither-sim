"""Run the same bounded, offline checks locally and in CI (standard library only)."""

import argparse
import os
from pathlib import Path
import shlex
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
HOST_GROUPS = ("rust", "tools", "package")
GROUPS = (*HOST_GROUPS, "wasm")


def commands(group):
    if group == "rust":
        return [
            ["cargo", "fmt", "--all", "--check"],
            ["cargo", "test", "--all-targets", "--locked"],
            ["cargo", "test", "--all-targets", "--no-default-features", "--locked"],
            ["cargo", "test", "--doc", "--locked"],
            ["cargo", "test", "--doc", "--no-default-features", "--locked"],
            ["cargo", "test", "-p", "slither-sim-ffi", "--all-targets", "--locked"],
            ["cargo", "clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"],
            ["cargo", "clippy", "--all-targets", "--no-default-features", "--locked", "--", "-D", "warnings"],
            ["cargo", "run", "--locked", "--example", "basic"],
        ]
    if group == "tools":
        target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
        if not target.is_absolute():
            target = ROOT / target
        binary = str(target / "debug" / ("slither-sim.exe" if os.name == "nt" else "slither-sim"))
        library = str(target / "debug" / ("slither_sim_ffi.dll" if os.name == "nt" else
                                          "libslither_sim_ffi.dylib" if sys.platform == "darwin" else
                                          "libslither_sim_ffi.so"))
        wheels = str(target / "wheels")
        return [
            [sys.executable, "-m", "unittest", "discover", "-s", "tools", "-p", "test_*.py"],
            [sys.executable, "-m", "unittest", "discover", "-s", "tools/fidelity", "-p", "test_*.py"],
            ["node", "--test", *sorted(str(path.relative_to(ROOT)) for path in (ROOT / "tools/fidelity").glob("*.test.mjs"))],
            # maturin builds the Python extension below, with its interpreter.
            ["cargo", "build", "--locked", "--workspace", "--exclude", "slither-sim-py"],
            [sys.executable, "-B", "tools/perf_transport.py", "--binary", binary,
             "--scenarios", "small", "dense-64", "worlds-16", "--steps", "3", "--repeats", "1"],
            [sys.executable, "examples/bot.py", "--binary", binary, "--steps", "10"],
            [sys.executable, "examples/ffi_bot.py", "--library", library, "--steps", "10"],
            ["node", "examples/bot.mjs", binary, "10"],
            # Test the wheel from target/, leaving the tools environment unchanged.
            [sys.executable, "-m", "maturin", "build", "--locked", "--manifest-path", "python/Cargo.toml",
             "--interpreter", sys.executable, "--out", wheels],
            [sys.executable, "python/tests/run.py", wheels],
        ]
    if group == "package":
        return [["cargo", "package", "--locked"]]
    if group == "wasm":
        return [["cargo", "check", "--lib", "--locked", "--no-default-features", "--target", "wasm32-unknown-unknown"]]
    raise ValueError(f"Unknown check group: {group}")


def select_groups(requested):
    result = []
    for name in requested:
        for group in HOST_GROUPS if name == "all" else (name,):
            if group not in result:
                result.append(group)
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("groups", nargs="*", metavar="GROUP", help="rust, tools, package, wasm, or all (default)")
    parser.add_argument("--list", action="store_true", help="print commands without running them")
    args = parser.parse_args(argv)
    requested = args.groups or ["all"]
    if any(group not in (*GROUPS, "all") for group in requested):
        parser.error("groups must be rust, tools, package, wasm, or all")
    environment = os.environ.copy()
    environment.setdefault("CARGO_BUILD_JOBS", "2")
    environment.setdefault("RUST_TEST_THREADS", "2")
    for group in select_groups(requested):
        for command in commands(group):
            print(f"[{group}] {shlex.join(command)}", flush=True)
            if args.list:
                continue
            try:
                result = subprocess.run(command, cwd=ROOT, env=environment, check=False)
            except OSError as error:
                print(f"Cannot run {command[0]}: {error}", file=sys.stderr)
                return 1
            if result.returncode:
                return result.returncode if result.returncode > 0 else 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
