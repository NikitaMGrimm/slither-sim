"""Run a tiny food-seeking controller through the local JSON-lines API."""

import argparse
import json
import math
import os
from pathlib import Path
import subprocess

PROTOCOL = "slither-sim-json-v2"


def heading(frame, view):
    """Steer to the nearest visible food, or back toward the center near the rim."""
    snakes, food = frame["snakes"], frame["food"]
    x, y = snakes["x"][view["self"]], snakes["y"][view["self"]]
    if view["food"] and math.hypot(x, y) < frame["radius"] * 0.8:
        target = min(view["food"], key=lambda i: (food["x"][i] - x) ** 2 + (food["y"][i] - y) ** 2)
        return math.atan2(food["y"][target] - y, food["x"][target] - x)
    return math.atan2(-y, -x)


def actions(frame):
    views = frame["views"]
    return {
        "heading": [None if view is None else heading(frame, view) for view in views],
        "boost": [False] * len(views),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    root = Path(__file__).resolve().parents[1]
    filename = "slither-sim.exe" if os.name == "nt" else "slither-sim"
    parser.add_argument("--binary", type=Path, default=root / "target" / "release" / filename)
    parser.add_argument("--steps", type=int, default=200)
    args = parser.parse_args()
    if args.steps < 1:
        parser.error("--steps must be positive")
    if not args.binary.is_file():
        parser.error("build the binary with cargo build --release --locked, or pass --binary")

    process = subprocess.Popen(
        [str(args.binary.resolve()), "serve"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )

    def request(message):
        process.stdin.write(json.dumps(message, allow_nan=False) + "\n")
        process.stdin.flush()
        line = process.stdout.readline()
        if not line:
            raise RuntimeError("simulator closed its output")
        reply = json.loads(line)
        if reply.get("protocol") != PROTOCOL:
            raise RuntimeError("incompatible simulator protocol")
        if "error" in reply:
            raise RuntimeError(reply["error"])
        return reply["result"]

    try:
        # One world; pass "worlds": N to step N independent worlds per request.
        frame = request({"op": "reset", "seed": 7, "agents": 4})["worlds"][0]
        completed = 0
        while completed < args.steps and frame["living"]:
            frame = request({"op": "step", "actions": [actions(frame)]})["worlds"][0]
            completed += 1
        print(f"decisions={completed} time={frame['time']:.3f}s living={frame['living']}")
    finally:
        process.stdin.close()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        process.stdout.close()
    if process.returncode:
        raise SystemExit(process.returncode)


if __name__ == "__main__":
    main()
