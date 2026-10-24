"""Time the `serve` JSON-lines transport as a controller sees it; standard library only.

Local and offline: starts only the given `slither-sim serve` executable. Each scenario
is a deterministic request script whose actions never depend on observations, so
every mode simulates the same worlds. Modes: `server` pipes the whole script into
`serve` and counts its output; `python` and `node` run closed-loop reference
controllers (tools/perf_transport_client.*) that time wait, parse and decide per
step. See docs/performance.md#controller-transport.
"""

import argparse
import collections
from dataclasses import dataclass
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time


ROOT = Path(__file__).resolve().parents[1]
FORMAT = "slither-perf-transport-v1"
PROTOCOL = "slither-sim-json-v2"
MODES = ("server", "python", "node")
SEED = 7
DECISION_TICKS = 6
RUN_TIMEOUT_SECONDS = 600
# read1 allocates its full size on every call: 1 MiB reads cost about 0.13 ms each
# on Windows, which made many small replies look six times slower than serve.
CHUNK_BYTES = 1 << 16
TAIL_CHUNKS = 1024  # Bounds the largest final reply the server-only check can read.


@dataclass(frozen=True)
class Scenario:
    name: str
    agents: int
    config: dict | None
    steps: int
    pattern: str  # "outward" or "circle"; see headings().
    note: str
    worlds: int = 1  # Every world gets the same headings; world w uses seed SEED + w.


SCENARIOS = (
    Scenario("small", 4, None, 1000, "circle", "default config"),
    Scenario("official-128", 128, {"radius": 20000, "dynamicRadius": False}, 300, "outward",
             "headings drift outward; every snake should stay alive"),
    Scenario("dense-64", 64, {"radius": 3000, "dynamicRadius": False}, 300, "circle",
             "Battle-Dome-like arena size; not Battle Dome rules"),
    Scenario("heavy-food", 128, {"radius": 6000, "dynamicRadius": False, "foodCount": 40000}, 50,
             "circle", "dense visible food"),
    Scenario("worlds-16", 32, {"radius": 6000, "dynamicRadius": False}, 200, "circle",
             "16 worlds stepped in parallel by serve", worlds=16),
)


def headings(scenario, step):
    """Scripted absolute headings for every slot, independent of observations.

    Snakes start on a ring facing inward. `outward` points slot i along its spawn
    angle with a small wobble; `circle` turns every snake steadily so it loops near
    its spawn point. Dead slots also receive a heading, which serve accepts.
    """
    count = scenario.agents
    if scenario.pattern == "outward":
        return [2 * math.pi * i / count + 0.3 * math.sin(step / 20 + i) for i in range(count)]
    return [2 * math.pi * i / count + math.pi / 2 + 0.25 * step for i in range(count)]


# Protocol slither-sim-json-v2 request encoding. The clients send these lines
# verbatim; another protocol replaces these two functions.
def encode_reset(scenario):
    request = {"op": "reset", "seed": SEED, "worlds": scenario.worlds, "agents": scenario.agents,
               "decisionTicks": DECISION_TICKS}
    if scenario.config is not None:
        request["config"] = scenario.config
    return json.dumps(request, separators=(",", ":"), allow_nan=False)


def encode_step(values, worlds):
    actions = {"heading": values, "boost": [False] * len(values)}
    return json.dumps({"op": "step", "actions": [actions] * worlds}, separators=(",", ":"), allow_nan=False)


def request_script(scenario, steps=None):
    steps = scenario.steps if steps is None else steps
    lines = [encode_reset(scenario)]
    lines.extend(encode_step(headings(scenario, step), scenario.worlds) for step in range(steps))
    return ("\n".join(lines) + "\n").encode()


def default_binary():
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    return target / "release" / ("slither-sim.exe" if os.name == "nt" else "slither-sim")


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


class Watchdog:
    """Kills `process` after `seconds`, so a stalled serve cannot hold a blocking read forever."""

    def __init__(self, process, seconds):
        self.fired = False
        self.timer = threading.Timer(seconds, self._fire, (process,))
        self.timer.daemon = True

    def _fire(self, process):
        self.fired = True
        process.kill()

    def __enter__(self):
        self.timer.start()
        return self

    def __exit__(self, *exception):
        self.timer.cancel()


