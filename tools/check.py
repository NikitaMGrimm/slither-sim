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
FIDELITY_HALVES = ("test_[a-d]*.py", "test_[!a-d]*.py")
# Concurrent output is buffered, so a hung suite is stopped and its output shown
# before CI's 15-minute job limit cancels the step.
CONCURRENT_TIMEOUT_SECONDS = 600
TIMED_OUT = 124


def clippy(*features):
    return ["cargo", "clippy", "--all-targets", *features, "--locked", "--", "-D", "warnings"]


# Simulator and shared protocol packages by name. The client crates share the workspace but
# have their own toolchain and checks (client.yml), and their desktop binaries
# don't build on Linux, so no command here selects the whole workspace.
SIM_PACKAGES = ("-p", "slither-sim", "-p", "slither-sim-ffi", "-p", "slither-sim-py",
                "-p", "slither-protocol")


def commands(group):
    if group == "rust":
        return commands("lint") + commands("test")
    if group == "lint":
        # Without -p, the root package builds alone, so the C ABI
        # crate's `parallel` dependency cannot re-enable a feature under test.
        return [
            ["cargo", "fmt", *SIM_PACKAGES, "--check"],
            clippy(*SIM_PACKAGES),
            clippy("--no-default-features"),
            clippy("--no-default-features", "--features", "bot"),
            clippy("--no-default-features", "--features", "networking"),
            clippy("--no-default-features", "--features", "parallel"),
            clippy("--no-default-features", "--features", "perf-internals"),
            clippy("--no-default-features", "--features", "latency-rig"),
            clippy("--features", "latency-rig"),
        ]
    if group == "test":
        return [
            ["cargo", "test", "-p", "slither-protocol", "--all-targets", "--locked"],
            ["cargo", "test", "-p", "slither-protocol", "--doc", "--locked"],
            ["cargo", "test", "--all-targets", "--locked"],
            ["cargo", "test", "--all-targets", "--no-default-features", "--locked"],
            ["cargo", "test", "--all-targets", "--no-default-features", "--features", "bot", "--locked"],
            ["cargo", "test", "--all-targets", "--no-default-features", "--features", "networking", "--locked"],
            ["cargo", "test", "--doc", "--locked"],
            ["cargo", "test", "--doc", "--no-default-features", "--locked"],
            ["cargo", "test", "-p", "slither-sim-ffi", "--all-targets", "--locked"],
            # The Python crate has no Rust tests; this builds it with each
            # toolchain, including the 1.99 it declares.
            ["cargo", "check", "-p", "slither-sim-py", "--all-targets", "--locked"],
            ["cargo", "run", "--locked", "--example", "basic"],
            ["cargo", "test", "--locked", "--features", "perf-internals", "--example", "perf_baseline"],
            ["cargo", "test", "--all-targets", "--locked", "--no-default-features", "--features", "latency-rig"],
            ["cargo", "test", "--all-targets", "--locked", "--features", "latency-rig"],
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

        def suite(directory, pattern):
            return [sys.executable, "-m", "unittest", "discover", "-s", directory, "-p", pattern, *durations]
        return [
            # Build first with every job; the fidelity tests and smoke runs reuse it.
            # maturin builds the Python extension below, with its interpreter.
            ["cargo", "build", "--locked", "-p", "slither-sim", "-p", "slither-sim-ffi"],
            suite("tools", "test_*.py"),
            # Complementary halves, so the long end-to-end comparison test runs
            # beside the rest of the fidelity suite, including transport parity.
            *(suite("tools/fidelity", pattern) for pattern in FIDELITY_HALVES),
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
        return [["cargo", "package", "--locked", "-p", "slither-protocol", "-p", "slither-sim",
                 "--registry", "crates-io"]]
    if group == "wasm":
        return [
            ["cargo", "check", "-p", "slither-protocol", "--lib", "--locked", "--target", "wasm32-unknown-unknown"],
            ["cargo", "check", "--lib", "--locked", "--no-default-features", "--target", "wasm32-unknown-unknown"],
            ["cargo", "check", "--lib", "--locked", "--no-default-features", "--features", "bot", "--target", "wasm32-unknown-unknown"],
        ]
    raise ValueError(f"Unknown check group: {group}")


def steps(group):
    """The group's commands in order; the commands of one step run concurrently."""
    if group == "tools":
        # The suites share nothing but the build, and each mostly waits on its
        # own subprocesses.
        build, tools, first, second, node, *rest = commands(group)
        return [[build], [tools, first, second, node], *([command] for command in rest)]
    return [[command] for command in commands(group)]


def interrupt(tree, grace_seconds=5):
    """Stops a still-running tree, first letting a POSIX suite clean up after SIGINT.

    The kill reaches the tree's session, but not processes a test started in
    sessions of their own, as perf_build's process-tree tests do; their cleanup
    must run first. Windows job objects already contain nested jobs.
    """
    if os.name != "nt" and tree.process.poll() is None:
        import signal
        try:
            os.killpg(tree.group, signal.SIGINT)
            tree.process.wait(timeout=grace_seconds)
        except (ProcessLookupError, subprocess.TimeoutExpired):
            pass
    tree.stop()


def run_concurrently(step, environment, timeout=CONCURRENT_TIMEOUT_SECONDS):
    """Runs a step's commands at once and replays each one's output in order.

    Returns (seconds, exit code) per command; commands still running at the
    timeout are stopped with exit code 124. A command that cannot start raises
    OSError with a `command` attribute.
    """
    # Each command runs as a process tree, so no cargo, rustc or node descendant
    # outlives its suite or an interruption.
    from perf_build import ProcessTree
    outputs, running, finished = [], {}, {}
    try:
        for index, command in enumerate(step):
            output = tempfile.TemporaryFile()
            outputs.append(output)
            try:
                running[index] = (ProcessTree(command, ROOT, environment, output), time.monotonic())
            except OSError as error:
                error.command = command
                raise
        deadline = time.monotonic() + timeout
        while running and time.monotonic() < deadline:
            for index, (tree, start) in list(running.items()):
                code = tree.process.poll()
                if code is not None:
                    tree.stop()
                    finished[index] = (time.monotonic() - start, code)
                    del running[index]
            time.sleep(0.05)
        for index, (tree, start) in list(running.items()):
            interrupt(tree)
            finished[index] = (time.monotonic() - start, TIMED_OUT)
            del running[index]
            print(f"Stopped after {timeout} s: {shlex.join(step[index])}", file=sys.stderr, flush=True)
    finally:
        for tree, _ in running.values():
            interrupt(tree)
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
                    command = getattr(error, "command", step[0])
                    print(f"Cannot run {shlex.join(command)}: {error}", file=sys.stderr)
                    timings.append((group, command, time.monotonic() - start, 1))
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
