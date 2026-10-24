"""Run the same bounded, offline checks locally and in CI (standard library only)."""

import argparse
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
HOST_GROUPS = ("rust", "tools", "package")
# `rust` is `lint` followed by `test`; CI runs the two parts as separate jobs.
GROUPS = (*HOST_GROUPS, "lint", "test", "wasm")


def clippy(*features):
    return ["cargo", "clippy", "--all-targets", *features, "--locked", "--", "-D", "warnings"]


def commands(group):
    if group == "rust":
        return commands("lint") + commands("test")
    if group == "lint":
        # Without -p or --workspace, the root package builds alone, so the C ABI
        # crate's `parallel` dependency cannot re-enable a feature under test.
        return [
            ["cargo", "fmt", "--all", "--check"],
            clippy("--workspace"),
            clippy("--no-default-features"),
            clippy("--no-default-features", "--features", "networking"),
            clippy("--no-default-features", "--features", "parallel"),
        ]
    if group == "test":
        return [
            ["cargo", "test", "--all-targets", "--locked"],
            ["cargo", "test", "--all-targets", "--no-default-features", "--locked"],
            ["cargo", "test", "--doc", "--locked"],
            ["cargo", "test", "--doc", "--no-default-features", "--locked"],
            ["cargo", "test", "-p", "slither-sim-ffi", "--all-targets", "--locked"],
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
        # Python 3.12 added --durations; the slowest tests guide CI tuning.
        durations = ["--durations", "5"] if sys.version_info >= (3, 12) else []
        return [
            # Build first with every job; the fidelity tests and smoke runs reuse it.
            # maturin builds the Python extension below, with its interpreter.
            ["cargo", "build", "--locked", "--workspace", "--exclude", "slither-sim-py"],
            [sys.executable, "-m", "unittest", "discover", "-s", "tools", "-p", "test_*.py", *durations],
            [sys.executable, "-m", "unittest", "discover", "-s", "tools/fidelity", "-p", "test_*.py", *durations],
            ["node", "--test", *sorted(str(path.relative_to(ROOT)) for path in (ROOT / "tools/fidelity").glob("*.test.mjs"))],
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


def steps(group):
    """The group's commands in order; the commands of one step run concurrently."""
    if group == "tools":
        # The three suites share nothing but the build, and each mostly waits on
        # its own subprocesses.
        build, tools, fidelity, node, *rest = commands(group)
        return [[build], [tools, fidelity, node], *([command] for command in rest)]
    return [[command] for command in commands(group)]


def run_concurrently(step, environment):
    """Runs a step's commands at once and replays each one's output in order.

    Returns (seconds, exit code) per command.
    """
    outputs, running, finished = [], {}, {}
    try:
        for index, command in enumerate(step):
            output = tempfile.TemporaryFile()
            outputs.append(output)
            running[index] = (subprocess.Popen(command, cwd=ROOT, env=environment,
                                               stdout=output, stderr=subprocess.STDOUT), time.monotonic())
        while running:
            for index, (process, start) in list(running.items()):
                if process.poll() is not None:
                    finished[index] = (time.monotonic() - start, process.returncode)
                    del running[index]
            time.sleep(0.05)
    finally:
        for process, _ in running.values():
            process.kill()
            process.wait()
    for output in outputs:
        output.seek(0)
        sys.stdout.flush()
        sys.stdout.buffer.write(output.read())
        sys.stdout.buffer.flush()
        output.close()
    return [finished[index] for index in range(len(step))]


def select_groups(requested):
    result = []
    for name in requested:
        for group in HOST_GROUPS if name == "all" else (name,):
            if group not in result:
                result.append(group)
    return result


def summarize(timings):
    """Appends a per-command timing table to the GitHub job summary, when there is one."""
    path = os.environ.get("GITHUB_STEP_SUMMARY")
    if not path or not timings:
        return
    rows = ["| Group | Command | Seconds | Result |", "| --- | --- | ---: | --- |"]
    for group, command, seconds, code in timings:
        shown = shlex.join(command).replace(str(ROOT), ".").replace("|", "\\|")
        result = "ok" if code == 0 else f"exit {code}"
        rows.append(f"| {group} | `{shown}` | {seconds:.1f} | {result} |")
    with open(path, "a", encoding="utf-8") as summary:
        summary.write("\n".join(rows) + "\n\n")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("groups", nargs="*", metavar="GROUP",
                        help="rust (lint and test), tools, package, wasm, lint, test, or all (default)")
    parser.add_argument("--list", action="store_true", help="print commands without running them")
    args = parser.parse_args(argv)
    requested = args.groups or ["all"]
    if any(group not in (*GROUPS, "all") for group in requested):
        parser.error("groups must be rust, tools, package, wasm, lint, test, or all")
    environment = os.environ.copy()
    # Bounded defaults for shared machines; CI sets both to its runners' cores.
    environment.setdefault("CARGO_BUILD_JOBS", "2")
    environment.setdefault("RUST_TEST_THREADS", "2")
    timings = []
    try:
        for group in select_groups(requested):
            for step in steps(group):
                for command in step:
                    concurrent = " (concurrent)" if len(step) > 1 else ""
                    print(f"[{group}]{concurrent} {shlex.join(command)}", flush=True)
                if args.list:
                    continue
                start = time.monotonic()
                try:
                    if len(step) > 1:
                        results = run_concurrently(step, environment)
                    else:
                        code = subprocess.run(step[0], cwd=ROOT, env=environment, check=False).returncode
                        results = [(time.monotonic() - start, code)]
                except OSError as error:
                    print(f"Cannot run {shlex.join(step[0])}: {error}", file=sys.stderr)
                    timings.append((group, step[0], time.monotonic() - start, 1))
                    return 1
                for command, (seconds, code) in zip(step, results):
                    timings.append((group, command, seconds, code))
                    print(f"[{group}] {seconds:.1f} s, exit {code}: {shlex.join(command[:6])}", flush=True)
                failed = next((code for _, code in results if code), 0)
                if failed:
                    return failed if failed > 0 else 1
        return 0
    finally:
        summarize(timings)


if __name__ == "__main__":
    raise SystemExit(main())