def run_server(binary, script_path, serve_args):
    """Pipe the whole script into serve; time from the end of the reset reply to EOF."""
    with open(script_path, "rb") as requests:
        start = time.perf_counter_ns()
        process = subprocess.Popen([str(binary), "serve", *serve_args], stdin=requests, stdout=subprocess.PIPE)
        watchdog = Watchdog(process, RUN_TIMEOUT_SECONDS)
        total = 0
        reset_done = None
        # Keep references to recent chunks for the final-reply check. Scanning every
        # chunk for newlines slowed the reader enough to stall serve on a full pipe.
        tail = collections.deque(maxlen=TAIL_CHUNKS)
        try:
            with watchdog:
                while chunk := process.stdout.read1(CHUNK_BYTES):
                    if reset_done is None and b"\n" in chunk:
                        reset_done = time.perf_counter_ns()
                        reset_bytes = total + chunk.index(b"\n") + 1
                    total += len(chunk)
                    tail.append(chunk)
                end = time.perf_counter_ns()
                process.wait(timeout=RUN_TIMEOUT_SECONDS)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            process.stdout.close()
    if watchdog.fired:
        raise RuntimeError(f"serve did not finish within {RUN_TIMEOUT_SECONDS} s")
    if process.returncode:
        raise RuntimeError(f"serve exited with {process.returncode}")
    if reset_done is None:
        raise RuntimeError("serve produced no reply")
    recent = b"".join(tail)
    if not recent.endswith(b"\n") or (b"\n" not in recent[:-1] and len(tail) == tail.maxlen):
        raise RuntimeError("serve output did not end with a complete reply line")
    final = json.loads(recent[recent.rfind(b"\n", 0, len(recent) - 1) + 1:])
    if final.get("protocol") != PROTOCOL or "error" in final:
        raise RuntimeError(f"unexpected final reply: {final.get('protocol')} {final.get('error')}")
    living = sum(frame["living"] for frame in final["result"]["worlds"])
    return {"totalBytes": total, "resetBytes": reset_bytes, "replyBytes": total - reset_bytes,
            "stepsNs": end - reset_done, "wallNs": end - start, "livingFinal": living}


def run_client(mode, binary, script_path, serve_args):
    if mode == "python":
        command = [sys.executable, "-B", str(ROOT / "tools" / "perf_transport_client.py")]
    else:
        command = ["node", str(ROOT / "tools" / "perf_transport_client.mjs")]
    # serve inherits the client's stderr. A captured pipe would keep subprocess.run
    # waiting on a stalled serve even after the timeout kills the client: on Windows
    # it drains captured pipes after the kill. A file has no such reader.
    with tempfile.TemporaryFile() as stderr:
        result = subprocess.run([*command, str(binary), str(script_path), *serve_args], cwd=ROOT,
                                stdout=subprocess.PIPE, stderr=stderr, timeout=RUN_TIMEOUT_SECONDS, check=False)
        if result.returncode:
            stderr.seek(0)
            detail = stderr.read().decode(errors="replace").strip()[-2000:]
            raise RuntimeError(f"{mode} client exited with {result.returncode}: {detail}")
    return json.loads(result.stdout.decode().strip().splitlines()[-1])


def per_step(run, mode, steps):
    """Per-step milliseconds and reply bytes for one run; totals divide by script steps."""
    if mode == "server":
        return {"replyBytes": run["replyBytes"] / steps, "totalMs": run["stepsNs"] / steps / 1e6}
    phases = {f"{key}Ms": run[f"{key}Ns"] / steps / 1e6 for key in ("wait", "parse", "decide")}
    return {"replyBytes": run["replyBytes"] / steps, **phases, "totalMs": sum(phases.values()),
            "living": run["living"] / steps, "foodSeen": run["foodSeen"] / steps}


def summarize(samples):
    keys = samples[0].keys()
    return {key: {"median": statistics.median(s[key] for s in samples),
                  "min": min(s[key] for s in samples), "max": max(s[key] for s in samples)}
            for key in keys}


def check_consistency(name, runs):
    """Every mode and repeat must see identical reply bytes; clients must agree on checksums."""
    problems = []
    sizes = {(r["mode"], r["repeat"]): r["raw"]["replyBytes"] + r["raw"]["reset"]["replyBytes"]
             if r["mode"] != "server" else r["raw"]["totalBytes"] for r in runs}
    if len(set(sizes.values())) > 1:
        problems.append(f"{name}: total reply bytes differ between runs: {sizes}")
    checksums = {(r["mode"], r["repeat"]): r["raw"]["checksum"] for r in runs if r["mode"] != "server"}
    if len(set(checksums.values())) > 1:
        problems.append(f"{name}: decision checksums differ between runs: {checksums}")
    return problems


