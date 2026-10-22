"""Run a tiny food-seeking controller through the local JSON-lines API."""

import argparse
import json
import math
import os
from pathlib import Path
import subprocess

PROTOCOL = "slither-sim-json-v1"


def action(observation):
    if observation is None:
        return None
    head = observation["self"]["head"]
    x, y = head["x"], head["y"]
    food = observation["food"]
    if food and math.hypot(x, y) < observation["arena"]["radius"] * 0.8:
        target = min(food, key=lambda item: (item["x"] - x) ** 2 + (item["y"] - y) ** 2)
        heading = math.atan2(target["y"] - y, target["x"] - x)
    else:
        heading = math.atan2(-y, -x)
    return {"heading": heading, "boost": False}


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
        result = request({"op": "reset", "seed": 7, "agents": 4})
        completed = 0
        for _ in range(args.steps):
            observations = result["observations"]
            if not any(item is not None for item in observations):
                break
            result = request({"op": "step", "actions": [action(item) for item in observations]})
            completed += 1
        living = sum(item is not None for item in result["observations"])
        print(f"decisions={completed} time={result['time']:.3f}s living={living}")
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