def markdown(report):
    lines = ["| Scenario | Mode | Steps | KB/reply | Wait ms | Parse ms | Decide ms | Total ms/step | Total min-max |",
             "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for row in report["summary"]:
        values = row["perStep"]

        def cell(key):
            return f"{values[key]['median']:.3f}" if key in values else ""
        lines.append(
            f"| {row['scenario']} | {row['mode']} | {row['steps']} | {values['replyBytes']['median'] / 1000:.1f} "
            f"| {cell('waitMs')} | {cell('parseMs')} | {cell('decideMs')} | {cell('totalMs')} "
            f"| {values['totalMs']['min']:.3f}-{values['totalMs']['max']:.3f} |")
    return "\n".join(lines) + "\n"


def command_text(command):
    try:
        return subprocess.run(command, capture_output=True, text=True, timeout=30,
                              check=False).stdout.strip() or None
    except OSError:
        return None


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--binary", type=Path, help="slither-sim executable (default: release build in the Cargo target)")
    parser.add_argument("--build", action="store_true", help="run cargo build --release --locked --bin slither-sim first")
    parser.add_argument("--scenarios", nargs="+", choices=[s.name for s in SCENARIOS], help="default: all")
    parser.add_argument("--modes", nargs="+", choices=MODES, default=list(MODES))
    parser.add_argument("--steps", type=int, help="override every scenario's step count")
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--threads", type=int, help="pass --threads N to serve (default: its full pool)")
    parser.add_argument("--output", type=Path, help="new directory for the binary copy, scripts and report.json")
    args = parser.parse_args(argv)
    if args.repeats < 1 or any(value is not None and value < 1 for value in (args.steps, args.threads)):
        parser.error("--repeats, --steps and --threads must be positive")
    if args.output is not None and args.output.exists():
        parser.error(f"{args.output} already exists; choose a new output directory")
    if args.build:
        if args.binary is not None:
            parser.error("--build uses the default release binary; omit --binary")
        subprocess.run(["cargo", "build", "--release", "--locked", "--bin", "slither-sim"], cwd=ROOT, check=True)
    binary = (args.binary or default_binary()).resolve()
    if not binary.is_file():
        parser.error(f"{binary} not found; build with cargo build --release --locked, pass --build or --binary")
    scenarios = [s for s in SCENARIOS if args.scenarios is None or s.name in args.scenarios]
    modes = list(dict.fromkeys(args.modes))
    serve_args = [] if args.threads is None else ["--threads", str(args.threads)]

    with tempfile.TemporaryDirectory(prefix="perf-transport-") as temporary:
        work = Path(temporary) if args.output is None else args.output.resolve()
        work.mkdir(parents=True, exist_ok=True)
        if args.output is not None:
            # Time a private copy so a concurrent cargo build cannot replace it mid-run.
            (work / "bin").mkdir()
            binary = Path(shutil.copy2(binary, work / "bin" / binary.name)).resolve()
        scripts = {}
        for scenario in scenarios:
            script = request_script(scenario, args.steps)
            scripts[scenario.name] = work / f"{scenario.name}.requests.jsonl"
            scripts[scenario.name].write_bytes(script)
        report = {
            "format": FORMAT, "protocol": PROTOCOL,
            "binary": {"path": str(binary), "sha256": sha256(binary)},
            "host": {"platform": platform.platform(), "processor": platform.processor(),
                     "cpuCount": os.cpu_count(), "python": sys.version.split()[0],
                     "node": command_text(["node", "--version"]) if "node" in modes else None},
            "toolCommit": command_text(["git", "-C", str(ROOT), "rev-parse", "HEAD"]),
            "repeats": args.repeats, "modes": modes, "serveArgs": serve_args,
            "scenarios": [{"name": s.name, "worlds": s.worlds, "agents": s.agents, "config": s.config, "seed": SEED,
                           "decisionTicks": DECISION_TICKS, "pattern": s.pattern, "note": s.note,
                           "steps": args.steps or s.steps,
                           "requestsSha256": hashlib.sha256(scripts[s.name].read_bytes()).hexdigest()}
                          for s in scenarios],
            "runs": [], "summary": [], "problems": []}
        # Repeats are the outer loop and the mode order rotates, so slow drift in host
        # load spreads over every row instead of favoring one mode.
        for repeat in range(args.repeats):
            shift = repeat % len(modes)
            for scenario in scenarios:
                for mode in modes[shift:] + modes[:shift]:
                    print(f"[{repeat + 1}/{args.repeats}] {scenario.name} {mode}", file=sys.stderr, flush=True)
                    raw = (run_server(binary, scripts[scenario.name], serve_args) if mode == "server"
                           else run_client(mode, binary, scripts[scenario.name], serve_args))
                    report["runs"].append({"scenario": scenario.name, "mode": mode, "repeat": repeat, "raw": raw})
        for scenario in scenarios:
            steps = args.steps or scenario.steps
            runs = [r for r in report["runs"] if r["scenario"] == scenario.name]
            report["problems"].extend(check_consistency(scenario.name, runs))
            for mode in modes:
                samples = [per_step(r["raw"], mode, steps) for r in runs if r["mode"] == mode]
                report["summary"].append({"scenario": scenario.name, "mode": mode, "steps": steps,
                                          "perStep": summarize(samples)})
        table = markdown(report)
        if args.output is not None:
            (work / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
            (work / "summary.md").write_text(table, encoding="utf-8", newline="\n")
    print(table, end="")
    for problem in report["problems"]:
        print(f"ERROR: {problem}", file=sys.stderr)
    return 1 if report["problems"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
